//! Minimal shell configuration; policies are handed over by value.
use anyhow::{Context, Result};
use std::{
    path::{Path, PathBuf},
    time::Duration,
};

// One module per capability: its section's struct, its checks and its
// translation. `schema` is the common file and composes them.
mod bed_posture;
mod calibration;
mod depth;
mod detect;
mod faces;
mod occupancy;
mod parts;
mod policy;
mod pose;
mod profile;
mod reinforce;
mod requirements;
mod schema;
mod shadow;
#[cfg(test)]
mod tests;
mod validate;
mod verify;

pub(crate) use bed_posture::BedPostureConfig;
pub(crate) use calibration::CalibrationConfig;
pub(crate) use detect::Focus;
pub(crate) use faces::Faces;
pub(crate) use occupancy::OccupancyConfig;
pub(crate) use parts::PartsConfig;
pub(crate) use reinforce::Reinforce;
pub(crate) use requirements::Requirements;
pub(crate) use schema::*;
pub use schema::{Config, NatsConfig};
pub(crate) use shadow::ShadowConfig;
pub(crate) use verify::VerifyConfig;

fn resolve_model_path(model: &Path, base: Option<&Path>) -> PathBuf {
    match base {
        Some(base) if !model.is_absolute() => base.join(model),
        _ => model.to_owned(),
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
}

impl Config {
    /// The stream stage's configuration. The URL is validated again here by
    /// `IngestConfig::new` on purpose: `Config::validate` has already parsed
    /// the same URL before anything starts, so this cannot fail at runtime.
    pub fn ingest(&self) -> ingest::IngestConfig {
        let credentials = self
            .stream
            .username
            .clone()
            .map(|username| ingest::Credentials {
                username,
                password: self.stream.password.clone().unwrap_or_default(),
            });
        let sampling = if self.stream.keyframe_only {
            ingest::Sampling::KeyframesOnly
        } else {
            ingest::Sampling::Continuous
        };
        ingest::IngestConfig::new(
            &self.stream.url,
            credentials,
            sampling,
            Duration::from_secs(self.stream.reconnect_delay_secs),
        )
        .expect("stream.url is validated when the configuration loads")
    }
    pub fn model_path(&self, model: &Path) -> PathBuf {
        let base = std::env::var_os("CALIPSO_MODELS").map(PathBuf::from);
        resolve_model_path(model, base.as_deref())
    }
}

impl Config {
    pub fn prefix(&self) -> String {
        self.nats
            .prefix
            .clone()
            .unwrap_or_else(|| format!("vision.{}.{}", self.camera.site, self.camera.id))
    }
}
