//! The chronicle: the memory of what was concluded, outside the engine. The
//! engine decides with what it sees and does not remember it; the chronicle
//! folds the concluded turns in the domain's vocabulary, and `project` is the
//! one that translates the fold into the contract. Nobody reads it to decide
//! (P1, P4).

use crate::belief::{Alone, Occupants, Room, Transition};
use crate::conclusion::Observation;
use crate::evidence::BodyEvidence;
use crate::time::SampleRef;
use crate::zone::Counts;

/// The chronicle: since when the room's name and the bed's kind hold, and the
/// last turn that observed bodies per zone. It is a fold of the concluded
/// turns, not of the wire (P1, P4): nobody reads it to decide.
///
/// A state change is both a new "since when" and, once translated, a derived
/// event (`room_changed`, `bed_changed`). It is not a fold of transitions: the
/// bed goes to unknown without layer 0 changing. Accompanied rooms share one
/// name: a change of counts among them is not a change of mode.
#[derive(Debug, Clone)]
pub(crate) struct Chronicle {
    room: Since<RoomName>,
    bed: Since<BedKind>,
    observed: Option<SampleRef>,
    /// The last layer-0 transition: its sample and the counts that name its cell.
    transition: Option<(SampleRef, TransitionKey)>,
}

impl Default for Chronicle {
    /// Before the first scene, the room and the bed are empty.
    fn default() -> Self {
        Self {
            room: Since::new(RoomName::Empty),
            bed: Since::new(BedKind::Empty),
            observed: None,
            transition: None,
        }
    }
}

/// What the fold concludes in a turn: since when each state holds, what it was
/// before if it changed, and the last observation.
pub(crate) struct Chronicled {
    /// The room, by name.
    pub room: Stamp<RoomName>,
    pub bed: Stamp<BedKind>,
    pub observed: Option<SampleRef>,
    /// The sample of the transition before this turn's: since when what we
    /// believed held. `None` if this turn had no transition or it is the first.
    pub transition_since: Option<SampleRef>,
    /// The last transition, this turn's included.
    pub last_transition: Option<TransitionKey>,
}

/// The room's name, as the fold remembers it. Every accompanied room is one
/// name: the counts are not the mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RoomName {
    Empty,
    Alone(Alone),
    Accompanied,
}

impl RoomName {
    pub(crate) fn of(occupants: Occupants) -> Self {
        match Room::of(occupants) {
            Room::Empty => Self::Empty,
            Room::Alone(alone) => Self::Alone(alone),
            Room::Accompanied(_) => Self::Accompanied,
        }
    }
}

/// The bed, as the fold remembers it. Unknown when this turn does not know
/// the zones; otherwise empty, held or seen. A change of counts inside one
/// kind is not a change.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BedKind {
    Unknown,
    Empty,
    Held,
    Seen,
}

impl BedKind {
    pub(crate) fn of(occupants: Occupants, known: bool) -> Self {
        match (known, occupants.bed, occupants.held) {
            (false, _, _) => Self::Unknown,
            (true, 0, 0) => Self::Empty,
            (true, 0, _) => Self::Held,
            (true, _, _) => Self::Seen,
        }
    }
}

/// The counts that name a transition's cell: before, what was seen, after.
/// `project` writes them as the published cell.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct TransitionKey {
    pub before: Occupants,
    pub seen: Counts,
    pub after: Occupants,
}

impl TransitionKey {
    pub(crate) fn of(transition: &Transition) -> Self {
        Self {
            before: transition.before.occupants,
            seen: transition.seen.counts,
            after: transition.after.occupants,
        }
    }
}

/// A concluded turn, in the vocabulary the chronicle remembers.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Entry {
    pub at: SampleRef,
    pub room: RoomName,
    pub bed: BedKind,
    /// Whether the turn saw bodies per zone.
    pub seen: bool,
    /// The transition, if layer 0 changed in this turn.
    pub transition: Option<TransitionKey>,
}

impl Entry {
    /// What the chronicle notes of a concluded turn: the room's name, the
    /// bed's kind and whether it saw bodies per zone.
    pub(crate) fn of(observation: &Observation) -> Self {
        let occupancy = &observation.believed.occupancy;
        let occupants = occupancy.occupants();
        Self {
            at: observation.at,
            room: RoomName::of(occupants),
            bed: BedKind::of(occupants, occupancy.bed_known),
            seen: matches!(observation.evidence.bodies.bodies, BodyEvidence::Seen(_)),
            transition: observation
                .believed
                .transition
                .as_ref()
                .map(TransitionKey::of),
        }
    }
}

impl Chronicle {
    /// Folds a turn: its states and whether it saw bodies per zone.
    pub(crate) fn record(&mut self, entry: Entry) -> Chronicled {
        let Entry {
            at,
            room,
            bed,
            seen,
            transition,
        } = entry;
        if seen {
            self.observed = Some(at);
        }
        let transition_since = transition
            .as_ref()
            .and_then(|_| self.transition.as_ref().map(|(since, _)| *since));
        if let Some(key) = transition {
            self.transition = Some((at, key));
        }
        Chronicled {
            room: self.room.update(room, at),
            bed: self.bed.update(bed, at),
            observed: self.observed,
            transition_since,
            last_transition: self.transition.as_ref().map(|(_, key)| *key),
        }
    }
}

