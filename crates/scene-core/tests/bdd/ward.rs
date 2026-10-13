//! The world of a scenario: one camera, its monitor and the scripted senses.
//! The steps use only the core's public API: they never write the belief by
//! hand, they prepare it by driving turns.

use std::fmt;

use crate::published::{Body, Fact, Kind};
pub use crate::script::Script;
use cucumber::World;
use scene_core::contract::{
    LookKind, LookResult, Provenance, SampleRef, Scene, SessionId, SourceSecs,
};
use scene_core::geom::Quad;
use scene_core::look::Spent;
use scene_core::map::FrameInfo;
use scene_core::map::SiteMap;
use scene_core::policy::Policy;
use scene_core::{Input, Monitor, Output};

/// Every scenario's geometry is that of the golden scenes: a single one.
pub use golden::world::{BED, FRAME};

/// A driven turn: what the core published, and its events as rows.
#[derive(Debug)]
pub struct Turn {
    pub output: Output,
    pub facts: Vec<Fact>,
}

#[derive(Default, World)]
pub struct Ward {
    pub policy: Policy,
    pub map: Option<SiteMap>,
    pub cadence_secs: Option<f64>,
    /// The size of the frames the camera delivers; `FRAME` by default.
    pub frame: Option<FrameInfo>,
    /// The source session; 1 by default.
    pub session: Option<u64>,
    /// The samples a new session starts with: the camera reconnected and
    /// source time starts over.
    pub reconnections: Vec<u32>,
    /// The reinforcement detector fails on every probe.
    pub reinforcement_fails: bool,
    /// The samples in which the main detector fails over the whole frame.
    pub wide_fails: Vec<u32>,
    /// The resident moved in bed and ended up elsewhere: their boxes in
    /// bed look shifted. Each "subject moves" toggles it.
    pub shifted: bool,
    /// The resident changed shape in bed without shifting: their boxes in
    /// bed look wider and shorter, with the same centre. Each "subject changes
    /// shape" toggles it.
    pub reshaped: bool,
    /// How many keypoints the pose brings and how many good ones; `None`:
    /// the usual ones.
    pub keypoints: Option<(usize, usize)>,
    /// Each turn with what the senses answered: the recorded night, to replay
    /// it without a camera.
    pub recording: Vec<(Input, Script)>,
    pub monitor: Option<Monitor>,
    pub turns: Vec<Turn>,
    /// The cell a contract-format scenario builds.
    pub cell: Option<String>,
    /// The scripted verifier: what it answers and after how many source
    /// seconds; the requests still owed, with when their answer is due.
    pub verifier: Verifier,
}

/// The scripted slow verifier: it answers each request after a latency, in
/// source time, before the first sample at or past it.
#[derive(Debug, Default)]
pub struct Verifier {
    pub answer: Option<scene_core::look::Answer>,
    pub latency_secs: f64,
    /// Each request still owed, with the sample it went out at: the latency
    /// counts from there, as a burst goes out after its first sample.
    pub owed: Vec<(scene_core::look::Ticket, SampleRef)>,
}

impl fmt::Debug for Ward {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Ward")
            .field("policy", &self.policy)
            .field("turns", &self.turns.len())
            .field("last", &self.turns.last().map(|t| &t.output.scene))
            .finish()
    }
}

impl Ward {
    /// The instant of sample N according to the cadence.
    pub fn sample(&self, n: u32) -> SampleRef {
        let earlier: Vec<u32> = self
            .reconnections
            .iter()
            .copied()
            .filter(|&r| r <= n)
            .collect();
        let origin = earlier.iter().max().copied().unwrap_or(0);
        SampleRef {
            session: SessionId(self.session.unwrap_or(1) + earlier.len() as u64),
            sequence: u64::from(n),
            source_secs: SourceSecs(f64::from(n - origin) * self.cadence_secs.unwrap_or(1.0)),
        }
    }

    pub fn bed(&mut self) {
        self.map = Some(SiteMap::new(Quad::from_rect(BED)));
    }

    /// A decoded sample with what the script dictates.
    pub fn turn_with(&mut self, at: SampleRef, mut script: Script) {
        if self.reinforcement_fails {
            script.reinforce = Err(scene_core::look::Failure::Model);
        }
        if self.wide_fails.iter().any(|&n| u64::from(n) == at.sequence) {
            script.wide = Err(scene_core::look::Failure::Model);
        }
        if self.shifted {
            shift_in_bed(&mut script, SHIFT);
        }
        if self.reshaped {
            reshape_in_bed(&mut script);
        }
        if self.keypoints.is_some() {
            script.keypoints = self.keypoints;
        }
        let frame = self.frame.unwrap_or(FRAME);
        self.drive(Input::Sample { at, frame }, &script);
    }

