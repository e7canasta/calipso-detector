//! The composition of one camera: the engine that decides, the chronicle that
//! remembers and the last scene for republishing. How a turn opens and what
//! comes out of it once concluded; the order in between is `sequence`.

use std::sync::Arc;

use super::pending::Pending;
use super::{
    Course, Engine, Input, Monitor, NoFrame, Open, Output, Phase, SampleTurn, Scope, Turn,
};
use crate::attention::Attention;
use crate::chronicle::{Chronicle, Entry};
use crate::conclusion::{Conclusion, RepublishReason};
use crate::look::Verified;
use crate::map::{FrameInfo, Geometry, SiteMap};
use crate::policy::Policy;
use crate::time::SampleRef;

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
        self.engine.answered(verified);
    }

    /// Opens a turn. It is the only way in. The monitor is taken apart here so
    /// that the course gets the engine alone, and the turn keeps the chronicle
    /// and the last scene for its close.
    pub fn handle(&mut self, input: Input) -> Turn<'_> {
        let Monitor {
            engine,
            chronicle,
            last,
        } = self;
        let scope = match input {
            Input::Sample { at, frame } => {
                let mut course = course(engine, at);
                let phase = course.open(frame);
                Scope::Sample(Box::new(SampleTurn { course, phase }))
            }
            Input::NoFrame { at, why } => Scope::Sample(Box::new(SampleTurn {
                course: course(engine, at),
                phase: Phase::Republish {
                    reason: match why {
                        NoFrame::DecodeFailed => RepublishReason::DecodeFailed,
                        NoFrame::Empty => RepublishReason::DecodeEmpty,
                    },
                    changed: None,
                },
            })),
            Input::Sensor { state, last, .. } => Scope::Reception {
                changed: engine.sensor_reported(last, state),
                engine,
            },
        };
        Turn {
            scope,
            chronicle,
            last,
            open: Open,
        }
    }
}

/// A turn over a sample: the course with the engine alone, at its sample.
fn course(engine: &mut Engine, at: SampleRef) -> Course<'_> {
    Course {
        engine,
        at,
        geometry: Geometry::Valid,
        room: None,
        filled: None,
        attention: Attention::default(),
    }
}

/// What was concluded, outwards: the chronicle notes the turn and the
/// projection builds the scene and events with what the chronicle remembers.
/// Only the close of a turn calls it, with the chronicle and the last scene it
/// borrowed apart from the course.
pub(super) fn project(
    chronicle: &mut Chronicle,
    last: &mut Option<(SampleRef, Arc<scene_contract::v5::Scene>)>,
    at: Option<SampleRef>,
    conclusion: Conclusion,
) -> Output {
    match conclusion {
        Conclusion::Observed(observation) => {
            let chronicled = chronicle.record(Entry::of(&observation));
            let v5 = crate::project::v5::observed(&observation, &chronicled);
            *last = Some((observation.at, Arc::clone(&v5.scene)));
            Output {
                at: Some(observation.at),
                v5,
            }
        }
        Conclusion::Republish {
            reason,
            changed,
            sensor,
            map,
        } => {
            // A republished scene is the last one of the contract, never a
            // turn's own observation.
            let v5 = crate::project::v5::republished(
                reason,
                at,
                changed,
                sensor,
                *map.clone(),
                last.as_ref().map(|(at, scene)| (scene.as_ref(), *at)),
            );
            Output { at, v5 }
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
