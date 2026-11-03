//! The room's parts, `[occupancy.parts]` (white book, volume IX, ch. 09): the
//! thresholds of the cascade with the sprint 1 values, and the checks the
//! section needs from the rest of the profile (the two planes, the pose, the
//! faces and the headboard as four points).

use super::validate::measured;
use anyhow::{Result, ensure};
use scene_core::policy::PartsPolicy;
use scene_core::units::{HeadboardW, MattressHeights, Metres, Proportion, Span};
use serde::{Deserialize, Serialize};

/// `PartsPolicy`: the thresholds of the cascade. Every key has its default, so
/// the section may be empty.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct PartsConfig {
    /// The strip on the far side of the mattress, in metres. An ankle seen in
    /// it does not prove the bed: behind the far edge the mattress hides the
    /// floor (tomo IX, cap. 05).
    pub far_strip_m: f32,
    /// The score a foot's keypoint needs to prove the ankle, in [0, 1].
    pub ankle_proof_score: f32,
    /// The pelvis band, in mattress heights, over a foot on the floor: `[low,
    /// high]`. Inside it the pelvis sits on the edge of the bed (E2).
    pub pelvis_band: [f32; 2],
    /// The tolerance of the cut, as a fraction of the far edge, in [0, 1].
    pub cut_tolerance: f32,
    /// The head's band against the headboard, `w`: `[low, high]`; `0` at the
    /// pillow, `1` at the headboard's top (E6).
    pub headboard_w: [f32; 2],
    /// `true`: the parts decide the zone, with memory (F4.1). Left out of the
    /// hash when false, so a profile without it keeps its hash.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub decide: bool,
    /// `true`: layer 1 reads the posture from the frame (F5.2). Left out of the
    /// hash when false.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub posture: bool,
    /// The posture's thresholds (F5.2), each in [0, 1]; the defaults are the
    /// sprint 1 values. Left out of the hash when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub posture_upright_cos: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub posture_along_cos: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub posture_pillow_w: Option<f32>,
}

impl Default for PartsConfig {
    fn default() -> Self {
        Self {
            far_strip_m: 0.18,
            ankle_proof_score: 0.75,
            pelvis_band: [0.6, 1.4],
            cut_tolerance: 0.12,
            headboard_w: [-0.15, 1.0],
            decide: false,
            posture: false,
            posture_upright_cos: None,
            posture_along_cos: None,
            posture_pillow_w: None,
        }
    }
}

/// How far, as a fraction of the bed's head edge, a headboard stop may sit
/// from the mattress corner it must meet.
const EDGE_TOLERANCE: f32 = 0.1;

impl PartsConfig {
    /// The section's own rules, against the rest of the profile: `two_planes`
    /// (the geometry), `pose` and `faces` (the senses), the headboard as four
    /// points (`headboard`) and the bed's corners (`bed`). Then every number.
    pub(super) fn validate(
        &self,
        two_planes: bool,
        pose: bool,
        faces: bool,
        headboard: Option<[[f32; 2]; 4]>,
        bed: [[f32; 2]; 4],
    ) -> Result<()> {
        ensure!(
            two_planes,
            "occupancy.parts needs occupancy.geometry = \"two_planes\""
        );
        ensure!(
            pose,
            "occupancy.parts needs [senses.pose]: the parts are read on the pose"
        );
        ensure!(
            faces,
            "occupancy.parts needs [senses.faces]: the head is read on the faces"
        );
        let Some(headboard) = headboard else {
            anyhow::bail!(
                "occupancy.parts needs map.headboard as four points, top-left, top-right, bottom-right, bottom-left"
            )
        };
        headboard_order(headboard, bed)?;
        self.policy()?;
        Ok(())
    }

    /// `Policy::parts`: each number in its unit; a span with its low end first.
    pub(super) fn policy(&self) -> Result<PartsPolicy> {
        Ok(PartsPolicy {
            far_strip_m: measured("occupancy.parts.far_strip_m", Metres::new(self.far_strip_m))?,
            ankle_proof_score: measured(
                "occupancy.parts.ankle_proof_score",
                scene_core::units::Confidence::new(self.ankle_proof_score),
            )?,
            pelvis_band: pelvis_band(self.pelvis_band)?,
            cut_tolerance: measured(
                "occupancy.parts.cut_tolerance",
                Proportion::new(self.cut_tolerance),
            )?,
            headboard_w: headboard_w(self.headboard_w)?,
            decide: self.decide,
            posture: self.posture,
            posture_upright_cos: measured(
                "occupancy.parts.posture_upright_cos",
                Proportion::new(self.posture_upright_cos.unwrap_or(0.93)),
            )?,
            posture_along_cos: measured(
                "occupancy.parts.posture_along_cos",
                Proportion::new(self.posture_along_cos.unwrap_or(0.80)),
            )?,
            posture_pillow_w: measured(
                "occupancy.parts.posture_pillow_w",
                Proportion::new(self.posture_pillow_w.unwrap_or(0.15)),
            )?,
        })
    }
}

/// `[low, high]` of mattress heights: each end a measure, the low end not
/// above the high end.
fn pelvis_band([low, high]: [f32; 2]) -> Result<Span<MattressHeights>> {
    let lo = measured("occupancy.parts.pelvis_band", MattressHeights::new(low))?;
    let hi = measured("occupancy.parts.pelvis_band", MattressHeights::new(high))?;
    measured("occupancy.parts.pelvis_band", Span::new(lo, hi))
}

/// `[low, high]` of `w`, the head against the headboard.
fn headboard_w([low, high]: [f32; 2]) -> Result<Span<HeadboardW>> {
    let lo = measured("occupancy.parts.headboard_w", HeadboardW::new(low))?;
    let hi = measured("occupancy.parts.headboard_w", HeadboardW::new(high))?;
    measured("occupancy.parts.headboard_w", Span::new(lo, hi))
}

/// The headboard's points, top-left, top-right, bottom-right, bottom-left,
/// in the bed's order: the bottom edge lies on the mattress's head edge
/// (`headboard[3]` near `bed[0]`, `headboard[2]` near `bed[1]`), and the stops
/// lie on the far side of that edge from the mattress.
fn headboard_order(headboard: [[f32; 2]; 4], bed: [[f32; 2]; 4]) -> Result<()> {
    let dist = |p: [f32; 2], q: [f32; 2]| ((p[0] - q[0]).powi(2) + (p[1] - q[1]).powi(2)).sqrt();
    let length = dist(bed[0], bed[1]);
    let near = |p: [f32; 2], q: [f32; 2]| dist(p, q) <= EDGE_TOLERANCE * length;
    ensure!(
        near(headboard[3], bed[0]) && near(headboard[2], bed[1]),
        "map.headboard: out of order; the bottom edge must be the mattress's head edge, headboard[3] near bed[0] and headboard[2] near bed[1]"
    );
    let side = |p: [f32; 2]| {
        (bed[1][0] - bed[0][0]) * (p[1] - bed[0][1]) - (bed[1][1] - bed[0][1]) * (p[0] - bed[0][0])
    };
    let centre = [
        bed.iter().map(|p| p[0]).sum::<f32>() / 4.0,
        bed.iter().map(|p| p[1]).sum::<f32>() / 4.0,
    ];
    ensure!(
        side(headboard[0]) * side(centre) < 0.0 && side(headboard[1]) * side(centre) < 0.0,
        "map.headboard: the stops must lie on the far side of the head edge from the mattress"
    );
    Ok(())
}
