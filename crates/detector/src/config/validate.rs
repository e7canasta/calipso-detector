//! What a profile must hold before anything starts. The common sections
//! (identifiers, transport, policy, the site map) are checked here; each
//! capability checks its own section in its module, and its error names the
//! section first: `[faces]: senses.faces.min_confidence must be…`. The first
//! error found is reported.

use super::*;
use anyhow::{Context, Result, anyhow, ensure};
use scene_core::units::{Confidence, Seconds, UnitError};
use std::path::Path;

/// A geometry measure, built by its constructor in the core: its refusal names
/// the field, as the other checks do.
pub(super) fn measured<T>(name: &str, value: std::result::Result<T, UnitError>) -> Result<T> {
    value.map_err(|e| anyhow!("{name} {e}"))
}

/// A score or a ratio: finite and between 0 and 1.
pub(super) fn fraction(name: &str, value: f32) -> Result<()> {
    ensure!(
        value.is_finite() && (0.0..=1.0).contains(&value),
        "{name} must be between 0 and 1"
    );
    Ok(())
}

/// A duration of the core: its section names the error, as the checks above.
pub(super) fn seconds(section: &str, name: &str, value: f64) -> Result<Seconds> {
    Seconds::new(value).map_err(|e| anyhow!("[{section}]: {name} {e}"))
}

/// A confidence or a threshold of the core, in `[0, 1]`.
pub(super) fn confidence(section: &str, name: &str, value: f32) -> Result<Confidence> {
    Confidence::new(value).map_err(|e| anyhow!("[{section}]: {name} {e}"))
}

/// A margin or a delay: finite and not negative.
pub(super) fn not_negative(name: &str, value: f64) -> Result<()> {
    ensure!(
        value.is_finite() && value >= 0.0,
        "{name} must be finite and not negative"
    );
    Ok(())
}

/// A period: finite and positive.
pub(super) fn positive(name: &str, value: f64) -> Result<()> {
    ensure!(
        value.is_finite() && value > 0.0,
        "{name} must be finite and positive"
    );
    Ok(())
}

/// Every sense names its model.
pub(super) fn model(name: &str, path: &Path) -> Result<()> {
    ensure!(!path.as_os_str().is_empty(), "{name} is empty");
    Ok(())
}

/// The section's name leads the error, so the operator knows where to look.
fn in_section(section: &str, checked: Result<()>) -> Result<()> {
    checked.map_err(|error| anyhow::anyhow!("[{section}]: {error}"))
}

impl Config {
    /// Each section checks itself; the first error found is reported.
    pub(super) fn validate(&self) -> Result<()> {
        let needs = Requirements::of(self);
        self.validate_identifiers()?;
        self.validate_transport()?;
        self.validate_senses(&needs)?;
        self.validate_policy()?;
        self.validate_map(&needs)?;
        self.validate_bed_posture(&needs)?;
        self.validate_verify(&needs)?;
        Ok(())
    }

    /// The verifier's two sections together: the sense and the policy come
    /// together, and each rule names its own section.
    fn validate_verify(&self, needs: &Requirements<'_>) -> Result<()> {
        in_section(
            "verify",
            verify::validate(self.verify.as_ref(), self.senses.verify.as_ref(), needs),
        )
    }

