//! The loop: one turn per sample, as a protocol. `cycle` only sequences and
//! keeps the ledger; each decision belongs to `attention`, `evidence`,
//! `belief` or `map` (rule 11).
//!
//! - here: the protocol the shell drives (`Input`, `Turn`, `Output`) and the
//!   state a turn carries;
//! - `monitor`: what one camera is made of, how a turn opens and what comes
//!   out of it;
//! - `engine`: what the engine answers and changes when the turn asks it, with
//!   its own parts of the policy; nothing else reads the engine's fields;
//! - `sequence`: the order of a turn, in a single place (`Course::next`);
//! - `readings`: how each reading moves the turn on;
//! - `questions`: the turn's questions to the engine, each with its own
//!   attention look;
//! - `conclude`: what the turn leaves behind (the second level, the observation,
//!   the ledger);
//! - `pending`: each question the turn waits on, with what its answer is read into.

use std::sync::Arc;

use crate::attention::{self, Attention};
use crate::belief::{Belief, Believed, OccupancyRules};
use crate::chronicle::Chronicle;
use crate::conclusion::RepublishReason;
use crate::evidence::Perceived;
use crate::look::{Failure, Probe, Reading, Spent};
use crate::map::{BedCalibrated, CalibrationMemory, FrameInfo, Geometry, Map};
use crate::outcome::Outcome;
use crate::policy::{
    BedPosturePolicy, CalibrationPolicy, EnrichPolicy, FacePolicy, FocusPolicy, ReinforcePolicy,
    VerifyPolicy,
};
use crate::reception::{self, SensorChanged, SensorState};
use crate::refine::BodyPlaces;
use crate::room::Room;
use crate::room::SpaceRules;
use crate::time::SampleRef;
use crate::units::Extension;
use crate::verification::{Verification, VerifyRules};
use scene_contract::{Event, Scene};

mod conclude;
mod engine;
mod monitor;
mod pending;
mod questions;
mod readings;
mod sequence;
use pending::Pending;

/// Everything the world tells the core.
#[derive(Debug, Clone, PartialEq)]
pub enum Input {
    /// A decoded sample.
    Sample {
        /// Which sample, in source time.
        at: SampleRef,
        /// Its size.
        frame: FrameInfo,
    },
    /// A sample that produced no image: there is nothing to look at.
    NoFrame {
        /// Which sample, in source time.
        at: SampleRef,
        /// Why there is no image.
        why: NoFrame,
    },
    /// The shell reports a reception change.
    Sensor {
        /// The last sample's reference, for the record.
        at: SampleRef,
        /// The new reception state.
        state: SensorState,
    },
}

/// Why a sample produced no image.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoFrame {
    /// The decoder failed.
    DecodeFailed,
    /// The decoder returned nothing.
    Empty,
}

/// Everything the core knows about one camera. A value: N cameras, N monitors.
///
/// It composes three pieces that do not mix: the engine, which decides with
/// what it sees; the chronicle, which remembers the domain; and the last
/// scene, for republishing. The engine reads neither of the other two (P4).
#[derive(Debug)]
pub struct Monitor {
    engine: Engine,
    /// The chronicle of the domain: the projection reads it, already folded.
    chronicle: Chronicle,
    /// The last concluded scene and its sample, for republishing. If it was a
    /// failure, the scene keeps the observed origin separately.
    last: Option<(SampleRef, Arc<Scene>)>,
    /// The last concluded scene in v5, kept apart from v4's: each version
    /// republishes its own (C4, the versions do not depend on each other).
    last_v5: Option<(SampleRef, Arc<scene_contract::v5::Scene>)>,
}

/// The deterministic engine: the policy, the map, the belief and the reception.
/// It decides what to look at and what to believe with what it sees; it does
/// not remember what was published.
#[derive(Debug)]
struct Engine {
    map: Map,
    /// What was learned of the empty bed: the loop learns it, the map does not.
    calibration_memory: CalibrationMemory,
    belief: Belief,
    reception: reception::Memory,
    /// What attention remembers of what it looked at: the lens and the last
    /// calibration attempt.
    attention_memory: attention::Memory,
    /// What the slow verifier was asked and what it owes.
    verification: Verification,
    frame: Option<FrameInfo>,
    // The policy slices, each with its owner (ADR 2026-11-02,
    // politica-en-rebanadas). `Engine::new` takes the whole `Policy` apart, so
    // the engine never keeps it and a new field of `Policy` must find an owner.
    /// Which body counts and where, the room in two planes and the shadow
    /// (evidence, room, map): one slice, `SpaceRules`.
    space: SpaceRules,
    /// The reinforcement detector (attention).
    reinforce: Option<ReinforcePolicy>,
    /// The face detector (attention).
    face_policy: Option<FacePolicy>,
    /// The pose model (attention, refine).
    enrich: Option<EnrichPolicy>,
    /// The empty-bed calibration (attention).
    calibration_policy: Option<CalibrationPolicy>,
    /// The depth pad of the bed and the subject (attention).
    depth_pad: Option<Extension>,
    /// The focus of the main detector (attention, map).
    focus: Option<FocusPolicy>,
    /// The slow verifier's questions (attention, refine).
    verify_policy: Option<VerifyPolicy>,
    occupancy_rules: OccupancyRules,
    verify_rules: VerifyRules,
    /// `None`: no `[bed_posture]`, so layer 1 does not decide.
    bed_posture: Option<BedPosturePolicy>,
}

