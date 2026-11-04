//! The profile translated into what the core receives by value: the policy
//! and the site map. Each capability translates its own section; this file
//! only composes them.

use super::validate::measured;
use super::validate::{confidence, seconds};
use super::*;
use scene_core::units::{Extension, Proportion, Ratio};
use scene_core::{
    map::{FrameInfo, SiteMap},
    policy::{BedStep, FarArrival, Policy, SideExit},
};
use scene_geom::{Point, Quad, Rect};

impl Config {
    pub fn policy(&self) -> Result<Policy> {
        let mut p = Policy::default();
        p.admission.min_confidence = confidence(
            "policy",
            "policy.min_confidence",
            self.policy.min_confidence,
        )?;
        p.admission.same_body_iou = measured(
            "policy.same_body_iou",
            Proportion::new(self.policy.same_body_iou),
        )?;
        p.admission.near_bed_margin = measured(
            "policy.near_bed_margin",
            Extension::new(self.policy.near_bed_margin),
        )?;
        p.admission.bed_frame = self
            .policy
            .bed_frame
            .map(|share| measured("policy.bed_frame", Ratio::new(share)))
            .transpose()?;
        p.shadow = self.shadow.as_ref().map(ShadowConfig::policy).transpose()?;
        p.two_planes = self
            .occupancy
            .as_ref()
            .map(OccupancyConfig::two_planes)
            .transpose()?
            .flatten();
        p.parts = self
            .occupancy
            .as_ref()
            .and_then(|o| o.parts.as_ref())
            .map(PartsConfig::policy)
            .transpose()?;
        p.held_exit = if self.occupancy.as_ref().is_some_and(|o| o.held_left) {
            SideExit::ByTheSide
        } else {
            SideExit::OnlyReappearing
        };
        p.far_arrival = if self.occupancy.as_ref().is_some_and(|o| o.near_first) {
            FarArrival::ThoseNeverNear
        } else {
            FarArrival::FromTheDoor
        };
        p.bed_step = if self.occupancy.as_ref().is_some_and(|o| o.step_near) {
            BedStep::SeenNear
        } else {
            BedStep::FloorRises
        };
        p.persistence.dwell_secs = seconds("policy", "policy.dwell_secs", self.policy.dwell_secs)?;
        p.reinforce = self
            .senses
            .reinforce
            .as_ref()
            .map(Reinforce::policy)
            .transpose()?;
        p.focus = self
            .senses
            .detect
            .focus
            .as_ref()
            .map(Focus::policy)
            .transpose()?;
        p.faces = self.senses.faces.as_ref().map(Faces::policy).transpose()?;
        p.enrich = self
            .senses
            .pose
            .as_ref()
            .map(|pose| pose.policy(self.senses.depth.as_ref()))
            .transpose()?;
        p.calibration = self
            .calibration
            .as_ref()
            .map(CalibrationConfig::policy)
            .transpose()?;
        p.depth_pad = self
            .senses
            .depth
            .as_ref()
            .and_then(|d| d.crop_pad)
            .map(|pad| measured("senses.depth.crop_pad", Extension::new(pad)))
            .transpose()?;
        p.bed_posture = self
            .bed_posture
            .as_ref()
            .map(BedPostureConfig::policy)
            .transpose()?;
        p.verify = self.verify.as_ref().map(VerifyConfig::policy).transpose()?;
        Ok(p)
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
