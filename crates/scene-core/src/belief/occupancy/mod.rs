//! Layer 0 of the loop: occupancy by zones. How many are on the
//! floor, how many in bed and who is still in bed unseen, under the six
//! conservation rules. It works alone: it does not know the second level
//! exists.
//!
//! - [`room`]: the named state, the machine: empty, a single occupant (on the
//!   floor, in bed seen or retained) or accompanied.
//! - [`span`]: the span, when a new count held and how it was reached.
//! - [`conservation`]: which moves explain the change (`settle`) and how each
//!   one moves the counts (`evolve`).
//! - [`machine`]: the machine: folds the moves and returns the transition.
//! - [`stabilizer`]: the source-time mechanism of the span.
//!
//! The values layer 0 concludes on each turn live here too.

mod conservation;
mod machine;
mod room;
mod span;
mod stabilizer;

pub(super) use machine::Machine;
pub(crate) use room::{Alone, InBed, Room};

use crate::belief::{Fact, ZoneCounts};
use crate::look::Detection;
use crate::time::SampleRef;
use crate::zone::Counts;
pub(crate) use conservation::{BedPhase, Seen};

/// A retained body (rule 1): it stopped being seen in bed without a step to
/// the floor and we believe it is still there. `since` is the last sample it
/// was seen in.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct HeldBody {
    pub detection: Detection,
    pub since: SampleRef,
}

/// A layer-0 transition: the table row, `state[t−1] | input[t] | state[t]`,
/// with the facts that explain it, in order. It is a value: the machine
/// returns it and neither emits nor stores it (P2); whoever
/// receives it need not query the machine (P6).
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Transition {
    /// The sample that confirmed the new count.
    pub at: SampleRef,
    /// The bed phase it was decided with (rule 6).
    pub phase: BedPhase,
    /// What we believed.
    pub before: Tally,
    /// What we saw: the count that held and how it was reached.
    pub seen: Seen,
    /// The first sample that saw the new count: the start of the span.
    pub first_seen: SampleRef,
    /// What we believe.
    pub after: Tally,
    /// The facts that explain the change, in publishing order.
    pub facts: Vec<Happened>,
}

/// The layer-0 count at an instant: occupants per zone and the bodies in bed
/// that are nobody (rule 6).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Tally {
    pub counts: ZoneCounts,
    pub unexplained: u32,
}

/// A fact and the count it leaves.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Happened {
    pub fact: Fact,
    pub after: Tally,
}

/// A change still stabilizing; the required dwell comes from the policy.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct PendingCounts {
    pub counts: Counts,
    /// Source seconds it has held.
    pub source_secs: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Occupancy {
    pub counts: Counts,
    pub held: Vec<HeldBody>,
    /// Stable bodies in bed that are not occupants (rule 6).
    pub unexplained: u32,
    pub bed_known: bool,
    pub pending: Option<PendingCounts>,
    /// The bed phase now (rule 6): what the next decision will be made with.
    pub phase: BedPhase,
}

impl Occupancy {
    pub(crate) fn totals(&self) -> ZoneCounts {
        ZoneCounts {
            floor: self.counts.floor,
            bed: self.counts.bed,
            held: u32::try_from(self.held.len()).unwrap_or(u32::MAX),
        }
    }

    pub(crate) fn occupants(&self) -> u32 {
        self.totals().occupants()
    }
}
