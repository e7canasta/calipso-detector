//! The profile as it is written: the common sections, and the composition of
//! one module per capability (`faces`, `pose`, `depth`, …). Each capability
//! owns its section's struct, its checks and its translation.

use super::bed_posture::BedPostureConfig;
use super::calibration::CalibrationConfig;
use super::depth::Depth;
use super::detect::Detect;
use super::faces::Faces;
use super::occupancy::OccupancyConfig;
use super::pose::Pose;
use super::publish::PublishConfig;
use super::reinforce::Reinforce;
use super::shadow::ShadowConfig;
use super::verify::{VerifyConfig, VerifySense};
use scene_core::policy::Policy;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub camera: Camera,
    pub stream: Stream,
    pub senses: SensesConfig,
    #[serde(default)]
    pub policy: PolicyConfig,
    pub map: MapConfig,
    pub nats: NatsConfig,
    /// Learn the empty bed with depth: without the section, no calibration.
    pub calibration: Option<CalibrationConfig>,
    /// Layer 1, the bed posture: without the section, nothing follows it.
    pub bed_posture: Option<BedPostureConfig>,
    /// The slow verifier: what this stream verifies. Without the section,
    /// nothing is verified (white book, volume III). It needs
    /// `[senses.verify]`.
    pub verify: Option<VerifyConfig>,
    /// The geometry's shadow: estimates published next to what is decided,
    /// deciding nothing (white book, volume VII, ch. 11). Without the
    /// section, nothing is computed. Left out of the hash when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shadow: Option<ShadowConfig>,
    /// The room's geometry, the piece that answers where a body is (white
    /// book, volume VII, ch. 14). Without the section, frontal: the engine as
    /// it was. Left out of the hash when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub occupancy: Option<OccupancyConfig>,
    /// The contract versions the shell publishes. Without the section, v4 only.
    #[serde(default)]
    pub publish: PublishConfig,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Camera {
    pub site: String,
    pub id: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Stream {
    pub url: String,
    pub username: Option<String>,
    pub password: Option<String>,
    #[serde(default = "defaults::keyframe_only")]
    pub keyframe_only: bool,
    #[serde(default = "defaults::reconnect_delay_secs")]
    pub reconnect_delay_secs: u64,
    #[serde(default = "defaults::sample_interval_ms")]
    pub sample_interval_ms: u64,
    #[serde(default = "defaults::stall_timeout_secs")]
    pub stall_timeout_secs: u64,
}

/// The senses: the main detector always, and each optional sense whose
/// section is present.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SensesConfig {
    pub detect: Detect,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reinforce: Option<Reinforce>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub faces: Option<Faces>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pose: Option<Pose>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub depth: Option<Depth>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verify: Option<VerifySense>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct PolicyConfig {
    pub min_confidence: f32,
    pub same_body_iou: f32,
    pub near_bed_margin: f32,
    /// Source seconds a new count must hold.
    pub dwell_secs: f64,
    /// For a camera that sees the bed at an angle: near the bed measured in
    /// the bed's own coordinates, and the person's box in bed when this share
    /// of it falls in the bed. Left out of the hash when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bed_frame: Option<f32>,
}

/// Without a `[policy]` section, the core's own defaults.
impl Default for PolicyConfig {
    fn default() -> Self {
        let Policy {
            admission,
            persistence,
            ..
        } = Policy::default();
        Self {
            min_confidence: admission.min_confidence.get(),
            same_body_iou: admission.same_body_iou.get(),
            near_bed_margin: admission.near_bed_margin.get(),
            dwell_secs: persistence.dwell_secs.get(),
            bed_frame: admission.bed_frame.map(|share| share.get()),
        }
    }
}

/// The headboard: a box `[x0, y0, x1, y1]`, or four points like the bed's
/// for a camera that sees it at an angle.
#[derive(Debug, Clone, Copy, PartialEq, Deserialize, Serialize)]
#[serde(untagged)]
pub enum HeadboardConfig {
    Box([f32; 4]),
    Quad([[f32; 2]; 4]),
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct MapConfig {
    pub resolution: [u32; 2],
    pub bed: [[f32; 2]; 4],
    pub headboard: Option<HeadboardConfig>,
    /// Where the main detector looks while focusing; without it, the bed
    /// grown by the focus's pad. Left out of the hash when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub focus: Option<[f32; 4]>,
    /// Where the bed stands on the floor, four points like the bed: the
    /// shadow measures points on the floor against it. Left out of the hash
    /// when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bed_floor: Option<[[f32; 2]; 4]>,
    /// The bed's width and length in metres. Left out of the hash when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bed_size_m: Option<[f32; 2]>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NatsConfig {
    pub url: String,
    pub prefix: Option<String>,
}

/// What a profile gets when it leaves a field out. Each value is named by what
/// it means: two fields that share a number do not share a default.
pub(super) mod defaults {
    pub(in crate::config) fn sharpness_window() -> u16 {
        20
    }

    pub(in crate::config) fn verify_subject() -> String {
        "calipso.verify.v1".to_owned()
    }
    /// Above the verifier's slowest answer: two calls of up to 30 s each (it
    /// retries a broken reply once); MiMo took up to 43.8 s on vador1's crops
    /// with a retry (2026-10-13).
    pub(in crate::config) fn verify_wall_timeout_secs() -> f64 {
        65.0
    }
    pub(in crate::config) fn keyframe_only() -> bool {
        true
    }
    pub(in crate::config) fn reconnect_delay_secs() -> u64 {
        5
    }
    pub(in crate::config) fn sample_interval_ms() -> u64 {
        1000
    }
    pub(in crate::config) fn stall_timeout_secs() -> u64 {
        10
    }
    pub(in crate::config) fn backend_confidence() -> f32 {
        0.25
    }
    pub(in crate::config) fn calibration_retry_secs() -> f64 {
        2.0
    }
    pub(in crate::config) fn posture_tolerance() -> f32 {
        0.05
    }
    pub(in crate::config) fn is_posture_tolerance(value: &f32) -> bool {
        *value == posture_tolerance()
    }
    pub(in crate::config) fn move_threshold() -> f32 {
        0.10
    }
    pub(in crate::config) fn bed_padding() -> f32 {
        0.15
    }
    pub(in crate::config) fn dwell_lying_secs() -> f64 {
        2.0
    }
    pub(in crate::config) fn dwell_sitting_up_secs() -> f64 {
        3.0
    }
    pub(in crate::config) fn dwell_getting_out_secs() -> f64 {
        1.0
    }
    pub(in crate::config) fn focus_pad() -> f32 {
        0.25
    }
    pub(in crate::config) fn face_min_confidence() -> f32 {
        0.30
    }
    pub(in crate::config) fn min_keypoint_score() -> f32 {
        0.30
    }
}
