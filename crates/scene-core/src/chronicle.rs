//! The chronicle: the memory of what was published, outside the engine. The
//! engine decides with what it sees and does not remember what was published;
//! the chronicle folds the concluded turns and the projection reads it to say
//! since when each state holds and what changed. Nobody reads it to decide
//! (project 23, P1; project 24, D6).

use crate::contract::v3::{BedState, RoomState};
use crate::contract::v4::{Cell, Mode};
use crate::time::SampleRef;

/// The chronicle of what was published: since when the room and the bed are
/// as they are, and the last turn that observed bodies per zone. It is a fold
/// of the observed turns and the memory of the projection, not of the machine
/// (project 23, P1 and P4): nobody reads it to decide.
///
/// A state change is both a new "since when" and a derived event
/// (`room_changed`, `bed_changed`): both come out of this fold. It is not a
/// fold of transitions: the bed goes to `unknown` without layer 0 changing.
#[derive(Debug, Clone)]
pub(crate) struct Chronicle {
    room: Since<RoomState>,
    mode: Since<Mode>,
    bed: Since<BedState>,
    observed: Option<SampleRef>,
    /// The last layer-0 transition: its sample and its cell.
    transition: Option<(SampleRef, Cell)>,
}

impl Default for Chronicle {
    /// Before the first scene, the room and the bed are published empty.
    fn default() -> Self {
        Self {
            room: Since::new(RoomState::Empty),
            mode: Since::new(Mode::Empty),
            bed: Since::new(BedState::Empty),
            observed: None,
            transition: None,
        }
    }
}

/// What the fold concludes in a turn: since when each state holds, what it was
/// before if it changed, and the last observation.
pub(crate) struct Chronicled {
    /// The room in v3, by state.
    pub room: Stamp<RoomState>,
    /// The room in v4, by mode.
    pub mode: Stamp<Mode>,
    pub bed: Stamp<BedState>,
    pub observed: Option<SampleRef>,
    /// The sample of the transition before this turn's: since when what we
    /// believed held. `None` if this turn had no transition or it is the first.
    pub transition_since: Option<SampleRef>,
    /// The cell of the last transition, this turn's included.
    pub last_cell: Option<Cell>,
}

/// A concluded turn, in the vocabulary the chronicle remembers.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Entry {
    pub at: SampleRef,
    pub room: RoomState,
    pub mode: Mode,
    pub bed: BedState,
    /// Whether the turn saw bodies per zone.
    pub seen: bool,
    /// The cell, if layer 0 changed in this turn.
    pub transition: Option<Cell>,
}

impl Chronicle {
    /// Folds a turn: its states and whether it saw bodies per zone.
    pub(crate) fn record(&mut self, entry: Entry) -> Chronicled {
        let Entry {
            at,
            room,
            mode,
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
        if let Some(cell) = transition {
            self.transition = Some((at, cell));
        }
        Chronicled {
            room: self.room.update(room, at),
            mode: self.mode.update(mode, at),
            bed: self.bed.update(bed, at),
            observed: self.observed,
            transition_since,
            last_cell: self.transition.as_ref().map(|(_, cell)| cell.clone()),
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

    fn entry(secs: f64, room: RoomState, bed: BedState, seen: bool) -> Entry {
        Entry {
            at: at(secs),
            room,
            mode: Mode::Empty,
            bed,
            seen,
            transition: None,
        }
    }

    #[test]
    fn the_first_turn_dates_without_change_if_still_empty() {
        let mut c = Chronicle::default();
        let r = c.record(entry(1.0, RoomState::Empty, BedState::Empty, true));
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
        let r = c.record(entry(1.0, RoomState::One, BedState::Seen, true));
        assert_eq!(r.room.changed_from, Some(RoomState::Empty));
        assert_eq!(r.bed.changed_from, Some(BedState::Empty));
    }

    #[test]
    fn the_same_state_keeps_its_date_and_a_change_says_where_it_came_from() {
        let mut c = Chronicle::default();
        c.record(entry(1.0, RoomState::One, BedState::Empty, true));
        let same = c.record(entry(2.0, RoomState::One, BedState::Empty, true));
        assert_eq!(
            same.room,
            Stamp {
                since: at(1.0),
                changed_from: None
            }
        );
        let change = c.record(entry(3.0, RoomState::One, BedState::Seen, true));
        assert_eq!(change.room.since, at(1.0));
        assert_eq!(
            change.bed,
            Stamp {
                since: at(3.0),
                changed_from: Some(BedState::Empty)
            }
        );
    }

    #[test]
    fn a_turn_without_bodies_does_not_renew_the_last_observation() {
        let mut c = Chronicle::default();
        assert_eq!(
            c.record(entry(1.0, RoomState::Empty, BedState::Empty, false))
                .observed,
            None
        );
        c.record(entry(2.0, RoomState::One, BedState::Empty, true));
        let without = c.record(entry(3.0, RoomState::One, BedState::Unknown, false));
        assert_eq!(without.observed, Some(at(2.0)));
    }

    #[test]
    fn remembers_since_when_what_we_believed_held_and_the_last_cell() {
        use crate::contract::v4::{Fv, Rbh};
        let cell = |a: u32, b: u32| Cell::new(Rbh(a, 0, 0), Fv(b, 0), Rbh(b, 0, 0));
        let with = |secs: f64, cell: Option<Cell>| Entry {
            transition: cell,
            ..entry(secs, RoomState::One, BedState::Empty, true)
        };
        let mut c = Chronicle::default();
        let first = c.record(with(1.0, Some(cell(0, 1))));
        assert_eq!(
            first.transition_since, None,
            "the first does not know since when"
        );
        assert_eq!(first.last_cell, Some(cell(0, 1)));
        let still = c.record(with(2.0, None));
        assert_eq!(still.transition_since, None, "no transition, no before");
        assert_eq!(still.last_cell, Some(cell(0, 1)), "the last cell is kept");
        let second = c.record(with(5.0, Some(cell(1, 2))));
        assert_eq!(second.transition_since, Some(at(1.0)));
        assert_eq!(second.last_cell, Some(cell(1, 2)));
    }
}
