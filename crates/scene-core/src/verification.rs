//! The verification plane (white book, volume III, ch. 10): what the slow
//! verifier was asked, which answer is still owed and how much observation
//! backs the body held in bed. It is evidence about the belief, never belief:
//! it never moves a count, and neither layer reads it (book 4, frontier 2).
//!
//! An answer is evidence of the sample it looked at. It arrives between turns
//! ([`Verification::answered`]) and is settled at the start of the next
//! sample's turn, only if what it asked about still holds: the same held body,
//! in the same session (rule 9).

use crate::belief::bed_posture::BedPostureState;
use crate::look::{Answer, Target, Ticket, Verified};
use crate::policy::{Frames, SharpnessPolicy, VerifyPolicy};
use crate::refine::{BodyPlaces, DisparitySide, FeetPlace};
use crate::time::SampleRef;
use crate::time::secs_since;
use crate::units::Seconds;

mod burst;
pub(crate) use burst::{BurstStep, Effect, Event, Phase};

/// An hour of source time: the budget's window.
pub(crate) const HOUR_SECS: f64 = 3600.0;

/// How much observation backs a body held in bed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum HeldCheckState {
    /// Nobody looked since it stopped being seen.
    Unverified,
    /// A verifier saw it: the belief has an observation behind it.
    Confirmed,
    /// A verifier looked and did not see it. It is not released: the blanket
    /// hides it from every verifier (book 4, frontier 3).
    NotConfirmed,
}

/// Who looked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Verifier {
    /// The slow verifier: a vision-language model over a crop of the bed.
    Vlm,
}

/// The provenance of a held body: whether a verifier looked, and when.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct HeldCheck {
    /// Unverified, confirmed or not confirmed.
    pub state: HeldCheckState,
    /// Who looked; `None` while unverified.
    pub by: Option<Verifier>,
    /// The sample it looked at; `None` while unverified.
    pub looked: Option<SampleRef>,
}

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

/// What one camera has asked the verifier and what it owes: the phase of the
/// burst machine (`burst.rs`), the budget, the crops' window, the answers still
/// to settle, and the held body the plane follows.
#[derive(Debug, Clone, Default)]
pub(crate) struct Verification {
    /// How many tickets were issued: the next one's id.
    issued: u64,
    /// Where the question is: idle, a burst being collected, or in flight.
    phase: Phase,
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
        followed: bool,
    ) {
        self.held = match (oldest.filter(|_| followed), self.held) {
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
        !matches!(self.phase, Phase::Idle)
    }

    /// Is the burst being collected a question about this same `target`? Then a
    /// doubt about it is not new: the ledger already has the burst.
    pub(crate) fn collecting_for(&self, target: Target) -> bool {
        matches!(&self.phase, Phase::Collecting(c) if c.ticket.target == target)
    }

    /// How many requests went out in the last `secs` of source time, in this
    /// session.
    pub(crate) fn asked_within(&self, at: SampleRef, secs: f64) -> usize {
        self.asked
            .iter()
            .filter(|a| secs_since(at, **a).is_some_and(|age| age < secs))
            .count()
    }

    /// A sample's turn opens: the answers that still apply settle and what it
    /// owes for too long expires. The places an answer filled come back here,
    /// to the turn that opened, which carries them to `refine`.
    pub(crate) fn open_turn(
        &mut self,
        at: SampleRef,
        layer_1: Option<BedPostureState>,
        timeout: Option<Seconds>,
    ) -> Option<BodyPlaces> {
        self.step(Event::Settle { at, layer_1 });
        self.step(Event::Expire {
            at,
            timeout,
            layer_1,
        });
        // Only settle and expire fill it, and both run here: nothing is left
        // behind for a later turn to find.
        self.filled.take()
    }

    /// A question is decided: its ticket, which the answer will carry, about this
    /// sample. It counts for the budget now, once per burst. A doubtful digit keeps
    /// the places it asks about: the answer fills them (V16).
    fn issue(&mut self, target: Target, at: SampleRef, places: Option<BodyPlaces>) -> Ticket {
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
}

#[cfg(test)]
mod burst_table;
#[cfg(test)]
mod table;
