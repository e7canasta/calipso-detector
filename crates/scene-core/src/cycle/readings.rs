//! How each reading moves the turn on: what the shell read is interpreted here,
//! with the evidence it belongs to, and the turn goes on at `Course::next`.

use super::sequence::Reached;
use super::waiting::{Continuation, Waiting};
use super::{Course, Phase};
use crate::attention::LookResult;
use crate::belief::Enrichment;
use crate::evidence::{self, Admission, Perceived};
use crate::look::Probe;
use crate::outcome::Outcome;
use crate::verification::Effect;

impl Course<'_> {
    /// The reading arrived: it is interpreted and the turn continues where it was.
    pub(super) fn advance(
        &mut self,
        stage: Continuation,
        probe: &Probe,
        outcome: Outcome,
    ) -> Phase {
        let spent = outcome.spent;
        let reached = match stage {
            Continuation::Primary => Reached::Primary(self.admit(probe, outcome)),
            Continuation::Focused => Reached::Focused(self.admit(probe, outcome)),
            Continuation::Reinforce { primary } => {
                // The loss counts as checked only if reinforcement read bodies:
                // a failing look, or a reading of another kind, is requested
                // again on the next sample.
                if outcome.bodies().is_ok() {
                    self.engine.reinforcement_looked();
                }
                let (bodies, result) =
                    self.engine
                        .consolidate(primary, outcome.bodies(), self.room.as_ref());
                self.asked(probe, spent, result);
                Reached::Bodies(bodies)
            }
            Continuation::Faces {
                bodies,
                region,
                policy,
            } => {
                let faces = evidence::faces(outcome.faces(), region.rect(), &policy);
                self.asked(probe, spent, faces.result);
                Reached::Perceived(Perceived {
                    bodies,
                    faces: faces.faces,
                    posed: None,
                })
            }
            Continuation::PoseBody {
                mut perceived,
                floor,
            } => {
                let result = self.engine.read_pose_body(
                    &mut perceived,
                    floor,
                    outcome.into_pose(),
                    self.room.as_ref(),
                );
                self.asked(probe, spent, result);
                Reached::Evidence(perceived)
            }
            Continuation::Calibrate { mut observed } => {
                let (depth, result) = evidence::depth(outcome.depths());
                self.asked(probe, spent, result);
                observed.calibrated = self.engine.learn_bed(self.at, depth);
                Reached::Calibrated(observed)
            }
            Continuation::Pose { observed, bbox } => {
                let (pose, result) = evidence::pose(outcome.into_pose());
                self.asked(probe, spent, result);
                Reached::Pose {
                    observed,
                    pose,
                    bbox,
                }
            }
            Continuation::Depth {
                observed,
                pose,
                bbox,
                at,
                head,
            } => {
                let (depths, result) = evidence::depths(outcome.depths(), &at, pose.len());
                self.asked(probe, spent, result);
                let primary_seen = observed.evidence.bodies.primary_seen();
                let enrichment = Enrichment::fresh(Some(pose), depths, head, bbox, primary_seen);
                Reached::Enriched(observed, enrichment)
            }
            Continuation::Keep { refined } => {
                let (result, effects) = self.engine.kept(self.at, outcome.kept());
                self.asked(probe, spent, result);
                // The burst is ready: the question goes out now, with the chosen crops.
                let ready = effects.into_iter().find_map(|effect| match effect {
                    Effect::ProbeVerify {
                        ticket,
                        look,
                        crops,
                    } => Some((ticket, look, crops)),
                    _ => None,
                });
                if let Some((ticket, look, crops)) = ready {
                    return Phase::Waiting(Waiting::verify(refined, ticket, look, crops));
                }
                Reached::Final(refined)
            }
            Continuation::Verify { refined } => {
                let result = match outcome.dispatched() {
                    Ok(()) => {
                        self.engine.dispatched(true);
                        LookResult::Dispatched
                    }
                    Err(failure) => {
                        self.engine.dispatched(false);
                        LookResult::failed(failure)
                    }
                };
                self.asked(probe, spent, result);
                Reached::Final(refined)
            }
        };
        self.next(reached)
    }

    /// The main detector's reading, admitted by zones and noted in the ledger.
    fn admit(&mut self, probe: &Probe, outcome: Outcome) -> Admission {
        let admission = self.engine.admit(outcome.bodies(), self.room.as_ref());
        self.asked(probe, outcome.spent, admission.result.clone());
        if probe.region().is_none() && matches!(admission.result, LookResult::Read { .. }) {
            self.engine.looked_wide(self.at);
        }
        admission
    }
}
