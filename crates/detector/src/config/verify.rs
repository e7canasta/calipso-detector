//! The slow verifier: `[verify]` says what this stream verifies, and
//! `[senses.verify]` says where the verifier is (a local service behind NATS).
//! Each stream pays only for what matters to it (white book, volume III).

use super::schema::defaults;
use super::validate::{not_negative, positive};
use anyhow::{Result, ensure};
use scene_core::policy::{Band, Burst, Frames, HeldVerifyPolicy, SharpnessPolicy, VerifyPolicy};
use serde::{Deserialize, Serialize};

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

/// The verifier's two sections together. `depth`: `[senses.depth]` is present
/// (the disparity needs it); `bed_posture`: `[bed_posture]` is present (layer 1
/// reads the doubtful digits). Without `[verify]`, nothing may be asked; with
/// it, the sense is required.
pub(super) fn validate(
    verify: Option<&VerifyConfig>,
    sense: Option<&VerifySense>,
    depth: bool,
    bed_posture: bool,
) -> Result<()> {
    let Some(v) = verify else {
        ensure!(
            sense.is_none(),
            "senses.verify without [verify]: nothing would be asked"
        );
        return Ok(());
    };
    let Some(sense) = sense else {
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
            depth,
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
            bed_posture,
            "verify.doubtful_* needs [bed_posture]: layer 1 reads the digit"
        );
    }
    Ok(())
}

impl VerifyConfig {
    /// `Policy::verify`.
    pub(super) fn policy(&self) -> VerifyPolicy {
        VerifyPolicy {
            held: self.held.as_ref().map(|h| HeldVerifyPolicy {
                after_secs: h.after_secs,
                valid_secs: h.valid_secs,
            }),
            timeout_secs: self.timeout_secs,
            max_per_hour: self.max_per_hour,
            doubtful_disparity: self.doubtful_disparity.map(|[lo, hi]| Band { lo, hi }),
            doubtful_feet_margin: self.doubtful_feet_margin,
            frames: {
                let f = self.frames.clone().unwrap_or_default();
                let of = |b: Option<BurstConfig>| b.map(BurstConfig::burst).unwrap_or_default();
                Frames {
                    held: of(f.held_in_bed),
                    sitting: of(f.sitting_in_bed),
                    leaving: of(f.leaving_bed),
                }
            },
            sharpness: self.sharpness.map(|s| SharpnessPolicy {
                min_ratio: s.min_ratio,
                window: s.window,
                max_wait_secs: s.max_wait_secs,
            }),
        }
    }
}