    /// Times finite and not negative; it needs where the head is (the
    /// headboard) and the body (a pose model).
    fn validate_bed_posture(&self, needs: &Requirements<'_>) -> Result<()> {
        let Some(b) = &self.bed_posture else {
            return Ok(());
        };
        in_section("bed_posture", b.validate(needs))
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
    fn validate_senses(&self, needs: &Requirements<'_>) -> Result<()> {
        in_section("detect", self.senses.detect.validate_sense())?;
        if let Some(reinforce) = &self.senses.reinforce {
            in_section("reinforce", reinforce.validate(self.policy.min_confidence))?;
        }
        if let Some(faces) = &self.senses.faces {
            in_section("faces", faces.validate())?;
        }
        if let Some(pose) = &self.senses.pose {
            in_section("pose", pose.validate())?;
        }
        if let Some(depth) = &self.senses.depth {
            in_section("depth", depth.validate(needs))?;
        }
        if let Some(calibration) = &self.calibration {
            in_section("calibration", calibration.validate(needs))?;
        }
        Ok(())
    }

    /// Confidences between 0 and 1; a backend never stricter than the policy.
    fn validate_policy(&self) -> Result<()> {
        fraction("policy.min_confidence", self.policy.min_confidence)?;
        fraction("policy.same_body_iou", self.policy.same_body_iou)?;
        in_section(
            "detect",
            self.senses
                .detect
                .validate_confidence(self.policy.min_confidence),
        )?;
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

    /// The map's own measures and shapes, the shadow and the room's geometry:
    /// one named piece each, in the order the first error is reported.
    fn validate_map(&self, needs: &Requirements<'_>) -> Result<()> {
        self.validate_map_measures()?;
        self.validate_shadow(needs)?;
        self.validate_headboard()?;
        self.validate_parts_section(needs)?;
        self.validate_occupancy(needs)?;
        self.validate_focus()
    }

    /// The bed inside the resolution and not degenerate; the floor it stands
    /// on, the same; its size in metres, positive.
    fn validate_map_measures(&self) -> Result<()> {
        let [w, h] = self.map.resolution;
        ensure!(w > 0 && h > 0, "map.resolution must be positive");
        let resolution = self.map.resolution;
        ensure!(
            within(&self.map.bed, resolution),
            "map.bed: points outside resolution"
        );
        ensure!(
            !degenerate(&self.map.bed),
            "map.bed: degenerate quadrilateral"
        );
        if let Some(points) = &self.map.bed_floor {
            ensure!(
                within(points, resolution),
                "map.bed_floor: points outside resolution"
            );
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
        Ok(())
    }

    /// The shadow's section: its numbers, and the bed's size it needs.
    fn validate_shadow(&self, needs: &Requirements<'_>) -> Result<()> {
        if let Some(shadow) = &self.shadow {
            in_section("shadow", shadow.validate(needs))?;
        }
        Ok(())
    }

    /// The headboard's own shape. Its stops may lie above the frame (galia: the
    /// headboard is cut by the top edge; white book, volume IX, ch. 02):
    /// finite and not degenerate.
    fn validate_headboard(&self) -> Result<()> {
        let [w, h] = self.map.resolution;
        if let Some(HeadboardConfig::Quad(points)) = &self.map.headboard {
            ensure!(
                points.iter().all(|[x, y]| x.is_finite() && y.is_finite()),
                "map.headboard: points must be finite"
            );
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
        Ok(())
    }

    /// The parts' order against the headboard and the bed. Checked after the
    /// headboard's own shape: a degenerate headboard is named as such.
    fn validate_parts_section(&self, needs: &Requirements<'_>) -> Result<()> {
        if let Some(parts) = self.occupancy.as_ref().and_then(|o| o.parts.as_ref()) {
            in_section("occupancy.parts", parts.validate(needs))?;
        }
        Ok(())
    }

    /// The room's geometry section and its pieces.
    fn validate_occupancy(&self, needs: &Requirements<'_>) -> Result<()> {
        if let Some(occupancy) = &self.occupancy {
            in_section("occupancy", occupancy.validate(needs))?;
        }
        Ok(())
    }

    /// Where the main detector looks while focusing: a rectangle over the bed,
    /// inside the resolution, and the detector must be able to focus.
    fn validate_focus(&self) -> Result<()> {
        let [w, h] = self.map.resolution;
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
}

/// The quadrilateral's points inside the frame's resolution, borders counted
/// in, all of them finite.
fn within(points: &[[f32; 2]; 4], [w, h]: [u32; 2]) -> bool {
    points.iter().all(|[x, y]| {
        x.is_finite() && y.is_finite() && *x >= 0.0 && *y >= 0.0 && *x <= w as f32 && *y <= h as f32
    })
}

/// A quadrilateral with no area — or a non-finite one: nothing to decide with.
fn degenerate(points: &[[f32; 2]; 4]) -> bool {
    let area: f32 = points
        .iter()
        .zip(points.iter().cycle().skip(1))
        .take(4)
        .map(|([x, y], [u, v])| x * v - y * u)
        .sum();
    !(area.is_finite() && area.abs() > 0.0)
}
