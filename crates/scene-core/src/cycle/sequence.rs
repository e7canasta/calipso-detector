//! The order of a turn and how each reading moves it on. It lives here and
//! nowhere else (rule 11): at each point `attention` is asked, a look is
//! requested or the turn goes on, and every answer is interpreted by
//! `evidence`, `belief` or `map`.

use super::waiting::{Continuation, Waiting};
use super::{Course, Engine, Observed, Phase, Refined};
use crate::attention::Consideration::{Decided, Disabled, Inapplicable};
use crate::attention::{
    self, BodyPoseLook, BurstStep, Consideration, Decision, DepthLook, EnrichPlan, Glimpse, Look,
    LookOutcome, LookResult, SkipReason, SubjectDepthLook, VerifyLook,
};
use crate::belief::{Believed, Enrichment};
use crate::body::Keypoint;
use crate::conclusion::{Conclusion, Observation, RepublishReason};
use crate::evidence::{self, Admission, FaceEvidence, Perceived};
use crate::geom::{Point, Rect};
use crate::head::HeadPlace;
use crate::look::{FaceRegion, LookKind, Outcome, Probe, Spent};
use crate::map::{FrameInfo, Geometry};
use crate::policy::FacePolicy;
use crate::room::Room;
use crate::sensor::SensorChanged;
use crate::verification::Collecting;

/// How far the turn got, with what it gathered: the point from which
/// [`Course::next`] continues.
enum Reached {
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
    /// With the subject's body concluded.
    Body(Observed),
    /// With the second level and layer 1.
    Refined(Refined),
    /// With every look done: only concluding remains.
    Final(Refined),
}

