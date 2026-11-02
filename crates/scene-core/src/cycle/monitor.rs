//! The composition of one camera: the engine that decides, the chronicle that
//! remembers and the last scene for republishing. How a turn opens and what
//! comes out of it once concluded; the order in between is `sequence`.

use std::sync::Arc;

use super::pending::Pending;
use super::{Course, Engine, Input, Monitor, NoFrame, Open, Output, Phase, Turn};
use crate::attention::Attention;
use crate::chronicle::{Chronicle, Entry};
use crate::conclusion::{Conclusion, RepublishReason};
use crate::look::Verified;
use crate::map::{FrameInfo, Geometry, SiteMap};
use crate::policy::Policy;
use crate::project::v4;
use crate::time::SampleRef;
use scene_contract::Scene;

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
            last_v5: None,
        }
    }

    /// The verifier answered a ticket. An answer is not a sample and opens no
    /// turn (rule 3): it is noted, and settled at the start of the next
    /// sample's turn as evidence of the sample it looked at.
    pub fn verified(&mut self, verified: Verified) {
        self.engine.answered(verified);
    }

    /// Opens a turn. It is the only way in. The monitor is taken apart here so
    /// that the course gets the engine alone, and the turn keeps the chronicle
    /// and the last scene for its close.
    pub fn handle(&mut self, input: Input) -> Turn<'_> {
        let at = input.at();
        let Monitor {
            engine,
            chronicle,
            last,
            last_v5,
        } = self;
        let mut course = Course {
            engine,
            at,
            geometry: Geometry::Valid,
            room: None,
            filled: None,
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
                changed: course.engine.sensor_reported(at, state),
            },
        };
        Turn {
            course,
            chronicle,
            last,
            last_v5,
            phase,
            open: Open,
        }
    }
}

/// What was concluded, outwards: the chronicle notes the turn and the
/// projection builds the scene and events with what the chronicle remembers.
/// Only the close of a turn calls it, with the chronicle and the last scene it
/// borrowed apart from the course.
pub(super) fn project(
    chronicle: &mut Chronicle,
    last: &mut Option<(SampleRef, Arc<Scene>)>,
    last_v5: &mut Option<(SampleRef, Arc<scene_contract::v5::Scene>)>,
    at: SampleRef,
    conclusion: Conclusion,
) -> Output {
    match conclusion {
        Conclusion::Observed(observation) => {
            let chronicled = chronicle.record(Entry::of(&observation));
            // Both versions read the same observation and chronicle (C4): v5
            // first, by reference, then v4 consumes the observation. v5 never
            // reads v4's output.
            let v5 = crate::project::v5::observed(&observation, &chronicled);
            let (scene, events) = v4::observed(*observation, &chronicled);
            let scene = Arc::new(scene);
            *last = Some((at, Arc::clone(&scene)));
            *last_v5 = Some((at, Arc::clone(&v5.scene)));
            Output {
                at,
                scene,
                events,
                v5,
            }
        }
        Conclusion::Republish {
            reason,
            changed,
            sensor,
            map,
        } => {
            let previous = last.as_ref().map(|(at, scene)| (scene.as_ref(), *at));
            // v5 republishes its own last scene, never v4's (the versions are
            // independent): `last` holds the v4 scene, so v5 keeps its own copy.
            let v5 = crate::project::v5::republished(
                reason,
                changed,
                sensor,
                *map.clone(),
                last_v5.as_ref().map(|(at, scene)| (scene.as_ref(), *at)),
            );
            let scene = v4::republish(previous, reason, sensor, *map);
            let events = v4::sensor_events(changed, previous.map(|(scene, _)| scene));
            Output {
                at,
                scene: Arc::new(scene),
                events,
                v5,
            }
        }
    }
}

impl Course<'_> {
    /// Opens a turn over a sample: the geometry and the main detector, which
    /// always runs. Neither a reconnection nor another image size releases
    /// anyone: the belief starts from scratch only with the process.
    pub(super) fn open(&mut self, frame: FrameInfo) -> Phase {
        self.filled = self.engine.settle(self.at);
        self.geometry = self.engine.open_frame(frame);
        self.room = self.engine.room_of(self.geometry);
        let region = self.engine.primary_region(self.at, self.geometry);
        Phase::Waiting(Pending::Detect { region })
    }
}
