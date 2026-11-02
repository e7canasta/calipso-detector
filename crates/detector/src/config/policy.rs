//! The profile translated into what the core receives by value: the policy
//! and the site map. Each capability translates its own section; this file
//! only composes them.

use super::*;
use scene_core::{
    geom::{Point, Quad, Rect},
    map::{FrameInfo, SiteMap},
    policy::Policy,
};

impl Config {
    pub fn policy(&self) -> Policy {
        let mut p = Policy::default();
        p.admission.min_confidence = self.policy.min_confidence;
        p.admission.same_body_iou = self.policy.same_body_iou;
        p.admission.near_bed_margin = self.policy.near_bed_margin;
        p.admission.bed_frame = self.policy.bed_frame;
        p.shadow = self.shadow.as_ref().map(ShadowConfig::policy);
        p.two_planes = self
            .occupancy
            .as_ref()
            .and_then(OccupancyConfig::two_planes);
        p.held_left = self.occupancy.as_ref().is_some_and(|o| o.held_left);
        p.near_first = self.occupancy.as_ref().is_some_and(|o| o.near_first);
        p.persistence.dwell_secs = self.policy.dwell_secs;
        p.reinforce = self.senses.reinforce.as_ref().map(Reinforce::policy);
        p.focus = self.senses.detect.focus.as_ref().map(Focus::policy);
        p.faces = self.senses.faces.as_ref().map(Faces::policy);
        p.enrich = self
            .senses
            .pose
            .as_ref()
            .map(|pose| pose.policy(self.senses.depth.as_ref()));
        p.calibration = self.calibration.as_ref().map(CalibrationConfig::policy);
        p.depth_pad = self.senses.depth.as_ref().and_then(|d| d.crop_pad);
        p.bed_posture = self.bed_posture.as_ref().map(BedPostureConfig::policy);
        p.verify = self.verify.as_ref().map(VerifyConfig::policy);
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
