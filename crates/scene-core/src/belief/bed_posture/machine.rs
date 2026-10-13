//! The layer-1 machine: it keeps the bed posture while layer 0 believes the
//! resident alone in bed, and nothing otherwise. Markov (P1): only the stable
//! state and the candidate that is holding, in source time (rule 9).

use super::rules::decide;
use super::{BedPostureSeen, BedPostureState, Concluded, Sighting};
use crate::belief::InBed;
use crate::belief::stabilizer::Stabilizer;
use crate::policy::{BedPosturePolicy, Persistence};
use crate::time::SampleRef;

/// The bed posture, while there is one to follow.
#[derive(Debug, Clone, Default)]
pub(crate) struct Machine {
    posture: Option<Stabilizer<BedPostureState>>,
}

impl Machine {
    /// One sample, with layer 0's state and what layer 1 perceived. The rules
    /// propose a state and it counts once it held its own time; without layer
    /// 1 the machine forgets. Seen with nothing new says nothing changed: the
    /// candidate keeps holding.
    pub(crate) fn step(
        &mut self,
        at: SampleRef,
        l0: Option<InBed>,
        sighting: Sighting,
        policy: &BedPosturePolicy,
    ) -> Option<Concluded> {
        let stable = self.posture.as_ref().map(Stabilizer::stable);
        let (Some(l0), Some(decided)) = (l0, decide(stable, l0, sighting)) else {
            self.posture = None;
            return None;
        };
        let posture = self
            .posture
            .get_or_insert_with(|| Stabilizer::new(BedPostureState::InBed));
        let proposed = match (l0, sighting) {
            (InBed::Seen, Sighting::Unknown) => posture.pending().map_or(decided, |c| c.value),
            _ => decided,
        };
        let dwell = Persistence {
            dwell_secs: dwell_secs(policy, proposed),
        };
        posture.observe(at, proposed, &dwell);
        Some(Concluded {
            state: posture.stable(),
            seen: BedPostureSeen::of(l0, sighting),
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
