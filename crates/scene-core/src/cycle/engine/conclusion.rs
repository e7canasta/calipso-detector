//! What the turn leaves behind: the observation the engine made, and the reception
//! and the site as they stand for a scene.

use crate::attention::Attention;
use crate::conclusion::Observation;
use crate::cycle::{Engine, Refined};
use crate::map::{Geometry, MapState};
use crate::reception::SensorState;
use crate::time::SampleRef;

impl Engine {
    /// What the engine observed in the turn. It reads neither the chronicle
    /// nor the last scene: the engine has no way to.
    pub(in crate::cycle) fn observation(
        &self,
        at: SampleRef,
        geometry: Geometry,
        refined: Refined,
        attention: Attention,
    ) -> Observation {
        let Refined {
            observed,
            refinement,
            bed_posture,
            parts,
        } = refined;
        Observation {
            at,
            believed: observed.believed,
            refined: refinement,
            bed_posture,
            parts,
            held_check: self.verification.held(),
            evidence: observed.evidence,
            frame: self.frame,
            calibrated: observed.calibrated,
            attention,
            map: self.map_state(geometry),
            sensor: self.sensor_state(),
            dwell_secs: self.occupancy_rules.persistence.dwell_secs,
        }
    }

    /// The reception as it stands, for a scene.
    pub(in crate::cycle) fn sensor_state(&self) -> SensorState {
        self.reception.state()
    }

    /// The site as it stands for a scene: the geometry of this turn, with the
    /// focus, the shadow and the calibration the policy asks for.
    pub(in crate::cycle) fn map_state(&self, geometry: Geometry) -> MapState {
        self.map.state(
            geometry,
            self.focus.as_ref(),
            self.shadow.as_ref(),
            &self.admission,
            &self.calibration,
        )
    }
}
