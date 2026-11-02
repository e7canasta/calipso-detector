//! Layer 1, the bed posture, `[bed_posture]`: how long each state must hold to
//! count. Its section, its checks and its policy.

use super::schema::defaults;
use super::validate::not_negative;
use anyhow::{Result, ensure};
use scene_core::policy::BedPosturePolicy;
use serde::{Deserialize, Serialize};

/// `BedPosturePolicy` (R18, 1804): how long each state must hold, in source
/// seconds, to count. The defaults are initial, to be measured per camera
/// (1801): getting out is the shortest, the one to report in time.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BedPostureConfig {
    #[serde(default = "defaults::dwell_lying_secs")]
    pub dwell_lying_secs: f64,
    #[serde(default = "defaults::dwell_sitting_up_secs")]
    pub dwell_sitting_up_secs: f64,
    #[serde(default = "defaults::dwell_getting_out_secs")]
    pub dwell_getting_out_secs: f64,
}

impl BedPostureConfig {
    /// Times not negative; it needs where the head is (the headboard) and the
    /// body (a pose model).
    pub(super) fn validate(&self, headboard: bool, pose: bool) -> Result<()> {
        not_negative("bed_posture.dwell_lying_secs", self.dwell_lying_secs)?;
        not_negative(
            "bed_posture.dwell_sitting_up_secs",
            self.dwell_sitting_up_secs,
        )?;
        not_negative(
            "bed_posture.dwell_getting_out_secs",
            self.dwell_getting_out_secs,
        )?;
        ensure!(
            headboard,
            "bed_posture needs map.headboard: where the head is"
        );
        ensure!(pose, "bed_posture needs senses.pose: the body");
        Ok(())
    }

    /// `Policy::bed_posture`.
    pub(super) fn policy(&self) -> BedPosturePolicy {
        BedPosturePolicy {
            dwell_lying_secs: self.dwell_lying_secs,
            dwell_sitting_up_secs: self.dwell_sitting_up_secs,
            dwell_getting_out_secs: self.dwell_getting_out_secs,
        }
    }
}
