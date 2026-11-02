//! The main detector, `[senses.detect]`: it sees every body in the frame, and
//! focuses on the bed while the resident is alone in it (`[senses.detect.focus]`).

use super::profile::sense_profile;
use super::validate::{fraction, model, not_negative, positive, seconds};
use anyhow::{Result, ensure};
use scene_contract::{LookKind, SenseProfile};
use scene_core::policy::FocusPolicy;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Detect {
    pub model: PathBuf,
    #[serde(default = "super::schema::defaults::backend_confidence")]
    pub backend_confidence: f32,
    /// Without it, the main detector always looks at the whole frame. Left
    /// out of the hash when absent: a profile without focus keeps its hash.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub focus: Option<Focus>,
}

/// The main detector looks at the bed up close while the resident is alone
/// in it, and at the whole frame at least every `wide_every_secs`.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Focus {
    #[serde(default = "super::schema::defaults::focus_pad")]
    pub pad: f32,
    pub wide_every_secs: f64,
    /// The least staff take to walk from the door to the bed: the sentinel
    /// cannot be longer, or whoever walks in reaches the bed unseen.
    pub walk_secs: f64,
}

impl Detect {
    /// The model, and the focus when there is one.
    pub(super) fn validate_sense(&self) -> Result<()> {
        model("senses.detect.model", &self.model)?;
        if let Some(focus) = &self.focus {
            not_negative("senses.detect.focus.pad", f64::from(focus.pad))?;
            positive("senses.detect.focus.walk_secs", focus.walk_secs)?;
            positive("senses.detect.focus.wide_every_secs", focus.wide_every_secs)?;
            ensure!(
                focus.wide_every_secs <= focus.walk_secs,
                "senses.detect.focus.wide_every_secs cannot exceed senses.detect.focus.walk_secs: \
                 whoever walks in would reach the bed unseen"
            );
        }
        Ok(())
    }

    /// The backend's score: between 0 and 1, never stricter than the policy's.
    pub(super) fn validate_confidence(&self, min_confidence: f32) -> Result<()> {
        fraction("senses.detect.backend_confidence", self.backend_confidence)?;
        ensure!(
            self.backend_confidence <= min_confidence,
            "senses.detect.backend_confidence cannot exceed policy.min_confidence"
        );
        Ok(())
    }

    /// Its line in the published profile: the detector admits by the policy's
    /// minimum.
    pub(super) fn sense(&self, min_confidence: f32) -> SenseProfile {
        sense_profile(
            LookKind::Detect,
            &self.model,
            self.backend_confidence,
            Some(min_confidence),
        )
    }
}

impl Focus {
    /// `Policy::focus`.
    pub(super) fn policy(&self) -> Result<FocusPolicy> {
        Ok(FocusPolicy {
            pad: self.pad,
            wide_every_secs: seconds(
                "detect",
                "senses.detect.focus.wide_every_secs",
                self.wide_every_secs,
            )?,
        })
    }
}
