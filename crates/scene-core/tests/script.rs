//! The harness has two layers, and nothing else answers a probe. The bottom one
//! is the world (`golden::world`): what the scene sees, with the real sense's
//! region semantics. The top one is `Script`, which decorates it: it answers the
//! senses the world does not model (pose, depth, the verifier), and it injects
//! what a scenario writes (failures, the verifier's refusals and blur). The
//! harness's turn runs `Script::respond` and nothing in front of it.
use golden::language::{Expected, InBed, Location, Samples, Span, spans_from_cells};
use golden::world::{self, Sample};
use scene_contract::Keypoint;
use scene_look::{DepthPurpose, Detection, Failure, Probe, Reading};

/// What each sense answers in a turn.
#[derive(Debug, Clone)]
pub struct Script {
    pub detect: Result<Vec<Detection>, Failure>,
    pub faces: Result<Vec<Detection>, Failure>,
    pub reinforce: Result<Vec<Detection>, Failure>,
    /// The main detector over the whole frame; over the focus it reads.
    pub wide: Result<(), Failure>,
    /// The pose: if it answers, five keypoints inside the requested box.
    pub pose: Result<(), Failure>,
    /// How many keypoints the pose brings and how many with a high score (0.9;
    /// the rest 0.1). The high-score ones are the **last**: that way it shows if
    /// depth is assigned to other keypoints than the measured ones. With
    /// configured keypoints, each one's depth depends on where it is
    /// ([`depth_at`]). `None`: the usual five, all with a high score, and the
    /// same depth for all.
    pub keypoints: Option<(usize, usize)>,
    /// A body figure with the 17 COCO keypoints; it takes precedence over
    /// `keypoints`.
    pub figure: Option<Figure>,
    /// The subject's depth at each requested point.
    pub depth: Result<f32, Failure>,
    /// The bed's depth at each point; `None`: no valid readings.
    pub bed: Option<f32>,
    /// Whether the verifier takes a request; its answer is the ward's.
    pub verify: Result<(), Failure>,
    /// The main detector answers with a reading of another kind than its probe
    /// (a failing sense, rule 1, not a panic).
    pub detect_other_kind: bool,
    /// Reinforcement answers with a reading of another kind than its probe.
    pub reinforce_other_kind: bool,
    /// The verifier refuses every request it is sent (`Failure::Model`).
    pub verify_refused: bool,
    /// The verifier refuses every crop it is asked to keep (`Failure::Model`).
    pub keep_refused: bool,
    /// This sample's kept crop is blurred: the shell measures it at 10, against
    /// 100 for every other.
    pub keep_blurred: bool,
}

impl Default for Script {
    fn default() -> Self {
        Self {
            detect: Ok(vec![]),
            faces: Ok(vec![]),
            reinforce: Ok(vec![]),
            wide: Ok(()),
            pose: Ok(()),
            keypoints: None,
            figure: None,
            depth: Ok(2.0),
            bed: Some(2.0),
            verify: Ok(()),
            detect_other_kind: false,
            reinforce_other_kind: false,
            verify_refused: false,
            keep_refused: false,
            keep_blurred: false,
        }
    }
}

impl Script {
    pub fn bodies(bodies: Vec<Detection>) -> Self {
        Self {
            detect: Ok(bodies),
            ..Self::default()
        }
    }

    /// What the senses see in a sample of a scene.
    pub fn of(sample: &Sample) -> Self {
        Self {
            detect: Ok(sample.detect.clone()),
            reinforce: Ok(sample.reinforce.clone()),
            faces: Ok(sample.faces.clone()),
            ..Self::default()
        }
    }

    /// The scripted senses: the reading the scenario dictates for a probe.
    /// Whether the verifier refuses this probe.
    pub fn refuses(&self, probe: &Probe) -> bool {
        match probe {
            Probe::Verify { .. } => self.verify_refused,
            Probe::Keep { .. } => self.keep_refused,
            _ => false,
        }
    }