/// What the turn is waiting for: one question, which carries what the turn has
/// gathered so far, so it cannot move on with missing data.
#[derive(Debug)]
enum Phase {
    /// The question and its continuation are one value: there is no wait
    /// without the probe that asks for it.
    Waiting(Pending),
    /// The turn concluded with all its evidence; projecting remains.
    Concluded { refined: Refined },
    /// No new observation: the last one is republished, declared as such.
    Republish {
        reason: RepublishReason,
        changed: Option<SensorChanged>,
    },
}

/// After believing, each continuation carries the evidence it needs.
#[derive(Debug)]
struct Observed {
    believed: Believed,
    evidence: Perceived,
    calibrated: Option<BedCalibrated>,
}

/// After the second level and layer 1: what the turn observed, refined. The
/// verifier is asked with it, and the turn concludes with it.
#[derive(Debug)]
struct Refined {
    observed: Observed,
    refinement: crate::refine::Refinement,
    bed_posture: Option<crate::belief::bed_posture::Concluded>,
    parts: Option<crate::belief::parts::Concluded>,
}

/// What comes out of a turn: the scene of the sample and the events it
/// explains, in the published contract.
#[derive(Debug, Clone, PartialEq)]
pub struct Output {
    /// The sample the turn was opened for.
    pub at: SampleRef,
    /// The scene of the sample.
    pub scene: Arc<Scene>,
    /// The events it explains, in order.
    pub events: Vec<Event>,
    /// The same turn in the v5 wire (C4): the scene and the events, from the
    /// same conclusion and chronicle as `scene` and `events`.
    pub v5: crate::project::v5::Projection,
}

/// A turn of the loop. It holds the belief exclusively: the compiler prevents
/// opening another while it lives. It is driven to the end with [`Turn::drive`]
/// or [`Turn::cancel`]; there is no other way through, and none is abandoned
/// (rule 10).
///
/// The turn borrows the chronicle and the last scene apart from the course, and
/// only the close reads them: the course, which is all the decisions see, has
/// the engine and nothing else (P4, by the borrow checker).
#[derive(Debug)]
#[must_use = "a turn is closed with drive or cancel (rule 10)"]
pub struct Turn<'m> {
    course: Course<'m>,
    chronicle: &'m mut Chronicle,
    last: &'m mut Option<(SampleRef, Arc<Scene>)>,
    last_v5: &'m mut Option<(SampleRef, Arc<scene_contract::v5::Scene>)>,
    phase: Phase,
    open: Open,
}

/// What a turn works on while it runs: the engine, the sample and the ledger.
#[derive(Debug)]
struct Course<'m> {
    engine: &'m mut Engine,
    at: SampleRef,
    geometry: Geometry,
    /// The room of this turn, built once when it opens: every question of
    /// space in the turn sees the same one. `None` without a sample, or if
    /// the bed's geometry does not apply to its frame.
    room: Option<Room>,
    /// The digit the verifier filled at the turn's opening: `refine` reads it.
    filled: Option<BodyPlaces>,
    attention: Attention,
}

/// The obligation to close the turn: only `drive` discharges it.
#[derive(Debug)]
struct Open;

impl Drop for Open {
    fn drop(&mut self) {
        debug_assert!(
            std::thread::panicking(),
            "rule 10: a turn is not abandoned; close it with drive or cancel"
        );
    }
}

