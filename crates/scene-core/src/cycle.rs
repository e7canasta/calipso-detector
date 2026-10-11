//! The loop: one turn per sample. `cycle` only sequences and keeps the ledger;
//! each decision belongs to `attention`, `evidence`, `belief` or `map` (rule
//! 11). The order of a turn lives in a single place: [`Turn::next`].

use std::sync::Arc;

use crate::attention::Consideration::{Decided, Disabled, Inapplicable};
use crate::attention::{
    self, Attention, Consideration, Decision, EnrichPlan, Glimpse, Look, LookOutcome, LookResult,
    SkipReason,
};
use crate::belief::Belief;
use crate::belief::{Believed, Enrichment};
use crate::body::Keypoint;
use crate::conclusion::RepublishReason;
use crate::conclusion::{Conclusion, Observation};
use crate::contract::v4::{Event, Scene};
use crate::evidence::{self, Admission, FaceEvidence, Perceived};
use crate::geom::Rect;
use crate::look::{
    FaceRegion, Failure, LookId, LookKind, Outcome, ProbeKindMismatch, Reading, Request, Spent,
};
use crate::map::{FrameInfo, Geometry, Map, SiteMap};
use crate::policy::{FacePolicy, Policy};
use crate::sensor::{SensorChanged, SensorMemory, SensorState};
use crate::time::SampleRef;

mod waiting;
use waiting::{Awaiting, Resolution, Waiting};

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
    chronicle: crate::chronicle::Chronicle,
    /// The last concluded scene and its sample, for republishing. If it was a
    /// failure, the scene keeps the observed origin separately.
    last: Option<(SampleRef, Arc<Scene>)>,
}

/// The deterministic engine: the policy, the map, the belief and the sensor.
/// It decides what to look at and what to believe with what it sees; it does
/// not remember what was published.
#[derive(Debug)]
struct Engine {
    policy: Policy,
    map: Map,
    belief: Belief,
    sensor: SensorMemory,
    next_id: u32,
    frame: Option<FrameInfo>,
}

impl Engine {
    fn new(policy: Policy, map: SiteMap) -> Self {
        Self {
            policy,
            map: map.into(),
            belief: Belief::default(),
            sensor: SensorMemory::default(),
            next_id: 0,
            frame: None,
        }
    }

    fn next_id(&mut self) -> LookId {
        self.next_id = self.next_id.wrapping_add(1);
        LookId(self.next_id)
    }
}

impl Monitor {
    /// A camera's monitor: its policies and its site. The belief starts empty.
    pub fn new(policy: Policy, map: SiteMap) -> Self {
        Self {
            engine: Engine::new(policy, map),
            chronicle: crate::chronicle::Chronicle::default(),
            last: None,
        }
    }

    /// What was concluded, outwards: the chronicle notes the turn and the
    /// projection builds the scene and events with what the chronicle remembers.
    fn project(&mut self, at: SampleRef, conclusion: Conclusion) -> Output {
        use crate::project::v4;
        match conclusion {
            Conclusion::Observed(observation) => {
                let chronicled = self.chronicle.record(crate::project::entry(&observation));
                let (scene, events) = crate::project::observed(*observation, &chronicled);
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
                    scene: Arc::new(v4::republish(last, reason, sensor, map)),
                    events: v4::sensor_events(changed, last.map(|(scene, _)| scene)),
                }
            }
        }
    }

    /// Opens a turn. It is the only way in.
    pub fn handle(&mut self, input: Input) -> Turn<'_> {
        let at = match &input {
            Input::Sample { at, .. } | Input::NoFrame { at, .. } | Input::Sensor { at, .. } => *at,
        };
        let mut turn = Turn {
            monitor: self,
            at,
            geometry: Geometry::Valid,
            phase: Phase::Closed,
            attention: Attention::default(),
        };
        match input {
            Input::Sample { frame, .. } => turn.open(frame),
            Input::NoFrame { why, .. } => {
                let reason = match why {
                    NoFrame::DecodeFailed => RepublishReason::DecodeFailed,
                    NoFrame::Empty => RepublishReason::DecodeEmpty,
                };
                turn.phase = Phase::Republish {
                    reason,
                    changed: None,
                };
            }
            Input::Sensor { state, .. } => {
                turn.phase = Phase::Republish {
                    reason: RepublishReason::Sensor,
                    changed: turn.monitor.engine.sensor.report(at, state),
                };
            }
        }
        turn
    }
}

