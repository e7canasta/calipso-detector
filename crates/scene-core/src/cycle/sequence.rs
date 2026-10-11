//! The order of a turn and how each reading moves it on. It lives here and
//! nowhere else (rule 11): at each point `attention` is asked, a look is
//! requested or the turn goes on, and every answer is interpreted by
//! `evidence`, `belief` or `map`.

use super::waiting::{Awaiting, Waiting};
use super::{Course, Engine, Observed, Phase};
use crate::attention::Consideration::{Decided, Disabled, Inapplicable};
use crate::attention::{
    self, Consideration, Decision, EnrichPlan, Glimpse, Look, LookOutcome, LookResult, SkipReason,
};
use crate::belief::{Believed, Enrichment};
use crate::body::Keypoint;
use crate::conclusion::{Conclusion, Observation, RepublishReason};
use crate::evidence::{self, Admission, FaceEvidence, Perceived};
use crate::geom::{Point, Rect};
use crate::look::{FaceRegion, LookKind, Outcome, Probe, Spent};
use crate::map::{FrameInfo, Geometry};
use crate::policy::FacePolicy;
use crate::sensor::SensorChanged;

/// How far the turn got, with what it gathered: the point from which
/// [`Course::next`] continues.
enum Reached {
    /// With the main detector's result.
    Primary(Admission),
    /// With the sample's bodies (consolidated with reinforcement, if any).
    Bodies(Admission),
    /// With all the evidence.
    Evidence(Perceived),
    /// With the updated belief.
    Belief(Observed),
    /// With the map up to date.
    Map(Observed),
    /// With the pose (or without it, if it failed).
    Pose {
        observed: Observed,
        pose: Option<Vec<Keypoint>>,
        bbox: Rect,
    },
    /// With the enrichment resolved.
    Enriched(Observed, Enrichment),
}