impl Course<'_> {
    /// Opens a turn over a sample: the geometry and the main detector, which
    /// always runs. Neither a reconnection nor another image size releases
    /// anyone: the belief starts from scratch only with the process.
    pub(super) fn open(&mut self, frame: FrameInfo) -> Phase {
        let Engine {
            policy,
            belief,
            verification,
            ..
        } = &mut self.monitor.engine;
        verification.settle(self.at, belief);
        verification.expire(self.at, policy, belief);
        self.monitor.engine.frame = Some(frame);
        self.geometry = self.monitor.engine.map.gate(frame);
        let Engine {
            policy,
            map,
            belief,
            lens,
            ..
        } = &self.monitor.engine;
        self.room = Room::of(map, self.geometry, policy);
        let region = attention::primary_region(belief, lens, self.at, map, self.geometry, policy);
        Phase::Waiting(Waiting::primary(region))
    }

    /// The order of a turn. At each point `attention` is asked and a look is
    /// requested or the turn goes on.
    ///
    /// ```text
    /// Primary → [Widen] → [Reinforce] → [Faces] → [PoseBodies] → believe → [Calibrate] → [Pose → [Depth]] → refine → [Verify] → conclude
    /// ```
    fn next(&mut self, mut reached: Reached) -> Phase {
        loop {
            reached = match reached {
                // The focus only confirms: otherwise, the whole frame.
                Reached::Focused(focused) => {
                    let considered = self.widen(&focused);
                    if self.wanted(LookKind::Detect, considered).is_some() {
                        return Phase::Waiting(Waiting::widen());
                    }
                    Reached::Primary(focused)
                }
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
                        return Phase::Waiting(Waiting::pose_body(perceived, look));
                    }
                    Reached::Evidence(perceived)
                }
                // Believe.
                Reached::Evidence(mut evidence) => {
                    let engine = &self.monitor.engine;
                    let site = crate::shadow::Site::of(&engine.map, self.geometry, &engine.policy);
                    evidence.cast_shadow(site.as_ref());
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
                        self.monitor.engine.map.attempted(self.at);
                        return Phase::Waiting(Waiting::calibrate(observed, look));
                    }
                    Reached::Map(observed)
                }
                // Enrich the subject?
                Reached::Map(observed) => match self.enrich(&observed.believed, &observed.evidence)
                {
                    EnrichPlan::Ask { bbox, region } => {
                        return Phase::Waiting(Waiting::pose(observed, bbox, region));
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
                },
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
                            return Phase::Waiting(Waiting::depth(observed, pose, bbox, points));
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
                    observed.believed = self.monitor.engine.belief.observe_body(
                        self.at,
                        observed.believed,
                        enrichment,
                    );
                    Reached::Body(observed)
                }
                // The second level and layer 1, on what was believed.
                Reached::Body(observed) => Reached::Refined(self.refine(observed)),
                // Last attention point: a burst being collected goes on; or
                // a doubt worth the slow verifier?
                Reached::Refined(refined) => {
                    let Some((step, burst)) = self.burst(&refined) else {
                        return self.ask_verifier(refined);
                    };
                    match step {
                        BurstStep::Wait => Reached::Final(refined),
                        BurstStep::Keep => {
                            let (ticket, region) = (burst.ticket, burst.look.region);
                            return Phase::Waiting(Waiting::keep(refined, ticket, region));
                        }
                        BurstStep::Stale | BurstStep::Resolved => {
                            self.monitor.engine.verification.drop_burst();
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
                // The loss counts as checked only if reinforcement looked: a
                // failing reinforcement is requested again on the next sample.
                if outcome.result.is_ok() {
                    self.monitor.engine.belief.reinforced();
                }
                let (bodies, result) = evidence::consolidate(
                    primary,
                    outcome.bodies(),
                    self.room.as_ref(),
                    &self.monitor.engine.policy.admission,
                );
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
                let result = evidence::pose_body(
                    &mut perceived,
                    floor,
                    outcome.into_pose(),
                    self.room.as_ref(),
                    &self.monitor.engine.policy,
                );
                self.asked(probe, spent, result);
                Reached::Evidence(perceived)
            }
            Continuation::Calibrate { mut observed } => {
                let (depth, result) = evidence::depth(outcome.depths());
                self.asked(probe, spent, result);
                observed.calibrated = self.monitor.engine.map.learn(self.at, depth);
                Reached::Map(observed)
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
                let Engine {
                    policy,
                    verification,
                    ..
                } = &mut self.monitor.engine;
                let sharpness_policy = policy.verify.as_ref().and_then(|v| v.sharpness);
                let result = match outcome.kept() {
                    Ok(sharpness) => {
                        let counted =
                            verification.kept(self.at, sharpness, sharpness_policy.as_ref());
                        LookResult::Kept { sharpness, counted }
                    }
                    Err(failure) => {
                        verification.failed();
                        LookResult::failed(failure)
                    }
                };
                self.asked(probe, spent, result);
                // The burst is ready: the question goes out now, with the chosen crops.
                let verification = &mut self.monitor.engine.verification;
                let ready = verification.collecting().cloned().and_then(|c| {
                    attention::burst_ready(self.at, &c, sharpness_policy.as_ref())
                        .map(|crops| (c, crops))
                });
                if let Some((burst, crops)) = ready {
                    verification.dispatch(burst.ticket, self.at);
                    return Phase::Waiting(Waiting::verify(
                        refined,
                        burst.ticket,
                        burst.look,
                        crops,
                    ));
                }
                Reached::Final(refined)
            }
            Continuation::Verify { refined } => {
                let result = match outcome.dispatched() {
                    Ok(()) => LookResult::Dispatched,
                    Err(failure) => {
                        self.monitor.engine.verification.failed();
                        LookResult::failed(failure)
                    }
                };
                self.asked(probe, spent, result);
                Reached::Final(refined)
            }
        };
        self.next(reached)
    }

    /// The second level reads what was concluded and delivers its result, with
    /// the digit the verifier filled if an answer arrived; layer 1 reads it.
    fn refine(&mut self, observed: Observed) -> Refined {
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
            min_score: engine.policy.enrich.as_ref().map(|e| e.min_keypoint_score),
        });
        // Layer 1, on layer 0 and what the second level perceived.
        let at = self.at;
        let Engine { policy, belief, .. } = &mut self.monitor.engine;
        let parts = belief.observe_parts(at, seen, policy);
        let bed_posture = belief.observe_bed_posture(at, refinement.sighting, policy);
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
            map: engine.map.state(self.geometry, &engine.policy),
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
            map: Box::new(engine.map.state(Geometry::Valid, &engine.policy)),
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

    fn widen(&self, focused: &Admission) -> Consideration<()> {
        let g = self.glimpse(focused, &FaceEvidence::NotLooked);
        attention::widen(&g, &self.monitor.engine.belief)
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

    fn pose_bodies(&self, perceived: &Perceived) -> Consideration<BodyPoseLook> {
        let g = self.glimpse(&perceived.bodies, &perceived.faces);
        attention::pose_bodies(&g, self.room.as_ref(), &self.monitor.engine.policy)
    }

    fn calibrate(&self, observed: &Observed) -> Consideration<DepthLook<Point>> {
        let Perceived { bodies, faces, .. } = &observed.evidence;
        let g = self.glimpse(bodies, faces);
        attention::calibrate(&g, &observed.believed, &self.monitor.engine.policy)
    }

    fn enrich(&self, believed: &Believed, evidence: &Perceived) -> EnrichPlan {
        let Engine { policy, belief, .. } = &self.monitor.engine;
        let (reference, since) = (belief.pose_reference(), belief.pose_since());
        let posed = evidence.posed.as_ref().map(|p| p.bbox);
        attention::enrich(believed, posed, reference, since, self.at, policy)
    }

    /// A question to the slow verifier, if a doubt is worth one. With a burst
    /// (white book, volume III, ch. 12) it starts by keeping this sample's
    /// crop; with one frame it goes out now.
    fn ask_verifier(&mut self, refined: Refined) -> Phase {
        let considered = self.verify(&refined);
        let Some(look) = self.wanted(LookKind::Verify, considered) else {
            return Phase::Concluded { refined };
        };
        let Engine {
            policy,
            verification,
            ..
        } = &mut self.monitor.engine;
        let burst = policy
            .verify
            .map(|v| v.frames.of(look.question))
            .unwrap_or_default();
        let sharpness = policy.verify.and_then(|v| v.sharpness);
        let ticket = verification.issue(look.target, self.at, look.places);
        // One frame and no sharpness to choose by: this sample's crop, now.
        if burst.frames <= 1 && sharpness.is_none() {
            verification.dispatch(ticket, self.at);
            return Phase::Waiting(Waiting::verify(refined, ticket, look, Vec::new()));
        }
        verification.collect(ticket, look, burst, self.at);
        Phase::Waiting(Waiting::keep(refined, ticket, look.region))
    }

    /// The step of the burst being collected, if any, with the burst.
    fn burst(&self, refined: &Refined) -> Option<(BurstStep, Collecting)> {
        let verification = &self.monitor.engine.verification;
        let collecting = verification.collecting()?.clone();
        let layer_1 = refined.bed_posture.map(|b| b.state);
        let held = verification.held().map(|h| h.since);
        let step = attention::burst(
            self.at,
            &collecting,
            layer_1,
            refined.refinement.places,
            held,
        );
        Some((step, collecting))
    }

    fn verify(&self, refined: &Refined) -> Consideration<VerifyLook> {
        let Perceived { bodies, faces, .. } = &refined.observed.evidence;
        let g = self.glimpse(bodies, faces);
        let Engine {
            policy,
            verification,
            ..
        } = &self.monitor.engine;
        let layer_1 = refined.bed_posture.map(|b| b.state);
        attention::verify(&g, verification, layer_1, refined.refinement.places, policy)
    }

    fn head_place(&self, observed: &Observed, pose: Option<&[Keypoint]>) -> HeadPlace {
        crate::head::of_turn(
            &observed.evidence.faces,
            pose,
            self.room.as_ref(),
            &self.monitor.engine.policy,
        )
    }

    fn deepen(
        &self,
        observed: &Observed,
        pose: Option<&[Keypoint]>,
        head: HeadPlace,
    ) -> Consideration<SubjectDepthLook> {
        let Perceived { bodies, faces, .. } = &observed.evidence;
        let g = self.glimpse(bodies, faces);
        attention::deepen(&g, pose, head, &self.monitor.engine.policy)
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

    /// The main detector's reading, admitted by zones and noted in the ledger.
    fn admit(&mut self, probe: &Probe, outcome: Outcome) -> Admission {
        let admission = evidence::admit(
            outcome.bodies(),
            self.room.as_ref(),
            &self.monitor.engine.policy.admission,
        );
        self.asked(probe, outcome.spent, admission.result.clone());
        if probe.region().is_none() && matches!(admission.result, LookResult::Read { .. }) {
            self.monitor.engine.lens.looked_wide(self.at);
        }
        admission
    }

    /// With all the evidence: the belief is updated.
    fn believe(&mut self, bodies: &Admission) -> Believed {
        let Engine {
            policy,
            belief,
            verification,
            ..
        } = &mut self.monitor.engine;
        let believed = belief.observe(self.at, bodies, policy);
        verification.observe_held(belief.held_since(), self.at, policy);
        believed
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
