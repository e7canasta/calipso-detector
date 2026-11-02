//! `[publish]`: which contract versions the shell publishes. v4 is always there,
//! since consumers still read it (C2); v5 goes beside it when the profile lists
//! it. The section is the shell's choice, not the engine's: it stays out of the
//! profile's hash.
use anyhow::{Result, bail};
use serde::Deserialize;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PublishConfig {
    /// `["v4"]` by default; `["v4", "v5"]` publishes both, each with its own
    /// numbering and its own subjects.
    #[serde(default = "default_versions")]
    pub versions: Vec<String>,
}

fn default_versions() -> Vec<String> {
    vec!["v4".to_owned()]
}

impl Default for PublishConfig {
    fn default() -> Self {
        Self {
            versions: default_versions(),
        }
    }
}

impl PublishConfig {
    /// The versions as the shell publishes them: validated, so each one parses.
    pub fn versions(&self) -> Vec<crate::publish::Version> {
        self.versions
            .iter()
            .filter_map(|v| crate::publish::Version::parse(v))
            .collect()
    }

    /// Each version is one the shell knows, once, and v4 is among them.
    pub(super) fn validate(&self) -> Result<()> {
        for (i, version) in self.versions.iter().enumerate() {
            if !matches!(version.as_str(), "v4" | "v5") {
                bail!("publish.versions: {version:?} is not a contract version (v4, v5)");
            }
            if self.versions[..i].contains(version) {
                bail!("publish.versions: {version:?} is listed twice");
            }
        }
        if !self.versions.iter().any(|v| v == "v4") {
            bail!("publish.versions must list \"v4\": consumers still read it");
        }
        Ok(())
    }
}
