//! What the engine does when the turn asks it: each operation changes the
//! engine's own memories together, in the order they depend on each other, and
//! answers each question with its own part of the policy. The turn says when
//! (`sequence`); the engine knows what to answer and with which of its parts.
//! Nothing outside this file reads a field of the engine.

use super::{Engine, Observed, Refined};
use crate::attention::{
    self, Attention, BodyPoseLook, Consideration, DepthLook, EnrichPlan, Glimpse, LookResult,
    SubjectDepthLook, VerifyLook,
};
use crate::belief::{Belief, Believed, Enrichment, OccupancyRules, bed_posture, parts};
use crate::body::{HeadPlace, Keypoint};
use crate::conclusion::Observation;
use crate::evidence::{self, Admission, FaceEvidence, Perceived, Sensed};
use crate::geom::{Point, Rect};
use crate::look::{Detection, FaceRegion, Failure, Verified};
use crate::map::{BedCalibrated, CalibrationMemory, FrameInfo, Geometry, MapState, SiteMap};
use crate::policy::{FacePolicy, Policy};
use crate::reception::{self, SensorChanged, SensorState};
use crate::room::Room;
use crate::time::SampleRef;
use crate::verification::{Effect, Event, Verification, VerifyRules};

impl Engine {
    /// The monitor is built: the policy is sliced here, once, and each decider
    /// keeps only its slice (ADR 2026-11-02, politica-en-rebanadas).
    pub(super) fn new(policy: Policy, map: SiteMap) -> Self {
        // Without `..`: a field added to `Policy` does not compile until
        // someone decides which slice of the engine owns it.
        let Policy {
            admission,
            persistence,
            faces,
            reinforce,
            focus,
            enrich,
            calibration,
            depth_pad,
            bed_posture,
            verify,
            shadow,
            two_planes,
            held_left,
            near_first,
        } = policy;
        let occupancy_rules = OccupancyRules {
            persistence,
            held_left,
            near_first,
        };
        let verify_rules = VerifyRules::of(verify.as_ref());
        Self {
            admission,
            reinforce,
            faces,
            enrich,
            calibration_policy: calibration,
            depth_pad,
            focus,
            shadow,
            two_planes,
            verify_policy: verify,
            calibration: CalibrationMemory::from(&map),
            attention_memory: attention::Memory::of(map.calibration_attempt),
            map: map.into(),
            belief: Belief::default(),
            reception: reception::Memory::default(),
            verification: Verification::default(),
            frame: None,
            occupancy_rules,
            verify_rules,
            bed_posture,
        }
    }

    // --- What arrives: the sensor, the frame, the answers.

    /// The shell reports a reception change: the engine notes it and says
    /// whether it changed.
    pub(super) fn sensor_reported(
        &mut self,
        at: SampleRef,
        state: SensorState,
    ) -> Option<SensorChanged> {
        self.reception.report(at, state)
    }

    /// A sample opens with its frame: the engine keeps its size and says what
    /// geometry the site has for it.
    pub(super) fn open_frame(&mut self, frame: FrameInfo) -> Geometry {
        self.frame = Some(frame);
        self.map.gate(frame)
    }

    /// A verifier's answer, between turns: noted, settled at the next turn.
    pub(super) fn answered(&mut self, verified: Verified) {
        self.verification.step(Event::Answer(verified));
    }

    /// A sample's turn starts: the verifier's answers are settled as evidence and
    /// what it owes for too long expires, before anything is looked at.
    pub(super) fn settle(&mut self, at: SampleRef) {
        let layer_1 = self.belief.bed_posture_state();
        self.verification.step(Event::Settle { at, layer_1 });
        self.verification.step(Event::Expire {
            at,
            timeout: self.verify_rules.timeout_secs,
            layer_1,
        });
    }

    // --- What the questions of space and attention are answered with.

    /// The room of this turn, built once when it opens.
    pub(super) fn room_of(&self, geometry: Geometry) -> Option<Room> {
        Room::of(
            &self.map,
            geometry,
            &self.admission,
            self.two_planes.as_ref(),
            self.shadow.as_ref(),
        )
    }

