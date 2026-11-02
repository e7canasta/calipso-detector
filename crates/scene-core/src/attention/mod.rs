//! Attention: what to look at, where, and why not. Pure policies: they mutate
//! nothing (rule 12). Each question reads its own part of the `Policy`:
//! without it there is nothing to decide, and that is decided here too, not
//! in `cycle` (rule 11).

use crate::evidence::BodyEvidence;
use crate::evidence::FaceEvidence;
use crate::map::{Geometry, Map};
use crate::time::SampleRef;

mod body;
mod depth;
mod faces;
mod ledger;
mod memory;
mod primary;
mod verify;

pub(crate) use body::{BodyPoseLook, EnrichPlan, enrich, pose_bodies};
pub(crate) use depth::{DepthLook, SubjectDepthLook, calibrate, deepen};
pub(crate) use faces::faces;
pub(crate) use ledger::{Attention, Look, LookOutcome, LookResult, SkipReason};
pub(crate) use memory::{LatchState, Memory};
pub(crate) use primary::{primary_region, reinforce, widen};
pub(crate) use verify::{BurstStep, VerifyLook, burst, burst_ready, verify};

/// What attention can see of the current turn. Read-only: deciding what to
/// look at never changes what is seen (rule 12).
#[derive(Debug, Clone, Copy)]
pub(crate) struct Glimpse<'a> {
    pub at: SampleRef,
    pub geometry: Geometry,
    pub map: &'a Map,
    pub bodies: &'a BodyEvidence,
    pub faces: &'a FaceEvidence,
}

/// Ask or do not ask, with a reason. Everything goes to the ledger.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Decision<T> {
    Ask(T),
    Skip(SkipReason),
}

/// Without a policy, or out of context, nothing is decided and no ledger entry is added.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Consideration<T> {
    Disabled,
    Inapplicable,
    Decided(Decision<T>),
}
