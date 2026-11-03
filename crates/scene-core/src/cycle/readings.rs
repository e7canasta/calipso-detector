//! How each reading moves the turn on: what the shell read is interpreted here,
//! with the evidence it belongs to, and the turn goes on at `Course::next`.

use super::pending::Pending;
use super::sequence::Reached;
use super::{Course, Phase};
use crate::attention::LookResult;
use crate::belief::Enrichment;
use crate::evidence::{self, Admission, Perceived};
use crate::look::Probe;
use crate::outcome::Outcome;
use crate::verification::Dispatch;

impl Course<'_> {
    /// The reading arrived: it is interpreted by the question it answers, and
    /// the turn continues where it was.
    pub(super) fn advance(&mut self, pending: Pending, probe: &Probe, outcome: Outcome) -> Phase {
        let spent = outcome.spent;
        let reached = match pending {
            Pending::Detect { region: Some(_) } => Reached::Focused(self.admit(probe, outcome)),
            Pending::Detect { region: None } => Reached::Primary(self.admit(probe, outcome)),
            Pending::Reinforce { primary, .. } => {
                // The loss counts as checked only if reinforcement read bodies:
                // a failing look, or a reading of another kind, is requested
                // again on the next sample.
                if outcome.bodies().is_ok() {
                    self.engine.reinforced();
                }
                let (bodies, result) =
                    self.engine
                        .consolidate(primary, outcome.bodies(), self.room.as_ref());
                self.asked(probe, spent, result);
                Reached::Bodies(bodies)
            }
            Pending::Faces {
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
            Pending::PoseBody {
                mut perceived,
                target,
                ..
            } => {
                let result = self.engine.read_pose_body(
                    &mut perceived,
                    target,
                    outcome.into_pose(),
                    self.room.as_ref(),
                );
                self.asked(probe, spent, result);
                Reached::Evidence(perceived)
            }
            Pending::Calibrate { mut observed, .. } => {
                let (depth, result) = evidence::depth(outcome.depths());
                self.asked(probe, spent, result);
                observed.calibrated = self.engine.learn_bed(self.at, depth);
                Reached::Calibrated(observed)
            }
            Pending::Pose { bbox, observed, .. } => {
                let (pose, result) = evidence::pose(outcome.into_pose());
                self.asked(probe, spent, result);
                Reached::Pose {
                    observed,
                    pose,
                    bbox,
                }
            }
            Pending::Depth {
                observed,
                pose,
                bbox,
                at,
                head,
                ..
            } => {
                let (depths, result) = evidence::depths(outcome.depths(), &at, pose.len());
                self.asked(probe, spent, result);
                let primary_seen = observed.evidence.bodies.primary_seen();
                let enrichment = Enrichment::fresh(Some(pose), depths, head, bbox, primary_seen);
                Reached::Enriched(observed, enrichment)
            }
            Pending::Keep { refined, .. } => {
                let (result, kept) = self.engine.kept(self.at, outcome.kept());
                self.asked(probe, spent, result);
                // The burst is ready: the question goes out now, with the chosen crops.
                if let Some(Dispatch {
                    ticket,
                    look,
                    crops,
                }) = kept.and_then(|kept| kept.ready)
                {
                    return Phase::Waiting(Pending::Verify {
                        ticket,
                        look,
                        crops,
                        refined,
                    });
                }
                Reached::Final(refined)
            }
            Pending::Verify { refined, .. } => {
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
