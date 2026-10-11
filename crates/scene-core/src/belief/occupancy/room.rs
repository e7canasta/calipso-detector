//! Layer-0 state as a machine with names (book 3, statechart): the room is
//! empty, with a single occupant (on the floor, or in bed seen or retained) or
//! accompanied. The counts are the machine's data; the mode is its name, and it
//! derives from them in a single place ([`Room::of`]), without loss (its tests
//! prove the inverse). The six rules keep deciding over the counts: they scale
//! to any number of people.
//!
//! ```text
//! Room
//! ├── Empty                                  000
//! ├── Alone
//! │   ├── Out                                100
//! │   └── InBed
//! │       ├── Seen                           110
//! │       └── Held                           111
//! └── Accompanied(Crowd)                     two or more: 200, 210, 211, …
//! ```

use crate::belief::ZoneCounts;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Room {
    Empty,
    Alone(Alone),
    Accompanied(Crowd),
}

/// A single occupant: the subject.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Alone {
    Out,
    InBed(InBed),
}

/// The only occupant, in bed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum InBed {
    Seen,
    /// Stopped being seen without a step to the floor: we believe they are still there (rule 1).
    Held,
}

/// Two or more occupants, with their counts per zone. Built only from
/// [`Room::of`]: an accompanied room with fewer than two does not exist.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Crowd(ZoneCounts);

impl Crowd {
    #[cfg(test)]
    pub(crate) fn counts(self) -> ZoneCounts {
        self.0
    }
}

impl Room {
    /// The mode of some occupant counts.
    pub(crate) fn of(c: ZoneCounts) -> Self {
        match (c.floor, c.bed, c.held) {
            (0, 0, 0) => Self::Empty,
            (1, 0, 0) => Self::Alone(Alone::Out),
            (0, 1, 0) => Self::Alone(Alone::InBed(InBed::Seen)),
            (0, 0, 1) => Self::Alone(Alone::InBed(InBed::Held)),
            _ => Self::Accompanied(Crowd(c)),
        }
    }

    /// The occupant counts of this mode. The inverse of [`Room::of`]: it exists
    /// to prove the mode loses no information.
    #[cfg(test)]
    pub(crate) fn counts(self) -> ZoneCounts {
        let c = |floor, bed, held| ZoneCounts { floor, bed, held };
        match self {
            Self::Empty => c(0, 0, 0),
            Self::Alone(Alone::Out) => c(1, 0, 0),
            Self::Alone(Alone::InBed(InBed::Seen)) => c(0, 1, 0),
            Self::Alone(Alone::InBed(InBed::Held)) => c(0, 0, 1),
            Self::Accompanied(crowd) => crowd.counts(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn all(up_to: u32) -> impl Iterator<Item = ZoneCounts> {
        (0..=up_to).flat_map(move |floor| {
            (0..=up_to)
                .flat_map(move |bed| (0..=up_to).map(move |held| ZoneCounts { floor, bed, held }))
        })
    }

    #[test]
    fn mode_and_counts_derive_from_each_other_without_loss() {
        for c in all(4) {
            assert_eq!(Room::of(c).counts(), c, "{c:?}");
        }
    }

    #[test]
    fn there_is_one_occupant_exactly_when_the_mode_is_alone() {
        for c in all(4) {
            let alone = matches!(Room::of(c), Room::Alone(_));
            assert_eq!(alone, c.occupants() == 1, "{c:?}");
        }
    }

    #[test]
    fn accompanied_is_two_or_more() {
        for c in all(4) {
            if let Room::Accompanied(crowd) = Room::of(c) {
                assert!(crowd.counts().occupants() >= 2, "{c:?}");
            }
        }
    }
}
