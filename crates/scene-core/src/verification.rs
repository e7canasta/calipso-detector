//! The verification plane (white book, volume III, ch. 10): what the slow
//! verifier was asked, which answer is still owed and how much observation
//! backs the body held in bed. It is evidence about the belief, never belief:
//! it never moves a count, and neither layer reads it (book 4, frontier 2).
//!
//! An answer is evidence of the sample it looked at. It arrives between turns
//! ([`Verification::answered`]) and is settled at the start of the next
//! sample's turn, only if what it asked about still holds: the same held body,
//! in the same session (rule 9).

use crate::attention::VerifyLook;
use crate::belief::Belief;
use crate::belief::bed_posture::BedPostureState;
use crate::look::{Answer, Target, Ticket, Verified};
use crate::policy::{Burst, Frames, SharpnessPolicy, VerifyPolicy};
use crate::refine::{BodyPlaces, DisparitySide, FeetPlace};
use crate::time::SampleRef;
use crate::time::secs_since;
use crate::units::Seconds;

pub(crate) use scene_contract::HeldCheckState;
pub(crate) use scene_contract::Verifier;
/// An hour of source time: the budget's window.
pub(crate) const HOUR_SECS: f64 = 3600.0;

pub(crate) use scene_contract::HeldCheck;
/// The check of a body held in bed that nobody has looked at yet: the domain's
/// starting state, not a property of the wire.
const UNVERIFIED_HELD_CHECK: HeldCheck = HeldCheck {
    state: HeldCheckState::Unverified,
    by: None,
    looked: None,
};

/// The held body the plane follows: the oldest one, by its identity.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct HeldEntry {
    /// The held body's `last_seen`: its identity.
    pub since: SampleRef,
    /// The turn it started being followed: since when it is held unverified.
    pub held_at: SampleRef,
    pub check: HeldCheck,
}

/// A burst being collected (white book, volume III, ch. 12): V's phase 1.
/// The question is decided and its ticket issued; the shell keeps one crop per
/// `every_secs`, and the question goes out with the sharp ones.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Collecting {
    pub(crate) ticket: Ticket,
    pub(crate) look: VerifyLook,
    pub(crate) burst: Burst,
    /// The crops the shell kept, in order.
    pub(crate) crops: Vec<KeptCrop>,
    /// The sample the burst opened at: the longest wait counts from it.
    pub(crate) opened: SampleRef,
    /// The sample the last crop was kept at.
    pub(crate) last: SampleRef,
}

/// A crop kept for a burst: how sharp, and whether it counts.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct KeptCrop {
    pub(crate) sharpness: f32,
    pub(crate) counted: bool,
}

/// What a question asked about, without its identity: a column of V's table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Asked {
    Held,
    Feet,
    Disparity,
}

impl Asked {
    fn of(target: Target) -> Self {
        match target {
            Target::Held { .. } => Self::Held,
            Target::Feet { .. } => Self::Feet,
            Target::Disparity { .. } => Self::Disparity,
        }
    }
}

/// What settling a question does to what it asked about: a cell of V's table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Settled {
    /// Nothing changes.
    Nothing,
    /// The doubtful feet of the looked sample are filled.
    Feet(FeetPlace),
    /// The doubtful disparity of the looked sample is filled.
    Disparity(DisparitySide),
    /// The held body's provenance changes.
    Held(HeldCheckState),
}

/// V's decider: what an answer does, or its absence once the request expired
/// (`None`), when what it asked about still `holds` (the same session, and
/// the same held body or the same layer-1 state). Pure, and a golden table
/// (`contract/verification-table.txt`). Feet left without an answer are taken
/// as out, on the safe side: a call for getting out is not lost to a verifier
/// that did not answer. A held body is never released: a no only says nobody
/// saw it (book 4, frontier 3).
pub(crate) fn decide(asked: Asked, answer: Option<Answer>, holds: bool) -> Settled {
    if !holds {
        return Settled::Nothing;
    }
    match (asked, answer) {
        (Asked::Feet, Some(Answer::Yes) | None) => Settled::Feet(FeetPlace::PastTheSides),
        (Asked::Feet, Some(Answer::No)) => Settled::Feet(FeetPlace::Seen),
        (Asked::Disparity, Some(Answer::Yes)) => Settled::Disparity(DisparitySide::Upright),
        (Asked::Disparity, Some(Answer::No)) => Settled::Disparity(DisparitySide::Lying),
        (Asked::Held, Some(Answer::Yes)) => Settled::Held(HeldCheckState::Confirmed),
        (Asked::Held, Some(Answer::No | Answer::Unsure)) => {
            Settled::Held(HeldCheckState::NotConfirmed)
        }
        (Asked::Feet | Asked::Disparity, Some(Answer::Unsure))
        | (Asked::Disparity | Asked::Held, None) => Settled::Nothing,
    }
}

/// Whether what `target` asked about still holds: layer 1 in the stable state
/// it was asked under, or the same body still held and followed. One rule for
/// settling an answer and for keeping a burst going.
pub(crate) fn holds(
    target: Target,
    layer_1: Option<BedPostureState>,
    held: Option<SampleRef>,
) -> bool {
    match target {
        Target::Feet { stable } | Target::Disparity { stable } => layer_1 == Some(stable),
        Target::Held { since } => held == Some(since),
    }
}

