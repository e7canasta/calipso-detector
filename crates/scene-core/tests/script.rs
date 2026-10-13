//! The harness adapter: a `Script` answers the probes of a turn. What the box
//! senses see is decided by the scene's world (`golden::world`): a single fake,
//! with the real sense's region semantics. Only failures and what the
//! scene does not model (pose, depth) are scripted here.
use golden::language::{Expected, InBed, Location, Samples, Span, spans_from_cells};
use golden::world::{self, Sample};
use scene_core::contract::Keypoint;
use scene_core::look::{DepthPurpose, Detection, Failure, Probe, Reading};

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
    pub fn respond(&self, probe: &Probe) -> Result<Reading, Failure> {
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
            Probe::Pose { bbox } => self.pose.map(|()| {
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
        }
    }
}

/// A body figure: the 17 COCO keypoints around the centre of the subject's box.
/// The box in bed is 100×200 and its centre is 50 px from the bed's edges
/// across and 100 along.
// Only the BDD harness builds figures; the other tests share this file.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Figure {
    /// Lying along the bed, everything in sight.
    Lying,
    /// Lying, with the ankles covered (low score).
    FeetCovered,
    /// Lying, with the feet far from the camera (raised, or off the end of the
    /// bed): their depth says nothing of the torso.
    FeetFar,
    /// Lying, with an arm hanging off the bed.
    ArmOut,
    /// Leaning out over the side: the head and shoulders past the bed's left
    /// edge (x 100 in the test world).
    LeaningOut,
    /// Sitting on the edge, with the legs off the bed.
    SittingOnTheEdge,
    /// Sitting up: the head and shoulders project beyond the headboard.
    SittingUp,
    /// Lying, with the shoulders covered.
    NoTorso,
    /// Standing: the same keypoints, with the head closer to the camera.
    Standing,
}

impl Figure {
    /// Offset from the box centre and score, in COCO order.
    pub fn keypoints(self) -> [(f32, f32, f32); 17] {
        let mut p = [
            (0.0, -85.0, 0.9),  // nose
            (-5.0, -90.0, 0.9), // eyes
            (5.0, -90.0, 0.9),
            (-10.0, -85.0, 0.9), // ears
            (10.0, -85.0, 0.9),
            (-25.0, -55.0, 0.9), // shoulders
            (25.0, -55.0, 0.9),
            (-35.0, -20.0, 0.9), // elbows
            (35.0, -20.0, 0.9),
            (-35.0, 10.0, 0.9), // wrists
            (35.0, 10.0, 0.9),
            (-15.0, 10.0, 0.9), // hips
            (15.0, 10.0, 0.9),
            (-15.0, 50.0, 0.9), // knees
            (15.0, 50.0, 0.9),
            (-15.0, 90.0, 0.9), // ankles
            (15.0, 90.0, 0.9),
        ];
        match self {
            Figure::Lying | Figure::FeetFar => {}
            Figure::FeetCovered => {
                p[15].2 = 0.1;
                p[16].2 = 0.1;
            }
            Figure::ArmOut => {
                p[7] = (-170.0, -20.0, 0.9);
                p[9] = (-200.0, 10.0, 0.9);
            }
            Figure::LeaningOut => {
                for point in p.iter_mut().take(7) {
                    point.0 -= 200.0;
                }
            }
            Figure::SittingOnTheEdge => {
                p[13] = (280.0, 30.0, 0.9);
                p[14] = (290.0, 30.0, 0.9);
                p[15] = (320.0, 90.0, 0.9);
                p[16] = (330.0, 90.0, 0.9);
            }
            Figure::SittingUp => {
                for (i, point) in p.iter_mut().enumerate().take(7) {
                    point.1 = -170.0 + if i < 5 { -20.0 } else { 0.0 };
                }
            }
            Figure::NoTorso => {
                p[5].2 = 0.1;
                p[6].2 = 0.1;
            }
            Figure::Standing => {}
        }
        p
    }

    /// The depth of a figure keypoint at height `y` in the frame: lying,
    /// the head (higher in the image) is farther from the camera than the hips;
    /// sitting up or standing, the other way round, as measured through the
    /// bed's depth crop (R16: sitting up −0.06…+0.01); sitting on the edge,
    /// all the same. Slope of 0.002 per pixel around 2.0.
    pub fn depth(self, y: f32) -> f32 {
        let slope = 0.002 * (300.0 - y);
        match self {
            Figure::SittingOnTheEdge => 2.0,
            Figure::SittingUp | Figure::Standing => 2.0 - slope,
            _ => 2.0 + slope,
        }
    }
}

impl Figure {
    /// How much farther a keypoint at height `y` reads when it is one of the
    /// feet (`feet`, the lowest height of the pose): only for `FeetFar`.
    pub fn feet_offset(self, y: f32, feet: f32) -> f32 {
        if self == Figure::FeetFar && y >= feet {
            1.0
        } else {
            0.0
        }
    }
}

/// The scripted depth of a point, when the pose has configured keypoints:
/// different for each place, to check that each keypoint carries its own.
pub fn depth_at(p: scene_core::geom::Point) -> f32 {
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
