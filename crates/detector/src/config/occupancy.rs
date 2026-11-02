//! The room's geometry, `[occupancy]`: which piece answers where a body is
//! (white book, volume VII, ch. 14), and the rules of the room that come with
//! it. Its section, its checks and its policy.

use super::validate::measured;
use anyhow::{Result, ensure};
use scene_core::policy::TwoPlanesPolicy;
use scene_core::units::{Degrees, Metres, PositiveMetres, Ratio};
use serde::{Deserialize, Serialize};

/// Which room geometry answers the questions of space, and its thresholds.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct OccupancyConfig {
    /// `"frontal"` (by default) or `"two_planes"`.
    #[serde(default)]
    pub geometry: RoomGeometry,
    /// The thresholds of the room in two planes; required with it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub two_planes: Option<TwoPlanesConfig>,
    /// The knee of a doubtful body, in two planes (white book, volume VII,
    /// ch. 15); it needs `[senses.pose]`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub knee: Option<KneeConfig>,
    /// `Policy::held_left`, rule 4 bis: a held body may leave by the side
    /// (white book, volume VII, ch. 05). Off by default; any geometry.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub held_left: bool,
    /// `Policy::near_first`, rule 5 refined: whoever appeared near the bed
    /// and walked away did not come from the door (white book, volume VII,
    /// ch. 17). Off by default; any geometry.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub near_first: bool,
}

/// `TwoPlanesPolicy::knee_bent_deg`: a doubtful body's knee bent to this many
/// degrees or less puts it in bed.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct KneeConfig {
    /// The hip–knee–ankle angle, in (0, 180).
    pub bent_deg: f32,
}

/// The pieces of the room's geometry.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RoomGeometry {
    /// The camera that sees the bed along the image: the engine as it was.
    #[default]
    Frontal,
    /// The camera that sees the bed on the diagonal: the mattress and the
    /// footprint on the floor. It needs `map.bed_floor` and `map.bed_size_m`.
    TwoPlanes,
}

/// `TwoPlanesPolicy`: when a body is in bed, and near it, in two planes.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TwoPlanesConfig {
    /// The share of the box over the mattress to be in bed, in (0, 1].
    pub share: f32,
    /// The support at most this many metres from the footprint to be in bed.
    pub support_m: f32,
    /// On the floor, the support within this many metres is near the bed.
    pub near_m: f32,
}

impl OccupancyConfig {
    /// The geometry and its pieces. `bed_floor` and `bed_size_m`: the map has
    /// them; `pose`: `[senses.pose]` is present (the knee is read on it).
    pub(super) fn validate(&self, bed_floor: bool, bed_size_m: bool, pose: bool) -> Result<()> {
        match (self.geometry, &self.two_planes) {
            (RoomGeometry::Frontal, Some(_)) => {
                anyhow::bail!("occupancy.two_planes needs occupancy.geometry = \"two_planes\"")
            }
            (RoomGeometry::TwoPlanes, None) => {
                anyhow::bail!("occupancy.geometry = \"two_planes\" needs [occupancy.two_planes]")
            }
            (RoomGeometry::TwoPlanes, Some(two)) => {
                ensure!(
                    two.share.is_finite() && two.share > 0.0 && two.share <= 1.0,
                    "occupancy.two_planes.share: the share of the box in bed, in (0, 1]"
                );
                ensure!(
                    two.support_m.is_finite() && two.support_m >= 0.0,
                    "occupancy.two_planes.support_m: a distance in metres, not negative"
                );
                ensure!(
                    two.near_m.is_finite() && two.near_m > 0.0,
                    "occupancy.two_planes.near_m: a distance in metres, positive"
                );
                ensure!(
                    bed_floor,
                    "occupancy in two planes needs map.bed_floor: the support rests on it"
                );
                ensure!(
                    bed_size_m,
                    "occupancy in two planes needs map.bed_size_m: distances are in metres"
                );
            }
            (RoomGeometry::Frontal, None) => {}
        }
        if let Some(knee) = &self.knee {
            ensure!(
                self.geometry == RoomGeometry::TwoPlanes,
                "occupancy.knee needs occupancy.geometry = \"two_planes\""
            );
            ensure!(
                pose,
                "occupancy.knee needs [senses.pose]: the knee is read on the pose"
            );
            ensure!(
                knee.bent_deg.is_finite() && knee.bent_deg > 0.0 && knee.bent_deg < 180.0,
                "occupancy.knee.bent_deg: an angle in degrees, in (0, 180)"
            );
        }
        Ok(())
    }

    /// `Policy::two_planes`: only with the two-plane geometry, and with the
    /// knee when there is one.
    pub(super) fn two_planes(&self) -> Result<Option<TwoPlanesPolicy>> {
        if self.geometry != RoomGeometry::TwoPlanes {
            return Ok(None);
        }
        self.two_planes
            .as_ref()
            .map(|two| -> Result<TwoPlanesPolicy> {
                Ok(TwoPlanesPolicy {
                    share: measured("occupancy.two_planes.share", Ratio::new(two.share))?,
                    support_m: measured(
                        "occupancy.two_planes.support_m",
                        Metres::new(two.support_m),
                    )?,
                    near_m: measured(
                        "occupancy.two_planes.near_m",
                        PositiveMetres::new(two.near_m),
                    )?,
                    knee_bent_deg: self
                        .knee
                        .as_ref()
                        .map(|knee| {
                            measured("occupancy.knee.bent_deg", Degrees::new(knee.bent_deg))
                        })
                        .transpose()?,
                })
            })
            .transpose()
    }
}