    /// Where the main detector looks first: the focus, if the policy has one.
    pub(super) fn primary_region(&self, at: SampleRef, geometry: Geometry) -> Option<Rect> {
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
    pub(super) fn glimpse<'g>(
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
    pub(super) fn widening(&self, g: &Glimpse<'_>) -> Consideration<()> {
        attention::widen(g, &self.belief)
    }

    /// Whether a loss is to be reinforced, over the region it asks for.
    pub(super) fn reinforcement(&self, g: &Glimpse<'_>) -> Consideration<Rect> {
        attention::reinforce(
            g,
            self.belief.bed_seen(),
            self.attention_memory.latch(),
            self.reinforce.as_ref(),
        )
    }

    /// Where to look for faces, with the face policy the look will read with.
    pub(super) fn faces_look(&self, g: &Glimpse<'_>) -> Consideration<(FaceRegion, FacePolicy)> {
        attention::faces(g, self.faces.as_ref())
    }

    /// A doubtful body to place with its pose, before believing.
    pub(super) fn pose_look(
        &self,
        g: &Glimpse<'_>,
        room: Option<&Room>,
    ) -> Consideration<BodyPoseLook> {
        attention::pose_bodies(g, room, self.enrich.as_ref())
    }

    /// Whether the empty bed is to be measured to learn it.
    pub(super) fn calibration_look(
        &self,
        g: &Glimpse<'_>,
        believed: &Believed,
    ) -> Consideration<DepthLook<Point>> {
        attention::calibrate(
            g,
            believed,
            &self.calibration,
            &self.attention_memory,
            self.depth_pad,
            self.calibration_policy.as_ref(),
        )
    }

    /// Whether the subject is to be enriched with its pose, and where.
    pub(super) fn enrichment(
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

    /// Whether a doubt is worth the slow verifier, over what the turn observed.
    pub(super) fn verify_look(
        &self,
        g: &Glimpse<'_>,
        layer_1: Option<bed_posture::BedPostureState>,
        places: Option<crate::refine::BodyPlaces>,
    ) -> Consideration<VerifyLook> {
        attention::verify(
            g,
            &self.verification,
            layer_1,
            places,
            self.verify_policy.as_ref(),
        )
    }

    /// Where the subject's head is, for the depth look.
    pub(super) fn head_place(
        &self,
        faces: &FaceEvidence,
        pose: Option<&[Keypoint]>,
        room: Option<&Room>,
    ) -> HeadPlace {
        crate::refine::head::of_turn(faces, pose, room, self.enrich.as_ref())
    }

    /// Whether the subject's depth is to be measured, and at which points.
    pub(super) fn depth_look(
        &self,
        g: &Glimpse<'_>,
        pose: Option<&[Keypoint]>,
        head: HeadPlace,
    ) -> Consideration<SubjectDepthLook> {
        attention::deepen(g, pose, head, self.depth_pad, self.enrich.as_ref())
    }

    // --- What the readings fill: the evidence, with the policy it reads with.

    /// The main detector's reading, admitted by the zones of the room.
    pub(super) fn admit(&self, reading: Sensed<&[Detection]>, room: Option<&Room>) -> Admission {
        evidence::admit(reading, room, &self.admission)
    }

    /// Reinforcement's reading joined with the main detector's.
    pub(super) fn consolidate(
        &self,
        primary: Admission,
        reinforcement: Sensed<&[Detection]>,
        room: Option<&Room>,
    ) -> (Admission, LookResult) {
        evidence::consolidate(primary, reinforcement, room, &self.admission)
    }

    /// A doubtful body's pose, placed by the room: `perceived` changes if the
    /// pose says where the body is.
    pub(super) fn read_pose_body(
        &self,
        perceived: &mut Perceived,
        floor: usize,
        reading: Sensed<Option<Vec<Keypoint>>>,
        room: Option<&Room>,
    ) -> LookResult {
        evidence::pose_body(perceived, floor, reading, room, self.enrich.as_ref())
    }

    /// The empty bed's depth, learned if the turn measured it.
    pub(super) fn learn_bed(&mut self, at: SampleRef, depth: Option<f32>) -> Option<BedCalibrated> {
        self.calibration.learn(at, depth)
    }

    /// A reinforcement look that answered: the loss counts as checked.
    pub(super) fn reinforcement_looked(&mut self) {
        self.attention_memory.reinforced();
    }

    /// The main detector looked over the whole frame at `at`.
    pub(super) fn looked_wide(&mut self, at: SampleRef) {
        self.attention_memory.looked_wide(at);
    }

    /// The bed's calibration was asked at `at`: its retry is measured from there.
    pub(super) fn calibration_attempted(&mut self, at: SampleRef) {
        self.attention_memory.calibration_attempted(at);
    }

    // --- What the turn decides with the evidence.

    /// With all the evidence: the belief is updated, the reinforcement latch
    /// rearmed with what was seen in bed, and the held occupant followed.
    pub(super) fn believe(&mut self, at: SampleRef, bodies: &Admission) -> Believed {
        let believed = self.belief.observe(at, bodies, &self.occupancy_rules);
        self.attention_memory
            .rearm(bodies.primary.bed, self.belief.bed_seen());
        self.verification.step(Event::ObserveHeld {
            at,
            oldest: self.belief.held_since(),
            followed: self.verify_rules.held,
        });
        believed
    }

    /// The subject's pose and depth, with the enrichment resolved.
    pub(super) fn observe_body(
        &mut self,
        at: SampleRef,
        believed: Believed,
        enrichment: Enrichment,
    ) -> Believed {
        self.belief.observe_body(at, believed, enrichment)
    }

    /// The parts and layer 1, on layer 0 and what the second level perceived.
    pub(super) fn follow(
        &mut self,
        at: SampleRef,
        seen: Option<parts::ByPart<parts::Look>>,
        sighting: bed_posture::Sighting,
    ) -> (Option<parts::Concluded>, Option<bed_posture::Concluded>) {
        let parts = self
            .belief
            .observe_parts(at, seen, &self.occupancy_rules.persistence);
        let bed_posture = self
            .belief
            .observe_bed_posture(at, sighting, self.bed_posture.as_ref());
        (parts, bed_posture)
    }

    /// The second level reads what was believed and delivers its result, with
    /// the digit the verifier filled if an answer arrived; layer 1 reads it.
    pub(super) fn refine(
        &mut self,
        at: SampleRef,
        observed: Observed,
        room: Option<&Room>,
    ) -> Refined {
        let filled = self.verification.take_filled();
        let refinement = crate::refine::refine(
            observed.believed.subject.as_ref().map(|s| s.zone),
            observed.believed.body.as_ref(),
            room,
            self.enrich.as_ref(),
            self.verify_policy.as_ref(),
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
            room,
            min_score: self.enrich.as_ref().map(|e| e.min_keypoint_score.get()),
        });
        // Layer 1, on layer 0 and what the second level perceived.
        let (parts, bed_posture) = self.follow(at, seen, refinement.sighting);
        Refined {
            observed,
            refinement,
            bed_posture,
            parts,
        }
    }

    /// A kept crop's reading: the verifier's burst takes it, or it fails.
    pub(super) fn kept(
        &mut self,
        at: SampleRef,
        kept: Result<f32, Failure>,
    ) -> (LookResult, Vec<Effect>) {
        match kept {
            Ok(sharpness) => {
                let effects = self.verification.step(Event::KeptCrop {
                    at,
                    sharpness,
                    policy: self.verify_rules.sharpness,
                });
                // A kept crop's first effect is its verdict.
                let counted = matches!(effects.first(), Some(Effect::Kept { counted: true }));
                (LookResult::Kept { sharpness, counted }, effects)
            }
            Err(failure) => {
                self.verification.step(Event::KeepFailed);
                (LookResult::failed(failure), Vec::new())
            }
        }
    }

    /// A question to the verifier is decided: sent now, or collected as a burst
    /// (white book, volume III, ch. 12). The effects say which.
    pub(super) fn issue(&mut self, at: SampleRef, look: &VerifyLook) -> Vec<Effect> {
        let burst = self
            .verify_rules
            .frames
            .map(|frames| frames.of(look.question))
            .unwrap_or_default();
        self.verification.step(Event::Ask {
            at,
            look: *look,
            burst,
            judged: self.verify_rules.sharpness.is_some(),
        })
    }

    /// The verifier's request left (`ok`) or could not.
    pub(super) fn dispatched(&mut self, ok: bool) {
        self.verification.step(Event::Dispatched { ok });
    }

    /// The turn reaches the verifier's last attention point: a burst being
    /// collected advances with this sample. The effects say what it does.
    pub(super) fn verifier_turn(
        &mut self,
        at: SampleRef,
        layer_1: Option<bed_posture::BedPostureState>,
        places: Option<crate::refine::BodyPlaces>,
    ) -> Vec<Effect> {
        self.verification.step(Event::Turn {
            at,
            layer_1,
            places,
        })
    }

    // --- What the turn leaves behind.

    /// What the engine observed in the turn. It reads neither the chronicle
    /// nor the last scene: the engine has no way to.
    pub(super) fn observation(
        &self,
        at: SampleRef,
        geometry: Geometry,
        refined: Refined,
        attention: Attention,
    ) -> Observation {
        let Refined {
            observed,
            refinement,
            bed_posture,
            parts,
        } = refined;
        Observation {
            at,
            believed: observed.believed,
            refined: refinement,
            bed_posture,
            parts,
            held_check: self.verification.held(),
            evidence: observed.evidence,
            frame: self.frame,
            calibrated: observed.calibrated,
            attention,
            map: self.map_state(geometry),
            sensor: self.sensor_state(),
            dwell_secs: self.occupancy_rules.persistence.dwell_secs,
        }
    }

    /// The reception as it stands, for a scene.
    pub(super) fn sensor_state(&self) -> SensorState {
        self.reception.state()
    }

    /// The site as it stands for a scene: the geometry of this turn, with the
    /// focus, the shadow and the calibration the policy asks for.
    pub(super) fn map_state(&self, geometry: Geometry) -> MapState {
        self.map.state(
            geometry,
            self.focus.as_ref(),
            self.shadow.as_ref(),
            &self.admission,
            &self.calibration,
        )
    }

    /// The box of the pose the belief follows, if any.
    pub(super) fn pose_reference(&self) -> Option<Rect> {
        self.belief.pose_reference()
    }
}
