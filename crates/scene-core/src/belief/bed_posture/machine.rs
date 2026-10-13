//! The layer-1 machine: it keeps the bed posture while layer 0 believes the
//! resident alone in bed, and nothing otherwise. Markov (P1): only the stable
//! state and the candidate that is holding, in source time (rule 9).

use super::rules::decide;
use super::{BedPostureSeen, BedPostureState, Concluded};
use crate::belief::stabilizer::Stabilizer;
use crate::policy::{BedPosturePolicy, Persistence};
use crate::time::SampleRef;

/// The bed posture, while there is one to follow.
#[derive(Debug, Clone, Default)]
pub(crate) struct Machine {
    posture: Option<Stabilizer<BedPostureState>>,
}

impl Machine {
    /// One sample. `None`: nobody alone in bed, and the machine forgets.
    /// Otherwise it starts in bed, if it was not following. What is seen
    /// proposes a state by the rules, and it counts once it held its own
    /// time. Unknown says nothing changed: the candidate keeps holding.
    pub(crate) fn step(
        &mut self,
        at: SampleRef,
        seen: Option<BedPostureSeen>,
        policy: &BedPosturePolicy,
    ) -> Option<Concluded> {
        let Some(seen) = seen else {
            self.posture = None;
            return None;
        };
        let posture = self
            .posture
            .get_or_insert_with(|| Stabilizer::new(BedPostureState::InBed));
        let stable = posture.stable();
        let proposed = match seen {
            BedPostureSeen::Unknown => posture.pending().map_or(stable, |c| c.value),
            seen => decide(stable, seen),
        };
        let dwell = Persistence {
            dwell_secs: dwell_secs(policy, proposed),
        };
        posture.observe(at, proposed, &dwell);
        Some(Concluded {
            state: posture.stable(),
            seen,
        })
    }
}

/// How long a state must hold to count. In bed is where it starts, never a
/// change to hold.
fn dwell_secs(policy: &BedPosturePolicy, state: BedPostureState) -> f64 {
    match state {
        BedPostureState::InBed => 0.0,
        BedPostureState::Lying => policy.dwell_lying_secs,
        BedPostureState::SittingUp => policy.dwell_sitting_up_secs,
        BedPostureState::GettingOut => policy.dwell_getting_out_secs,
    }
}
