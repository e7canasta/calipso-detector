//! What the turn leaves behind: the second level and layer 1 read what was
//! believed (`refine`); the turn concludes with an observation or a republication
//! (`observed`, `republished`); and the belief and the ledger take what was
//! decided (`believe`, `asked`, `skipped`).

use super::{Course, Observed, Refined};
use crate::attention::{Look, LookOutcome, LookResult, SkipReason};
use crate::belief::Believed;
use crate::conclusion::{Conclusion, RepublishReason};
use crate::evidence::Admission;
use crate::look::{LookKind, Probe, Spent};
use crate::map::Geometry;
use crate::reception::SensorChanged;

impl Course<'_> {
    /// The second level reads what was concluded and delivers its result, with
    /// the digit the verifier filled if an answer arrived; layer 1 reads it.
    pub(super) fn refine(&mut self, observed: Observed) -> Refined {
        self.engine
            .refine(self.at, observed, self.room.as_ref(), self.filled)
    }

    /// What the engine observed in the turn. It reads neither the chronicle
    /// nor the last scene.
    pub(super) fn observed(&mut self, refined: Refined) -> Conclusion {
        let attention = std::mem::take(&mut self.attention);
        Conclusion::Observed(Box::new(self.engine.observation(
            self.at,
            self.geometry,
            refined,
            attention,
        )))
    }

    /// No new observation: what to republish, and why.
    pub(super) fn republished(
        &self,
        reason: RepublishReason,
        changed: Option<SensorChanged>,
    ) -> Conclusion {
        Conclusion::Republish {
            reason,
            changed,
            sensor: self.engine.sensor_state(),
            map: Box::new(self.engine.map_state(Geometry::Valid)),
        }
    }

    /// With all the evidence: the belief is updated.
    pub(super) fn believe(&mut self, bodies: &Admission) -> Believed {
        self.engine.believe(self.at, bodies)
    }

    pub(super) fn asked(&mut self, probe: &Probe, spent: Spent, result: LookResult) {
        self.attention.looks.push(Look {
            kind: probe.kind(),
            outcome: LookOutcome::Asked {
                region: probe.region(),
                result,
                spent_ms: spent.0,
            },
        });
    }

    pub(super) fn skipped(&mut self, kind: LookKind, reason: SkipReason) {
        self.attention.looks.push(Look {
            kind,
            outcome: LookOutcome::Skipped { reason },
        });
    }
}