    pub fn respond(&self, probe: &Probe) -> Result<Reading, Failure> {
        if self.refuses(probe) {
            return Err(Failure::Model);
        }
        if self.keep_blurred && matches!(probe, Probe::Keep { .. }) {
            return Ok(Reading::Kept { sharpness: 10.0 });
        }
        let other_kind = match probe {
            Probe::Detect { .. } => self.detect_other_kind,
            Probe::Reinforce { .. } => self.reinforce_other_kind,
            _ => false,
        };
        if other_kind {
            // The verifier's reading: neither the detector's nor reinforcement's.
            return Ok(Reading::Dispatched);
        }
        match probe {
            Probe::Detect { .. } | Probe::Reinforce { .. } | Probe::Faces { .. } => {
                if let Probe::Detect { region: None } = probe {
                    self.wide?;
                }
                let scripted = match probe {
                    Probe::Detect { .. } => &self.detect,
                    Probe::Reinforce { .. } => &self.reinforce,
                    _ => &self.faces,
                };
                scripted.as_ref().map_err(|f| *f)?;
                let sample = Sample {
                    n: 0,
                    source_secs: 0.0,
                    detect: self.detect.clone().unwrap_or_default(),
                    reinforce: self.reinforce.clone().unwrap_or_default(),
                    faces: self.faces.clone().unwrap_or_default(),
                };
                world::respond(&sample, probe)
            }
            Probe::Pose { bbox, .. } => self.pose.map(|()| {
                let c = bbox.centroid();
                let kp = |dx: f32, dy: f32| Keypoint {
                    x: c.x + dx,
                    y: c.y + dy,
                    score: 0.9,
                };
                if let Some(figure) = self.figure {
                    return Reading::Pose(Some(
                        figure
                            .keypoints()
                            .iter()
                            .map(|&(dx, dy, score)| Keypoint {
                                score,
                                ..kp(dx, dy)
                            })
                            .collect(),
                    ));
                }
                let Some((good, total)) = self.keypoints else {
                    return Reading::Pose(Some(vec![
                        kp(0.0, -60.0),
                        kp(-20.0, -20.0),
                        kp(20.0, -20.0),
                        kp(-15.0, 40.0),
                        kp(15.0, 40.0),
                    ]));
                };
                Reading::Pose(Some(
                    (0..total)
                        .map(|i| Keypoint {
                            score: if i >= total - good { 0.9 } else { 0.1 },
                            ..kp((i % 5) as f32 * 10.0 - 20.0, (i / 5) as f32 * 20.0 - 40.0)
                        })
                        .collect(),
                ))
            }),
            Probe::Depth {
                purpose: DepthPurpose::Subject,
                points,
                ..
            } => self.depth.map(|d| {
                Reading::Depth(match (self.figure, self.keypoints) {
                    (Some(figure), _) => {
                        // The feet are the lowest keypoints in the image.
                        let feet = points.iter().map(|p| p.y).fold(f32::MIN, f32::max);
                        points
                            .iter()
                            .map(|p| Some(figure.depth(p.y) + figure.feet_offset(p.y, feet)))
                            .collect()
                    }
                    (None, None) => vec![Some(d); points.len()],
                    (None, Some(_)) => points.iter().map(|p| Some(depth_at(*p))).collect(),
                })
            }),
            Probe::Depth {
                purpose: DepthPurpose::Bed,
                points,
                ..
            } => Ok(Reading::Depth(vec![self.bed; points.len()])),
            Probe::Verify { .. } => self.verify.map(|()| Reading::Dispatched),
            Probe::Keep { .. } => self.verify.map(|()| Reading::Kept { sharpness: 100.0 }),
        }
    }
}

/// The figures live with the world (`golden::world::Figure`): a golden scene
/// names them as a scenario does. Their depth is the harness's, which answers
/// the depth sense the world does not model.
pub use golden::world::Figure;

/// The scripted depth of a figure's keypoints.
pub trait FigureDepth {
    fn depth(self, y: f32) -> f32;
    fn feet_offset(self, y: f32, feet: f32) -> f32;
}

impl FigureDepth for Figure {
    /// The depth of a figure keypoint at height `y` in the frame: lying,
    /// the head (higher in the image) is farther from the camera than the hips;
    /// sitting up or standing, the other way round, as measured through the
    /// bed's depth crop (R16: sitting up −0.06…+0.01); sitting on the edge,
    /// all the same. Slope of 0.002 per pixel around 2.0.
    fn depth(self, y: f32) -> f32 {
        let slope = 0.002 * (300.0 - y);
        match self {
            Figure::SittingOnTheEdge
            | Figure::SittingOnTheEdgeHeadDown
            | Figure::SittingOnTheFoot => 2.0,
            Figure::SittingUp
            | Figure::SittingUpAgainstTheHeadboard
            | Figure::Standing
            | Figure::StandingTall
            | Figure::StandingTallFeetCovered => 2.0 - slope,
            _ => 2.0 + slope,
        }
    }

