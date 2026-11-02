//! The faces sense, `[senses.faces]`: a face detector over the subject's box or
//! the headboard. Its section, its checks, its policy and its sense profile.

use super::profile::sense_profile;
use super::schema::defaults;
use super::validate::{fraction, model, not_negative};
use anyhow::{Result, ensure};
use scene_core::{
    contract::{LookKind, SenseProfile},
    policy::FacePolicy,
};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Faces {
    pub model: PathBuf,
    #[serde(default = "defaults::backend_confidence")]
    pub backend_confidence: f32,
    #[serde(default = "defaults::face_min_confidence")]
    pub min_confidence: f32,
    /// Margin around the headboard where faces are looked for with nobody in
    /// sight, as a fraction of its width and height on each side; without it,
    /// the headboard itself. Serialized only if present, so the hash does not
    /// change.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub headboard_pad: Option<f32>,
}

impl Faces {
    /// Scores between 0 and 1; the backend never stricter than the minimum.
    pub(super) fn validate(&self) -> Result<()> {
        model("senses.faces.model", &self.model)?;
        fraction("senses.faces.backend_confidence", self.backend_confidence)?;
        fraction("senses.faces.min_confidence", self.min_confidence)?;
        ensure!(
            self.backend_confidence <= self.min_confidence,
            "senses.faces.backend_confidence cannot exceed senses.faces.min_confidence"
        );
        if let Some(pad) = self.headboard_pad {
            not_negative("senses.faces.headboard_pad", f64::from(pad))?;
        }
        Ok(())
    }

    /// `Policy::faces`.
    pub(super) fn policy(&self) -> FacePolicy {
        FacePolicy {
            min_confidence: self.min_confidence,
            headboard_pad: self.headboard_pad.unwrap_or(0.0),
        }
    }

    /// Its line in the published profile.
    pub(super) fn sense(&self) -> SenseProfile {
        sense_profile(
            LookKind::Faces,
            &self.model,
            self.backend_confidence,
            Some(self.min_confidence),
        )
    }
}
