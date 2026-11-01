//! The profile translated into what the core receives by value: the policy
//! and the site map.

use super::schema::defaults;
use super::*;
use scene_core::{
    geom::{Point, Quad, Rect},
    map::{FrameInfo, SiteMap},
    policy::{
        Band, BedPosturePolicy, CalibrationPolicy, EnrichPolicy, FacePolicy, FocusPolicy, Frames,
        HeldVerifyPolicy, Policy, ReinforcePolicy, ShadowPolicy, SharpnessPolicy, TwoPlanesPolicy,
        VerifyPolicy,
    },
};

impl Config {
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
                knee_bent_deg: self
                    .occupancy
                    .as_ref()
                    .and_then(|o| o.knee.as_ref())
                    .map(|knee| knee.bent_deg),
            });
        p.held_left = self.occupancy.as_ref().is_some_and(|o| o.held_left);
        p.near_first = self.occupancy.as_ref().is_some_and(|o| o.near_first);
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
}