    /// Drives a turn with the core's driver (`Turn::drive`): the scripted
    /// senses answer each probe.
    pub fn drive(&mut self, input: Input, script: &Script) {
        let monitor = self.monitor.get_or_insert_with(|| {
            Monitor::new(
                self.policy.clone(),
                self.map
                    .clone()
                    .expect("missing \"a room with the bed delimited\""),
            )
        });
        if let (Input::Sample { at, .. }, Some(answer)) = (&input, self.verifier.answer) {
            let latency = self.verifier.latency_secs;
            let (due, owed): (Vec<_>, Vec<_>) = std::mem::take(&mut self.verifier.owed)
                .into_iter()
                .partition(|(_, sent)| {
                    // From an earlier session, it arrives late, at once.
                    sent.session != at.session || at.source_secs.0 - sent.source_secs.0 >= latency
                });
            self.verifier.owed = owed;
            for (ticket, _) in due {
                monitor.verified(scene_core::look::Verified { ticket, answer });
            }
        }
        let mut asked = Vec::new();
        let sent = match &input {
            Input::Sample { at, .. } => Some(*at),
            _ => None,
        };
        let output = monitor.handle(input.clone()).drive(|request| {
            if let (scene_core::look::Probe::Verify { ticket, .. }, Some(sent)) = (request, sent) {
                asked.push((*ticket, sent));
            }
            (script.respond(request), Spent(1.0))
        });
        self.verifier.owed.extend(asked);
        let facts = crate::published::facts(&output);
        invariants(&output.scene, &facts);
        crate::published::check(&output);
        self.recording.push((input, script.clone()));
        self.turns.push(Turn { output, facts });
    }

    pub fn last(&self) -> &Turn {
        self.turns.last().expect("no sample arrived")
    }

    /// Every event published in the scenario, as rows, in order.
    pub fn events(&self) -> impl Iterator<Item = &Fact> {
        self.turns.iter().flat_map(|t| &t.facts)
    }
}

/// How far whoever moved shifts in bed: 40 px, more than the scenarios' motion
/// threshold (0.1 × the 100×200 diagonal ≈ 22 px), without leaving the bed.
pub const SHIFT: f32 = 40.0;

/// Shifts by `dx` the body boxes whose centre is in bed, in what the main
/// detector and reinforcement see: the resident changed place on the mattress.
fn shift_in_bed(script: &mut Script, dx: f32) {
    let shift = |bodies: &mut Vec<scene_core::look::Detection>| {
        for d in bodies.iter_mut() {
            let c = d.bbox.centroid();
            if c.x >= BED.x0 && c.x <= BED.x1 && c.y >= BED.y0 && c.y <= BED.y1 {
                d.bbox.x0 += dx;
                d.bbox.x1 += dx;
            }
        }
    };
    if let Ok(bodies) = script.detect.as_mut() {
        shift(bodies);
    }
    if let Ok(bodies) = script.reinforce.as_mut() {
        shift(bodies);
    }
}

/// Widens by 40 px and flattens by 40 px the body boxes whose centre is in
/// bed, without moving the centre: the width changes by 40 %, more than the
/// threshold.
fn reshape_in_bed(script: &mut Script) {
    let reshape = |bodies: &mut Vec<scene_core::look::Detection>| {
        for d in bodies.iter_mut() {
            let c = d.bbox.centroid();
            if c.x >= BED.x0 && c.x <= BED.x1 && c.y >= BED.y0 && c.y <= BED.y1 {
                d.bbox.x0 -= 20.0;
                d.bbox.x1 += 20.0;
                d.bbox.y0 += 20.0;
                d.bbox.y1 -= 20.0;
            }
        }
    };
    if let Ok(bodies) = script.detect.as_mut() {
        reshape(bodies);
    }
    if let Ok(bodies) = script.reinforce.as_mut() {
        reshape(bodies);
    }
}

/// Is `a` later than `b`? Source time is only compared within a session; the
/// harness's sessions grow.
pub fn later(a: SampleRef, b: SampleRef) -> bool {
    (a.session.0, a.source_secs.0) > (b.session.0, b.source_secs.0)
}

/// The rules' guardians, checked on every turn of every scenario.
fn invariants(scene: &Scene, events: &[Fact]) {
    use scene_core::contract::BedState;
    if scene.bed.state == BedState::Held {
        assert_eq!(scene.bed.seen, 0);
        assert!(!scene.bed.held.is_empty());
    }
    if scene.bed.state == BedState::Seen {
        assert!(scene.bed.seen > 0);
    }
    // Rule 2: a scene without observed bodies declares it did not observe.
    if scene.evidence.bodies.status == scene_core::contract::BodiesStatus::Unavailable {
        assert!(
            scene.republished.is_some(),
            "rule 2: scene without a body observation that does not declare it"
        );
    }
    // Rule 2: nothing is `Current` without a reading in this turn.
    let read =
        |kinds: &[LookKind]| {
            scene.attention.looks.iter().any(|l| {
                kinds.contains(&l.kind) && matches!(l.result(), Some(LookResult::Read { .. }))
            })
        };
    if let Some(s) = &scene.subject {
        if s.provenance == Provenance::Current {
            assert!(
                scene.republished.is_none(),
                "rule 2: Current in a republication"
            );
            assert!(
                read(&[LookKind::Detect, LookKind::Reinforce]),
                "rule 2: Current subject without a body reading"
            );
        }
        if s.pose
            .as_ref()
            .is_some_and(|p| p.provenance == Provenance::Current)
        {
            assert!(
                read(&[LookKind::Pose]),
                "rule 2: Current pose without a pose reading"
            );
        }
        if s.depth.is_some_and(|d| d.provenance == Provenance::Current) {
            assert!(
                read(&[LookKind::Depth]),
                "rule 2: Current depth without a reading"
            );
        }
    }
    // Rule 4: sensor → occupancy → calibration → derived.
    let rank = |e: &Fact| match (e.kind, &e.body) {
        (Kind::Sensor, _) => 0,
        (Kind::Domain, Body::BedCalibrated { .. }) => 3,
        (Kind::Domain, _) => 2,
        (Kind::Derived, _) => 4,
    };
    let ranks: Vec<u8> = events.iter().map(rank).collect();
    assert!(
        ranks.windows(2).all(|w| w[0] <= w[1]),
        "rule 4: event order {events:?}"
    );
}
