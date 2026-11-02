//! What the turn believes: the occupancy with the evidence, the subject's body
//! with its enrichment, and the parts and layer 1 with the second level's sighting.

use crate::belief::{Believed, Enrichment, bed_posture, parts};
use crate::cycle::Engine;
use crate::evidence::Admission;
use crate::geom::Rect;
use crate::time::SampleRef;
use crate::verification::Event;

impl Engine {
    /// With all the evidence: the belief is updated, the reinforcement latch
    /// rearmed with what was seen in bed, and the held occupant followed.
    pub(in crate::cycle) fn believe(&mut self, at: SampleRef, bodies: &Admission) -> Believed {
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
    pub(in crate::cycle) fn observe_body(
        &mut self,
        at: SampleRef,
        believed: Believed,
        enrichment: Enrichment,
    ) -> Believed {
        self.belief.observe_body(at, believed, enrichment)
    }

    /// The parts and layer 1, on layer 0 and what the second level perceived.
    pub(in crate::cycle) fn follow(
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

    /// The box of the pose the belief follows, if any.
    pub(in crate::cycle) fn pose_reference(&self) -> Option<Rect> {
        self.belief.pose_reference()
    }
}
