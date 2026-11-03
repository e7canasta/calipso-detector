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

/// A body figure: the 17 COCO keypoints around the centre of the subject's box.
/// The box in bed is 100×200 and its centre is 50 px from the bed's edges
/// across and 100 along.
// The three binaries that compile this module use different subsets of it.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Figure {
    /// Lying along the bed, everything in sight.
    Lying,
    /// Lying, with the ankles covered (low score).
    FeetCovered,
    /// Lying, with the ankles past the bed's near side (x 100 in the test world):
    /// the legs out of the bed (E7).
    FeetOutSide,
    /// Lying, with the head at (120,130) of the test world: past the headboard's
    /// side on its plane, within the mattress's sides (E6 says beside the headboard).
    HeadBesideTheHeadboard,
    /// Corner bed (F5.2): lying along the bed on the diagonal, the head on the pillow.
    CornerLying,
    /// Corner bed: lying along the bed, the feet past the near side.
    CornerFeetOut,
    /// Corner bed: lying along the bed, the head past the headboard's side.
    CornerHeadBeside,
    /// Corner bed: sitting up against the headboard, the torso vertical in the image.
    CornerSittingUp,
    /// Sitting on the edge with the feet on the floor under the knees: the pelvis
    /// a mattress height above them, over the feet (E2).
    SittingOnTheEdgeFeetBelow,
    /// Standing tall: the hips a mattress height and a half over the ankles
    /// (the pelvis high over the feet, out of the band of the edge).
    StandingTall,
    /// Standing tall, with the ankles covered (low score).
    StandingTallFeetCovered,
    /// Lying, with the feet far from the camera (raised, or off the end of the
    /// bed): their depth says nothing of the torso.
    FeetFar,
    /// Lying, with an arm hanging off the bed.
    ArmOut,
    /// Lying, slid down the bed: everything 60 px towards the feet, the head
    /// below the headboard, the depth of lying.
    SlidDown,
    /// Leaning out over the side: the head and shoulders past the bed's left
    /// edge (x 100 in the test world).
    LeaningOut,
    /// Sitting on the edge, with the legs off the bed.
    SittingOnTheEdge,
    /// Sitting on the edge, legs off the bed, leaning forward: the head
    /// below the headboard as well.
    SittingOnTheEdgeHeadDown,
    /// Sitting on the foot of the bed, the feet on the floor past its foot
    /// end (y 460 in the test world, the footprint ends at 440), not past
    /// its sides: a corner camera's edge (white book, volume VII, ch. 04).
    SittingOnTheFoot,
    /// Sitting up: the head and shoulders project beyond the headboard.
    SittingUp,
    /// Sitting up against the headboard: the head where lying puts it, the
    /// depth of sitting up (the head and shoulders closer than the hips).
    SittingUpAgainstTheHeadboard,
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
            Figure::Lying | Figure::FeetFar | Figure::SittingUpAgainstTheHeadboard => {}
            Figure::FeetCovered => {
                p[15].2 = 0.1;
                p[16].2 = 0.1;
            }
            Figure::FeetOutSide => {
                p[15].0 = -215.0;
                p[16].0 = -200.0;
            }
            Figure::CornerLying
            | Figure::CornerFeetOut
            | Figure::CornerHeadBeside
            | Figure::CornerSittingUp => return corner_keypoints(self),
            Figure::HeadBesideTheHeadboard => {
                // The centre of the scenes' lying box is (300,265): the eyes at (120,130).
                for point in p.iter_mut().take(5) {
                    point.0 = -180.0;
                    point.1 = -135.0;
                }
            }
            Figure::SittingOnTheEdgeFeetBelow => {
                // The scenes' footprint lies 40 px under the bed: a mattress height
                // is 40 px here. The hips 10 px under the centre are a mattress
                // height and a bit above the feet (E2), and the feet are past the
                // mattress's foot (y 411 against 400): on the floor.
                p[13] = (-15.0, 25.0, 0.9);
                p[14] = (15.0, 25.0, 0.9);
                p[15] = (-15.0, 46.0, 0.9);
                p[16] = (15.0, 46.0, 0.9);
            }
            Figure::StandingTall | Figure::StandingTallFeetCovered => {
                // Upright: the hips 100 px above the centre, the knees between them
                // and the ankles, so the leg is straight (180 degrees).
                let heights = [
                    -255.0, -255.0, -225.0, -225.0, -200.0, -200.0, -170.0, -170.0,
                ];
                for (point, y) in p.iter_mut().zip(heights) {
                    point.1 = y;
                }
                p[9].1 = -140.0;
                p[10].1 = -140.0;
                p[11].1 = -100.0;
                p[12].1 = -100.0;
                p[13].1 = -5.0;
                p[14].1 = -5.0;
                if self == Figure::StandingTallFeetCovered {
                    p[15].2 = 0.1;
                    p[16].2 = 0.1;
                }
            }
            Figure::ArmOut => {
                p[7] = (-170.0, -20.0, 0.9);
                p[9] = (-200.0, 10.0, 0.9);
            }
            Figure::SlidDown => {
                for point in &mut p {
                    point.1 += 60.0;
                }
            }
            Figure::LeaningOut => {
                for point in p.iter_mut().take(7) {
                    point.0 -= 200.0;
                }
            }
            Figure::SittingOnTheEdgeHeadDown => {
                p = Figure::SittingOnTheEdge.keypoints();
                for point in p.iter_mut().take(5) {
                    point.1 += 60.0;
                }
            }
            Figure::SittingOnTheFoot => {
                p[13] = (-15.0, 150.0, 0.9);
                p[14] = (15.0, 150.0, 0.9);
                p[15] = (-15.0, 210.0, 0.9);
                p[16] = (15.0, 210.0, 0.9);
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

/// The corner bed's figures (F5.2), relative to the box's centre (314,274), in
/// the corner bed's geometry: the mattress on the diagonal, the footprint 40 px
/// lower, the headboard on the head edge (`bdd/bed_posture.rs`).
fn corner_keypoints(figure: Figure) -> [(f32, f32, f32); 17] {
    let seen = 0.9;
    let (head, shoulders, elbows, wrists, hips, knees, ankles) = match figure {
        Figure::CornerLying => (
            (-106.0, -106.0),
            [(-62.4, -43.6), (-43.6, -62.4)],
            [(-70.0, -20.0), (-20.0, -70.0)],
            [(-80.0, 0.0), (0.0, -80.0)],
            [(34.9, 49.9), (49.9, 34.9)],
            [(60.0, 75.0), (75.0, 60.0)],
            [(81.5, 96.6), (96.6, 81.5)],
        ),
        Figure::CornerFeetOut => (
            (-106.0, -106.0),
            [(-62.4, -43.6), (-43.6, -62.4)],
            [(-70.0, -20.0), (-20.0, -70.0)],
            [(-80.0, 0.0), (0.0, -80.0)],
            [(34.9, 49.9), (49.9, 34.9)],
            [(60.0, 75.0), (75.0, 60.0)],
            [(14.3, 155.3), (-2.2, 129.4)],
        ),
        Figure::CornerHeadBeside => (
            // (151,175): past the headboard's left side (u < 0 on its plane), inside
            // the mattress's sides (u about 0.16) and past the head end.
            (-163.0, -99.0),
            [(-62.4, -43.6), (-43.6, -62.4)],
            [(-70.0, -20.0), (-20.0, -70.0)],
            [(-80.0, 0.0), (0.0, -80.0)],
            [(34.9, 49.9), (49.9, 34.9)],
            [(60.0, 75.0), (75.0, 60.0)],
            [(81.5, 96.6), (96.6, 81.5)],
        ),
        _ => (
            (-106.0, -126.0),
            [(-30.0, -60.0), (-10.0, -60.0)],
            [(-40.0, -30.0), (0.0, -30.0)],
            [(-40.0, 0.0), (0.0, 0.0)],
            [(-30.0, 10.0), (-10.0, 10.0)],
            [(-30.0, 50.0), (-10.0, 50.0)],
            [(-30.0, 90.0), (-10.0, 90.0)],
        ),
    };
    let mut p = [(0.0, 0.0, seen); 17];
    for point in p.iter_mut().take(5) {
        *point = (head.0, head.1, seen);
    }
    p[5] = (shoulders[0].0, shoulders[0].1, seen);
    p[6] = (shoulders[1].0, shoulders[1].1, seen);
    p[7] = (elbows[0].0, elbows[0].1, seen);
    p[8] = (elbows[1].0, elbows[1].1, seen);
    p[9] = (wrists[0].0, wrists[0].1, seen);
    p[10] = (wrists[1].0, wrists[1].1, seen);
    p[11] = (hips[0].0, hips[0].1, seen);
    p[12] = (hips[1].0, hips[1].1, seen);
    p[13] = (knees[0].0, knees[0].1, seen);
    p[14] = (knees[1].0, knees[1].1, seen);
    p[15] = (ankles[0].0, ankles[0].1, seen);
    p[16] = (ankles[1].0, ankles[1].1, seen);
    p
}
