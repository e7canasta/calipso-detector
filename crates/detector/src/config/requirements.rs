//! What each section may require of the rest of the profile (rule 12: what is
//! not looked at has a reason). A section's checks ask it by name — `needs.pose()`,
//! `needs.two_planes()` — never by positional flags, so a call site reads out
//! loud and a new requirement is a name, not another parameter.

use super::Config;
use super::schema::HeadboardConfig;

/// The facts a section's rules need from the rest of the profile: which senses
/// and sections are present, and the map's measures they are read against.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Requirements<'a> {
    config: &'a Config,
}

impl<'a> Requirements<'a> {
    /// Everything the profile says, as questions.
    pub(super) fn of(config: &'a Config) -> Self {
        Self { config }
    }

    /// `[senses.pose]` is present: the body can be read.
    pub(super) fn pose(&self) -> bool {
        self.config.senses.pose.is_some()
    }

    /// `[senses.faces]` is present: the head can be read.
    pub(super) fn faces(&self) -> bool {
        self.config.senses.faces.is_some()
    }

    /// `[senses.depth]` is present.
    pub(super) fn depth(&self) -> bool {
        self.config.senses.depth.is_some()
    }

    /// `[calibration]` is present: the empty bed is learned.
    pub(super) fn calibration(&self) -> bool {
        self.config.calibration.is_some()
    }

    /// `[bed_posture]` is present: layer 1 exists.
    pub(super) fn bed_posture(&self) -> bool {
        self.config.bed_posture.is_some()
    }

    /// `occupancy.geometry = "two_planes"`: the room is measured in two planes.
    pub(super) fn two_planes(&self) -> bool {
        self.config
            .occupancy
            .as_ref()
            .is_some_and(|o| o.geometry == super::occupancy::RoomGeometry::TwoPlanes)
    }

    /// `map.headboard` is present, in any shape: where the head is.
    pub(super) fn has_headboard(&self) -> bool {
        self.config.map.headboard.is_some()
    }

    /// The headboard as four points; `None`, a box or none at all.
    pub(super) fn headboard_quad(&self) -> Option<[[f32; 2]; 4]> {
        match self.config.map.headboard {
            Some(HeadboardConfig::Quad(points)) => Some(points),
            _ => None,
        }
    }

    /// The bed's corners.
    pub(super) fn bed(&self) -> [[f32; 2]; 4] {
        self.config.map.bed
    }

    /// `map.bed_floor` is present: where the support rests.
    pub(super) fn has_bed_floor(&self) -> bool {
        self.config.map.bed_floor.is_some()
    }

    /// `map.bed_size_m` is present: distances are in metres.
    pub(super) fn has_bed_size_m(&self) -> bool {
        self.config.map.bed_size_m.is_some()
    }
}
