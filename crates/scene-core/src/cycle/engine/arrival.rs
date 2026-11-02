//! What reaches the engine: the shell's reception, the frame of a sample, the
//! verifier's answers between turns, and the settling at a turn's opening.

use crate::cycle::Engine;
use crate::look::Verified;
use crate::map::{FrameInfo, Geometry};
use crate::reception::{SensorChanged, SensorState};
use crate::refine::BodyPlaces;
use crate::time::SampleRef;
use crate::verification::Event;

impl Engine {
    /// The shell reports a reception change: the engine notes it and says
    /// whether it changed.
    pub(in crate::cycle) fn sensor_reported(
        &mut self,
        at: SampleRef,
        state: SensorState,
    ) -> Option<SensorChanged> {
        self.reception.report(at, state)
    }

    /// A sample opens with its frame: the engine keeps its size and says what
    /// geometry the site has for it.
    pub(in crate::cycle) fn open_frame(&mut self, frame: FrameInfo) -> Geometry {
        self.frame = Some(frame);
        self.map.gate(frame)
    }

    /// A verifier's answer, between turns: noted, settled at the next turn.
    pub(in crate::cycle) fn answered(&mut self, verified: Verified) {
        self.verification.step(Event::Answer(verified));
    }

    /// A sample's turn starts: the verifier's answers are settled as evidence and
    /// what it owes for too long expires, before anything is looked at. The
    /// digit they filled is returned: the turn carries it to `refine`.
    pub(in crate::cycle) fn settle(&mut self, at: SampleRef) -> Option<BodyPlaces> {
        let layer_1 = self.belief.bed_posture_state();
        self.verification
            .open_turn(at, layer_1, self.verify_rules.timeout_secs)
    }
}
