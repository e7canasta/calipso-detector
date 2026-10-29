//! The composition of one camera: the engine that decides, the chronicle that
//! remembers and the last scene for republishing. How a turn opens and what
//! comes out of it once concluded; the order in between is `sequence`.

use std::sync::Arc;

use super::{Course, Engine, Input, Monitor, NoFrame, Open, Output, Phase, Turn};
use crate::attention::{Attention, Lens};
use crate::belief::Belief;
use crate::chronicle::{Chronicle, Entry};
use crate::conclusion::{Conclusion, RepublishReason};
use crate::look::Verified;
use crate::map::{Geometry, SiteMap};
use crate::policy::Policy;
use crate::project::v4;
use crate::sensor::SensorMemory;
use crate::time::SampleRef;
use crate::verification::Verification;

impl Engine {
    fn new(policy: Policy, map: SiteMap) -> Self {
        Self {
            policy,
            map: map.into(),
            belief: Belief::default(),
            sensor: SensorMemory::default(),
            lens: Lens::default(),
            verification: Verification::default(),
            frame: None,
        }
    }
}

impl Input {
    fn at(&self) -> SampleRef {
        match self {
            Input::Sample { at, .. } | Input::NoFrame { at, .. } | Input::Sensor { at, .. } => *at,
        }
    }
}

impl Monitor {
    /// A camera's monitor: its policies and its site. The belief starts empty.
    pub fn new(policy: Policy, map: SiteMap) -> Self {
        Self {
            engine: Engine::new(policy, map),
            chronicle: Chronicle::default(),
            last: None,
        }
    }

    /// The verifier answered a ticket. An answer is not a sample and opens no
    /// turn (rule 3): it is noted, and settled at the start of the next
    /// sample's turn as evidence of the sample it looked at.
    pub fn verified(&mut self, verified: Verified) {
        self.engine.verification.answered(verified);
    }

    /// Opens a turn. It is the only way in.
    pub fn handle(&mut self, input: Input) -> Turn<'_> {
        let at = input.at();
        let mut course = Course {
            monitor: self,
            at,
            geometry: Geometry::Valid,
            attention: Attention::default(),
        };
        let phase = match input {
            Input::Sample { frame, .. } => course.open(frame),
            Input::NoFrame { why, .. } => Phase::Republish {
                reason: match why {
                    NoFrame::DecodeFailed => RepublishReason::DecodeFailed,
                    NoFrame::Empty => RepublishReason::DecodeEmpty,
                },
                changed: None,
            },
            Input::Sensor { state, .. } => Phase::Republish {
                reason: RepublishReason::Sensor,
                changed: course.monitor.engine.sensor.report(at, state),
            },
        };
        Turn {
            course,
            phase,
            open: Open,
        }
    }

    /// What was concluded, outwards: the chronicle notes the turn and the
    /// projection builds the scene and events with what the chronicle remembers.
    pub(super) fn project(&mut self, at: SampleRef, conclusion: Conclusion) -> Output {
        match conclusion {
            Conclusion::Observed(observation) => {
                let chronicled = self.chronicle.record(Entry::of(&observation));
                let (scene, events) = v4::observed(*observation, &chronicled);
                let scene = Arc::new(scene);
                self.last = Some((at, Arc::clone(&scene)));
                Output { at, scene, events }
            }
            Conclusion::Republish {
                reason,
                changed,
                sensor,
                map,
            } => {
                let last = self.last.as_ref().map(|(at, scene)| (scene.as_ref(), *at));
                Output {
                    at,
                    scene: Arc::new(v4::republish(last, reason, sensor, *map)),
                    events: v4::sensor_events(changed, last.map(|(scene, _)| scene)),
                }
            }
        }
    }
}
