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
use crate::contract::v3::{Event, Scene};
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
    Sample { at: SampleRef, frame: FrameInfo },
    /// A sample that produced no image: there is nothing to look at.
    NoFrame { at: SampleRef, why: NoFrame },
    /// The shell reports a reception change.
    Sensor { at: SampleRef, state: SensorState },
}

/// Why a sample produced no image.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoFrame {
    DecodeFailed,
    Empty,
}

/// Everything the core knows about one camera. A value: N cameras, N monitors.
///
/// It composes three pieces that do not mix: the engine, which decides with
/// what it sees; the chronicle, which remembers what was published; and the
/// last scene, for republishing. The engine reads neither of the other two
/// (P4).
#[derive(Debug)]
pub struct Monitor {
    engine: Engine,
    /// The chronicle of what was published: the projection's memory.
    chronicle: crate::chronicle::Chronicle,
    /// The last concluded scene, in each version, and its sample, for
    /// republishing. If it was a failure, the scene keeps the observed origin
    /// separately.
    last: Option<Last>,
}

#[derive(Debug)]
struct Last {
    at: SampleRef,
    v3: Arc<Scene>,
    v4: Arc<crate::contract::v4::Scene>,
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
        use crate::project::{v3, v4};
        match conclusion {
            Conclusion::Observed(observation) => {
                let chronicled = self.chronicle.record(crate::project::entry(&observation));
                let projection = crate::project::observed(*observation, &chronicled);
                let (scene, events) = projection.v3;
                let (scene_v4, events_v4) = projection.v4;
                let last = Last {
                    at,
                    v3: Arc::new(scene),
                    v4: Arc::new(scene_v4),
                };
                let output = Output {
                    at,
                    scene: Arc::clone(&last.v3),
                    events,
                    v4: Published {
                        scene: Arc::clone(&last.v4),
                        events: events_v4,
                    },
                };
                self.last = Some(last);
                output
            }
            Conclusion::Republish {
                reason,
                changed,
                sensor,
                map,
            } => {
                let last = self.last.as_ref();
                let last_v3 = last.map(|l| (l.v3.as_ref(), l.at));
                let last_v4 = last.map(|l| (l.v4.as_ref(), l.at));
                Output {
                    at,
                    scene: Arc::new(v3::republish(last_v3, reason, sensor, map)),
                    events: v3::sensor_events(changed, last_v3.map(|(scene, _)| scene)),
                    v4: Published {
                        scene: Arc::new(v4::republish(last_v4, reason, sensor, map)),
                        events: v4::sensor_events(changed, last_v4.map(|(scene, _)| scene)),
                    },
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
    /// A batch of looks, in order.
    Look(&'a [Request]),
    Done,
}

/// Protocol misuse by the shell. It never changes anything.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FeedError {
    /// The id does not belong to the current batch.
    UnknownId(LookId),
    /// A result was already delivered for that id; the first is kept.
    Duplicate(LookId),
    /// The reading is not of the requested kind.
    KindMismatch(ProbeKindMismatch),
}

/// `finish` too early: returns the turn so it can be completed.
#[derive(Debug)]
pub struct Incomplete<'m> {
    pub turn: Box<Turn<'m>>,
}

/// What comes out of a turn.
#[derive(Debug, Clone, PartialEq)]
pub struct Output {
    pub at: SampleRef,
    /// The v3 scene.
    pub scene: Arc<Scene>,
    /// The v3 events.
    pub events: Vec<Event>,
    /// The same turn in v4: it always comes out of the core; the
    /// shell decides whether to publish it.
    pub v4: Published<crate::contract::v4::Scene, crate::contract::v4::Event>,
}

/// What a contract version publishes of a turn.
#[derive(Debug, Clone, PartialEq)]
pub struct Published<S, E> {
    pub scene: Arc<S>,
    pub events: Vec<E>,
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
    /// What must be looked at? With the batch incomplete, returns the same one.
    pub fn poll(&mut self) -> Step<'_> {
        self.advance_ready();
        match &self.phase {
            Phase::Waiting(waiting) => Step::Look(std::slice::from_ref(waiting.request())),
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

    /// Delivers what the shell saw for a probe of the batch.
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

    /// The order of a turn (chapter 02). It lives here and nowhere else: at each
    /// point `attention` is asked and a batch is requested or the turn goes on.
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
                // Creer.
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
                // Concluir.
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

    /// The batch arrived: it is interpreted and the turn continues where it was.
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

    // --- Mecanismo.

    /// With all the evidence: the belief is updated.
    fn believe(&mut self, bodies: &Admission) -> Believed {
        let Engine { policy, belief, .. } = &mut self.monitor.engine;
        belief.observe(self.at, bodies, policy)
    }

    /// Requests a batch and waits in `phase`.
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
    use crate::contract::v3::{BedState, RoomState};
    use crate::geom::{Quad, Rect};
    use crate::look::{Failure, Probe, Reading, Spent};
    use crate::time::{SessionId, SourceSecs};

    fn monitor() -> Monitor {
        Monitor::new(
            Policy::default(),
            SiteMap::new(Quad::from_rect(Rect::new(100.0, 100.0, 500.0, 400.0))),
        )
    }

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

    fn empty(id: LookId) -> Outcome {
        Outcome {
            id,
            result: Ok(Reading::Bodies(vec![])),
            spent: Spent(1.0),
        }
    }

    fn first_request(turn: &mut Turn<'_>) -> Request {
        match turn.poll() {
            Step::Look(batch) => batch[0].clone(),
            Step::Done => panic!("a probe was expected"),
        }
    }

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

    #[test]
    fn poll_with_an_incomplete_batch_returns_the_same_one() {
        let mut m = monitor();
        let mut turn = m.handle(sample(1.0));
        let a = first_request(&mut turn);
        let b = first_request(&mut turn);
        assert_eq!(a, b);
        turn.feed(empty(a.id)).unwrap();
        assert_eq!(turn.poll(), Step::Done);
        turn.finish().unwrap();
    }

    #[test]
    fn an_id_foreign_to_the_batch_changes_nothing() {
        let mut m = monitor();
        let mut turn = m.handle(sample(1.0));
        let r = first_request(&mut turn);
        assert_eq!(
            turn.feed(empty(LookId(999))),
            Err(FeedError::UnknownId(LookId(999)))
        );
        assert_eq!(first_request(&mut turn), r);
        turn.feed(empty(r.id)).unwrap();
        let _ = turn.poll();
        turn.finish().unwrap();
    }

    #[test]
    fn a_repeated_result_is_rejected_and_the_first_is_kept() {
        let mut m = monitor();
        let mut turn = m.handle(sample(1.0));
        let r = first_request(&mut turn);
        turn.feed(empty(r.id)).unwrap();
        let repeated = Outcome {
            result: Err(Failure::Cancelled),
            ..empty(r.id)
        };
        assert_eq!(turn.feed(repeated), Err(FeedError::Duplicate(r.id)));
        let _ = turn.poll();
        let out = turn.finish().unwrap();
        assert!(matches!(
            out.scene.attention.looks[0].result(),
            Some(crate::attention::LookResult::Read { .. })
        ));
    }

    #[test]
    fn a_reading_of_another_kind_is_rejected() {
        let mut m = monitor();
        let mut turn = m.handle(sample(1.0));
        let r = first_request(&mut turn);
        let faces = Outcome {
            result: Ok(Reading::Faces(vec![])),
            ..empty(r.id)
        };
        assert!(matches!(turn.feed(faces), Err(FeedError::KindMismatch(_))));
        turn.feed(empty(r.id)).unwrap();
        let _ = turn.poll();
        turn.finish().unwrap();
    }

    #[test]
    fn early_finish_returns_the_turn() {
        let mut m = monitor();
        let turn = m.handle(sample(1.0));
        let Err(Incomplete { mut turn }) = turn.finish() else {
            panic!("Incomplete was expected");
        };
        let r = first_request(&mut turn);
        turn.feed(empty(r.id)).unwrap();
        let _ = turn.poll();
        (*turn).finish().unwrap();
    }

    #[test]
    fn a_shutdown_closes_the_turn_without_moving_the_belief() {
        let mut m = monitor();
        for s in 1..=3 {
            let mut turn = m.handle(sample(f64::from(s)));
            let r = first_request(&mut turn);
            turn.feed(Outcome {
                result: Ok(Reading::Bodies(vec![crate::look::Detection {
                    bbox: Rect::new(200.0, 150.0, 300.0, 350.0),
                    confidence: 0.8,
                    label: crate::look::Label::Person,
                }])),
                ..empty(r.id)
            })
            .unwrap();
            let _ = turn.poll();
            turn.finish().unwrap();
        }
        let mut turn = m.handle(sample(4.0));
        let r = first_request(&mut turn);
        turn.feed(Outcome {
            result: Err(Failure::Cancelled),
            ..empty(r.id)
        })
        .unwrap();
        let _ = turn.poll();
        let out = turn.finish().unwrap();
        assert_eq!(out.scene.room.occupants, 1);
        assert_eq!(out.scene.bed.state, BedState::Seen);
        assert!(out.events.is_empty());
    }

    #[test]
    fn drive_runs_the_whole_turn() {
        let mut m = monitor();
        let mut probes = 0;
        let out = m.handle(sample(1.0)).drive(|_| {
            probes += 1;
            (Ok(Reading::Bodies(vec![])), Spent(1.0))
        });
        assert_eq!(probes, 1);
        assert_eq!(out.scene.room.state, RoomState::Empty);
    }

    #[test]
    fn a_reading_of_another_kind_in_drive_is_a_failing_sense() {
        let mut m = monitor();
        let out = m
            .handle(sample(1.0))
            .drive(|_| (Ok(Reading::Faces(vec![])), Spent(1.0)));
        assert!(matches!(
            out.scene.attention.looks[0].result(),
            Some(crate::attention::LookResult::Failed { .. })
        ));
    }

    #[test]
    fn a_detector_failure_declares_the_last_observation() {
        // Without the detector the turn did not observe: it republishes the
        // belief and says which sample it is from (rule 2).
        let mut m = monitor();
        let _ = m
            .handle(sample(1.0))
            .drive(|_| (Ok(Reading::Bodies(vec![])), Spent(1.0)));
        let out = m
            .handle(sample(2.0))
            .drive(|_| (Ok(Reading::Faces(vec![])), Spent(1.0)));
        let republished = out
            .scene
            .republished
            .expect("the turn is declared republished");
        assert_eq!(
            republished.reason,
            crate::conclusion::RepublishReason::DetectFailed
        );
        assert_eq!(
            republished.observed_at.map(|at| at.source_secs),
            Some(SourceSecs(1.0))
        );
    }

    #[test]
    fn cancel_closes_the_turn_like_a_shutdown() {
        let mut m = monitor();
        let out = m.handle(sample(1.0)).cancel();
        assert!(matches!(
            out.scene.attention.looks[0].result(),
            Some(crate::attention::LookResult::Failed { .. })
        ));
    }

    #[test]
    #[cfg(debug_assertions)]
    #[should_panic(expected = "rule 10")]
    fn abandoning_a_turn_is_an_error() {
        let mut m = monitor();
        let turn = m.handle(sample(1.0));
        drop(turn);
    }
}