impl Turn<'_> {
    /// Drives the turn to the end: for each probe, `look` looks and returns what
    /// it saw and how long it took. The core still does no IO because the shell
    /// provides the sense. A reading of another kind than the probe counts as a
    /// failing sense.
    pub fn drive(
        self,
        mut look: impl FnMut(&Probe) -> (Result<Reading, Failure>, Spent),
    ) -> Output {
        let Turn {
            mut course,
            chronicle,
            last,
            last_v5,
            mut phase,
            open,
        } = self;
        let conclusion = loop {
            phase = match phase {
                Phase::Waiting(pending) => {
                    let probe = pending.probe();
                    let (result, spent) = look(&probe);
                    course.advance(pending, &probe, Outcome { result, spent })
                }
                Phase::Concluded { refined } => break course.observed(refined),
                Phase::Republish { reason, changed } => break course.republished(reason, changed),
            };
        };
        std::mem::forget(open);
        monitor::project(chronicle, last, last_v5, course.at, conclusion)
    }

    /// The shell cannot complete (shutdown): `Cancelled` for what is pending,
    /// then close. It is a failure of the senses, which the domain already
    /// knows how to interpret.
    pub fn cancel(self) -> Output {
        self.drive(|_| (Err(Failure::Cancelled), Spent(0.0)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geom::{Quad, Rect};
    use crate::look::{LookKind, Probe, Reading, Spent};
    use crate::map::SiteMap;
    use crate::policy::Policy;
    use crate::time::SessionId;
    use crate::units::Confidence;
    use crate::units::{DiagonalFraction, Extension, RelativeDepth};
    use scene_contract::SourceSecs;

    fn sample(secs: f64) -> Input {
        Input::Sample {
            at: SampleRef {
                session: SessionId(1),
                sequence: 0,
                source_secs: SourceSecs(secs),
            },
            frame: FrameInfo {
                width: 640,
                height: 480,
            },
        }
    }

    /// Reads the belief: the pose reference is not on the published scene.
    #[test]
    fn a_subject_seen_only_by_reinforcement_does_not_move_the_pose_reference() {
        use crate::policy::{EnrichPolicy, ReinforcePolicy};
        let mut m = Monitor::new(
            Policy {
                reinforce: Some(ReinforcePolicy {
                    bed_padding: Extension::new(0.1).expect("a constant extension"),
                }),
                enrich: Some(EnrichPolicy {
                    move_threshold: DiagonalFraction::new(0.1).expect("a constant fraction"),
                    depth: false,
                    min_keypoint_score: Confidence::new(0.3).expect("a score"),
                    posture_tolerance: RelativeDepth::new(0.05).expect("a constant depth"),
                    max_pose_age_secs: None,
                    crop_pad: Extension::new(0.0).expect("a constant extension"),
                }),
                ..Policy::default()
            },
            SiteMap::new(Quad::from_rect(Rect::new(100.0, 100.0, 500.0, 400.0))),
        );
        let person = |bbox| crate::look::Detection {
            bbox,
            confidence: 0.9,
            label: crate::look::Label::Person,
        };
        let seen = Rect::new(200.0, 150.0, 300.0, 350.0);
        let moved = Rect::new(300.0, 150.0, 400.0, 350.0);
        let pose = Reading::Pose(Some(vec![crate::body::Keypoint {
            x: 250.0,
            y: 200.0,
            score: 0.9,
        }]));
        for s in 1..=3 {
            m.handle(sample(f64::from(s))).drive(|r| match r {
                Probe::Detect { .. } => (Ok(Reading::Bodies(vec![person(seen)])), Spent(1.0)),
                Probe::Faces { .. }
                | Probe::Reinforce { .. }
                | Probe::Pose { .. }
                | Probe::Depth { .. }
                | Probe::Keep { .. }
                | Probe::Verify { .. } => (Ok(pose.clone()), Spent(1.0)),
            });
        }
        assert_eq!(m.engine.pose_reference(), Some(seen));
        // The main detector loses it; reinforcement sees it moved and a new pose is taken.
        let out = m.handle(sample(4.0)).drive(|r| match r {
            Probe::Detect { .. } => (Ok(Reading::Bodies(vec![])), Spent(1.0)),
            Probe::Reinforce { .. } => (Ok(Reading::Bodies(vec![person(moved)])), Spent(1.0)),
            Probe::Faces { .. }
            | Probe::Pose { .. }
            | Probe::Depth { .. }
            | Probe::Keep { .. }
            | Probe::Verify { .. } => (Ok(pose.clone()), Spent(1.0)),
        });
        let kinds: Vec<_> = out.scene.attention.looks.iter().map(|l| l.kind).collect();
        assert_eq!(
            kinds,
            [LookKind::Detect, LookKind::Reinforce, LookKind::Pose]
        );
        assert_eq!(m.engine.pose_reference(), Some(seen));
    }
}
