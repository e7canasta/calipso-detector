//! Attention: what to look at, where, and why not. Pure policies: they mutate
//! nothing (rule 12). Each question reads its own part of the `Policy`:
//! without it there is nothing to decide, and that is decided here too, not
//! in `cycle` (rule 11).

use crate::evidence::BodyEvidence;
use crate::evidence::FaceEvidence;
use crate::look::LookKind;
use crate::map::{Geometry, Map};
use crate::time::SampleRef;

mod body;
mod depth;
mod faces;
mod ledger;
mod memory;
mod primary;
mod verify;

pub(crate) use body::{BodyPoseLook, EnrichPlan, PoseTarget, enrich, pose_bodies};
pub(crate) use depth::{DepthLook, SubjectDepthLook, calibrate, deepen};
pub(crate) use faces::faces;
pub(crate) use ledger::{Attention, Look, LookOutcome, LookResult, SkipReason};
pub(crate) use memory::{LatchState, Memory};
pub(crate) use primary::{primary_region, reinforce, widen};
pub(crate) use verify::{VerifyLook, verify};

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

/// One look's consideration, with the look's kind: the question of the engine
/// names the look once and attaches its kind here (ADR 40, P7), so the sequence
/// asks by the question and cannot pair a decision with the wrong kind.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Considered<T> {
    pub(crate) kind: LookKind,
    pub(crate) consideration: Consideration<T>,
}

impl<T> Considered<T> {
    /// The consideration of the look `kind`, as attention decided it.
    pub(crate) fn of(kind: LookKind, consideration: Consideration<T>) -> Self {
        Self {
            kind,
            consideration,
        }
    }
}
