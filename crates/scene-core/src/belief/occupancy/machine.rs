//! The layer-0 machine: it remembers the stable count, who is retained in
//! bed, which bodies in bed are nobody and where each was last seen. The
//! decisions are taken by [`span`](super::span) (when the count changed and how
//! it was reached) and [`conservation`](super::conservation) (which moves
//! explain it); here they are only applied, counted and returned as a
//! transition.

use super::conservation::{self, BedPhase, Before, Movement};
use super::span::{Settled, Span};
use super::{
    Happened, HeldBody, Occupancy, OccupancyRules, PendingCounts, Room, Tally, Transition,
};
use crate::belief::{Fact, Occupants};
use crate::evidence::Bodies;
use crate::look::Detection;
use crate::time::{SampleRef, SessionId};
use crate::units::count;
use crate::zone::Counts;

/// The last time someone was seen in a zone.
#[derive(Debug, Clone, PartialEq)]
pub(in crate::belief) struct Sighting {
    pub(in crate::belief) bodies: Vec<Detection>,
    pub(in crate::belief) at: SampleRef,
}

#[derive(Debug, Clone)]
pub(in crate::belief) struct Machine {
    span: Span,
    /// Retained in bed, oldest to newest.
    held: Vec<HeldBody>,
    /// Bodies in the bed's stable count that are not occupants: they appeared
    /// without coming from the floor (rule 6).
    unexplained: u32,
    /// Has the bed gone up at least once since the process started? (rule 6)
    bed_phase: BedPhase,
    /// The bodies of the last sample with someone in bed: the boxes of newly
    /// retained bodies come from here, and the subject's when it is not seen
    /// in this sample.
    last_seen_in_bed: Option<Sighting>,
    /// The bodies of the last sample with someone on the floor: the subject's
    /// box on the floor when it is not seen in this sample.
    last_seen_on_floor: Option<Sighting>,
    /// The session of the last sample with bodies per zone: if it changes, the
    /// span starts over.
    session: Option<SessionId>,
}

impl Default for Machine {
    fn default() -> Self {
        Self {
            span: Span::new(Counts { floor: 0, bed: 0 }),
            held: Vec::new(),
            unexplained: 0,
            bed_phase: BedPhase::Adopting,
            last_seen_in_bed: None,
            last_seen_on_floor: None,
            session: None,
        }
    }
}

impl Machine {
    /// A sample with bodies per zone: it stabilizes and, if the count changed,
    /// applies the moves that explain it and returns the transition. The
    /// machine does not emit: its caller decides what to do with it. It decides
    /// with its own rules, the occupancy's slice of the profile.
    pub(in crate::belief) fn step(
        &mut self,
        at: SampleRef,
        bodies: &Bodies,
        rules: &OccupancyRules,
    ) -> Option<Transition> {
        if self.session.is_some_and(|previous| previous != at.session) {
            self.span.new_session();
        }
        self.session = Some(at.session);
        if !bodies.bed.is_empty() {
            self.last_seen_in_bed = Some(Sighting {
                bodies: bodies.bed.iter().map(|b| b.detection).collect(),
                at,
            });
        }
        if !bodies.floor.is_empty() {
            self.last_seen_on_floor = Some(Sighting {
                bodies: bodies.floor.iter().map(|b| b.detection).collect(),
                at,
            });
        }
        let settled = self.span.observe(at, bodies, rules)?;
        let before = Before {
            stable: settled.from,
            held: count(self.held.len()),
            unexplained: self.unexplained,
            bed_phase: self.bed_phase,
            held_exit: rules.held_exit,
        };
        let moves = conservation::settle(before, settled.seen);
        Some(self.apply(at, before, settled, moves))
    }

