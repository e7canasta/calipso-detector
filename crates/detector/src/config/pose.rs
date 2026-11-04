//! The subject's pose, `[senses.pose]`: the body, read for the keypoints that
//! measure depth and the bed posture. Its section, its checks, its policy and
//! its sense profile.

use super::depth::Depth;
use super::profile::sense_profile;
use super::schema::defaults;
use super::validate::measured;
use super::validate::{confidence, fraction, model, not_negative, seconds};
use anyhow::{Result, ensure};
use scene_contract::{LookKind, SenseProfile};
use scene_core::policy::{DepthPolicy, EnrichPolicy};
use scene_core::units::{DiagonalFraction, Extension, RelativeDepth};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Pose {
    pub model: PathBuf,
    #[serde(default = "defaults::backend_confidence")]
    pub backend_confidence: f32,
    /// Fraction of the reference box diagonal from which the subject moved and
    /// the pose is requested again.
    #[serde(default = "defaults::move_threshold")]
    pub move_threshold: f32,
    /// Minimum score of a pose keypoint to measure depth at it.
    #[serde(default = "defaults::min_keypoint_score")]
    pub min_keypoint_score: f32,
    /// Age from which a still pose is requested again; without it, as
    /// always. Serialized only if present, so the hash does not change.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_age_secs: Option<f64>,
    /// Margin around the subject's box where the pose is looked for, as a
    /// fraction of its width and height on each side; without it, the box
    /// itself. Serialized only if present, so the hash does not change.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub crop_pad: Option<f32>,
}

impl Pose {
    /// Scores between 0 and 1; the movement, the pad and the age not negative.
    pub(super) fn validate(&self) -> Result<()> {
        model("senses.pose.model", &self.model)?;
        fraction("senses.pose.backend_confidence", self.backend_confidence)?;
        fraction("senses.pose.min_keypoint_score", self.min_keypoint_score)?;
        not_negative("senses.pose.move_threshold", f64::from(self.move_threshold))?;
        if let Some(pad) = self.crop_pad {
            not_negative("senses.pose.crop_pad", f64::from(pad))?;
        }
        ensure!(
            self.max_age_secs.is_none_or(|s| s.is_finite() && s > 0.0),
            "senses.pose.max_age_secs must be finite and positive"
        );
        Ok(())
    }

    /// `Policy::enrich`. The depth is read from the same profile: with depth,
    /// the keypoints measure it, and the bed's posture tolerance is its own.
    pub(super) fn policy(&self, depth: Option<&Depth>) -> Result<EnrichPolicy> {
        Ok(EnrichPolicy {
            move_threshold: measured(
                "senses.pose.move_threshold",
                DiagonalFraction::new(self.move_threshold),
            )?,
            depth: depth.map(|_| DepthPolicy),
            min_keypoint_score: confidence(
                "pose",
                "senses.pose.min_keypoint_score",
                self.min_keypoint_score,
            )?,
            posture_tolerance: measured(
                "senses.depth.posture_tolerance",
                RelativeDepth::new(
                    depth.map_or_else(defaults::posture_tolerance, |d| d.posture_tolerance),
                ),
            )?,
            max_pose_age_secs: self
                .max_age_secs
                .map(|s| seconds("pose", "senses.pose.max_age_secs", s))
                .transpose()?,
            crop_pad: measured(
                "senses.pose.crop_pad",
                Extension::new(self.crop_pad.unwrap_or(0.0)),
            )?,
        })
    }

    /// Its line in the published profile: the pose is not admitted by score,
    /// `min_keypoint_score` picks keypoints.
    pub(super) fn sense(&self) -> SenseProfile {
        SenseProfile {
            min_keypoint_score: Some(self.min_keypoint_score),
            ..sense_profile(LookKind::Pose, &self.model, self.backend_confidence, None)
        }
    }
}
