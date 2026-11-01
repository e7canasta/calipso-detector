//! What a profile must hold before anything starts: each section checks
//! itself, and the first error found is reported.

use super::*;
use anyhow::{Context, Result, ensure};
use std::path::Path;

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
            if let Some(knee) = &occupancy.knee {
                ensure!(
                    occupancy.geometry == RoomGeometry::TwoPlanes,
                    "occupancy.knee needs occupancy.geometry = \"two_planes\""
                );
                ensure!(
                    self.senses.pose.is_some(),
                    "occupancy.knee needs [senses.pose]: the knee is read on the pose"
                );
                ensure!(
                    knee.bent_deg.is_finite() && knee.bent_deg > 0.0 && knee.bent_deg < 180.0,
                    "occupancy.knee.bent_deg: an angle in degrees, in (0, 180)"
                );
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
}
