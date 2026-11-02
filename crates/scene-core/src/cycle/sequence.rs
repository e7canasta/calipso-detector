//! The order of a turn, in a single place (rule 11): `Course::next` decides what
//! the turn asks next and where it goes on. Each reading is interpreted in
//! `readings`, each question to `attention` is asked in `questions`, and what the
//! turn leaves behind is in `conclude`.

use super::pending::Pending;
use super::{Course, Observed, Phase, Refined};
use crate::attention::{EnrichPlan, SkipReason, SubjectDepthLook};
use crate::belief::Enrichment;
use crate::body::Keypoint;
use crate::evidence::{Admission, FaceEvidence, Perceived};
use crate::geom::Rect;
use crate::look::LookKind;
use crate::room::Room;
use crate::verification::{BurstStep, TurnOut};

/// How far the turn got, with what it gathered: the point from which
/// [`Course::next`] continues.
pub(super) enum Reached {
    /// With the main detector's result over the focus.
    Focused(Admission),
    /// With the main detector's result over the whole frame.
    Primary(Admission),
    /// With the sample's bodies (consolidated with reinforcement, if any).
    Bodies(Admission),
    /// With the bodies and the faces, before placing a doubtful body.
    Perceived(Perceived),
    /// With all the evidence.
    Evidence(Perceived),
    /// With the updated belief.
    Belief(Observed),
    /// With the empty bed learned, if it was measured.
    Calibrated(Observed),
    /// With the pose (or without it, if it failed).
    Pose {
        observed: Observed,
        pose: Option<Vec<Keypoint>>,
        bbox: Rect,
    },
    /// With the enrichment resolved.
    Enriched(Observed, Enrichment),
    /// With the subject's body concluded.
    Body(Observed),
    /// With the second level and layer 1.
    Refined(Refined),
    /// With every look done: only concluding remains.
    Final(Refined),
}

impl Course<'_> {
    /// The order of a turn. At each point `attention` is asked and a look is
    /// requested or the turn goes on.
    ///
    /// ```text
    /// Primary → [Widen] → [Reinforce] → [Faces] → [PoseBodies] → believe → [Calibrate] → [Pose → [Depth]] → refine → [Verify] → conclude
    /// ```
    pub(super) fn next(&mut self, mut reached: Reached) -> Phase {
        loop {
            reached = match reached {
                // The focus only confirms: otherwise, the whole frame.
                Reached::Focused(focused) => {
                    let considered = self.widen(&focused);
                    if self.wanted(LookKind::Detect, considered).is_some() {
                        return Phase::Waiting(Pending::Detect { region: None });
                    }
                    Reached::Primary(focused)
                }
                // First attention point, before believing: a loss to reinforce?
                Reached::Primary(primary) => {
                    let considered = self.reinforce(&primary);
                    if let Some(region) = self.wanted(LookKind::Reinforce, considered) {
                        return Phase::Waiting(Pending::Reinforce { region, primary });
                    }
                    Reached::Bodies(primary)
                }
                // Where to look for faces?
                Reached::Bodies(bodies) => {
                    let considered = self.faces(&bodies);
                    if let Some((region, policy)) = self.wanted(LookKind::Faces, considered) {
                        return Phase::Waiting(Pending::Faces {
                            region,
                            policy,
                            bodies,
                        });
                    }
                    Reached::Perceived(Perceived {
                        bodies,
                        faces: FaceEvidence::NotLooked,
                        posed: None,
                    })
                }
                // A doubtful body to place with its pose, before believing?
                Reached::Perceived(perceived) => {
                    let considered = self.pose_bodies(&perceived);
                    if let Some(look) = self.wanted(LookKind::PoseBody, considered) {
                        return Phase::Waiting(Pending::PoseBody {
                            region: look.region,
                            floor: look.floor,
                            perceived,
                        });
                    }
                    Reached::Evidence(perceived)
                }
                // Believe.
                Reached::Evidence(mut evidence) => {
                    evidence.cast_shadow(self.room.as_ref().and_then(Room::shadow));
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
                    if let Some(look) = self.wanted(LookKind::Calibrate, considered) {
                        self.engine.calibration_attempted(self.at);
                        return Phase::Waiting(Pending::Calibrate {
                            region: look.region,
                            points: look.points,
                            observed,
                        });
                    }
                    Reached::Calibrated(observed)
                }
                // Enrich the subject?
                Reached::Calibrated(observed) => {
                    match self.enrich(&observed.believed, &observed.evidence) {
                        EnrichPlan::Ask { bbox, region } => {
                            return Phase::Waiting(Pending::Pose {
                                region,
                                bbox,
                                observed,
                            });
                        }
                        EnrichPlan::Posed { bbox } => {
                            self.skipped(LookKind::Pose, SkipReason::PosedBefore);
                            let pose = observed.evidence.posed.as_ref().map(|p| p.pose.clone());
                            Reached::Pose {
                                observed,
                                pose,
                                bbox,
                            }
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
                    }
                }
                // Measure the subject's depth?
                Reached::Pose {
                    observed,
                    pose,
                    bbox,
                } => {
                    let head = self.head_place(&observed, pose.as_deref());
                    let considered = self.deepen(&observed, pose.as_deref(), head);
                    match (self.wanted(LookKind::Depth, considered), pose) {
                        (Some(points), Some(pose)) => {
                            let SubjectDepthLook { look, head } = points;
                            return Phase::Waiting(Pending::Depth {
                                region: look.region,
                                at: look.points,
                                pose,
                                bbox,
                                observed,
                                head,
                            });
                        }
                        (_, pose) => {
                            let primary_seen = observed.evidence.bodies.primary_seen();
                            let enrichment =
                                Enrichment::fresh(pose, None, head, bbox, primary_seen);
                            Reached::Enriched(observed, enrichment)
                        }
                    }
                }
                // Conclude.
                Reached::Enriched(mut observed, enrichment) => {
                    observed.believed =
                        self.engine
                            .observe_body(self.at, observed.believed, enrichment);
                    Reached::Body(observed)
                }
                // The second level and layer 1, on what was believed.
                Reached::Body(observed) => Reached::Refined(self.refine(observed)),
                // Last attention point: a burst being collected goes on; or
                // a doubt worth the slow verifier?
                Reached::Refined(refined) => {
                    match self.engine.verifier_turn(
                        self.at,
                        refined.bed_posture.map(|b| b.state),
                        refined.refinement.places,
                    ) {
                        // No burst being collected: a doubt may ask now.
                        TurnOut::Idle => return self.ask_verifier(refined),
                        TurnOut::Waited => {
                            self.note_busy_doubt(&refined);
                            Reached::Final(refined)
                        }
                        TurnOut::Keep { ticket, region } => {
                            self.note_busy_doubt(&refined);
                            return Phase::Waiting(Pending::Keep {
                                ticket,
                                region,
                                refined,
                            });
                        }
                        TurnOut::Dropped(step) => {
                            let reason = if step == BurstStep::Stale {
                                SkipReason::BurstStale
                            } else {
                                SkipReason::BurstResolved
                            };
                            self.skipped(LookKind::Verify, reason);
                            return self.ask_verifier(refined);
                        }
                    }
                }
                Reached::Final(refined) => return Phase::Concluded { refined },
            };
        }
    }
}