    /// How much farther a keypoint at height `y` reads when it is one of the
    /// feet (`feet`, the lowest height of the pose): only for `FeetFar`.
    fn feet_offset(self, y: f32, feet: f32) -> f32 {
        if self == Figure::FeetFar && y >= feet {
            1.0
        } else {
            0.0
        }
    }
}

/// The scripted depth of a point, when the pose has configured keypoints:
/// different for each place, to check that each keypoint carries its own.
pub fn depth_at(p: scene_geom::Point) -> f32 {
    1.0 + p.x / 1_000.0 + p.y / 100_000.0
}

/// The turns of some spans, each with its script.
pub fn turns(spans: &[Span]) -> Vec<(u32, Script)> {
    world::samples(spans, 1.0)
        .iter()
        .map(|m| (m.n, Script::of(m)))
        .collect()
}

/// Shared set-up of the resident; the same the BDD Given uses: the first half
/// at the door, the second in bed.
pub fn came_in_alone(from: u32, to: u32) -> Vec<(u32, Script)> {
    let half = from + (to - from) / 2;
    let span = |samples, floor, bed| Span {
        samples,
        floor,
        location: Location::Door,
        bed: InBed::Fixed(bed),
        reinforcement: 0,
        faces: Default::default(),
        expected: Expected::default(),
    };
    turns(&[
        span(Samples { from, to: half }, 1, 0),
        span(Samples { from: half + 1, to }, 0, 1),
    ])
}

/// Reads the approved tables: there is no other copy of the spans in code.
pub fn episode(n: u32) -> Vec<(u32, Script)> {
    let (setup, table) = episode_scenario(n);
    let mut script = match setup {
        Setup::None => Vec::new(),
        Setup::CameInAlone => came_in_alone(1, 10),
        Setup::StaffPutThemToBed => episode(1),
    };
    script.extend(turns(&table));
    script
}

/// What an episode's `Given` sets up before its table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Setup {
    None,
    /// "the resident came in alone and lay down in samples 1 to 10"
    CameInAlone,
    /// "the staff put the resident to bed and left in samples 1 to 25" (S1)
    StaffPutThemToBed,
}

/// The scenario of a canonical episode, read with the cucumber parser: its
/// set-up and its span table.
pub fn episode_scenario(n: u32) -> (Setup, Vec<Span>) {
    const RETENTION: &str = include_str!("../../../features/occupancy/retention.feature");
    const ACCOMPANIED: &str = include_str!("../../../features/occupancy/accompanied.feature");
    const CONTRACT: &str = include_str!("../../../features/contract/scene.feature");
    let (feature, name) = match n {
        1 => (ACCOMPANIED, "The staff put the resident to bed and leave"),
        2 => (
            RETENTION,
            "Sleeps covered: the flicker does not empty the bed",
        ),
        3 => (RETENTION, "Gets up alone and leaves"),
        4 => (RETENTION, "Comes back alone, lies down and covers up"),
        5 => (ACCOMPANIED, "They come for them"),
        6 => (ACCOMPANIED, "Taken away"),
        7 => (CONTRACT, "S7: someone enters while the resident is covered"),
        8 => (ACCOMPANIED, "The carer comes in and covers them at once"),
        _ => panic!("unknown episode: {n}"),
    };
    let feature = cucumber::gherkin::Feature::parse(feature, Default::default())
        .unwrap_or_else(|e| panic!("episode {n}'s feature does not parse: {e}"));
    let scenario = feature
        .scenarios
        .iter()
        .find(|s| s.name == name)
        .unwrap_or_else(|| panic!("scenario \"{name}\" is missing"));
    let given = |text: &str| scenario.steps.iter().any(|s| s.value.starts_with(text));
    let setup = if given("the resident came in alone and lay down") {
        Setup::CameInAlone
    } else if given("the staff put the resident to bed and left") {
        Setup::StaffPutThemToBed
    } else {
        Setup::None
    };
    let table = scenario
        .steps
        .iter()
        .find_map(|s| s.table.as_ref())
        .expect("the scenario has its table");
    (setup, spans_from_cells(&table.rows).unwrap())
}
