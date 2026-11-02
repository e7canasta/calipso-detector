//! What the readings fill: each reading is admitted, joined or read with the
//! slice of the policy it belongs to, and the empty bed is learned from it.

use crate::attention::LookResult;
use crate::body::Keypoint;
use crate::cycle::Engine;
use crate::evidence::{self, Admission, Perceived, Sensed};
use crate::look::Detection;
use crate::map::BedCalibrated;
use crate::room::Room;
use crate::time::SampleRef;

impl Engine {
    /// The main detector's reading, admitted by the zones of the room.
    pub(in crate::cycle) fn admit(
        &self,
        reading: Sensed<&[Detection]>,
        room: Option<&Room>,
    ) -> Admission {
        evidence::admit(reading, room, &self.admission)
    }

    /// Reinforcement's reading joined with the main detector's.
    pub(in crate::cycle) fn consolidate(
        &self,
        primary: Admission,
        reinforcement: Sensed<&[Detection]>,
        room: Option<&Room>,
    ) -> (Admission, LookResult) {
        evidence::consolidate(primary, reinforcement, room, &self.admission)
    }

    /// A doubtful body's pose, placed by the room: `perceived` changes if the
    /// pose says where the body is.
    pub(in crate::cycle) fn read_pose_body(
        &self,
        perceived: &mut Perceived,
        floor: usize,
        reading: Sensed<Option<Vec<Keypoint>>>,
        room: Option<&Room>,
    ) -> LookResult {
        evidence::pose_body(perceived, floor, reading, room, self.enrich.as_ref())
    }

    /// The empty bed's depth, learned if the turn measured it.
    pub(in crate::cycle) fn learn_bed(
        &mut self,
        at: SampleRef,
        depth: Option<f32>,
    ) -> Option<BedCalibrated> {
        self.calibration.learn(at, depth)
    }
}
