//! What the engine does when the turn asks it: each operation changes the
//! engine's own memories together, in the order they depend on each other.
//! The turn says when (`sequence`); the engine knows what to change and with
//! which of its parts.

use super::Engine;
use crate::attention::{self, LookResult, VerifyLook};
use crate::belief::{Belief, Believed, OccupancyRules, bed_posture, parts};
use crate::evidence::Admission;
use crate::look::{Failure, Ticket};
use crate::map::{CalibrationMemory, SiteMap};
use crate::policy::Policy;
use crate::reception;
use crate::time::SampleRef;
use crate::verification::{Verification, VerifyRules};

/// How a question to the verifier was issued.
pub(super) enum Issued {
    /// One frame and no sharpness to choose by: this sample's crop goes out now.
    Now(Ticket),
    /// A burst: it starts by keeping this sample's crop.
    Collecting(Ticket),
}

impl Engine {
    /// The monitor is built: the policy is sliced here, once, and each decider
    /// keeps only its slice (ADR 2026-11-02, politica-en-rebanadas).
    pub(super) fn new(policy: Policy, map: SiteMap) -> Self {
        let occupancy_rules = OccupancyRules {
            persistence: policy.persistence.clone(),
            held_left: policy.held_left,
            near_first: policy.near_first,
        };
        let verify_rules = VerifyRules::of(policy.verify.as_ref());
        let bed_posture = policy.bed_posture;
        Self {
            policy,
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

    /// A sample arrives: the verifier's answers are settled as evidence and
    /// what it owes for too long expires, before anything is looked at.
    pub(super) fn settle(&mut self, at: SampleRef) {
        self.verification.settle(at, &self.belief);
        self.verification
            .expire(at, &self.verify_rules, &self.belief);
    }

    /// With all the evidence: the belief is updated, the reinforcement latch
    /// rearmed with what was seen in bed, and the held occupant followed.
    pub(super) fn believe(&mut self, at: SampleRef, bodies: &Admission) -> Believed {
        let believed = self.belief.observe(at, bodies, &self.occupancy_rules);
        self.attention_memory
            .rearm(bodies.primary.bed, self.belief.bed_seen());
        self.verification
            .observe_held(self.belief.held_since(), at, &self.verify_rules);
        believed
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

    /// The shell kept the burst's crop, this sharp, or could not.
    pub(super) fn kept(&mut self, at: SampleRef, kept: Result<f32, Failure>) -> LookResult {
        let sharpness_policy = self.verify_rules.sharpness;
        match kept {
            Ok(sharpness) => {
                let counted = self
                    .verification
                    .kept(at, sharpness, sharpness_policy.as_ref());
                LookResult::Kept { sharpness, counted }
            }
            Err(failure) => {
                self.verification.failed();
                LookResult::failed(failure)
            }
        }
    }

    /// A question to the verifier is decided: its ticket, sent now or
    /// collected as a burst (white book, volume III, ch. 12).
    pub(super) fn issue(&mut self, at: SampleRef, look: &VerifyLook) -> Issued {
        let burst = self
            .verify_rules
            .frames
            .map(|frames| frames.of(look.question))
            .unwrap_or_default();
        let sharpness = self.verify_rules.sharpness;
        let ticket = self.verification.issue(look.target, at, look.places);
        if burst.frames <= 1 && sharpness.is_none() {
            self.verification.dispatch(ticket, at);
            return Issued::Now(ticket);
        }
        self.verification.collect(ticket, *look, burst, at);
        Issued::Collecting(ticket)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geom::{Quad, Rect};
    use crate::policy::{Persistence, SharpnessPolicy, VerifyPolicy};
    use crate::units::Seconds;

    /// The slices are read once, from the same policy the engine keeps: a
    /// decider that reads a slice and one that reads the policy see one value.
    #[test]
    fn each_slice_is_the_value_the_policy_holds() {
        let policy = Policy {
            persistence: Persistence {
                dwell_secs: Seconds::ONE,
            },
            verify: Some(VerifyPolicy {
                sharpness: Some(SharpnessPolicy {
                    min_ratio: 0.8,
                    window: 4,
                    max_wait_secs: Seconds::TWO,
                }),
                ..VerifyPolicy::default()
            }),
            ..Policy::default()
        };
        let engine = Engine::new(
            policy,
            SiteMap::new(Quad::from_rect(Rect::new(0.0, 0.0, 10.0, 10.0))),
        );
        assert_eq!(
            engine.occupancy_rules.persistence.dwell_secs,
            engine.policy.persistence.dwell_secs
        );
        assert_eq!(
            engine.verify_rules.sharpness,
            engine.policy.verify.as_ref().and_then(|v| v.sharpness)
        );
        assert!(engine.verify_rules.sharpness.is_some());
    }
}