    /// Folds the moves already decided by `settle`, in order: each one moves the
    /// counts (`evolve`) and becomes a fact with its identity. A move without
    /// identity (a retained body with no bed seen, a reappearance with nobody
    /// retained) is not a fact and does not move the counts.
    fn apply(
        &mut self,
        at: SampleRef,
        before: Before,
        settled: Settled,
        moves: Vec<Movement>,
    ) -> Transition {
        let believed = Occupants {
            floor: before.stable.floor,
            bed: before.stable.bed.saturating_sub(before.unexplained),
            held: before.held,
        };
        let mut occupants = believed;
        let mut facts = Vec::new();
        // Newly retained bodies are chosen together: the last ones seen in bed.
        let held = moves.iter().filter(|m| **m == Movement::Held).count();
        let mut newly_held = self.newly_held(count(held)).into_iter();
        for m in moves {
            let fact = match m {
                Movement::LeftBed => Fact::LeftBed,
                Movement::Held => {
                    let Some(kept) = newly_held.next() else {
                        continue;
                    };
                    let last_seen = kept.since;
                    self.held.push(kept);
                    Fact::HeldInBed { last_seen }
                }
                Movement::Reappeared => {
                    let Some(kept) = self.held.pop() else {
                        continue;
                    };
                    let held_since = kept.since;
                    Fact::Reappeared { held_since }
                }
                // Getting up by the side is getting up: who reads the scene
                // sees the bed left, as with `LeftBed`.
                Movement::HeldLeft => {
                    if self.held.pop().is_none() {
                        continue;
                    }
                    Fact::LeftBed
                }
                Movement::LayDownFromFloor | Movement::LayDownWhilePassing => {
                    self.bed_phase = BedPhase::Tracking;
                    Fact::LayDown
                }
                Movement::Adopted => {
                    self.bed_phase = BedPhase::Tracking;
                    Fact::Adopted
                }
                Movement::LeftRoom => Fact::LeftRoom,
                Movement::Entered => Fact::Entered,
                Movement::UnexplainedIn => {
                    self.unexplained += 1;
                    continue;
                }
                Movement::UnexplainedOut => {
                    self.unexplained = self.unexplained.saturating_sub(1);
                    continue;
                }
            };
            occupants = conservation::evolve(occupants, m);
            facts.push(Happened {
                fact,
                after: Tally {
                    occupants,
                    unexplained: self.unexplained,
                },
            });
        }
        Transition {
            at,
            phase: before.bed_phase,
            before: Tally {
                occupants: believed,
                unexplained: before.unexplained,
            },
            seen: settled.seen,
            first_seen: settled.first,
            after: Tally {
                occupants: self.occupants(),
                unexplained: self.unexplained,
            },
            facts,
        }
    }

    /// Who stays retained: the last ones seen in bed.
    fn newly_held(&self, n: u32) -> Vec<HeldBody> {
        let Some(last) = &self.last_seen_in_bed else {
            return Vec::new();
        };
        let fallback = last.bodies.first().into_iter().cycle();
        last.bodies
            .iter()
            .chain(fallback)
            .take(usize::try_from(n).unwrap_or(usize::MAX))
            .map(|d| HeldBody {
                detection: *d,
                since: last.at,
            })
            .collect()
    }

    /// The room's mode: the machine with names.
    pub(in crate::belief) fn room(&self) -> Room {
        Room::of(self.occupants())
    }

    /// A new count is still stabilizing: the mode may be about to change.
    pub(in crate::belief) fn changing(&self) -> bool {
        self.span.pending().is_some()
    }

    fn occupants(&self) -> Occupants {
        let seen = self.seen();
        Occupants {
            floor: seen.floor,
            bed: seen.bed,
            held: count(self.held.len()),
        }
    }

    /// The occupants seen in each zone, stabilized: the stable count without
    /// the bodies in bed that are nobody.
    pub(in crate::belief) fn seen(&self) -> Counts {
        let stable = self.span.stable();
        Counts {
            floor: stable.floor,
            bed: stable.bed.saturating_sub(self.unexplained),
        }
    }

    pub(in crate::belief) fn held(&self) -> &[HeldBody] {
        &self.held
    }

    pub(in crate::belief) fn last_seen_in_bed(&self) -> Option<&Sighting> {
        self.last_seen_in_bed.as_ref()
    }

    pub(in crate::belief) fn last_seen_on_floor(&self) -> Option<&Sighting> {
        self.last_seen_on_floor.as_ref()
    }

    pub(in crate::belief) fn snapshot(&self, at: SampleRef, bed_known: bool) -> Occupancy {
        Occupancy {
            counts: self.seen(),
            held: self.held.clone(),
            unexplained: self.unexplained,
            bed_known,
            phase: self.bed_phase,
            pending: self.span.pending().map(|c| PendingCounts {
                counts: c.value,
                source_secs: c.elapsed(at),
            }),
        }
    }
}

#[cfg(test)]
mod table;