/// What the verification decides from its policy, resolved once when the
/// monitor is built (ADR 2026-11-02, politica-en-rebanadas): whether held
/// bodies are followed and verified, and how long a request may wait for its
/// answer, and how sharp a crop must be to count. Without a `[verify]` section,
/// nothing is followed, nothing expires and every crop counts as sharp enough.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct VerifyRules {
    /// The policy verifies held bodies (`held`).
    pub(crate) held: bool,
    /// Seconds a request may wait for its answer (`timeout_secs`).
    pub(crate) timeout_secs: Option<Seconds>,
    /// The sharpness a crop is judged by (`sharpness`); `None`: no judgement.
    pub(crate) sharpness: Option<SharpnessPolicy>,
    /// The burst of each question (`frames`); `None`: no `[verify]`.
    pub(crate) frames: Option<Frames>,
}

impl VerifyRules {
    /// The rules of a profile's `[verify]` section, or of none.
    pub(crate) fn of(verify: Option<&VerifyPolicy>) -> Self {
        Self {
            held: verify.and_then(|v| v.held).is_some(),
            timeout_secs: verify.and_then(|v| v.timeout_secs),
            sharpness: verify.and_then(|v| v.sharpness),
            frames: verify.map(|v| v.frames),
        }
    }
}

/// What one camera has asked the verifier and what it owes.
#[derive(Debug, Clone, Default)]
pub(crate) struct Verification {
    /// How many tickets were issued: the next one's id.
    issued: u64,
    /// One request at a time per camera: V's phase 2.
    in_flight: Option<Ticket>,
    /// The sample the request in flight went out at: its timeout counts from
    /// there, not from the burst's first sample (V18).
    sent: Option<SampleRef>,
    /// A burst being collected: V's phase 1.
    collecting: Option<Collecting>,
    /// The sharpness of the camera's last kept crops: the reference a crop is
    /// sharp against (bounded by the policy's window).
    sharpness: std::collections::VecDeque<f32>,
    /// For a doubtful digit, the places of the looked sample the answer fills.
    asked_places: Option<BodyPlaces>,
    /// The looked sample's places, filled by an answer that still applied: what
    /// layer 1 reads in the next turn if its own are not clear.
    filled: Option<BodyPlaces>,
    /// The samples requests were issued at, for the budget: at most an hour of
    /// them, in the current session.
    asked: Vec<SampleRef>,
    held: Option<HeldEntry>,
    /// Answers that arrived since the last turn, to settle at the next one.
    answers: Vec<Verified>,
}

impl Verification {
    /// The oldest held body after believing. A new one starts unverified, from
    /// this turn; none, nothing to follow. Only if the policy verifies held
    /// bodies: otherwise nothing is followed and nothing is published (C1).
    pub(crate) fn observe_held(
        &mut self,
        oldest: Option<SampleRef>,
        at: SampleRef,
        rules: &VerifyRules,
    ) {
        self.held = match (oldest.filter(|_| rules.held), self.held) {
            (None, _) => None,
            // A new session: source time starts over, and so does the wait.
            (Some(since), Some(held)) if held.since == since => Some(HeldEntry {
                held_at: if held.held_at.session == at.session {
                    held.held_at
                } else {
                    at
                },
                ..held
            }),
            (Some(since), _) => Some(HeldEntry {
                since,
                held_at: at,
                check: UNVERIFIED_HELD_CHECK,
            }),
        };
    }

    /// The held body the plane follows, if any.
    pub(crate) fn held(&self) -> Option<HeldEntry> {
        self.held
    }

    /// Is a question open, being collected or waiting for its answer? One at
    /// a time per camera.
    pub(crate) fn busy(&self) -> bool {
        self.in_flight.is_some() || self.collecting.is_some()
    }

    /// The burst being collected, if any.
    pub(crate) fn collecting(&self) -> Option<&Collecting> {
        self.collecting.as_ref()
    }

    /// How many requests went out in the last `secs` of source time, in this
    /// session.
    pub(crate) fn asked_within(&self, at: SampleRef, secs: f64) -> usize {
        self.asked
            .iter()
            .filter(|a| secs_since(at, **a).is_some_and(|age| age < secs))
            .count()
    }

    /// The places an answer filled, once: the turn that reads them takes them.
    pub(crate) fn take_filled(&mut self) -> Option<BodyPlaces> {
        self.filled.take()
    }

    /// A question is decided: its ticket, which the answer will carry, about
    /// this sample. It counts for the budget now, once per burst. A doubtful
    /// digit keeps the places it asks about: the answer fills them (V16).
    pub(crate) fn issue(
        &mut self,
        target: Target,
        at: SampleRef,
        places: Option<BodyPlaces>,
    ) -> Ticket {
        self.asked_places = places;
        self.asked
            .retain(|a| secs_since(at, *a).is_some_and(|age| age < HOUR_SECS));
        self.asked.push(at);
        self.issued += 1;
        Ticket {
            id: self.issued,
            looked: at,
            target,
        }
    }