/// A state and the sample it holds since. Before the first turn the initial
/// one holds, still undated.
#[derive(Debug, Clone)]
struct Since<S> {
    state: S,
    since: Option<SampleRef>,
}

/// A state dated in a turn: since when it holds and, if this turn changed it,
/// what it was before.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Stamp<S> {
    pub since: SampleRef,
    pub changed_from: Option<S>,
}

impl<S: Copy + PartialEq> Since<S> {
    fn new(initial: S) -> Self {
        Self {
            state: initial,
            since: None,
        }
    }

    /// This turn's state: the date changes only when the state changes (or on
    /// the first turn, which dates it).
    fn update(&mut self, now: S, at: SampleRef) -> Stamp<S> {
        match self.since {
            Some(since) if self.state == now => Stamp {
                since,
                changed_from: None,
            },
            _ => {
                let from = std::mem::replace(&mut self.state, now);
                self.since = Some(at);
                Stamp {
                    since: at,
                    changed_from: (from != now).then_some(from),
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::time::{SessionId, SourceSecs};

    fn at(secs: f64) -> SampleRef {
        SampleRef {
            session: SessionId(1),
            sequence: 0,
            source_secs: SourceSecs(secs),
        }
    }

    fn counts(floor: u32, bed: u32, held: u32) -> Occupants {
        Occupants { floor, bed, held }
    }

    fn entry(secs: f64, room: RoomName, bed: BedKind, seen: bool) -> Entry {
        Entry {
            at: at(secs),
            room,
            bed,
            seen,
            transition: None,
        }
    }

    #[test]
    fn the_first_turn_dates_without_change_if_still_empty() {
        let mut c = Chronicle::default();
        let r = c.record(entry(1.0, RoomName::Empty, BedKind::Empty, true));
        assert_eq!(
            r.room,
            Stamp {
                since: at(1.0),
                changed_from: None
            }
        );
        assert_eq!(r.bed.changed_from, None);
    }

    #[test]
    fn the_first_occupied_turn_changes_from_empty() {
        let mut c = Chronicle::default();
        let r = c.record(entry(1.0, RoomName::Alone(Alone::Out), BedKind::Seen, true));
        assert_eq!(r.room.changed_from, Some(RoomName::Empty));
        assert_eq!(r.bed.changed_from, Some(BedKind::Empty));
    }

    #[test]
    fn the_same_state_keeps_its_date_and_a_change_says_where_it_came_from() {
        let mut c = Chronicle::default();
        c.record(entry(
            1.0,
            RoomName::Alone(Alone::Out),
            BedKind::Empty,
            true,
        ));
        let same = c.record(entry(
            2.0,
            RoomName::Alone(Alone::Out),
            BedKind::Empty,
            true,
        ));
        assert_eq!(
            same.room,
            Stamp {
                since: at(1.0),
                changed_from: None
            }
        );
        let change = c.record(entry(3.0, RoomName::Alone(Alone::Out), BedKind::Seen, true));
        assert_eq!(change.room.since, at(1.0));
        assert_eq!(
            change.bed,
            Stamp {
                since: at(3.0),
                changed_from: Some(BedKind::Empty)
            }
        );
    }

    #[test]
    fn accompanied_counts_are_one_name() {
        let mut c = Chronicle::default();
        c.record(entry(
            1.0,
            RoomName::of(counts(2, 0, 0)),
            BedKind::Empty,
            true,
        ));
        let same = c.record(entry(
            2.0,
            RoomName::of(counts(3, 1, 0)),
            BedKind::Empty,
            true,
        ));
        assert_eq!(same.room.changed_from, None);
        assert_eq!(same.room.since, at(1.0));
    }

    #[test]
    fn a_turn_without_bodies_does_not_renew_the_last_observation() {
        let mut c = Chronicle::default();
        assert_eq!(
            c.record(entry(1.0, RoomName::Empty, BedKind::Empty, false))
                .observed,
            None
        );
        c.record(entry(
            2.0,
            RoomName::Alone(Alone::Out),
            BedKind::Empty,
            true,
        ));
        let without = c.record(entry(
            3.0,
            RoomName::Alone(Alone::Out),
            BedKind::Unknown,
            false,
        ));
        assert_eq!(without.observed, Some(at(2.0)));
    }

    #[test]
    fn remembers_since_when_what_we_believed_held_and_the_last_transition() {
        let key = |before: u32, after: u32| TransitionKey {
            before: counts(before, 0, 0),
            seen: Counts {
                floor: after,
                bed: 0,
            },
            after: counts(after, 0, 0),
        };
        let with = |secs: f64, transition: Option<TransitionKey>| Entry {
            transition,
            ..entry(secs, RoomName::Alone(Alone::Out), BedKind::Empty, true)
        };
        let mut c = Chronicle::default();
        let first = c.record(with(1.0, Some(key(0, 1))));
        assert_eq!(
            first.transition_since, None,
            "the first does not know since when"
        );
        assert_eq!(first.last_transition, Some(key(0, 1)));
        let still = c.record(with(2.0, None));
        assert_eq!(still.transition_since, None, "no transition, no before");
        assert_eq!(
            still.last_transition,
            Some(key(0, 1)),
            "the last transition is kept"
        );
        let second = c.record(with(5.0, Some(key(1, 2))));
        assert_eq!(second.transition_since, Some(at(1.0)));
        assert_eq!(second.last_transition, Some(key(1, 2)));
    }
}
