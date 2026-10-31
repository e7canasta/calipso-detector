//! Minimal shell configuration; policies are handed over by value.
use anyhow::{Context, Result, ensure};
use scene_core::{
    geom::{Point, Quad, Rect},
    map::{FrameInfo, SiteMap},
    policy::{
        Band, BedPosturePolicy, Burst, CalibrationPolicy, EnrichPolicy, FacePolicy, FocusPolicy,
        Frames, HeldVerifyPolicy, Policy, ReinforcePolicy, ShadowPolicy, SharpnessPolicy,
        TwoPlanesPolicy, VerifyPolicy,
    },
};
use serde::{Deserialize, Serialize};
use std::{
    path::{Path, PathBuf},
    time::Duration,
};

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
    fn burst(self) -> Burst {
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
mod defaults {
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
    pub(super) fn posture_tolerance() -> f32 {
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

/// A score or a ratio: finite and between 0 and 1.
fn fraction(name: &str, value: f32) -> Result<()> {
    ensure!(
        value.is_finite() && (0.0..=1.0).contains(&value),
        "{name} must be between 0 and 1"
    );
    Ok(())
}

/// A margin or a delay: finite and not negative.
fn not_negative(name: &str, value: f64) -> Result<()> {
    ensure!(
        value.is_finite() && value >= 0.0,
        "{name} must be finite and not negative"
    );
    Ok(())
}

/// A period: finite and positive.
fn positive(name: &str, value: f64) -> Result<()> {
    ensure!(
        value.is_finite() && value > 0.0,
        "{name} must be finite and positive"
    );
    Ok(())
}

/// Every sense names its model.
fn model(name: &str, path: &Path) -> Result<()> {
    ensure!(!path.as_os_str().is_empty(), "{name} is empty");
    Ok(())
}

fn resolve_model_path(model: &Path, base: Option<&Path>) -> PathBuf {
    match base {
        Some(base) if !model.is_absolute() => base.join(model),
        _ => model.to_owned(),
    }
}

fn sense_profile(
    kind: scene_core::contract::LookKind,
    model: &Path,
    backend_confidence: f32,
    min_confidence: Option<f32>,
) -> scene_core::contract::SenseProfile {
    scene_core::contract::SenseProfile {
        kind,
        model: model.file_name().map_or_else(
            || model.display().to_string(),
            |name| name.to_string_lossy().into_owned(),
        ),
        backend_confidence,
        min_confidence,
        min_keypoint_score: None,
    }
}

impl Config {
    pub fn load(path: &Path) -> Result<Self> {
        Self::parse(
            &std::fs::read_to_string(path)
                .with_context(|| format!("could not read {}", path.display()))?,
        )
    }
    pub fn parse(text: &str) -> Result<Self> {
        let config: Self =
            toml::from_str(text).map_err(|e| anyhow::anyhow!("invalid TOML: {e}"))?;
        config.validate()?;
        Ok(config)
    }
    /// Each section checks itself; the first error found is reported.
    fn validate(&self) -> Result<()> {
        self.validate_identifiers()?;
        self.validate_transport()?;
        self.validate_senses()?;
        self.validate_policy()?;
        self.validate_map()?;
        self.validate_bed_posture()?;
        self.validate_verify()
    }

    /// The policy and the sense come together; bands are ordered and finite;
    /// the doubtful digits need layer 1, and the disparity needs depth.
    fn validate_verify(&self) -> Result<()> {
        let Some(v) = &self.verify else {
            ensure!(
                self.senses.verify.is_none(),
                "senses.verify without [verify]: nothing would be asked"
            );
            return Ok(());
        };
        let Some(sense) = &self.senses.verify else {
            anyhow::bail!("verify needs senses.verify: where the verifier is");
        };
        ensure!(!sense.subject.is_empty(), "senses.verify.subject is empty");
        positive("senses.verify.wall_timeout_secs", sense.wall_timeout_secs)?;
        if let Some(max) = v.max_per_hour {
            ensure!(max > 0, "verify.max_per_hour must be positive");
        }
        if let Some(t) = v.timeout_secs {
            positive("verify.timeout_secs", t)?;
        }
        if let Some(held) = &v.held {
            not_negative("verify.held.after_secs", held.after_secs)?;
            if let Some(valid) = held.valid_secs {
                positive("verify.held.valid_secs", valid)?;
            }
        }
        if let Some([lo, hi]) = v.doubtful_disparity {
            ensure!(
                lo.is_finite() && hi.is_finite() && lo <= hi,
                "verify.doubtful_disparity must be [lo, hi], finite, lo <= hi"
            );
            ensure!(
                self.senses.depth.is_some(),
                "verify.doubtful_disparity needs senses.depth: the disparity"
            );
        }
        if let Some(m) = v.doubtful_feet_margin {
            ensure!(
                m.is_finite() && m > 0.0 && m < 0.5,
                "verify.doubtful_feet_margin must be in (0, 0.5)"
            );
        }
        let frames = v.frames.clone().unwrap_or_default();
        for (name, burst) in [
            ("held_in_bed", frames.held_in_bed),
            ("sitting_in_bed", frames.sitting_in_bed),
            ("leaving_bed", frames.leaving_bed),
        ] {
            if let Some(b) = burst {
                ensure!(
                    (1..=8).contains(&b.frames),
                    "verify.frames.{name}.frames must be 1 to 8"
                );
                ensure!(
                    b.every_secs.is_finite() && b.every_secs >= 0.0,
                    "verify.frames.{name}.every_secs must be a source time, 0 or more"
                );
            }
        }
        if let Some(s) = v.sharpness {
            ensure!(
                s.min_ratio.is_finite() && s.min_ratio > 0.0 && s.min_ratio <= 1.0,
                "verify.sharpness.min_ratio must be in (0, 1]"
            );
            ensure!(s.window > 0, "verify.sharpness.window must be positive");
            positive("verify.sharpness.max_wait_secs", s.max_wait_secs)?;
        }
        if v.doubtful_disparity.is_some() || v.doubtful_feet_margin.is_some() {
            ensure!(
                self.bed_posture.is_some(),
                "verify.doubtful_* needs [bed_posture]: layer 1 reads the digit"
            );
        }
        Ok(())
    }

    /// Times finite and not negative; it needs where the head is (the
    /// headboard) and the body (a pose model).
    fn validate_bed_posture(&self) -> Result<()> {
        let Some(b) = &self.bed_posture else {
            return Ok(());
        };
        not_negative("bed_posture.dwell_lying_secs", b.dwell_lying_secs)?;
        not_negative("bed_posture.dwell_sitting_up_secs", b.dwell_sitting_up_secs)?;
        not_negative(
            "bed_posture.dwell_getting_out_secs",
            b.dwell_getting_out_secs,
        )?;
        ensure!(
            self.map.headboard.is_some(),
            "bed_posture needs map.headboard: where the head is"
        );
        ensure!(
            self.senses.pose.is_some(),
            "bed_posture needs senses.pose: the body"
        );
        Ok(())
    }

    /// `camera.site` and `camera.id` become subject tokens.
    fn validate_identifiers(&self) -> Result<()> {
        for (name, token) in [
            ("camera.site", &self.camera.site),
            ("camera.id", &self.camera.id),
        ] {
            ensure!(
                !token.is_empty()
                    && token
                        .chars()
                        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'),
                "{name}: invalid identifier"
            );
        }
        Ok(())
    }

    /// The RTSP stream, NATS and their timings.
    fn validate_transport(&self) -> Result<()> {
        let stream_url = url::Url::parse(&self.stream.url).context("invalid stream.url")?;
        ensure!(
            stream_url.scheme() == "rtsp" && stream_url.host_str().is_some(),
            "stream.url: RTSP with a host is required"
        );
        let nats_url = url::Url::parse(&self.nats.url).context("invalid nats.url")?;
        ensure!(
            matches!(nats_url.scheme(), "nats" | "tls") && nats_url.host_str().is_some(),
            "nats.url: NATS with a host is required"
        );
        if let Some(prefix) = &self.nats.prefix {
            ensure!(
                prefix.split('.').all(|t| !t.is_empty()
                    && t.chars()
                        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')),
                "nats.prefix: invalid subject"
            );
        }
        ensure!(
            self.stream.sample_interval_ms > 0,
            "stream.sample_interval_ms must be positive"
        );
        ensure!(
            self.stream.stall_timeout_secs > 0,
            "stream.stall_timeout_secs must be positive"
        );
        ensure!(
            self.stream.reconnect_delay_secs > 0,
            "stream.reconnect_delay_secs must be positive"
        );
        Ok(())
    }

    /// Each sense section, and that depth and calibration ask for each other.
    fn validate_senses(&self) -> Result<()> {
        model("senses.detect.model", &self.senses.detect.model)?;
        if let Some(focus) = &self.senses.detect.focus {
            not_negative("senses.detect.focus.pad", f64::from(focus.pad))?;
            positive("senses.detect.focus.walk_secs", focus.walk_secs)?;
            positive("senses.detect.focus.wide_every_secs", focus.wide_every_secs)?;
            ensure!(
                focus.wide_every_secs <= focus.walk_secs,
                "senses.detect.focus.wide_every_secs cannot exceed senses.detect.focus.walk_secs: \
                 whoever walks in would reach the bed unseen"
            );
        }
        if let Some(reinforce) = &self.senses.reinforce {
            model("senses.reinforce.model", &reinforce.model)?;
            fraction(
                "senses.reinforce.backend_confidence",
                reinforce.backend_confidence,
            )?;
            ensure!(
                reinforce.backend_confidence <= self.policy.min_confidence,
                "senses.reinforce.backend_confidence cannot exceed policy.min_confidence"
            );
            not_negative(
                "senses.reinforce.bed_padding",
                f64::from(reinforce.bed_padding),
            )?;
        }
        if let Some(faces) = &self.senses.faces {
            model("senses.faces.model", &faces.model)?;
            fraction("senses.faces.backend_confidence", faces.backend_confidence)?;
            fraction("senses.faces.min_confidence", faces.min_confidence)?;
            ensure!(
                faces.backend_confidence <= faces.min_confidence,
                "senses.faces.backend_confidence cannot exceed senses.faces.min_confidence"
            );
            if let Some(pad) = faces.headboard_pad {
                not_negative("senses.faces.headboard_pad", f64::from(pad))?;
            }
        }
        if let Some(pose) = &self.senses.pose {
            model("senses.pose.model", &pose.model)?;
            fraction("senses.pose.backend_confidence", pose.backend_confidence)?;
            fraction("senses.pose.min_keypoint_score", pose.min_keypoint_score)?;
            not_negative("senses.pose.move_threshold", f64::from(pose.move_threshold))?;
            if let Some(pad) = pose.crop_pad {
                not_negative("senses.pose.crop_pad", f64::from(pad))?;
            }
            ensure!(
                pose.max_age_secs.is_none_or(|s| s.is_finite() && s > 0.0),
                "senses.pose.max_age_secs must be finite and positive"
            );
        }
        if let Some(depth) = &self.senses.depth {
            model("senses.depth.model", &depth.model)?;
            if let Some(pad) = depth.crop_pad {
                not_negative("senses.depth.crop_pad", f64::from(pad))?;
            }
            ensure!(
                self.senses.pose.is_some() || self.calibration.is_some(),
                "senses.depth: nobody asks for depth; [senses.pose] or [calibration] is needed"
            );
        }
        if let Some(calibration) = &self.calibration {
            ensure!(
                self.senses.depth.is_some(),
                "calibration: calibrating the bed requires [senses.depth]"
            );
            not_negative("calibration.retry_secs", calibration.retry_secs)?;
        }
        Ok(())
    }

    /// Confidences between 0 and 1; a backend never stricter than the policy.
    fn validate_policy(&self) -> Result<()> {
        fraction("policy.min_confidence", self.policy.min_confidence)?;
        fraction("policy.same_body_iou", self.policy.same_body_iou)?;
        fraction(
            "senses.detect.backend_confidence",
            self.senses.detect.backend_confidence,
        )?;
        ensure!(
            self.senses.detect.backend_confidence <= self.policy.min_confidence,
            "senses.detect.backend_confidence cannot exceed policy.min_confidence"
        );
        not_negative(
            "policy.near_bed_margin",
            f64::from(self.policy.near_bed_margin),
        )?;
        if let Some(share) = self.policy.bed_frame {
            ensure!(
                share > 0.0 && share <= 1.0,
                "policy.bed_frame: the share of the box in bed, in (0, 1]"
            );
        }
        not_negative("policy.dwell_secs", self.policy.dwell_secs)
    }

    /// The bed inside the resolution and not degenerate; a valid headboard.
    fn validate_map(&self) -> Result<()> {
        let [w, h] = self.map.resolution;
        ensure!(w > 0 && h > 0, "map.resolution must be positive");
        let within = |points: &[[f32; 2]; 4]| {
            points.iter().all(|[x, y]| {
                x.is_finite()
                    && y.is_finite()
                    && *x >= 0.0
                    && *y >= 0.0
                    && *x <= w as f32
                    && *y <= h as f32
            })
        };
        let degenerate = |points: &[[f32; 2]; 4]| {
            let area: f32 = points
                .iter()
                .zip(points.iter().cycle().skip(1))
                .take(4)
                .map(|([x, y], [u, v])| x * v - y * u)
                .sum();
            !(area.is_finite() && area.abs() > 0.0)
        };
        ensure!(within(&self.map.bed), "map.bed: points outside resolution");
        ensure!(
            !degenerate(&self.map.bed),
            "map.bed: degenerate quadrilateral"
        );
        if let Some(points) = &self.map.bed_floor {
            ensure!(within(points), "map.bed_floor: points outside resolution");
            ensure!(
                !degenerate(points),
                "map.bed_floor: degenerate quadrilateral"
            );
        }
        if let Some([width, length]) = self.map.bed_size_m {
            ensure!(
                width.is_finite() && length.is_finite() && width > 0.0 && length > 0.0,
                "map.bed_size_m: the bed's width and length in metres, positive"
            );
        }
        if let Some(shadow) = &self.shadow {
            ensure!(
                shadow.near_m.is_finite() && shadow.near_m > 0.0,
                "shadow.near_m: a distance in metres, positive"
            );
            ensure!(
                shadow.support_margin_m.is_finite() && shadow.support_margin_m >= 0.0,
                "shadow.support_margin_m: a distance in metres, not negative"
            );
            ensure!(
                self.map.bed_size_m.is_some(),
                "shadow needs map.bed_size_m: near the bed is measured in metres"
            );
        }
        if let Some(occupancy) = &self.occupancy {
            match (occupancy.geometry, &occupancy.two_planes) {
                (RoomGeometry::Frontal, Some(_)) => {
                    anyhow::bail!("occupancy.two_planes needs occupancy.geometry = \"two_planes\"")
                }
                (RoomGeometry::TwoPlanes, None) => anyhow::bail!(
                    "occupancy.geometry = \"two_planes\" needs [occupancy.two_planes]"
                ),
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
                        self.map.bed_floor.is_some(),
                        "occupancy in two planes needs map.bed_floor: the support rests on it"
                    );
                    ensure!(
                        self.map.bed_size_m.is_some(),
                        "occupancy in two planes needs map.bed_size_m: distances are in metres"
                    );
                }
                (RoomGeometry::Frontal, None) => {}
            }
        }
        if let Some(HeadboardConfig::Quad(points)) = &self.map.headboard {
            ensure!(within(points), "map.headboard: points outside resolution");
            ensure!(
                !degenerate(points),
                "map.headboard: degenerate quadrilateral"
            );
        }
        if let Some(HeadboardConfig::Box([x0, y0, x1, y1])) = self.map.headboard {
            ensure!(
                [x0, y0, x1, y1].iter().all(|v| v.is_finite())
                    && x0 >= 0.0
                    && y0 >= 0.0
                    && x1 > x0
                    && y1 > y0
                    && x1 <= w as f32
                    && y1 <= h as f32,
                "map.headboard: invalid rectangle"
            );
        }
        if let Some([x0, y0, x1, y1]) = self.map.focus {
            ensure!(
                self.senses.detect.focus.is_some(),
                "map.focus: the main detector does not focus; [senses.detect.focus] is needed"
            );
            let inside = |x: f32, y: f32| x >= x0 && x <= x1 && y >= y0 && y <= y1;
            ensure!(
                [x0, y0, x1, y1].iter().all(|v| v.is_finite())
                    && x0 >= 0.0
                    && y0 >= 0.0
                    && x1 > x0
                    && y1 > y0
                    && x1 <= w as f32
                    && y1 <= h as f32,
                "map.focus: invalid rectangle"
            );
            ensure!(
                self.map.bed.iter().all(|[x, y]| inside(*x, *y)),
                "map.focus: it must contain the bed"
            );
        }
        Ok(())
    }
    pub fn policy(&self) -> Policy {
        let mut p = Policy::default();
        p.admission.min_confidence = self.policy.min_confidence;
        p.admission.same_body_iou = self.policy.same_body_iou;
        p.admission.near_bed_margin = self.policy.near_bed_margin;
        p.admission.bed_frame = self.policy.bed_frame;
        p.shadow = self.shadow.as_ref().map(|shadow| ShadowPolicy {
            near_m: shadow.near_m,
            support_margin_m: shadow.support_margin_m,
        });
        p.two_planes = self
            .occupancy
            .as_ref()
            .filter(|o| o.geometry == RoomGeometry::TwoPlanes)
            .and_then(|o| o.two_planes.as_ref())
            .map(|two| TwoPlanesPolicy {
                share: two.share,
                support_m: two.support_m,
                near_m: two.near_m,
            });
        p.persistence.dwell_secs = self.policy.dwell_secs;
        p.reinforce = self
            .senses
            .reinforce
            .as_ref()
            .map(|config| ReinforcePolicy {
                bed_padding: config.bed_padding,
            });
        p.focus = self.senses.detect.focus.as_ref().map(|config| FocusPolicy {
            pad: config.pad,
            wide_every_secs: config.wide_every_secs,
        });
        p.faces = self.senses.faces.as_ref().map(|config| FacePolicy {
            min_confidence: config.min_confidence,
            headboard_pad: config.headboard_pad.unwrap_or(0.0),
        });
        p.enrich = self.senses.pose.as_ref().map(|config| EnrichPolicy {
            move_threshold: config.move_threshold,
            depth: self.senses.depth.is_some(),
            min_keypoint_score: config.min_keypoint_score,
            posture_tolerance: self
                .senses
                .depth
                .as_ref()
                .map_or_else(defaults::posture_tolerance, |d| d.posture_tolerance),
            max_pose_age_secs: config.max_age_secs,
            crop_pad: config.crop_pad.unwrap_or(0.0),
        });
        p.calibration = self.calibration.as_ref().map(|config| CalibrationPolicy {
            retry_secs: config.retry_secs,
        });
        p.depth_pad = self.senses.depth.as_ref().and_then(|d| d.crop_pad);
        p.bed_posture = self.bed_posture.as_ref().map(|b| BedPosturePolicy {
            dwell_lying_secs: b.dwell_lying_secs,
            dwell_sitting_up_secs: b.dwell_sitting_up_secs,
            dwell_getting_out_secs: b.dwell_getting_out_secs,
        });
        p.verify = self.verify.as_ref().map(|v| VerifyPolicy {
            held: v.held.as_ref().map(|h| HeldVerifyPolicy {
                after_secs: h.after_secs,
                valid_secs: h.valid_secs,
            }),
            timeout_secs: v.timeout_secs,
            max_per_hour: v.max_per_hour,
            doubtful_disparity: v.doubtful_disparity.map(|[lo, hi]| Band { lo, hi }),
            doubtful_feet_margin: v.doubtful_feet_margin,
            frames: {
                let f = v.frames.clone().unwrap_or_default();
                let of = |b: Option<BurstConfig>| b.map(BurstConfig::burst).unwrap_or_default();
                Frames {
                    held: of(f.held_in_bed),
                    sitting: of(f.sitting_in_bed),
                    leaving: of(f.leaving_bed),
                }
            },
            sharpness: v.sharpness.map(|s| SharpnessPolicy {
                min_ratio: s.min_ratio,
                window: s.window,
                max_wait_secs: s.max_wait_secs,
            }),
        });
        p
    }
    pub fn site_map(&self) -> SiteMap {
        let mut map = SiteMap::new(Quad(self.map.bed.map(|[x, y]| Point { x, y })));
        let [width, height] = self.map.resolution;
        map.resolution = Some(FrameInfo { width, height });
        match self.map.headboard {
            Some(HeadboardConfig::Box([x0, y0, x1, y1])) => {
                map.headboard = Some(Rect::new(x0, y0, x1, y1));
            }
            Some(HeadboardConfig::Quad(points)) => {
                map.headboard_area = Some(Quad(points.map(|[x, y]| Point { x, y })));
            }
            None => {}
        }
        map.focus = self
            .map
            .focus
            .map(|[x0, y0, x1, y1]| Rect::new(x0, y0, x1, y1));
        map.bed_floor = self
            .map
            .bed_floor
            .map(|points| Quad(points.map(|[x, y]| Point { x, y })));
        map.bed_size_m = self.map.bed_size_m;
        map
    }
    pub fn ingest(&self) -> ingest::IngestConfig {
        ingest::IngestConfig {
            url: self.stream.url.clone(),
            username: self.stream.username.clone(),
            password: self.stream.password.clone(),
            keyframe_only: self.stream.keyframe_only,
            reconnect_delay: Duration::from_secs(self.stream.reconnect_delay_secs),
        }
    }
    pub fn model_path(&self, model: &Path) -> PathBuf {
        let base = std::env::var_os("CALIPSO_MODELS").map(PathBuf::from);
        resolve_model_path(model, base.as_deref())
    }
    /// Fixed field order and normalized defaults: independent of the TOML.
    pub fn profile(&self) -> Result<scene_core::contract::Profile> {
        use scene_core::contract::{LayerRef, LookKind, Profile, ProfileHash};
        use sha2::{Digest, Sha256};
        // Calibration and the bed posture enter the hash only if present:
        // without them, the previous one.
        let mut canonical = match &self.calibration {
            None => serde_json::to_vec(&(&self.policy, &self.map, &self.senses))?,
            Some(c) => serde_json::to_vec(&(&self.policy, &self.map, &self.senses, c))?,
        };
        if let Some(b) = &self.bed_posture {
            canonical.extend(serde_json::to_vec(b)?);
        }
        if let Some(v) = &self.verify {
            canonical.extend(serde_json::to_vec(v)?);
        }
        // The shadow adds to the scene: it enters the hash only if present.
        if let Some(shadow) = &self.shadow {
            canonical.extend(serde_json::to_vec(shadow)?);
        }
        // The room's geometry decides: it enters the hash only if present.
        if let Some(occupancy) = &self.occupancy {
            canonical.extend(serde_json::to_vec(occupancy)?);
        }
        let digest = Sha256::digest(canonical);
        let hex: String = digest
            .iter()
            .take(8)
            .map(|byte| format!("{byte:02x}"))
            .collect();
        let mut senses = vec![sense_profile(
            LookKind::Detect,
            &self.senses.detect.model,
            self.senses.detect.backend_confidence,
            Some(self.policy.min_confidence),
        )];
        if let Some(reinforce) = &self.senses.reinforce {
            senses.push(sense_profile(
                LookKind::Reinforce,
                &reinforce.model,
                reinforce.backend_confidence,
                Some(self.policy.min_confidence),
            ));
        }
        if let Some(faces) = &self.senses.faces {
            senses.push(sense_profile(
                LookKind::Faces,
                &faces.model,
                faces.backend_confidence,
                Some(faces.min_confidence),
            ));
        }
        // The pose is not admitted by score: `min_keypoint_score` picks keypoints.
        if let Some(pose) = &self.senses.pose {
            senses.push(scene_core::contract::SenseProfile {
                min_keypoint_score: Some(pose.min_keypoint_score),
                ..sense_profile(LookKind::Pose, &pose.model, pose.backend_confidence, None)
            });
        }
        // One model, two looks: the subject's (with pose) and the bed's. The depth
        // backend discards nothing.
        if let Some(depth) = &self.senses.depth {
            if self.senses.pose.is_some() {
                senses.push(sense_profile(LookKind::Depth, &depth.model, 0.0, None));
            }
            if self.calibration.is_some() {
                senses.push(sense_profile(LookKind::Calibrate, &depth.model, 0.0, None));
            }
        }
        Ok(Profile {
            camera: Some(LayerRef {
                id: self.camera.id.clone(),
                version: 0,
            }),
            resident: None,
            hash: ProfileHash(format!("sha256:{hex}")),
            senses,
        })
    }
    pub fn prefix(&self) -> String {
        self.nats
            .prefix
            .clone()
            .unwrap_or_else(|| format!("vision.{}.{}", self.camera.site, self.camera.id))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    pub(super) const VALID: &str = include_str!("../tests/support/profile.toml");
    /// The verifier: the policy maps one to one; the sense and the policy come
    /// together; the bands are checked; a profile without it keeps its hash.
    #[test]
    fn verify_section_becomes_the_policy_and_is_checked() {
        let base = format!(
            "{VALID}\n[senses.depth]\nmodel = \"yolo26s-depth.onnx\"\n[senses.pose]\nmodel = \"yolo26s-pose.onnx\"\n"
        );
        let map = "headboard = [100, 100, 500, 170]\n";
        let with_headboard = base.replacen("[map]\n", &format!("[map]\n{map}"), 1);
        let posture = "\n[bed_posture]\n";
        let sense = "\n[senses.verify]\n";
        let verify = "\n[verify]\nmax_per_hour = 20\ntimeout_secs = 8\ndoubtful_disparity = [0.03, 0.12]\ndoubtful_feet_margin = 0.06\n[verify.held]\nafter_secs = 30\nvalid_secs = 3600\n[verify.frames]\nsitting_in_bed = { frames = 3, every_secs = 1.5 }\n[verify.sharpness]\nmin_ratio = 0.5\nmax_wait_secs = 4\n";
        let text = format!("{with_headboard}{sense}{posture}{verify}");
        let config = Config::parse(&text).unwrap_or_else(|e| panic!("{e}\n{text}"));
        let policy = config.policy().verify.unwrap();
        assert_eq!(policy.max_per_hour, Some(20));
        assert_eq!(policy.timeout_secs, Some(8.0));
        assert_eq!(policy.doubtful_disparity, Some(Band { lo: 0.03, hi: 0.12 }));
        assert_eq!(policy.doubtful_feet_margin, Some(0.06));
        assert_eq!(
            (
                policy.frames.sitting.frames,
                policy.frames.sitting.every_secs
            ),
            (3, 1.5)
        );
        assert_eq!(policy.frames.held, Burst::ONE);
        let sharp = policy.sharpness.unwrap();
        assert_eq!(
            (sharp.min_ratio, sharp.window, sharp.max_wait_secs),
            (0.5, 20, 4.0)
        );
        let held = policy.held.unwrap();
        assert_eq!((held.after_secs, held.valid_secs), (30.0, Some(3600.0)));
        assert_eq!(
            config.senses.verify.as_ref().unwrap().subject,
            "calipso.verify.v1"
        );
        assert!(Config::parse(VALID).unwrap().policy().verify.is_none());
        for (bad, field) in [
            (
                format!("{with_headboard}{posture}{verify}"),
                "senses.verify",
            ),
            (
                format!("{with_headboard}{sense}"),
                "senses.verify without [verify]",
            ),
            (
                format!(
                    "{with_headboard}{sense}{posture}{}",
                    verify.replace("[0.03, 0.12]", "[0.12, 0.03]")
                ),
                "doubtful_disparity",
            ),
            (format!("{with_headboard}{sense}{verify}"), "bed_posture"),
            (
                format!(
                    "{with_headboard}{sense}{posture}{}",
                    verify.replace("0.06", "0.7")
                ),
                "doubtful_feet_margin",
            ),
            (
                format!(
                    "{with_headboard}{sense}{posture}{}",
                    verify.replace("max_per_hour = 20", "max_per_hour = 0")
                ),
                "max_per_hour",
            ),
        ] {
            let error = Config::parse(&bad).unwrap_err();
            assert!(error.to_string().contains(field), "{field}: {error}");
        }
        let hashed = Config::parse(&text).unwrap().profile().unwrap().hash;
        let without = format!("{with_headboard}{posture}");
        assert_ne!(
            hashed,
            Config::parse(&without).unwrap().profile().unwrap().hash
        );
    }

    #[test]
    fn reinforce_section_enables_policy_and_rejects_invalid_parameters() {
        let section = "\n[senses.reinforce]\nmodel = \"yolo26l.onnx\"\nbackend_confidence = 0.25\nbed_padding = 0.15\n";
        let config = Config::parse(&format!("{VALID}{section}")).unwrap();
        assert_eq!(config.policy().reinforce.unwrap().bed_padding, 0.15);
        assert!(Config::parse(VALID).unwrap().policy().reinforce.is_none());
        for (from, to, field) in [
            ("yolo26l.onnx", "", "model"),
            ("0.25", "-0.1", "backend_confidence"),
            ("0.25", "1.1", "backend_confidence"),
            ("0.25", "nan", "backend_confidence"),
            ("0.25", "inf", "backend_confidence"),
            ("0.25", "0.60", "backend_confidence"),
            ("0.15", "-0.1", "bed_padding"),
            ("0.15", "nan", "bed_padding"),
            ("0.15", "inf", "bed_padding"),
        ] {
            let error =
                Config::parse(&format!("{VALID}{}", section.replace(from, to))).unwrap_err();
            assert!(error.to_string().contains(field), "{error}");
        }
        assert!(Config::parse(&format!("{VALID}{section}unknown = 1\n")).is_err());
    }

    #[test]
    fn reinforcement_preserves_the_baseline_hash_and_comes_after_detection() {
        use scene_core::contract::LookKind;
        let legacy = Config::parse(VALID).unwrap().profile().unwrap();
        assert_eq!(legacy.hash.0, "sha256:239a5803f0c15901");
        let text =
            format!("{VALID}\n[senses.reinforce]\nmodel = \"yolo26l.onnx\"\nbed_padding = 0.15\n");
        let profile = Config::parse(&text).unwrap().profile().unwrap();
        assert_ne!(profile.hash, legacy.hash);
        assert_eq!(
            profile.senses.iter().map(|s| s.kind).collect::<Vec<_>>(),
            vec![LookKind::Detect, LookKind::Reinforce]
        );
        assert_eq!(profile.senses[1].model, "yolo26l.onnx");
        assert_eq!(profile.senses[1].backend_confidence, 0.25);
        assert_eq!(profile.senses[1].min_confidence, Some(0.5));
        let changed_padding = Config::parse(&text.replace("0.15", "0.20"))
            .unwrap()
            .profile()
            .unwrap();
        assert_ne!(profile.hash, changed_padding.hash);
    }

    #[test]
    fn model_paths_resolve_relative_and_absolute_paths_for_every_role() {
        for model in ["yolo26s.onnx", "yolo26l.onnx", "face/yolov12l-face.onnx"] {
            let model = Path::new(model);
            assert_eq!(
                resolve_model_path(model, Some(Path::new("/models"))),
                Path::new("/models").join(model)
            );
            assert_eq!(resolve_model_path(model, None), model);
        }
        let absolute = Path::new("/external/yolo26l.onnx");
        assert_eq!(
            resolve_model_path(absolute, Some(Path::new("/models"))),
            absolute
        );
        assert_eq!(Config::parse(VALID).unwrap().model_path(absolute), absolute);
    }

    #[test]
    fn faces_section_enables_policy_names_roles_and_preserves_legacy_hash() {
        use scene_core::contract::LookKind;
        let baseline = Config::parse(VALID).unwrap();
        assert!(baseline.policy().faces.is_none());
        assert_eq!(
            baseline.profile().unwrap().hash.0,
            "sha256:239a5803f0c15901"
        );
        let section = "\n[senses.faces]\nmodel = \"face/yolov12l-face.onnx\"\nbackend_confidence = 0.25\nmin_confidence = 0.30\n";
        let config = Config::parse(&format!("{VALID}{section}")).unwrap();
        assert_eq!(config.policy().faces.unwrap().min_confidence, 0.30);
        let profile = config.profile().unwrap();
        assert_ne!(profile.hash, baseline.profile().unwrap().hash);
        assert_eq!(
            profile.senses.iter().map(|s| s.kind).collect::<Vec<_>>(),
            vec![LookKind::Detect, LookKind::Faces]
        );
        assert_eq!(profile.senses[1].min_confidence, Some(0.30));
        let both = Config::parse(&format!(
            "{VALID}{section}\n[senses.reinforce]\nmodel = \"yolo26l.onnx\"\nbed_padding = 0.15\n"
        ))
        .unwrap()
        .profile()
        .unwrap();
        assert_eq!(
            both.senses.iter().map(|s| s.kind).collect::<Vec<_>>(),
            vec![LookKind::Detect, LookKind::Reinforce, LookKind::Faces]
        );
        for (from, to, field) in [
            ("face/yolov12l-face.onnx", "", "model"),
            ("0.25", "-0.1", "backend_confidence"),
            ("0.25", "1.1", "backend_confidence"),
            ("0.25", "nan", "backend_confidence"),
            ("0.25", "inf", "backend_confidence"),
            ("0.25", "0.40", "backend_confidence"),
            ("0.30", "-0.1", "min_confidence"),
            ("0.30", "1.1", "min_confidence"),
            ("0.30", "nan", "min_confidence"),
            ("0.30", "inf", "min_confidence"),
        ] {
            let error =
                Config::parse(&format!("{VALID}{}", section.replace(from, to))).unwrap_err();
            assert!(error.to_string().contains(field), "{error}");
        }
        assert!(Config::parse(&format!("{VALID}{section}unknown = 1\n")).is_err());
    }

    #[test]
    fn pose_section_enables_enrich_validates_and_preserves_legacy_hash() {
        use scene_core::contract::LookKind;
        let baseline = Config::parse(VALID).unwrap();
        assert!(baseline.policy().enrich.is_none());
        let section = "\n[senses.pose]\nmodel = \"yolo26s-pose.onnx\"\nbackend_confidence = 0.25\nmove_threshold = 0.1\nmin_keypoint_score = 0.3\n";
        let config = Config::parse(&format!("{VALID}{section}")).unwrap();
        let enrich = config.policy().enrich.unwrap();
        assert_eq!(
            (
                enrich.move_threshold,
                enrich.depth,
                enrich.min_keypoint_score
            ),
            (0.1, false, 0.3)
        );
        let defaults = Config::parse(&format!(
            "{VALID}\n[senses.pose]\nmodel = \"yolo26s-pose.onnx\"\n"
        ))
        .unwrap();
        assert_eq!(defaults.policy().enrich, config.policy().enrich);
        assert_eq!(
            defaults.profile().unwrap().hash,
            config.profile().unwrap().hash
        );
        let profile = config.profile().unwrap();
        assert_ne!(profile.hash.0, "sha256:239a5803f0c15901");
        assert_eq!(profile.senses[1].kind, LookKind::Pose);
        assert_eq!(profile.senses[1].model, "yolo26s-pose.onnx");
        assert_eq!(profile.senses[1].min_confidence, None);
        for (from, to, field) in [
            ("yolo26s-pose.onnx", "", "model"),
            ("0.25", "1.1", "backend_confidence"),
            ("0.25", "nan", "backend_confidence"),
            ("0.1\n", "-0.1\n", "move_threshold"),
            ("0.1\n", "inf\n", "move_threshold"),
            ("0.3\n", "-0.1\n", "min_keypoint_score"),
            ("0.3\n", "1.5\n", "min_keypoint_score"),
        ] {
            let error =
                Config::parse(&format!("{VALID}{}", section.replace(from, to))).unwrap_err();
            assert!(error.to_string().contains(field), "{error}");
        }
        assert!(Config::parse(&format!("{VALID}{section}unknown = 1\n")).is_err());
    }

    #[test]
    fn the_shadow_needs_the_bed_size_validates_and_leaves_the_hash_when_absent() {
        let baseline = Config::parse(VALID).unwrap();
        assert!(baseline.policy().shadow.is_none());
        assert!(baseline.site_map().bed_floor.is_none());
        let map = "bed_floor = [[110, 150], [490, 150], [490, 430], [110, 430]]\nbed_size_m = [0.9, 2.0]\n";
        let with_map = VALID.replacen("[map]\n", &format!("[map]\n{map}"), 1);
        let shadow = "\n[shadow]\nnear_m = 0.6\n";
        let config = Config::parse(&format!("{with_map}{shadow}")).unwrap();
        assert_eq!(
            config.policy().shadow,
            Some(ShadowPolicy {
                near_m: 0.6,
                support_margin_m: 0.0
            })
        );
        let site = config.site_map();
        assert_eq!(site.bed_size_m, Some([0.9, 2.0]));
        assert!(site.bed_floor.is_some());
        // The map's keys alone change no policy and keep their own hash.
        let map_only = Config::parse(&with_map).unwrap();
        assert!(map_only.policy().shadow.is_none());
        assert_ne!(
            map_only.profile().unwrap().hash,
            config.profile().unwrap().hash
        );
        let error = Config::parse(&format!("{VALID}{shadow}")).unwrap_err();
        assert!(error.to_string().contains("bed_size_m"), "{error}");
        for (from, to, field) in [
            ("near_m = 0.6", "near_m = 0", "shadow.near_m"),
            ("near_m = 0.6", "near_m = nan", "shadow.near_m"),
            ("[0.9, 2.0]", "[0.9, -2.0]", "map.bed_size_m"),
            ("[490, 430]", "[990, 430]", "map.bed_floor"),
        ] {
            let error =
                Config::parse(&format!("{with_map}{shadow}").replace(from, to)).unwrap_err();
            assert!(error.to_string().contains(field), "{error}");
        }
        assert!(Config::parse(&format!("{with_map}{shadow}unknown = 1\n")).is_err());
    }

    #[test]
    fn two_planes_needs_its_geometry_validates_and_leaves_the_hash_when_absent() {
        let baseline = Config::parse(VALID).unwrap();
        assert!(baseline.policy().two_planes.is_none());
        let map = "bed_floor = [[110, 150], [490, 150], [490, 430], [110, 430]]\nbed_size_m = [0.9, 2.0]\n";
        let with_map = VALID.replacen("[map]\n", &format!("[map]\n{map}"), 1);
        let occupancy = "\n[occupancy]\ngeometry = \"two_planes\"\n\n[occupancy.two_planes]\nshare = 0.45\nsupport_m = 0.10\nnear_m = 0.60\n";
        let config = Config::parse(&format!("{with_map}{occupancy}")).unwrap();
        assert_eq!(
            config.policy().two_planes,
            Some(TwoPlanesPolicy {
                share: 0.45,
                support_m: 0.10,
                near_m: 0.60
            })
        );
        // Frontal said out loud is frontal, with the hash of its own section.
        let frontal = Config::parse(&format!(
            "{with_map}\n[occupancy]\ngeometry = \"frontal\"\n"
        ))
        .unwrap();
        assert!(frontal.policy().two_planes.is_none());
        let map_only = Config::parse(&with_map).unwrap();
        assert_ne!(
            map_only.profile().unwrap().hash,
            config.profile().unwrap().hash
        );
        for (base, from, to, field) in [
            (VALID.to_string(), "", "", "map.bed_floor"),
            (
                with_map.clone(),
                "share = 0.45",
                "share = 0",
                "two_planes.share",
            ),
            (
                with_map.clone(),
                "share = 0.45",
                "share = 1.5",
                "two_planes.share",
            ),
            (
                with_map.clone(),
                "support_m = 0.10",
                "support_m = -1",
                "two_planes.support_m",
            ),
            (
                with_map.clone(),
                "near_m = 0.60",
                "near_m = 0",
                "two_planes.near_m",
            ),
            (
                with_map.clone(),
                "geometry = \"two_planes\"",
                "geometry = \"frontal\"",
                "needs occupancy.geometry",
            ),
        ] {
            let text = format!("{base}{occupancy}");
            let text = if from.is_empty() {
                text
            } else {
                text.replace(from, to)
            };
            let error = Config::parse(&text).unwrap_err();
            assert!(error.to_string().contains(field), "{error}");
        }
        let sizeless = with_map.replace("bed_size_m = [0.9, 2.0]\n", "");
        let error = Config::parse(&format!("{sizeless}{occupancy}")).unwrap_err();
        assert!(error.to_string().contains("bed_size_m"), "{error}");
        let sectionless = format!("{with_map}\n[occupancy]\ngeometry = \"two_planes\"\n");
        let error = Config::parse(&sectionless).unwrap_err();
        assert!(
            error.to_string().contains("[occupancy.two_planes]"),
            "{error}"
        );
        assert!(Config::parse(&format!("{with_map}{occupancy}unknown = 1\n")).is_err());
    }

    #[test]
    fn depth_and_calibration_enable_policies_validate_together_and_keep_legacy_hash() {
        use scene_core::contract::LookKind;
        let baseline = Config::parse(VALID).unwrap();
        assert_eq!(
            baseline.profile().unwrap().hash.0,
            "sha256:239a5803f0c15901"
        );
        assert!(baseline.policy().calibration.is_none());
        let pose = "\n[senses.pose]\nmodel = \"yolo26s-pose.onnx\"\n";
        let depth = "\n[senses.depth]\nmodel = \"yolo26s-depth.onnx\"\n";
        let calibration = "\n[calibration]\nretry_secs = 30.0\n";
        let kinds = |text: &str| {
            let config = Config::parse(&format!("{VALID}{text}")).unwrap();
            let profile = config.profile().unwrap();
            assert!(
                profile.senses[1..]
                    .iter()
                    .all(|s| s.min_confidence.is_none())
            );
            (
                config.policy(),
                profile.hash,
                profile.senses.iter().map(|s| s.kind).collect::<Vec<_>>(),
            )
        };
        let (policy, with_pose, roles) = kinds(&format!("{pose}{depth}"));
        assert!(policy.enrich.unwrap().depth && policy.calibration.is_none());
        assert_eq!(roles, [LookKind::Detect, LookKind::Pose, LookKind::Depth]);
        let (policy, with_calibration, roles) = kinds(&format!("{depth}{calibration}"));
        assert!(policy.enrich.is_none());
        assert_eq!(policy.calibration.unwrap().retry_secs, 30.0);
        assert_eq!(roles, [LookKind::Detect, LookKind::Calibrate]);
        let (_, all, roles) = kinds(&format!("{pose}{depth}{calibration}"));
        assert_eq!(
            roles,
            [
                LookKind::Detect,
                LookKind::Pose,
                LookKind::Depth,
                LookKind::Calibrate
            ]
        );
        let (_, other_retry, _) = kinds(&format!("{depth}\n[calibration]\nretry_secs = 60.0\n"));
        let (_, default_retry, _) = kinds(&format!("{depth}\n[calibration]\n"));
        let (_, explicit_default, _) =
            kinds(&format!("{depth}\n[calibration]\nretry_secs = 2.0\n"));
        assert_eq!(default_retry, explicit_default);
        for hash in [&with_pose, &with_calibration, &all, &other_retry] {
            assert_ne!(hash.0, "sha256:239a5803f0c15901");
        }
        assert_ne!(with_calibration, other_retry);
        for (text, field) in [
            (calibration.to_string(), "calibration"),
            (depth.to_string(), "senses.depth"),
            (
                format!("{pose}{}", depth.replace("yolo26s-depth.onnx", "")),
                "senses.depth.model",
            ),
            (
                format!("{depth}\n[calibration]\nretry_secs = -1.0\n"),
                "calibration.retry_secs",
            ),
            (
                format!("{depth}\n[calibration]\nretry_secs = nan\n"),
                "calibration.retry_secs",
            ),
        ] {
            let error = Config::parse(&format!("{VALID}{text}")).unwrap_err();
            assert!(error.to_string().contains(field), "{error}");
        }
        assert!(Config::parse(&format!("{VALID}{pose}{depth}unknown = 1\n")).is_err());
        assert!(Config::parse(&format!("{VALID}{depth}{calibration}unknown = 1\n")).is_err());
    }

    #[test]
    fn the_hash_covers_only_the_scene_and_normalizes_defaults() {
        let baseline = Config::parse(VALID).unwrap().profile().unwrap();
        assert_eq!(baseline.hash.0.len(), 23);
        let changed_transport = VALID
            .replace("room-1", "room-2")
            .replace("18554", "18555")
            .replace("4222", "4223")
            .replace(
                "[senses.detect]",
                "username = \"private\"\npassword = \"secret\"\n[senses.detect]",
            );
        assert_eq!(
            Config::parse(&changed_transport)
                .unwrap()
                .profile()
                .unwrap()
                .hash,
            baseline.hash
        );
        let explicit_defaults = format!(
            "{VALID}\n[policy]\ndwell_secs = 2.0\nmin_confidence = 0.5\nsame_body_iou = 0.5\nnear_bed_margin = 0.1\n"
        );
        assert_eq!(
            Config::parse(&explicit_defaults)
                .unwrap()
                .profile()
                .unwrap()
                .hash,
            baseline.hash
        );
        for changed in [
            VALID.replace("yolo26s", "yolo26m"),
            VALID.replace("500,400", "490,400"),
            format!("{VALID}\n[policy]\ndwell_secs = 3.0"),
        ] {
            assert_ne!(
                Config::parse(&changed).unwrap().profile().unwrap().hash,
                baseline.hash
            );
        }
        let mut c = Config::parse(VALID).unwrap();
        c.senses.detect.backend_confidence = 0.1;
        assert_ne!(c.profile().unwrap().hash, baseline.hash);
        c = Config::parse(VALID).unwrap();
        c.map.resolution = [801, 600];
        assert_ne!(c.profile().unwrap().hash, baseline.hash);
    }
    #[test]
    fn config_validates_and_translates_without_io() {
        let c = Config::parse(VALID).unwrap();
        assert_eq!(c.site_map().resolution.unwrap().width, 800);
        assert_eq!(c.policy().persistence.dwell_secs, 2.0);
        assert_eq!(c.prefix(), "vision.test.room-1");
        assert!(c.ingest().keyframe_only);
    }
    #[test]
    fn resolution_is_required_with_a_reason() {
        let e = Config::parse(&VALID.replace("resolution = [800, 600]", "")).unwrap_err();
        assert!(e.to_string().contains("resolution"), "{e}");
    }
    #[test]
    fn rejects_invalid_policies_and_v2_options() {
        for extra in [
            "dwell_secs = -1.0",
            "dwell_secs = nan",
            "min_confidence = nan",
            "same_body_iou = 2.0",
            "observations = 3",
        ] {
            assert!(
                Config::parse(&format!("{VALID}\n[policy]\n{extra}")).is_err(),
                "{extra}"
            );
        }
        assert!(Config::parse(&format!("{VALID}\n[stability]\nobservations = 3")).is_err());
        assert!(Config::parse(&VALID.replace("[800, 600]", "[0, 600]")).is_err());
    }
}
