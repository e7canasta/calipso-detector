//! The questions to attention: each one gets the whole policy, and the sequencer
//! never asks which senses are configured (rule 11). Each answer comes back as a
//! consideration: a look is asked, or the turn goes on.

use super::waiting::Waiting;
use super::{Course, Engine, Observed, Phase, Refined};
use crate::attention::Consideration::{Decided, Disabled, Inapplicable};
use crate::attention::{
    self, BodyPoseLook, Consideration, Decision, DepthLook, EnrichPlan, Glimpse, SubjectDepthLook,
    VerifyLook,
};
use crate::belief::Believed;
use crate::body::HeadPlace;
use crate::body::Keypoint;
use crate::evidence::{Admission, FaceEvidence, Perceived};
use crate::geom::{Point, Rect};
use crate::look::{FaceRegion, LookKind};
use crate::policy::FacePolicy;
use crate::verification::Effect;

impl Course<'_> {
    // --- The questions to attention: each one gets the whole policy and the
    // sequencer never asks which senses are configured (rule 11).

    /// What attention sees of this turn.
    pub(super) fn glimpse<'g>(
        &'g self,
        bodies: &'g Admission,
        faces: &'g FaceEvidence,
    ) -> Glimpse<'g> {
        Glimpse {
            at: self.at,
            geometry: self.geometry,
            map: &self.monitor.engine.map,
            bodies: &bodies.bodies,
            faces,
        }
    }

    pub(super) fn widen(&self, focused: &Admission) -> Consideration<()> {
        let g = self.glimpse(focused, &FaceEvidence::NotLooked);
        attention::widen(&g, &self.monitor.engine.belief)
    }

    pub(super) fn reinforce(&self, primary: &Admission) -> Consideration<Rect> {
        let g = self.glimpse(primary, &FaceEvidence::NotLooked);
        let Engine {
            policy,
            belief,
            attention_memory,
            ..
        } = &self.monitor.engine;
        attention::reinforce(
            &g,
            belief.bed_seen(),
            attention_memory.latch(),
            policy.reinforce.as_ref(),
        )
    }

    pub(super) fn faces(&self, bodies: &Admission) -> Consideration<(FaceRegion, FacePolicy)> {
        let g = self.glimpse(bodies, &FaceEvidence::NotLooked);
        attention::faces(&g, self.monitor.engine.policy.faces.as_ref())
    }

    pub(super) fn pose_bodies(&self, perceived: &Perceived) -> Consideration<BodyPoseLook> {
        let g = self.glimpse(&perceived.bodies, &perceived.faces);
        attention::pose_bodies(
            &g,
            self.room.as_ref(),
            self.monitor.engine.policy.enrich.as_ref(),
        )
    }

    pub(super) fn calibrate(&self, observed: &Observed) -> Consideration<DepthLook<Point>> {
        let Perceived { bodies, faces, .. } = &observed.evidence;
        let g = self.glimpse(bodies, faces);
        let engine = &self.monitor.engine;
        attention::calibrate(
            &g,
            &observed.believed,
            &engine.calibration,
            &engine.attention_memory,
            engine.policy.depth_pad.map(|d| d.get()),
            engine.policy.calibration.as_ref(),
        )
    }

    pub(super) fn enrich(&self, believed: &Believed, evidence: &Perceived) -> EnrichPlan {
        let Engine { policy, belief, .. } = &self.monitor.engine;
        let (reference, since) = (belief.pose_reference(), belief.pose_since());
        let posed = evidence.posed.as_ref().map(|p| p.bbox);
        attention::enrich(
            believed,
            posed,
            reference,
            since,
            self.at,
            policy.enrich.as_ref(),
        )
    }

    /// A question to the slow verifier, if a doubt is worth one. With a burst
    /// (white book, volume III, ch. 12) it starts by keeping this sample's
    /// crop; with one frame it goes out now.
    pub(super) fn ask_verifier(&mut self, refined: Refined) -> Phase {
        let considered = self.verify(&refined);
        let Some(look) = self.wanted(LookKind::Verify, considered) else {
            return Phase::Concluded { refined };
        };
        // An ask emits exactly one probe: the request, or the first crop of its burst.
        match self.monitor.engine.issue(self.at, &look).first() {
            Some(Effect::ProbeVerify {
                ticket,
                look,
                crops,
            }) => Phase::Waiting(Waiting::verify(refined, *ticket, *look, crops.clone())),
            Some(Effect::ProbeKeep { ticket, region }) => {
                Phase::Waiting(Waiting::keep(refined, *ticket, *region))
            }
            Some(Effect::Skipped(reason)) => {
                self.skipped(LookKind::Verify, *reason);
                Phase::Concluded { refined }
            }
            _ => Phase::Concluded { refined },
        }
    }

    /// A doubt that comes while a burst is being collected cannot be asked yet:
    /// the ledger says so (P4). Attention refuses it as busy, and the refusal is
    /// what gets noted; a doubt that is not there notes nothing.
    pub(super) fn note_busy_doubt(&mut self, refined: &Refined) {
        let considered = self.verify(refined);
        let _ = self.wanted(LookKind::Verify, considered);
    }

    pub(super) fn verify(&self, refined: &Refined) -> Consideration<VerifyLook> {
        let Perceived { bodies, faces, .. } = &refined.observed.evidence;
        let g = self.glimpse(bodies, faces);
        let Engine {
            policy,
            verification,
            ..
        } = &self.monitor.engine;
        let layer_1 = refined.bed_posture.map(|b| b.state);
        attention::verify(
            &g,
            verification,
            layer_1,
            refined.refinement.places,
            policy.verify.as_ref(),
        )
    }

    pub(super) fn head_place(&self, observed: &Observed, pose: Option<&[Keypoint]>) -> HeadPlace {
        crate::refine::head::of_turn(
            &observed.evidence.faces,
            pose,
            self.room.as_ref(),
            self.monitor.engine.policy.enrich.as_ref(),
        )
    }

    pub(super) fn deepen(
        &self,
        observed: &Observed,
        pose: Option<&[Keypoint]>,
        head: HeadPlace,
    ) -> Consideration<SubjectDepthLook> {
        let Perceived { bodies, faces, .. } = &observed.evidence;
        let g = self.glimpse(bodies, faces);
        let policy = &self.monitor.engine.policy;
        attention::deepen(
            &g,
            pose,
            head,
            policy.depth_pad.map(|d| d.get()),
            policy.enrich.as_ref(),
        )
    }

    /// What attention wants looked at, if anything. A no with its reason goes
    /// to the ledger (rule 12); with nothing to decide, nothing does.
    pub(super) fn wanted<T>(&mut self, kind: LookKind, considered: Consideration<T>) -> Option<T> {
        match considered {
            Decided(Decision::Ask(what)) => Some(what),
            Decided(Decision::Skip(reason)) => {
                self.skipped(kind, reason);
                None
            }
            Disabled | Inapplicable => None,
        }
    }
}
