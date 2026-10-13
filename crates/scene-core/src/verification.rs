//! The verification plane (white book, volume III, ch. 10): what the slow
//! verifier was asked, which answer is still owed and how much observation
//! backs the body held in bed. It is evidence about the belief, never belief:
//! it never moves a count, and neither layer reads it (book 4, frontier 2).
//!
//! An answer is evidence of the sample it looked at. It arrives between turns
//! ([`Verification::answered`]) and is settled at the start of the next
//! sample's turn, only if what it asked about still holds: the same held body,
//! in the same session (rule 9).

use crate::belief::Belief;
use crate::look::{Answer, Target, Ticket, Verified};
use crate::policy::Policy;
use crate::refine::{BodyPlaces, DisparitySide, FeetPlace};
use crate::time::SampleRef;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// How much observation backs a body held in bed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum HeldCheckState {
    /// Nobody looked since it stopped being seen.
    Unverified,
    /// A verifier saw it: the belief has an observation behind it.
    Confirmed,
    /// A verifier looked and did not see it. It is not released: the blanket
    /// hides it from every verifier (book 4, frontier 3).
    NotConfirmed,
}

/// Who looked.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Verifier {
    /// The slow verifier: a vision-language model over a crop of the bed.
    Vlm,
}

/// An hour of source time: the budget's window.
pub(crate) const HOUR_SECS: f64 = 3600.0;

/// The provenance of a held body: whether a verifier looked, and when.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct HeldCheck {
    /// Unverified, confirmed or not confirmed.
    pub state: HeldCheckState,
    /// Who looked; `None` while unverified.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub by: Option<Verifier>,
    /// The sample it looked at; `None` while unverified.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub looked: Option<SampleRef>,
}

impl HeldCheck {
    const UNVERIFIED: Self = Self {
        state: HeldCheckState::Unverified,
        by: None,
        looked: None,
    };
}

/// The held body the plane follows: the oldest one, by its identity.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct HeldEntry {
    /// The held body's `last_seen`: its identity.
    pub since: SampleRef,
    /// The turn it started being followed: since when it is held unverified.
    pub held_at: SampleRef,
    pub check: HeldCheck,
}

/// What one camera has asked the verifier and what it owes.
#[derive(Debug, Clone, Default)]
pub(crate) struct Verification {
    /// How many tickets were issued: the next one's id.
    issued: u64,
    /// One request at a time per camera.
    in_flight: Option<Ticket>,
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
        policy: &Policy,
    ) {
        let followed = policy.verify.and_then(|v| v.held).is_some();
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
                check: HeldCheck::UNVERIFIED,
            }),
        };
    }

    /// The held body the plane follows, if any.
    pub(crate) fn held(&self) -> Option<HeldEntry> {
        self.held
    }

    /// Is a request waiting for its answer?
    pub(crate) fn in_flight(&self) -> bool {
        self.in_flight.is_some()
    }

    /// How many requests went out in the last `secs` of source time, in this
    /// session.
    pub(crate) fn asked_within(&self, at: SampleRef, secs: f64) -> usize {
        self.asked
            .iter()
            .filter(|a| at.secs_since(**a).is_some_and(|age| age < secs))
            .count()
    }

    /// The places an answer filled, once: the turn that reads them takes them.
    pub(crate) fn take_filled(&mut self) -> Option<BodyPlaces> {
        self.filled.take()
    }

    /// A request goes out: its ticket, which the answer will carry. A doubtful
    /// digit keeps the places it asks about.
    pub(crate) fn issue(
        &mut self,
        target: Target,
        at: SampleRef,
        places: Option<BodyPlaces>,
    ) -> Ticket {
        self.asked_places = places;
        self.asked
            .retain(|a| at.secs_since(*a).is_some_and(|age| age < HOUR_SECS));
        self.asked.push(at);
        self.issued += 1;
        let ticket = Ticket {
            id: self.issued,
            looked: at,
            target,
        };
        self.in_flight = Some(ticket);
        ticket
    }

    /// The shell could not take the request: nothing is owed.
    pub(crate) fn failed(&mut self) {
        self.in_flight = None;
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
    pub(crate) fn expire(&mut self, at: SampleRef, policy: &Policy, belief: &Belief) {
        let timeout = policy.verify.and_then(|v| v.timeout_secs);
        let expired = self.in_flight.filter(|t| match at.secs_since(t.looked) {
            None => true,
            Some(age) => timeout.is_some_and(|max| age >= max),
        });
        let Some(ticket) = expired else {
            return;
        };
        self.in_flight = None;
        if let Target::Feet { stable } = ticket.target
            && ticket.looked.session == at.session
            && belief.bed_posture_state() == Some(stable)
        {
            self.filled = self.asked_places.map(|places| BodyPlaces {
                feet: FeetPlace::PastTheSides,
                ..places
            });
        }
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
            match ticket.target {
                Target::Feet { stable } => {
                    let same = belief.bed_posture_state() == Some(stable);
                    let feet = match answer {
                        Answer::Yes => Some(FeetPlace::PastTheSides),
                        Answer::No => Some(FeetPlace::Seen),
                        Answer::Unsure => None,
                    };
                    if let (true, Some(feet), Some(places)) = (same, feet, self.asked_places) {
                        self.filled = Some(BodyPlaces { feet, ..places });
                    }
                }
                Target::Disparity { stable } => {
                    let same = belief.bed_posture_state() == Some(stable);
                    let side = match answer {
                        Answer::Yes => Some(DisparitySide::Upright),
                        Answer::No => Some(DisparitySide::Lying),
                        Answer::Unsure => None,
                    };
                    if let (true, Some(side), Some(places)) = (same, side, self.asked_places) {
                        self.filled = Some(BodyPlaces {
                            disparity: side,
                            ..places
                        });
                    }
                }
                Target::Held { since } => {
                    if let Some(held) = self.held.as_mut().filter(|h| h.since == since) {
                        held.check = HeldCheck {
                            state: match answer {
                                Answer::Yes => HeldCheckState::Confirmed,
                                Answer::No | Answer::Unsure => HeldCheckState::NotConfirmed,
                            },
                            by: Some(Verifier::Vlm),
                            looked: Some(ticket.looked),
                        };
                    }
                }
            }
        }
    }
}
