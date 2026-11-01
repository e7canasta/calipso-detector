//! What the engine does when the turn asks it: each operation changes the
//! engine's own memories together, in the order they depend on each other.
//! The turn says when (`sequence`); the engine knows what to change and with
//! which of its parts.

use super::Engine;
use crate::attention::{self, LookResult, VerifyLook};
use crate::belief::{Belief, Believed, bed_posture, parts};
use crate::evidence::Admission;
use crate::look::{Failure, Ticket};
use crate::map::{CalibrationMemory, SiteMap};
use crate::policy::Policy;
use crate::sensor::SensorMemory;
use crate::time::SampleRef;
use crate::verification::Verification;

/// How a question to the verifier was issued.
pub(super) enum Issued {
    /// One frame and no sharpness to choose by: this sample's crop goes out now.
    Now(Ticket),
    /// A burst: it starts by keeping this sample's crop.
    Collecting(Ticket),
}

impl Engine {
    pub(super) fn new(policy: Policy, map: SiteMap) -> Self {
        Self {
            policy,
            calibration: CalibrationMemory::from(&map),
            attention_memory: attention::Memory::of(map.calibration_attempt),
            map: map.into(),
            belief: Belief::default(),
            sensor: SensorMemory::default(),
            verification: Verification::default(),
            frame: None,
        }
    }

    /// A sample arrives: the verifier's answers are settled as evidence and
    /// what it owes for too long expires, before anything is looked at.
    pub(super) fn settle(&mut self, at: SampleRef) {
        self.verification.settle(at, &self.belief);
        self.verification.expire(at, &self.policy, &self.belief);
    }

    /// With all the evidence: the belief is updated, the reinforcement latch
    /// rearmed with what was seen in bed, and the held occupant followed.
    pub(super) fn believe(&mut self, at: SampleRef, bodies: &Admission) -> Believed {
        let believed = self.belief.observe(at, bodies, &self.policy);
        self.attention_memory
            .rearm(bodies.primary.bed, self.belief.bed_seen());
        self.verification
            .observe_held(self.belief.held_since(), at, &self.policy);
        believed
    }

    /// The parts and layer 1, on layer 0 and what the second level perceived.
    pub(super) fn follow(
        &mut self,
        at: SampleRef,
        seen: Option<parts::ByPart<parts::Look>>,
        sighting: bed_posture::Sighting,
    ) -> (Option<parts::Concluded>, Option<bed_posture::Concluded>) {
        let parts = self.belief.observe_parts(at, seen, &self.policy);
        let bed_posture = self.belief.observe_bed_posture(at, sighting, &self.policy);
        (parts, bed_posture)
    }

    /// The shell kept the burst's crop, this sharp, or could not.
    pub(super) fn kept(&mut self, at: SampleRef, kept: Result<f32, Failure>) -> LookResult {
        let sharpness_policy = self.policy.verify.as_ref().and_then(|v| v.sharpness);
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
            .policy
            .verify
            .map(|v| v.frames.of(look.question))
            .unwrap_or_default();
        let sharpness = self.policy.verify.and_then(|v| v.sharpness);
        let ticket = self.verification.issue(look.target, at, look.places);
        if burst.frames <= 1 && sharpness.is_none() {
            self.verification.dispatch(ticket, at);
            return Issued::Now(ticket);
        }
        self.verification.collect(ticket, *look, burst, at);
        Issued::Collecting(ticket)
    }
}