/// What the turn is waiting for. Each waiting stage carries what the turn has
/// gathered so far: it cannot move on with missing data.
#[derive(Debug)]
enum Phase {
    /// Probe and continuation exist together: there is no wait without a probe.
    Waiting(Waiting),
    /// The turn concluded with all its evidence; projecting remains.
    Concluded { observed: Observed },
    /// No new observation: the last one is republished, declared as such.
    Republish {
        reason: RepublishReason,
        changed: Option<SensorChanged>,
    },
    /// The turn closed (or has not started yet).
    Closed,
}

/// After believing, each continuation carries the evidence it needs.
#[derive(Debug)]
struct Observed {
    believed: Believed,
    evidence: Perceived,
    calibrated: Option<crate::map::BedCalibrated>,
}

/// How far the turn got, with what it gathered: the point from which
/// [`Turn::next`] continues.
enum Reached {
    /// With the main detector's result.
    Primary(Admission),
    /// With the sample's bodies (consolidated with reinforcement, if any).
    Bodies(Admission),
    /// With all the evidence.
    Evidence(Perceived),
    /// With the updated belief.
    Belief(Observed),
    /// With the map up to date.
    Map(Observed),
    /// With the pose (or without it, if it failed).
    Pose {
        observed: Observed,
        pose: Option<Vec<Keypoint>>,
        bbox: Rect,
    },
    /// With the enrichment resolved.
    Enriched(Observed, Enrichment),
}

/// What the core does after each `poll`.
#[derive(Debug, PartialEq)]
pub enum Step<'a> {
    /// The next look the core needs to continue the turn.
    Look(&'a Request),
    /// Nothing else to look at: `finish` closes the turn.
    Done,
}

/// Protocol misuse by the shell. It never changes anything.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FeedError {
    /// The id is not the look the turn is waiting for.
    UnknownId(LookId),
    /// A result was already delivered for that id; the first is kept.
    Duplicate(LookId),
    /// The reading is not of the requested kind.
    KindMismatch(ProbeKindMismatch),
}

impl std::fmt::Display for FeedError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownId(id) => write!(f, "{id} is not the look the turn is waiting for"),
            Self::Duplicate(id) => write!(f, "{id} was already answered; the first answer is kept"),
            Self::KindMismatch(mismatch) => mismatch.fmt(f),
        }
    }
}

impl std::error::Error for FeedError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::KindMismatch(mismatch) => Some(mismatch),
            Self::UnknownId(_) | Self::Duplicate(_) => None,
        }
    }
}

/// `finish` too early: returns the turn so it can be completed.
#[derive(Debug)]
pub struct Incomplete<'m> {
    /// The turn, to keep feeding it.
    pub turn: Box<Turn<'m>>,
}

impl std::fmt::Display for Incomplete<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("the turn still waits for a look; feed it or cancel it")
    }
}

impl std::error::Error for Incomplete<'_> {}

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
}

/// A turn of the loop. It holds the belief exclusively: the compiler prevents
/// opening another while it lives. No turn is abandoned (rule 10).
#[derive(Debug)]
#[must_use = "a turn is driven with poll/feed and closed with finish (rule 10)"]
pub struct Turn<'m> {
    monitor: &'m mut Monitor,
    at: SampleRef,
    geometry: Geometry,
    phase: Phase,
    attention: Attention,
}

