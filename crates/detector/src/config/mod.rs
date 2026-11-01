//! Minimal shell configuration; policies are handed over by value.
use anyhow::{Context, Result};
use std::{
    path::{Path, PathBuf},
    time::Duration,
};

mod policy;
mod profile;
mod schema;
#[cfg(test)]
mod tests;
mod validate;

pub use schema::*;

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
}

impl Config {
    pub fn prefix(&self) -> String {
        self.nats
            .prefix
            .clone()
            .unwrap_or_else(|| format!("vision.{}.{}", self.camera.site, self.camera.id))
    }
}
