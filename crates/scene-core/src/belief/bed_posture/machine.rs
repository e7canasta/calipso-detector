//! The layer-1 machine: it keeps the bed posture while layer 0 believes the
//! resident alone in bed, and nothing otherwise. Markov (P1): only the state.

use super::rules::decide;
use super::{BedPostureSeen, BedPostureState, Concluded};

/// The bed posture, while there is one to follow.
#[derive(Debug, Clone, Default)]
pub(crate) struct Machine {
    state: Option<BedPostureState>,
}

impl Machine {
    /// One sample. `None`: nobody alone in bed, and the machine forgets.
    /// Otherwise it starts in bed, if it was not following, and the rules
    /// decide from there.
    pub(crate) fn step(&mut self, seen: Option<BedPostureSeen>) -> Option<Concluded> {
        let Some(seen) = seen else {
            self.state = None;
            return None;
        };
        let before = self.state.unwrap_or(BedPostureState::InBed);
        let state = decide(before, seen);
        self.state = Some(state);
        Some(Concluded { state, seen })
    }
}