impl<'m> Turn<'m> {
    /// What must be looked at? Until it is answered, returns the same look.
    pub fn poll(&mut self) -> Step<'_> {
        self.advance_ready();
        match &self.phase {
            Phase::Waiting(waiting) => Step::Look(waiting.request()),
            _ => Step::Done,
        }
    }

    fn advance_ready(&mut self) {
        match std::mem::replace(&mut self.phase, Phase::Closed) {
            Phase::Waiting(waiting) => match waiting.resolve() {
                Resolution::Ready {
                    request,
                    outcome,
                    stage,
                } => self.advance(stage, &request, outcome),
                Resolution::Pending(waiting) => self.phase = Phase::Waiting(waiting),
            },
            other => self.phase = other,
        }
    }

    /// Delivers what the shell saw for the pending look.
    pub fn feed(&mut self, outcome: Outcome) -> Result<(), FeedError> {
        let Phase::Waiting(waiting) = &mut self.phase else {
            return Err(FeedError::UnknownId(outcome.id));
        };
        waiting.feed(outcome)
    }

    /// Closes the turn: the scene and the events.
    pub fn finish(mut self) -> Result<Output, Incomplete<'m>> {
        let Some(conclusion) = self.conclude() else {
            return Err(Incomplete {
                turn: Box::new(self),
            });
        };
        Ok(self.monitor.project(self.at, conclusion))
    }

    /// What the engine concludes from the turn, if it finished: what was
    /// observed or what to republish. It reads neither the chronicle nor the
    /// last scene.
    fn conclude(&mut self) -> Option<Conclusion> {
        let engine = &self.monitor.engine;
        let sensor = engine.sensor.state();
        match std::mem::replace(&mut self.phase, Phase::Closed) {
            Phase::Concluded { observed } => {
                // The second level reads what was concluded and delivers its result.
                let refined = crate::refine::refine(
                    observed.believed.body.as_ref(),
                    engine.map.bed_for(self.geometry),
                    engine.policy.enrich.as_ref(),
                );
                Some(Conclusion::Observed(Box::new(Observation {
                    at: self.at,
                    believed: observed.believed,
                    refined,
                    evidence: observed.evidence,
                    frame: engine.frame,
                    calibrated: observed.calibrated,
                    attention: std::mem::take(&mut self.attention),
                    map: crate::project::map_state(&engine.map, self.geometry),
                    sensor,
                    dwell_secs: engine.policy.persistence.dwell_secs,
                })))
            }
            Phase::Republish { reason, changed } => Some(Conclusion::Republish {
                reason,
                changed,
                sensor,
                map: crate::project::map_state(&engine.map, Geometry::Valid),
            }),
            other => {
                self.phase = other;
                None
            }
        }
    }

    /// Drives the turn to the end: for each probe, `look` looks and returns what
    /// it saw and how long it took. It is the whole driver in one call; the core
    /// still does no IO because the shell provides the sense. A reading of
    /// another kind than the probe counts as a failing sense.
    pub fn drive(
        mut self,
        mut look: impl FnMut(&Request) -> (Result<Reading, Failure>, Spent),
    ) -> Output {
        loop {
            self.advance_ready();
            let Phase::Waiting(waiting) = &self.phase else {
                break;
            };
            let request = waiting.request();
            let (result, spent) = look(request);
            let result = result.and_then(|reading| {
                if request.probe.accepts(&reading) {
                    Ok(reading)
                } else {
                    Err(Failure::Model)
                }
            });
            let outcome = Outcome {
                id: request.id,
                result,
                spent,
            };
            // The probe's borrow ends before the answer is delivered.
            let fed = self.feed(outcome);
            debug_assert!(fed.is_ok(), "drive respects the protocol: {fed:?}");
        }
        match self.finish() {
            Ok(output) => output,
            // After `Done` nothing is pending; if something were, it closes like
            // a shutdown.
            Err(Incomplete { turn }) => (*turn).cancel(),
        }
    }

    /// The shell cannot complete (shutdown): `Cancelled` for what is pending,
    /// then close. It is a failure of the senses, which the domain already
    /// knows how to interpret.
    pub fn cancel(self) -> Output {
        self.drive(|_| (Err(Failure::Cancelled), Spent(0.0)))
    }

    /// Opens a turn over a sample: the geometry and the main detector, which
    /// always runs. Neither a reconnection nor another image size releases
    /// anyone: the belief starts from scratch only with the process.
    fn open(&mut self, frame: FrameInfo) {
        self.monitor.engine.frame = Some(frame);
        self.geometry = self.monitor.engine.map.gate(frame);
        let region = attention::primary_region();
        self.wait(|id| Waiting::primary(id, region));
    }

    /// The order of a turn. It lives here and nowhere else: at each
    /// point `attention` is asked and a look is requested or the turn goes on.
    ///
    /// ```text
    /// Primary → [Reinforce] → [Faces] → believe → [Calibrate] → [Pose → [Depth]] → conclude
    /// ```
    fn next(&mut self, mut reached: Reached) {
        loop {
            reached = match reached {
                // First attention point, before believing: a loss to reinforce?
                Reached::Primary(primary) => match self.reinforce(&primary) {
                    Decided(Decision::Ask(region)) => {
                        return self.wait(|id| Waiting::reinforce(id, region, primary));
                    }
                    Decided(Decision::Skip(reason)) => {
                        self.skipped(LookKind::Reinforce, reason);
                        Reached::Bodies(primary)
                    }
                    Disabled | Inapplicable => Reached::Bodies(primary),
                },
                // Where to look for faces?
                Reached::Bodies(bodies) => match self.faces(&bodies) {
                    Decided(Decision::Ask((region, policy))) => {
                        return self.wait(|id| Waiting::faces(id, bodies, region, policy));
                    }
                    Decided(Decision::Skip(reason)) => {
                        self.skipped(LookKind::Faces, reason);
                        Reached::Evidence(Perceived {
                            bodies,
                            faces: FaceEvidence::NotLooked,
                        })
                    }
                    Disabled | Inapplicable => Reached::Evidence(Perceived {
                        bodies,
                        faces: FaceEvidence::NotLooked,
                    }),
                },
                // Believe.
                Reached::Evidence(evidence) => {
                    let believed = self.believe(&evidence.bodies);
                    Reached::Belief(Observed {
                        believed,
                        evidence,
                        calibrated: None,
                    })
                }
                // Second attention point, after believing: learn the bed?
                Reached::Belief(observed) => match self.calibrate(&observed) {
                    Decided(Decision::Ask(points)) => {
                        self.monitor.engine.map.attempted(self.at);
                        return self.wait(|id| Waiting::calibrate(id, observed, points));
                    }
                    Decided(Decision::Skip(reason)) => {
                        self.skipped(LookKind::Calibrate, reason);
                        Reached::Map(observed)
                    }
                    Disabled | Inapplicable => Reached::Map(observed),
                },
                // Enrich the subject?
                Reached::Map(observed) => match self.enrich(&observed.believed) {
                    EnrichPlan::Ask(bbox) => {
                        return self.wait(|id| Waiting::pose(id, observed, bbox));
                    }
                    EnrichPlan::Keep(reason) => {
                        self.skipped(LookKind::Pose, reason);
                        Reached::Enriched(observed, Enrichment::Kept)
                    }
                    EnrichPlan::Skip(reason) => {
                        self.skipped(LookKind::Pose, reason);
                        Reached::Enriched(observed, Enrichment::None)
                    }
                    EnrichPlan::Disabled | EnrichPlan::Inapplicable => {
                        Reached::Enriched(observed, Enrichment::None)
                    }
                },
                // Measure the subject's depth?
                Reached::Pose {
                    observed,
                    pose,
                    bbox,
                } => match (self.deepen(pose.as_deref()), pose) {
                    (Decided(Decision::Ask(points)), Some(pose)) => {
                        return self.wait(|id| Waiting::depth(id, observed, pose, bbox, points));
                    }
                    (decision, pose) => {
                        if let Decided(Decision::Skip(reason)) = decision {
                            self.skipped(LookKind::Depth, reason);
                        }
                        let e = Enrichment::fresh(pose, None, bbox, &observed.evidence.bodies);
                        Reached::Enriched(observed, e)
                    }
                },
                // Conclude.
                Reached::Enriched(mut observed, e) => {
                    observed.believed =
                        self.monitor
                            .engine
                            .belief
                            .observe_body(self.at, observed.believed, e);
                    self.phase = Phase::Concluded { observed };
                    return;
                }
            };
        }
    }

    /// The reading arrived: it is interpreted and the turn continues where it was.
    fn advance(&mut self, stage: Awaiting, request: &Request, outcome: Outcome) {
        let spent = outcome.spent;
        let reached = match stage {
            Awaiting::Primary => {
                let admission = evidence::admit(
                    outcome.bodies(),
                    self.monitor.engine.map.bed_for(self.geometry),
                    &self.monitor.engine.policy.admission,
                );
                self.asked(request, spent, admission.result.clone());
                Reached::Primary(admission)
            }
            Awaiting::Reinforce { primary } => {
                // The loss counts as checked only if reinforcement looked: a
                // failing reinforcement is requested again on the next sample.
                if outcome.result.is_ok() {
                    self.monitor.engine.belief.reinforced();
                }
                let (bodies, result) = evidence::consolidate(
                    primary,
                    outcome.bodies(),
                    self.monitor.engine.map.bed_for(self.geometry),
                    &self.monitor.engine.policy.admission,
                );
                self.asked(request, spent, result);
                Reached::Bodies(bodies)
            }
            Awaiting::Faces {
                bodies,
                region,
                policy,
            } => {
                let faces = evidence::faces(outcome.faces(), region.rect(), &policy);
                self.asked(request, spent, faces.result);
                Reached::Evidence(Perceived {
                    bodies,
                    faces: faces.faces,
                })
            }
            Awaiting::Calibrate { mut observed } => {
                let (meters, result) = evidence::depth(outcome.depths());
                self.asked(request, spent, result);
                observed.calibrated = self.monitor.engine.map.learn(self.at, meters);
                Reached::Map(observed)
            }
            Awaiting::Pose { observed, bbox } => {
                let (pose, result) = evidence::pose(outcome.into_pose());
                self.asked(request, spent, result);
                Reached::Pose {
                    observed,
                    pose,
                    bbox,
                }
            }
            Awaiting::Depth {
                observed,
                pose,
                bbox,
                at,
            } => {
                let (depths, result) = evidence::depths(outcome.depths(), &at, pose.len());
                self.asked(request, spent, result);
                let e = Enrichment::fresh(Some(pose), depths, bbox, &observed.evidence.bodies);
                Reached::Enriched(observed, e)
            }
        };
        self.next(reached);
    }

    // --- The questions to attention. Without a policy, there is nothing to decide.

    /// What attention sees of this turn.
    fn glimpse<'g>(&'g self, bodies: &'g Admission, faces: &'g FaceEvidence) -> Glimpse<'g> {
        Glimpse {
            at: self.at,
            geometry: self.geometry,
            map: &self.monitor.engine.map,
            bodies: &bodies.bodies,
            faces,
        }
    }

    fn reinforce(&self, primary: &Admission) -> Consideration<Rect> {
        let Some(policy) = self.monitor.engine.policy.reinforce else {
            return Disabled;
        };
        let g = self.glimpse(primary, &FaceEvidence::NotLooked);
        let belief = &self.monitor.engine.belief;
        attention::reinforce(&g, belief.bed_seen(), belief.latch(), &policy)
    }

    fn faces(&self, bodies: &Admission) -> Consideration<(FaceRegion, FacePolicy)> {
        let Some(policy) = self.monitor.engine.policy.faces else {
            return Disabled;
        };
        let decision = attention::faces(&self.glimpse(bodies, &FaceEvidence::NotLooked));
        Decided(match decision {
            Decision::Ask(region) => Decision::Ask((region, policy)),
            Decision::Skip(reason) => Decision::Skip(reason),
        })
    }

    fn calibrate(&self, observed: &Observed) -> Consideration<Vec<crate::geom::Point>> {
        let Some(policy) = self.monitor.engine.policy.calibration else {
            return Disabled;
        };
        let Perceived { bodies, faces } = &observed.evidence;
        attention::calibrate(&self.glimpse(bodies, faces), &observed.believed, &policy)
    }

    fn enrich(&self, believed: &Believed) -> EnrichPlan {
        self.monitor
            .engine
            .policy
            .enrich
            .map_or(EnrichPlan::Disabled, |policy| {
                attention::enrich(
                    believed,
                    self.monitor.engine.belief.pose_reference(),
                    self.monitor.engine.belief.pose_since(),
                    self.at,
                    &policy,
                )
            })
    }

    fn deepen(&self, pose: Option<&[Keypoint]>) -> Consideration<Vec<(usize, crate::geom::Point)>> {
        let Some(policy) = self.monitor.engine.policy.enrich else {
            return Disabled;
        };
        attention::deepen(pose, &policy)
    }

    // --- Mechanism.

    /// With all the evidence: the belief is updated.
    fn believe(&mut self, bodies: &Admission) -> Believed {
        let Engine { policy, belief, .. } = &mut self.monitor.engine;
        belief.observe(self.at, bodies, policy)
    }

    /// Requests a look and waits in `phase`.
    fn wait(&mut self, make: impl FnOnce(LookId) -> Waiting) {
        self.phase = Phase::Waiting(make(self.monitor.engine.next_id()));
    }

    fn asked(&mut self, request: &Request, spent: Spent, result: LookResult) {
        self.attention.looks.push(Look {
            kind: request.probe.kind(),
            outcome: LookOutcome::Asked {
                region: request.probe.region(),
                result,
                spent_ms: spent.0,
            },
        });
    }

    fn skipped(&mut self, kind: LookKind, reason: SkipReason) {
        self.attention.looks.push(Look {
            kind,
            outcome: LookOutcome::Skipped { reason },
        });
    }
}

