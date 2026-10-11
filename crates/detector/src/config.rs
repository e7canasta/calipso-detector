//! Minimal shell configuration; policies are handed over by value.
use anyhow::{Context, Result, ensure};
use scene_core::{
    geom::{Point, Quad, Rect},
    map::{FrameInfo, SiteMap},
    policy::{CalibrationPolicy, EnrichPolicy, FacePolicy, Policy, ReinforcePolicy},
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
}
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Faces {
    pub model: PathBuf,
    #[serde(default = "defaults::backend_confidence")]
    pub backend_confidence: f32,
    #[serde(default = "defaults::face_min_confidence")]
    pub min_confidence: f32,
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
}
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct PolicyConfig {
    pub min_confidence: f32,
    pub same_body_iou: f32,
    pub near_bed_margin: f32,
    /// Source seconds a new count must hold.
    pub dwell_secs: f64,
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
        }
    }
}
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct MapConfig {
    pub resolution: [u32; 2],
    pub bed: [[f32; 2]; 4],
    pub headboard: Option<[f32; 4]>,
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
    pub fn keyframe_only() -> bool {
        true
    }
    pub fn reconnect_delay_secs() -> u64 {
        5
    }
    pub fn sample_interval_ms() -> u64 {
        1000
    }
    pub fn stall_timeout_secs() -> u64 {
        10
    }
    pub fn backend_confidence() -> f32 {
        0.25
    }
    pub fn calibration_retry_secs() -> f64 {
        2.0
    }
    pub fn posture_tolerance() -> f32 {
        0.05
    }
    pub fn is_posture_tolerance(value: &f32) -> bool {
        *value == posture_tolerance()
    }
    pub fn move_threshold() -> f32 {
        0.10
    }
    pub fn bed_padding() -> f32 {
        0.15
    }
    pub fn face_min_confidence() -> f32 {
        0.30
    }
    pub fn min_keypoint_score() -> f32 {
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
        self.validate_map()
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
        }
        if let Some(pose) = &self.senses.pose {
            model("senses.pose.model", &pose.model)?;
            fraction("senses.pose.backend_confidence", pose.backend_confidence)?;
            fraction("senses.pose.min_keypoint_score", pose.min_keypoint_score)?;
            not_negative("senses.pose.move_threshold", f64::from(pose.move_threshold))?;
            ensure!(
                pose.max_age_secs.is_none_or(|s| s.is_finite() && s > 0.0),
                "senses.pose.max_age_secs must be finite and positive"
            );
        }
        if let Some(depth) = &self.senses.depth {
            model("senses.depth.model", &depth.model)?;
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
        not_negative("policy.dwell_secs", self.policy.dwell_secs)
    }

    /// The bed inside the resolution and not degenerate; a valid headboard.
    fn validate_map(&self) -> Result<()> {
        let [w, h] = self.map.resolution;
        ensure!(w > 0 && h > 0, "map.resolution must be positive");
        ensure!(
            self.map.bed.iter().all(|[x, y]| x.is_finite()
                && y.is_finite()
                && *x >= 0.0
                && *y >= 0.0
                && *x <= w as f32
                && *y <= h as f32),
            "map.bed: points outside resolution"
        );
        let area: f32 = self
            .map
            .bed
            .iter()
            .zip(self.map.bed.iter().cycle().skip(1))
            .take(4)
            .map(|([x, y], [u, v])| x * v - y * u)
            .sum();
        ensure!(
            area.is_finite() && area.abs() > 0.0,
            "map.bed: degenerate quadrilateral"
        );
        if let Some([x0, y0, x1, y1]) = self.map.headboard {
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
        Ok(())
    }
    pub fn policy(&self) -> Policy {
        let mut p = Policy::default();
        p.admission.min_confidence = self.policy.min_confidence;
        p.admission.same_body_iou = self.policy.same_body_iou;
        p.admission.near_bed_margin = self.policy.near_bed_margin;
        p.persistence.dwell_secs = self.policy.dwell_secs;
        p.reinforce = self
            .senses
            .reinforce
            .as_ref()
            .map(|config| ReinforcePolicy {
                bed_padding: config.bed_padding,
            });
        p.faces = self.senses.faces.as_ref().map(|config| FacePolicy {
            min_confidence: config.min_confidence,
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
        });
        p.calibration = self.calibration.as_ref().map(|config| CalibrationPolicy {
            retry_secs: config.retry_secs,
        });
        p
    }
    pub fn site_map(&self) -> SiteMap {
        let mut map = SiteMap::new(Quad(self.map.bed.map(|[x, y]| Point { x, y })));
        let [width, height] = self.map.resolution;
        map.resolution = Some(FrameInfo { width, height });
        map.headboard = self
            .map
            .headboard
            .map(|[x0, y0, x1, y1]| Rect::new(x0, y0, x1, y1));
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
        // Calibration enters the hash only if present: without it, the previous one.
        let canonical = match &self.calibration {
            None => serde_json::to_vec(&(&self.policy, &self.map, &self.senses))?,
            Some(c) => serde_json::to_vec(&(&self.policy, &self.map, &self.senses, c))?,
        };
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
            senses.push(sense_profile(
                LookKind::Pose,
                &pose.model,
                pose.backend_confidence,
                None,
            ));
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
    pub const VALID: &str = include_str!("../tests/support/profile.toml");
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
