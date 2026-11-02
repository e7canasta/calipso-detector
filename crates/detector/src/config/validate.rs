//! What a profile must hold before anything starts. The common sections
//! (identifiers, transport, policy, the site map) are checked here; each
//! capability checks its own section in its module, and its error names the
//! section first: `[faces]: senses.faces.min_confidence must be…`. The first
//! error found is reported.

use super::*;
use anyhow::{Context, Result, ensure};
use std::path::Path;

/// A score or a ratio: finite and between 0 and 1.
pub(super) fn fraction(name: &str, value: f32) -> Result<()> {
    ensure!(
        value.is_finite() && (0.0..=1.0).contains(&value),
        "{name} must be between 0 and 1"
    );
    Ok(())
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
        self.validate_identifiers()?;
        self.validate_transport()?;
        self.validate_senses()?;
        self.validate_policy()?;
        self.validate_map()?;
        self.validate_bed_posture()?;
        self.validate_verify()
    }

    /// The verifier's two sections together: the sense and the policy come
    /// together, and each rule names its own section.
    fn validate_verify(&self) -> Result<()> {
        in_section(
            "verify",
            verify::validate(
                self.verify.as_ref(),
                self.senses.verify.as_ref(),
                self.senses.depth.is_some(),
                self.bed_posture.is_some(),
            ),
        )
    }

    /// Times finite and not negative; it needs where the head is (the
    /// headboard) and the body (a pose model).
    fn validate_bed_posture(&self) -> Result<()> {
        let Some(b) = &self.bed_posture else {
            return Ok(());
        };
        in_section(
            "bed_posture",
            b.validate(self.map.headboard.is_some(), self.senses.pose.is_some()),
        )
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
            let asked = self.senses.pose.is_some() || self.calibration.is_some();
            in_section("depth", depth.validate(asked))?;
        }
        if let Some(calibration) = &self.calibration {
            in_section(
                "calibration",
                calibration.validate(self.senses.depth.is_some()),
            )?;
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

    /// The bed inside the resolution and not degenerate; a valid headboard;
    /// the shadow and the room's geometry, which need the map's measures.
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
            in_section("shadow", shadow.validate(self.map.bed_size_m.is_some()))?;
        }
        if let Some(occupancy) = &self.occupancy {
            in_section(
                "occupancy",
                occupancy.validate(
                    self.map.bed_floor.is_some(),
                    self.map.bed_size_m.is_some(),
                    self.senses.pose.is_some(),
                ),
            )?;
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
}
