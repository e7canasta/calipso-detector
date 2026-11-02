//! What the turn leaves behind: the second level and layer 1 read what was
//! believed (`refine`); the turn concludes with an observation or a republication
//! (`observed`, `republished`); and the belief and the ledger take what was
//! decided (`believe`, `asked`, `skipped`).

use super::{Course, Observed, Refined};
use crate::attention::{Look, LookOutcome, LookResult, SkipReason};
use crate::belief::Believed;
use crate::conclusion::{Conclusion, Observation, RepublishReason};
use crate::evidence::Admission;
use crate::look::{LookKind, Probe, Spent};
use crate::map::Geometry;
use crate::reception::SensorChanged;

impl Course<'_> {
    /// The second level reads what was concluded and delivers its result, with
    /// the digit the verifier filled if an answer arrived; layer 1 reads it.
    pub(super) fn refine(&mut self, observed: Observed) -> Refined {
        let filled = self.monitor.engine.verification.take_filled();
        let room = self.room;
        let engine = &self.monitor.engine;
        let refinement = crate::refine::refine(
            observed.believed.subject.as_ref().map(|s| s.zone),
            observed.believed.body.as_ref(),
            room.as_ref(),
            engine.policy.enrich.as_ref(),
            engine.policy.verify.as_ref(),
            filled,
        );
        // The parts, at layer 0's compass: what the sample saw of them.
        let seen = crate::refine::parts::seen(&crate::refine::parts::Sample {
            looked: !matches!(
                observed.evidence.bodies.bodies,
                crate::evidence::BodyEvidence::Unavailable(_)
            ),
            subject: observed.believed.subject.as_ref(),
            body: observed.believed.body.as_ref(),
            faces: &observed.evidence.faces,
            room: room.as_ref(),
            min_score: engine
                .policy
                .enrich
                .as_ref()
                .map(|e| e.min_keypoint_score.get()),
        });
        // Layer 1, on layer 0 and what the second level perceived.
        let (parts, bed_posture) = self
            .monitor
            .engine
            .follow(self.at, seen, refinement.sighting);
        Refined {
            observed,
            refinement,
            bed_posture,
            parts,
        }
    }

    /// What the engine observed in the turn. It reads neither the chronicle
    /// nor the last scene.
    pub(super) fn observed(&mut self, refined: Refined) -> Conclusion {
        let Refined {
            observed,
            refinement: refined,
            bed_posture,
            parts,
        } = refined;
        let engine = &self.monitor.engine;
        Conclusion::Observed(Box::new(Observation {
            at: self.at,
            believed: observed.believed,
            refined,
            bed_posture,
            parts,
            held_check: engine.verification.held(),
            evidence: observed.evidence,
            frame: engine.frame,
            calibrated: observed.calibrated,
            attention: std::mem::take(&mut self.attention),
            map: engine.map.state(
                self.geometry,
                engine.policy.focus.as_ref(),
                engine.policy.shadow.as_ref(),
                &engine.policy.admission,
                &engine.calibration,
            ),
            sensor: engine.reception.state(),
            dwell_secs: engine.occupancy_rules.persistence.dwell_secs,
        }))
    }

    /// No new observation: what to republish, and why.
    pub(super) fn republished(
        &self,
        reason: RepublishReason,
        changed: Option<SensorChanged>,
    ) -> Conclusion {
        let engine = &self.monitor.engine;
        Conclusion::Republish {
            reason,
            changed,
            sensor: engine.reception.state(),
            map: Box::new(engine.map.state(
                Geometry::Valid,
                engine.policy.focus.as_ref(),
                engine.policy.shadow.as_ref(),
                &engine.policy.admission,
                &engine.calibration,
            )),
        }
    }

    /// With all the evidence: the belief is updated.
    pub(super) fn believe(&mut self, bodies: &Admission) -> Believed {
        self.monitor.engine.believe(self.at, bodies)
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