impl Drop for Turn<'_> {
    fn drop(&mut self) {
        debug_assert!(
            matches!(self.phase, Phase::Closed) || std::thread::panicking(),
            "rule 10: a turn is not abandoned; close it with finish"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geom::{Quad, Rect};
    use crate::look::{Probe, Reading, Spent};
    use crate::time::{SessionId, SourceSecs};

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
                reinforce: Some(ReinforcePolicy { bed_padding: 0.1 }),
                enrich: Some(EnrichPolicy {
                    move_threshold: 0.1,
                    depth: false,
                    min_keypoint_score: 0.3,
                    posture_tolerance: 0.05,
                    max_pose_age_secs: None,
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
            m.handle(sample(f64::from(s))).drive(|r| match r.probe {
                Probe::Detect { .. } => (Ok(Reading::Bodies(vec![person(seen)])), Spent(1.0)),
                _ => (Ok(pose.clone()), Spent(1.0)),
            });
        }
        assert_eq!(m.engine.belief.pose_reference(), Some(seen));
        // The main detector loses it; reinforcement sees it moved and a new pose is taken.
        let out = m.handle(sample(4.0)).drive(|r| match r.probe {
            Probe::Detect { .. } => (Ok(Reading::Bodies(vec![])), Spent(1.0)),
            Probe::Reinforce { .. } => (Ok(Reading::Bodies(vec![person(moved)])), Spent(1.0)),
            _ => (Ok(pose.clone()), Spent(1.0)),
        });
        let kinds: Vec<_> = out.scene.attention.looks.iter().map(|l| l.kind).collect();
        assert_eq!(
            kinds,
            [LookKind::Detect, LookKind::Reinforce, LookKind::Pose]
        );
        assert_eq!(m.engine.belief.pose_reference(), Some(seen));
    }
}
