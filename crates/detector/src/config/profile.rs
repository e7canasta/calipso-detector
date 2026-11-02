//! The published profile: what the scene declares about how it was produced,
//! and the hash that covers it.

use super::*;
use anyhow::Result;
use std::path::Path;

/// One line of the published profile: the sense's model by its file name.
pub(super) fn sense_profile(
    kind: scene_contract::LookKind,
    model: &Path,
    backend_confidence: f32,
    min_confidence: Option<f32>,
) -> scene_contract::SenseProfile {
    scene_contract::SenseProfile {
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
    /// Fixed field order and normalized defaults: independent of the TOML.
    pub fn profile(&self) -> Result<scene_contract::Profile> {
        use scene_contract::{LayerRef, LookKind, Profile, ProfileHash};
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
        let min_confidence = self.policy.min_confidence;
        let mut senses = vec![self.senses.detect.sense(min_confidence)];
        if let Some(reinforce) = &self.senses.reinforce {
            senses.push(reinforce.sense(min_confidence));
        }
        if let Some(faces) = &self.senses.faces {
            senses.push(faces.sense());
        }
        if let Some(pose) = &self.senses.pose {
            senses.push(pose.sense());
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
}
