//! The bed's calibration, `[calibration]`: learn the empty bed with depth. Its
//! section, its checks and its policy.

use super::schema::defaults;
use super::validate::not_negative;
use anyhow::{Result, ensure};
use scene_core::policy::CalibrationPolicy;
use serde::{Deserialize, Serialize};

/// `CalibrationPolicy`: the delay between attempts, in source seconds.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CalibrationConfig {
    #[serde(default = "defaults::calibration_retry_secs")]
    pub retry_secs: f64,
}

impl CalibrationConfig {
    /// It learns with depth: `depth` is whether `[senses.depth]` is present.
    pub(super) fn validate(&self, depth: bool) -> Result<()> {
        ensure!(
            depth,
            "calibration: calibrating the bed requires [senses.depth]"
        );
        not_negative("calibration.retry_secs", self.retry_secs)
    }

    /// `Policy::calibration`.
    pub(super) fn policy(&self) -> CalibrationPolicy {
        CalibrationPolicy {
            retry_secs: self.retry_secs,
        }
    }
}
