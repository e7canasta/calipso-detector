//! What the engine does when the turn asks it: each operation changes the
//! engine's own memories together, in the order they depend on each other, and
//! answers each question with its own part of the policy. The turn says when
//! (`sequence`); the engine knows what to answer and with which of its parts.
//! Nothing outside this folder reads a field of the engine.
//!
//! - here: the engine taken from the policy, once;
//! - `arrival`: what reaches the engine, and the settling at a turn's opening;
//! - `questions`: the questions of space and attention, each with its slice;
//! - `evidence`: what the readings fill;
//! - `belief`: what the turn believes;
//! - `refinement`: the second level and layer 1;
//! - `verifier`: the slow verifier's questions and answers, in the turn;
//! - `conclusion`: what the turn leaves behind.

mod arrival;
mod belief;
mod conclusion;
mod evidence;
mod questions;
mod refinement;
mod verifier;

use super::Engine;
use crate::attention;
use crate::belief::{Belief, OccupancyRules};
use crate::map::{CalibrationMemory, SiteMap};
use crate::policy::Policy;
use crate::reception;
use crate::room::SpaceRules;
use crate::verification::{Verification, VerifyRules};

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
            space: SpaceRules {
                admission,
                two_planes,
                shadow,
            },
            reinforce,
            face_policy: faces,
            enrich,
            calibration_policy: calibration,
            depth_pad,
            focus,
            verify_policy: verify,
            calibration_memory: CalibrationMemory::from(&map),
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
}
