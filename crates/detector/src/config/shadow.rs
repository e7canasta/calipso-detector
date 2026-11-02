//! The geometry's shadow, `[shadow]`: estimates published next to what is
//! decided, deciding nothing (white book, volume VII, ch. 11). Its section, its
//! checks and its policy.

use super::validate::measured;
use anyhow::{Result, ensure};
use scene_core::policy::ShadowPolicy;
use scene_core::units::{Metres, PositiveMetres};
use serde::{Deserialize, Serialize};

/// `ShadowPolicy`: what the shadow measures.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ShadowConfig {
    /// Near the bed, in metres; it needs `map.bed_size_m`.
    pub near_m: f32,
    /// A body's support within this many metres of the bed is in bed for the
    /// shadow; 0 by default.
    #[serde(default)]
    pub support_margin_m: f32,
}

impl ShadowConfig {
    /// Distances in metres; the bed's size in metres is needed (`bed_size_m`).
    pub(super) fn validate(&self, bed_size_m: bool) -> Result<()> {
        ensure!(
            self.near_m.is_finite() && self.near_m > 0.0,
            "shadow.near_m: a distance in metres, positive"
        );
        ensure!(
            self.support_margin_m.is_finite() && self.support_margin_m >= 0.0,
            "shadow.support_margin_m: a distance in metres, not negative"
        );
        ensure!(
            bed_size_m,
            "shadow needs map.bed_size_m: near the bed is measured in metres"
        );
        Ok(())
    }

    /// `Policy::shadow`.
    pub(super) fn policy(&self) -> Result<ShadowPolicy> {
        Ok(ShadowPolicy {
            near_m: measured("shadow.near_m", PositiveMetres::new(self.near_m))?,
            support_margin_m: measured(
                "shadow.support_margin_m",
                Metres::new(self.support_margin_m),
            )?,
        })
    }
}
