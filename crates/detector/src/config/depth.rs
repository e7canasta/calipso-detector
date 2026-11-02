//! The depth sense, `[senses.depth]`: one model for the subject's keypoints and
//! for the bed's calibration. Its section, its checks and its policy.

use super::schema::defaults;
use super::validate::{model, not_negative};
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Depth: one model for the subject and for the bed.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Depth {
    pub model: PathBuf,
    /// Relative disparity of "sitting"; serialized only if it changes, so
    /// the hash of a profile that does not touch it does not change.
    #[serde(
        default = "defaults::posture_tolerance",
        skip_serializing_if = "defaults::is_posture_tolerance"
    )]
    pub posture_tolerance: f32,
    /// Where depth is measured, for the subject and the empty bed alike: the
    /// bed grown by this fraction of its width and height on each side;
    /// without it, the whole frame. Serialized only if present, so the hash
    /// does not change.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub crop_pad: Option<f32>,
}

impl Depth {
    /// `asked`: the pose or the calibration asks for depth; without either,
    /// the model would run for nobody.
    pub(super) fn validate(&self, asked: bool) -> Result<()> {
        model("senses.depth.model", &self.model)?;
        if let Some(pad) = self.crop_pad {
            not_negative("senses.depth.crop_pad", f64::from(pad))?;
        }
        ensure!(
            self.posture_tolerance.is_finite() && self.posture_tolerance >= 0.0,
            "senses.depth.posture_tolerance must be finite and not negative"
        );
        ensure!(
            asked,
            "senses.depth: nobody asks for depth; [senses.pose] or [calibration] is needed"
        );
        Ok(())
    }
}
