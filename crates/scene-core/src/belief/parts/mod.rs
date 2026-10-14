//! The occupancy of the subject's parts: where each part is against the bed
//! and the headboard, the two planes with their own `u/v`. The sister of
//! layer 0, with its shape: what a sample saw ([`ByPart<Look>`]), a pure
//! decider (`rules`, a golden table) and a machine that keeps it in source
//! time (`machine`), at the same compass: once per sample, with layer 0's
//! dwell.
//!
//! It is not the posture: the posture relates the parts to each other, this
//! relates them to places. It sits on layer 0 and never breaks it: without a
//! subject there are no places. Its row is the user's,
//! `l0[t] + places[t−1] : seen[t] : places[t]`.
//!
//! Occlusion is the rule here, so the state is what is retained: a part not
//! seen keeps where it was last seen, marked held, until it is seen again or
//! layer 0 loses the subject.
//!
//! See also: white book, volume I, ch. 09.

mod machine;
mod rules;

pub(crate) use machine::Machine;

use crate::time::SampleRef;

/// Something per position of the trama, in its order.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct ByPart<T> {
    /// Against the bed (`cfhsaip`): the centre of the person's box, the face,
    /// the head, the shoulders, the arms, the groin and the legs.
    pub bed: [T; 7],
    /// Against the headboard (`fhs`): the face, the head and the shoulders.
    pub headboard: [T; 3],
    /// Whole boxes inside (`BFK`): the person's in the bed, the face's in the
    /// bed and the face's in the headboard.
    pub whole: [T; 3],
}

/// What one sample says of one part against one region.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) enum Look {
    /// The sense that sees this part did not look in this sample (`-`).
    #[default]
    NotLooked,
    /// It looked: whether some of the part fell inside the region and whether
    /// some fell outside. Both: split (one leg in, the other out). Neither:
    /// looked and not seen.
    Looked { inside: bool, outside: bool },
}

impl Look {
    /// Looked and not seen.
    pub(crate) const UNSEEN: Self = Self::Looked {
        inside: false,
        outside: false,
    };

    /// Looked, and all of it on one side.
    pub(crate) fn one_side(inside: bool) -> Self {
        Self::Looked {
            inside,
            outside: !inside,
        }
    }

    /// Looked, and some of it inside or outside.
    fn seen(self) -> bool {
        matches!(self, Self::Looked { inside, outside } if inside || outside)
    }
}

/// Where a part is believed: inside the region, outside, both (split) or
/// neither (unknown). Held: not seen in this sample, kept from where it was
/// last seen.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct Place {
    pub inside: bool,
    pub outside: bool,
    pub held: bool,
}

impl Place {
    /// Has it ever been seen?
    pub(crate) fn known(self) -> bool {
        self.inside || self.outside
    }
}

/// What the parts concluded in a turn: what was seen, the places believed
/// (held where not seen now), the stable places, the places that are holding
/// and, if new places held in this turn, the change. The same shape as a
/// layer-0 turn: a state, a pending candidate and a transition.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Concluded {
    pub seen: ByPart<Look>,
    pub state: ByPart<Place>,
    /// The stable places, without the held marks.
    pub places: ByPart<Place>,
    /// New places waiting to hold, and the first sample that saw them.
    pub pending: Option<(ByPart<Place>, SampleRef)>,
    /// The places changed in this turn.
    pub transition: Option<PartsTransition>,
}

/// A change of the stable places: what we believed, what this sample saw,
/// what we believe, and the first sample that saw the new places. The
/// parts' row as a value, like layer 0's `Transition`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct PartsTransition {
    pub at: SampleRef,
    pub before: ByPart<Place>,
    pub seen: ByPart<Look>,
    pub first_seen: SampleRef,
    pub after: ByPart<Place>,
}
