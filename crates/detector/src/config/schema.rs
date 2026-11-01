//! The profile as it is written: one struct per section of the TOML, with
//! the defaults of what a profile leaves out.

use scene_core::policy::{Burst, Policy};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

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
}

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

/// `VerifyPolicy`: what this stream verifies, and nothing else. Each stream
/// pays only for what matters to it.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct VerifyConfig {
    /// Requests per hour of source time; without it, no limit.
    pub max_per_hour: Option<u32>,
    /// Source seconds a request waits for its answer; without it, forever.
    pub timeout_secs: Option<f64>,
    /// Verify a body held in bed (O1, O2).
    pub held: Option<HeldVerifyConfig>,
    /// The disparity's grey band, `[lo, hi]` (P1): needs depth.
    pub doubtful_disparity: Option<[f32; 2]>,
    /// The feet's grey band, a fraction of the bed's width (P2, P3).
    pub doubtful_feet_margin: Option<f32>,
    /// How many crops each question sends (white book, volume III, ch. 12);
    /// without it, one.
    pub frames: Option<FramesConfig>,
    /// Which kept crops are sharp enough to count; without it, every one.
    pub sharpness: Option<SharpnessConfig>,
}

/// A crop counts if it is at least `min_ratio` of the median sharpness of the
/// camera's last `window` crops; after `max_wait_secs` the burst asks with the
/// sharpest it has.
#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SharpnessConfig {
    pub min_ratio: f32,
    #[serde(default = "defaults::sharpness_window")]
    pub window: u16,
    pub max_wait_secs: f64,
}

/// The burst of each question, by its name; a missing one sends one crop.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FramesConfig {
    pub held_in_bed: Option<BurstConfig>,
    pub sitting_in_bed: Option<BurstConfig>,
    pub leaving_bed: Option<BurstConfig>,
}

/// A burst: how many crops, and the source seconds between them.
#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BurstConfig {
    pub frames: u8,
    #[serde(default)]
    pub every_secs: f64,
}

impl BurstConfig {
    pub(super) fn burst(self) -> Burst {
        Burst {
            frames: self.frames,
            every_secs: self.every_secs,
        }
    }
}

/// Verify a body held in bed.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct HeldVerifyConfig {
    /// Source seconds from `held_in_bed` until it is asked about.
    pub after_secs: f64,
    /// Source seconds a check holds; without it, it is asked once.
    pub valid_secs: Option<f64>,
}

/// Where the slow verifier is: a local service behind NATS, on the profile's
/// server. Nothing leaves the site (V1).
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct VerifySense {
    #[serde(default = "defaults::verify_subject")]
    pub subject: String,
    /// Wall seconds the shell waits for a reply before answering unsure.
    #[serde(default = "defaults::verify_wall_timeout_secs")]
    pub wall_timeout_secs: f64,
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
/// `CalibrationPolicy`: the delay between attempts, in source seconds.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CalibrationConfig {
    #[serde(default = "defaults::calibration_retry_secs")]
    pub retry_secs: f64,
}
/// The subject's pose: the model and the `EnrichPolicy` parameters.
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
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Reinforce {
    pub model: PathBuf,
    #[serde(default = "defaults::backend_confidence")]
    pub backend_confidence: f32,
    #[serde(default = "defaults::bed_padding")]
    pub bed_padding: f32,
}
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Detect {
    pub model: PathBuf,
    #[serde(default = "defaults::backend_confidence")]
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
    #[serde(default = "defaults::focus_pad")]
    pub pad: f32,
    pub wide_every_secs: f64,
    /// The least staff take to walk from the door to the bed: the sentinel
    /// cannot be longer, or whoever walks in reaches the bed unseen.
    pub walk_secs: f64,
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
            min_confidence: admission.min_confidence,
            same_body_iou: admission.same_body_iou,
            near_bed_margin: admission.near_bed_margin,
            dwell_secs: persistence.dwell_secs,
            bed_frame: admission.bed_frame,
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
    pub(super) fn sharpness_window() -> u16 {
        20
    }

    pub(super) fn verify_subject() -> String {
        "calipso.verify.v1".to_owned()
    }
    /// Above the verifier's slowest answer: two calls of up to 30 s each (it
    /// retries a broken reply once); MiMo took up to 43.8 s on vador1's crops
    /// with a retry (2026-10-13).
    pub(super) fn verify_wall_timeout_secs() -> f64 {
        65.0
    }
    pub(super) fn keyframe_only() -> bool {
        true
    }
    pub(super) fn reconnect_delay_secs() -> u64 {
        5
    }
    pub(super) fn sample_interval_ms() -> u64 {
        1000
    }
    pub(super) fn stall_timeout_secs() -> u64 {
        10
    }
    pub(super) fn backend_confidence() -> f32 {
        0.25
    }
    pub(super) fn calibration_retry_secs() -> f64 {
        2.0
    }
    pub(in crate::config) fn posture_tolerance() -> f32 {
        0.05
    }
    pub(super) fn is_posture_tolerance(value: &f32) -> bool {
        *value == posture_tolerance()
    }
    pub(super) fn move_threshold() -> f32 {
        0.10
    }
    pub(super) fn bed_padding() -> f32 {
        0.15
    }
    pub(super) fn dwell_lying_secs() -> f64 {
        2.0
    }
    pub(super) fn dwell_sitting_up_secs() -> f64 {
        3.0
    }
    pub(super) fn dwell_getting_out_secs() -> f64 {
        1.0
    }
    pub(super) fn focus_pad() -> f32 {
        0.25
    }
    pub(super) fn face_min_confidence() -> f32 {
        0.30
    }
    pub(super) fn min_keypoint_score() -> f32 {
        0.30
    }
}
