//! The questions of space and attention: each is answered with the slice of the
//! policy that owns it, over what the turn sees through the glimpse.

use crate::attention::{
    self, BodyPoseLook, Considered, DepthLook, EnrichPlan, Glimpse, SubjectDepthLook,
};
use crate::belief::Believed;
use crate::body::{HeadPlace, Keypoint};
use crate::cycle::Engine;
use crate::evidence::{Admission, FaceEvidence, Perceived};
use crate::geom::{Point, Rect};
use crate::look::{FaceRegion, LookKind};
use crate::map::Geometry;
use crate::policy::FacePolicy;
use crate::room::Room;
use crate::time::SampleRef;

impl Engine {
    /// The room of this turn, built once when it opens.
    pub(in crate::cycle) fn room_of(&self, geometry: Geometry) -> Option<Room> {
        Room::of(&self.map, geometry, &self.space)
    }

    /// Where the main detector looks first: the focus, if the policy has one.
    pub(in crate::cycle) fn primary_region(
        &self,
        at: SampleRef,
        geometry: Geometry,
    ) -> Option<Rect> {
        attention::primary_region(
            &self.belief,
            &self.attention_memory,
            at,
            &self.map,
            geometry,
            self.focus.as_ref(),
        )
    }

    /// What attention sees of this turn: the sample's bodies and faces, read
    /// through the site.
    pub(in crate::cycle) fn glimpse<'g>(
        &'g self,
        at: SampleRef,
        geometry: Geometry,
        bodies: &'g Admission,
        faces: &'g FaceEvidence,
    ) -> Glimpse<'g> {
        Glimpse {
            at,
            geometry,
            map: &self.map,
            bodies: &bodies.bodies,
            faces,
        }
    }

    /// Whether the focus must widen to the whole frame.
    pub(in crate::cycle) fn widening(&self, g: &Glimpse<'_>) -> Considered<()> {
        Considered::of(LookKind::Detect, attention::widen(g, &self.belief))
    }

    /// Whether a loss is to be reinforced, over the region it asks for.
    pub(in crate::cycle) fn reinforcement(&self, g: &Glimpse<'_>) -> Considered<Rect> {
        Considered::of(
            LookKind::Reinforce,
            attention::reinforce(
                g,
                self.belief.bed_seen(),
                self.attention_memory.latch(),
                self.reinforce.as_ref(),
            ),
        )
    }

    /// Where to look for faces, with the face policy the look will read with.
    pub(in crate::cycle) fn faces_look(
        &self,
        g: &Glimpse<'_>,
    ) -> Considered<(FaceRegion, FacePolicy)> {
        Considered::of(LookKind::Faces, attention::faces(g, self.faces.as_ref()))
    }

    /// A doubtful body to place with its pose, before believing.
    pub(in crate::cycle) fn pose_look(
        &self,
        g: &Glimpse<'_>,
        room: Option<&Room>,
    ) -> Considered<BodyPoseLook> {
        Considered::of(
            LookKind::PoseBody,
            attention::pose_bodies(g, room, self.enrich.as_ref()),
        )
    }

    /// Whether the empty bed is to be measured to learn it.
    pub(in crate::cycle) fn calibration_look(
        &self,
        g: &Glimpse<'_>,
        believed: &Believed,
    ) -> Considered<DepthLook<Point>> {
        Considered::of(
            LookKind::Calibrate,
            attention::calibrate(
                g,
                believed,
                &self.calibration,
                &self.attention_memory,
                self.depth_pad,
                self.calibration_policy.as_ref(),
            ),
        )
    }

    /// Whether the subject is to be enriched with its pose, and where.
    pub(in crate::cycle) fn enrichment(
        &self,
        at: SampleRef,
        believed: &Believed,
        evidence: &Perceived,
    ) -> EnrichPlan {
        attention::enrich(
            believed,
            evidence.posed.as_ref().map(|p| p.bbox),
            self.pose_reference(),
            self.belief.pose_since(),
            at,
            self.enrich.as_ref(),
        )
    }

    /// Where the subject's head is, for the depth look.
    pub(in crate::cycle) fn head_place(
        &self,
        faces: &FaceEvidence,
        pose: Option<&[Keypoint]>,
        room: Option<&Room>,
    ) -> HeadPlace {
        crate::refine::head::of_turn(faces, pose, room, self.enrich.as_ref())
    }

    /// Whether the subject's depth is to be measured, and at which points.
    pub(in crate::cycle) fn depth_look(
        &self,
        g: &Glimpse<'_>,
        pose: Option<&[Keypoint]>,
        head: HeadPlace,
    ) -> Considered<SubjectDepthLook> {
        Considered::of(
            LookKind::Depth,
            attention::deepen(g, pose, head, self.depth_pad, self.enrich.as_ref()),
        )
    }

    /// A reinforcement look that answered: the loss counts as checked.
    pub(in crate::cycle) fn reinforcement_looked(&mut self) {
        self.attention_memory.reinforced();
    }

    /// The main detector looked over the whole frame at `at`.
    pub(in crate::cycle) fn looked_wide(&mut self, at: SampleRef) {
        self.attention_memory.looked_wide(at);
    }

    /// The bed's calibration was asked at `at`: its retry is measured from there.
    pub(in crate::cycle) fn calibration_attempted(&mut self, at: SampleRef) {
        self.attention_memory.calibration_attempted(at);
    }
}