    /// The question goes out at `at`: it waits for its answer.
    pub(crate) fn dispatch(&mut self, ticket: Ticket, at: SampleRef) {
        self.collecting = None;
        self.in_flight = Some(ticket);
        self.sent = Some(at);
    }

    /// A burst starts collecting: the shell keeps this sample's crop first.
    pub(crate) fn collect(
        &mut self,
        ticket: Ticket,
        look: VerifyLook,
        burst: Burst,
        at: SampleRef,
    ) {
        self.collecting = Some(Collecting {
            ticket,
            look,
            burst,
            crops: Vec::new(),
            opened: at,
            last: at,
        });
    }

    /// The shell kept a crop of the burst at `at`, this sharp. It counts if it
    /// is at least `min_ratio` of the median of the camera's last crops (the
    /// first ones always count); without a policy, every crop counts. Returns
    /// whether it counted.
    pub(crate) fn kept(
        &mut self,
        at: SampleRef,
        sharpness: f32,
        policy: Option<&crate::policy::SharpnessPolicy>,
    ) -> bool {
        let counted = match policy {
            None => true,
            Some(p) => {
                let mut recent: Vec<f32> = self.sharpness.iter().copied().collect();
                recent.sort_by(f32::total_cmp);
                let median = recent.get(recent.len() / 2).copied();
                let counted = median.is_none_or(|m| sharpness >= p.min_ratio * m);
                self.sharpness.push_back(sharpness);
                while self.sharpness.len() > usize::from(p.window.max(1)) {
                    self.sharpness.pop_front();
                }
                counted
            }
        };
        if let Some(collecting) = self.collecting.as_mut() {
            collecting.crops.push(KeptCrop { sharpness, counted });
            collecting.last = at;
        }
        counted
    }

    /// The burst is dropped unasked: stale or resolved. Nothing is owed.
    pub(crate) fn drop_burst(&mut self) {
        self.collecting = None;
        self.asked_places = None;
    }

    /// The shell could not take the request or keep the crop: nothing is owed.
    pub(crate) fn failed(&mut self) {
        self.in_flight = None;
        self.collecting = None;
    }

    /// An answer arrived between turns.
    pub(crate) fn answered(&mut self, verified: Verified) {
        self.answers.push(verified);
    }

    /// At the start of a sample's turn, after settling: a request without an
    /// answer expires after the policy's timeout, or with a new session. The
    /// thing it asked about may be asked about again.
    /// Doubtful feet without an answer are taken as out, on the safe side: a
    /// call for getting out is not lost to a verifier that did not answer.
    pub(crate) fn expire(&mut self, at: SampleRef, rules: &VerifyRules, belief: &Belief) {
        let timeout = rules.timeout_secs;
        let sent = self.sent;
        let expired = self
            .in_flight
            .filter(|t| match secs_since(at, sent.unwrap_or(t.looked)) {
                None => true,
                Some(age) => timeout.is_some_and(|max| age >= max.get()),
            });
        let Some(ticket) = expired else {
            return;
        };
        self.in_flight = None;
        let holds = ticket.looked.session == at.session && self.holds(ticket.target, belief);
        if let Settled::Feet(feet) = decide(Asked::of(ticket.target), None, holds) {
            self.filled = self
                .asked_places
                .map(|places| BodyPlaces { feet, ..places });
        }
    }

    /// Whether what `target` asked about still holds, with the belief at the
    /// start of the turn.
    fn holds(&self, target: Target, belief: &Belief) -> bool {
        holds(
            target,
            belief.bed_posture_state(),
            self.held.map(|h| h.since),
        )
    }

    /// At the start of a sample's turn: each answer that still applies changes
    /// the provenance of what it asked about, or fills the doubtful digit it
    /// asked about. One that answers another ticket, another session, a body
    /// no longer held or a layer 1 that moved on changes nothing.
    pub(crate) fn settle(&mut self, at: SampleRef, belief: &Belief) {
        for Verified { ticket, answer } in std::mem::take(&mut self.answers) {
            if self.in_flight.map(|t| t.id) != Some(ticket.id) {
                continue;
            }
            self.in_flight = None;
            if ticket.looked.session != at.session {
                continue;
            }
            let holds = self.holds(ticket.target, belief);
            match decide(Asked::of(ticket.target), Some(answer), holds) {
                Settled::Nothing => {}
                Settled::Feet(feet) => {
                    if let Some(places) = self.asked_places {
                        self.filled = Some(BodyPlaces { feet, ..places });
                    }
                }
                Settled::Disparity(disparity) => {
                    if let Some(places) = self.asked_places {
                        self.filled = Some(BodyPlaces {
                            disparity,
                            ..places
                        });
                    }
                }
                Settled::Held(state) => {
                    if let Some(held) = self.held.as_mut() {
                        held.check = HeldCheck {
                            state,
                            by: Some(Verifier::Vlm),
                            looked: Some(ticket.looked),
                        };
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod table;
