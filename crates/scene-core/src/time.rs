//! Domain time: the sample, its session and its source time. It is the core's
//! only clock (rule 9). Shared vocabulary: the domain decides with it and
//! `contract` publishes it as is. What the domain asks of it is an [`Elapsed`]:
//! the time between two samples, or nothing when they come from different
//! sessions, and it is compared through `within` and `at_least`, never as a
//! bare number.

use crate::units::Seconds;
pub(crate) use scene_contract::{SampleRef, SessionId};

/// The source time between two samples. Across sessions there is none: source
/// time is not compared between sessions (rule 9).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum Elapsed {
    /// Source seconds since the earlier sample, in the same session.
    Known(f64),
    /// Two sessions apart: nothing is known about the time between them.
    Unknown,
}

impl Elapsed {
    /// Known, and less than `secs`. Unknown is never within.
    pub(crate) fn within(self, secs: Seconds) -> bool {
        matches!(self, Self::Known(age) if age < secs.get())
    }

    /// Known, and at least `secs`. Unknown is never at least.
    pub(crate) fn at_least(self, secs: Seconds) -> bool {
        matches!(self, Self::Known(age) if age >= secs.get())
    }

    /// The seconds to publish: `None` when unknown.
    pub(crate) fn published(self) -> Option<f64> {
        match self {
            Self::Known(age) => Some(age),
            Self::Unknown => None,
        }
    }
}

/// The time between two samples, asked from the later one: `at.since(earlier)`.
pub(crate) trait SourceTime {
    fn since(self, earlier: SampleRef) -> Elapsed;
}

impl SourceTime for SampleRef {
    fn since(self, earlier: SampleRef) -> Elapsed {
        if self.session == earlier.session {
            Elapsed::Known(self.source_secs.0 - earlier.source_secs.0)
        } else {
            Elapsed::Unknown
        }
    }
}