impl Course<'_> {
    /// Opens a turn over a sample: the geometry and the main detector, which
    /// always runs. Neither a reconnection nor another image size releases
    /// anyone: the belief starts from scratch only with the process.
    pub(super) fn open(&mut self, frame: FrameInfo) -> Phase {
        self.monitor.engine.frame = Some(frame);
        self.geometry = self.monitor.engine.map.gate(frame);
        let region = attention::primary_region();
        Phase::Waiting(Waiting::primary(region))
    }

    /// The order of a turn. At each point `attention` is asked and a look is
    /// requested or the turn goes on.
    ///
    /// ```text
    /// Primary → [Reinforce] → [Faces] → believe → [Calibrate] → [Pose → [Depth]] → conclude
    /// ```
    fn next(&mut self, mut reached: Reached) -> Phase {
        loop {
            reached = match reached {
                // First attention point, before believing: a loss to reinforce?
                Reached::Primary(primary) => {
                    let considered = self.reinforce(&primary);
                    if let Some(region) = self.wanted(LookKind::Reinforce, considered) {
                        return Phase::Waiting(Waiting::reinforce(region, primary));
                    }
                    Reached::Bodies(primary)
                }
                // Where to look for faces?
                Reached::Bodies(bodies) => {
                    let considered = self.faces(&bodies);
                    if let Some((region, policy)) = self.wanted(LookKind::Faces, considered) {
                        return Phase::Waiting(Waiting::faces(bodies, region, policy));
                    }
                    Reached::Evidence(Perceived {
                        bodies,
                        faces: FaceEvidence::NotLooked,
                    })
                }
                // Believe.
                Reached::Evidence(evidence) => {
                    let believed = self.believe(&evidence.bodies);
                    Reached::Belief(Observed {
                        believed,
                        evidence,
                        calibrated: None,
                    })
                }
                // Second attention point, after believing: learn the bed?
                Reached::Belief(observed) => {
                    let considered = self.calibrate(&observed);
                    if let Some(points) = self.wanted(LookKind::Calibrate, considered) {
                        self.monitor.engine.map.attempted(self.at);
                        return Phase::Waiting(Waiting::calibrate(observed, points));
                    }
                    Reached::Map(observed)
                }
                // Enrich the subject?
                Reached::Map(observed) => match self.enrich(&observed.believed) {
                    EnrichPlan::Ask(bbox) => {
                        return Phase::Waiting(Waiting::pose(observed, bbox));
                    }
                    EnrichPlan::Keep(reason) => {
                        self.skipped(LookKind::Pose, reason);
                        Reached::Enriched(observed, Enrichment::Kept)
                    }
                    EnrichPlan::Skip(reason) => {
                        self.skipped(LookKind::Pose, reason);
                        Reached::Enriched(observed, Enrichment::None)
                    }
                    EnrichPlan::Disabled | EnrichPlan::Inapplicable => {
                        Reached::Enriched(observed, Enrichment::None)
                    }
                },
                // Measure the subject's depth?
                Reached::Pose {
                    observed,
                    pose,
                    bbox,
                } => {
                    let considered = self.deepen(pose.as_deref());
                    match (self.wanted(LookKind::Depth, considered), pose) {
                        (Some(points), Some(pose)) => {
                            return Phase::Waiting(Waiting::depth(observed, pose, bbox, points));
                        }
                        (_, pose) => {
                            let e = Enrichment::fresh(pose, None, bbox, &observed.evidence.bodies);
                            Reached::Enriched(observed, e)
                        }
                    }
                }
                // Conclude.
                Reached::Enriched(mut observed, e) => {
                    observed.believed =
                        self.monitor
                            .engine
                            .belief
                            .observe_body(self.at, observed.believed, e);
                    return Phase::Concluded { observed };
                }
            };
        }
    }

    /// The reading arrived: it is interpreted and the turn continues where it was.
    pub(super) fn advance(&mut self, stage: Awaiting, probe: &Probe, outcome: Outcome) -> Phase {
        let spent = outcome.spent;
        let reached = match stage {
            Awaiting::Primary => {
                let admission = evidence::admit(
                    outcome.bodies(),
                    self.monitor.engine.map.bed_for(self.geometry),
                    &self.monitor.engine.policy.admission,
                );
                self.asked(probe, spent, admission.result.clone());
                Reached::Primary(admission)
            }
            Awaiting::Reinforce { primary } => {
                // The loss counts as checked only if reinforcement looked: a
                // failing reinforcement is requested again on the next sample.
                if outcome.result.is_ok() {
                    self.monitor.engine.belief.reinforced();
                }
                let (bodies, result) = evidence::consolidate(
                    primary,
                    outcome.bodies(),
                    self.monitor.engine.map.bed_for(self.geometry),
                    &self.monitor.engine.policy.admission,
                );
                self.asked(probe, spent, result);
                Reached::Bodies(bodies)
            }
            Awaiting::Faces {
                bodies,
                region,
                policy,
            } => {
                let faces = evidence::faces(outcome.faces(), region.rect(), &policy);
                self.asked(probe, spent, faces.result);
                Reached::Evidence(Perceived {
                    bodies,
                    faces: faces.faces,
                })
            }
            Awaiting::Calibrate { mut observed } => {
                let (meters, result) = evidence::depth(outcome.depths());
                self.asked(probe, spent, result);
                observed.calibrated = self.monitor.engine.map.learn(self.at, meters);
                Reached::Map(observed)
            }
            Awaiting::Pose { observed, bbox } => {
                let (pose, result) = evidence::pose(outcome.into_pose());
                self.asked(probe, spent, result);
                Reached::Pose {
                    observed,
                    pose,
                    bbox,
                }
            }
            Awaiting::Depth {
                observed,
                pose,
                bbox,
                at,
            } => {
                let (depths, result) = evidence::depths(outcome.depths(), &at, pose.len());
                self.asked(probe, spent, result);
                let e = Enrichment::fresh(Some(pose), depths, bbox, &observed.evidence.bodies);
                Reached::Enriched(observed, e)
            }
        };
        self.next(reached)
    }

    /// What the engine observed in the turn. It reads neither the chronicle
    /// nor the last scene.
    pub(super) fn observed(&mut self, observed: Observed) -> Conclusion {
        let engine = &self.monitor.engine;
        // The second level reads what was concluded and delivers its result.
        let refined = crate::refine::refine(
            observed.believed.body.as_ref(),
            engine.map.bed_for(self.geometry),
            engine.policy.enrich.as_ref(),
        );
        Conclusion::Observed(Box::new(Observation {
            at: self.at,
            believed: observed.believed,
            refined,
            evidence: observed.evidence,
            frame: engine.frame,
            calibrated: observed.calibrated,
            attention: std::mem::take(&mut self.attention),
            map: engine.map.state(self.geometry),
            sensor: engine.sensor.state(),
            dwell_secs: engine.policy.persistence.dwell_secs,
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
            sensor: engine.sensor.state(),
            map: engine.map.state(Geometry::Valid),
        }
    }

    // --- The questions to attention: each one gets the whole policy and the
    // sequencer never asks which senses are configured (rule 11).

    /// What attention sees of this turn.
    fn glimpse<'g>(&'g self, bodies: &'g Admission, faces: &'g FaceEvidence) -> Glimpse<'g> {
        Glimpse {
            at: self.at,
            geometry: self.geometry,
            map: &self.monitor.engine.map,
            bodies: &bodies.bodies,
            faces,
        }
    }

    fn reinforce(&self, primary: &Admission) -> Consideration<Rect> {
        let g = self.glimpse(primary, &FaceEvidence::NotLooked);
        let Engine { policy, belief, .. } = &self.monitor.engine;
        attention::reinforce(&g, belief.bed_seen(), belief.latch(), policy)
    }

    fn faces(&self, bodies: &Admission) -> Consideration<(FaceRegion, FacePolicy)> {
        let g = self.glimpse(bodies, &FaceEvidence::NotLooked);
        attention::faces(&g, &self.monitor.engine.policy)
    }

    fn calibrate(&self, observed: &Observed) -> Consideration<Vec<Point>> {
        let Perceived { bodies, faces } = &observed.evidence;
        let g = self.glimpse(bodies, faces);
        attention::calibrate(&g, &observed.believed, &self.monitor.engine.policy)
    }

    fn enrich(&self, believed: &Believed) -> EnrichPlan {
        let Engine { policy, belief, .. } = &self.monitor.engine;
        let (reference, since) = (belief.pose_reference(), belief.pose_since());
        attention::enrich(believed, reference, since, self.at, policy)
    }

    fn deepen(&self, pose: Option<&[Keypoint]>) -> Consideration<Vec<(usize, Point)>> {
        attention::deepen(pose, &self.monitor.engine.policy)
    }

    /// What attention wants looked at, if anything. A no with its reason goes
    /// to the ledger (rule 12); with nothing to decide, nothing does.
    fn wanted<T>(&mut self, kind: LookKind, considered: Consideration<T>) -> Option<T> {
        match considered {
            Decided(Decision::Ask(what)) => Some(what),
            Decided(Decision::Skip(reason)) => {
                self.skipped(kind, reason);
                None
            }
            Disabled | Inapplicable => None,
        }
    }

    // --- Mechanism.

    /// With all the evidence: the belief is updated.
    fn believe(&mut self, bodies: &Admission) -> Believed {
        let Engine { policy, belief, .. } = &mut self.monitor.engine;
        belief.observe(self.at, bodies, policy)
    }

    fn asked(&mut self, probe: &Probe, spent: Spent, result: LookResult) {
        self.attention.looks.push(Look {
            kind: probe.kind(),
            outcome: LookOutcome::Asked {
                region: probe.region(),
                result,
                spent_ms: spent.0,
            },
        });
    }

    fn skipped(&mut self, kind: LookKind, reason: SkipReason) {
        self.attention.looks.push(Look {
            kind,
            outcome: LookOutcome::Skipped { reason },
        });
    }
}
