//! The reinforcement sense, `[senses.reinforce]`: a finer detector over the bed
//! when the subject is lost. Its section, its checks, its policy and its sense
//! profile.

use super::profile::sense_profile;
use super::schema::defaults;
use super::validate::{fraction, model, not_negative};
use anyhow::{Result, ensure};
use scene_contract::{LookKind, SenseProfile};
use scene_core::policy::ReinforcePolicy;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Reinforce {
    pub model: PathBuf,
    #[serde(default = "defaults::backend_confidence")]
    pub backend_confidence: f32,
    #[serde(default = "defaults::bed_padding")]
    pub bed_padding: f32,
}

impl Reinforce {
    /// Scores between 0 and 1; never stricter than the policy's minimum.
    pub(super) fn validate(&self, min_confidence: f32) -> Result<()> {
        model("senses.reinforce.model", &self.model)?;
        fraction(
            "senses.reinforce.backend_confidence",
            self.backend_confidence,
        )?;
        ensure!(
            self.backend_confidence <= min_confidence,
            "senses.reinforce.backend_confidence cannot exceed policy.min_confidence"
        );
        not_negative("senses.reinforce.bed_padding", f64::from(self.bed_padding))
    }

    /// `Policy::reinforce`.
    pub(super) fn policy(&self) -> ReinforcePolicy {
        ReinforcePolicy {
            bed_padding: self.bed_padding,
        }
    }

    /// Its line in the published profile.
    pub(super) fn sense(&self, min_confidence: f32) -> SenseProfile {
        sense_profile(
            LookKind::Reinforce,
            &self.model,
            self.backend_confidence,
            Some(min_confidence),
        )
    }
}
