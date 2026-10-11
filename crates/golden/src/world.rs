//! The world of a scene: what each sense sees in each sample, in frame
//! coordinates. It is the scenarios' geometry (the same since R4): a 640×480
//! frame, the bed from x=100 to 500 and y=100 to 400, the headboard in its top
//! strip. Making it configurable waits until a scene asks for it.

use crate::language::{Faces, InBed, Location, Scene, Span};
use scene_core::FrameInfo;
use scene_core::geom::Rect;
use scene_core::look::{Detection, Failure, Label, Probe, Reading};

/// The size of a scene's frames.
pub const FRAME: FrameInfo = FrameInfo {
    width: 640,
    height: 480,
};

/// The scenes' bed.
pub const BED: Rect = Rect {
    x0: 100.0,
    y0: 100.0,
    x1: 500.0,
    y1: 400.0,
};

/// The scenes' headboard: the top strip of the bed.
pub const HEADBOARD: Rect = Rect {
    x0: 100.0,
    y0: 100.0,
    x1: 500.0,
    y1: 170.0,
};

/// What can be looked at in a sample: what each sense would see if it looked
/// at the whole frame. It is the scripted frame (R7, memory §3).
#[derive(Debug, Clone, PartialEq)]
pub struct Sample {
    pub n: u32,
    /// Source time: `n × cadence`.
    pub source_secs: f64,
    /// What the main detector sees.
    pub detect: Vec<Detection>,
    /// What the reinforcement detector sees.
    pub reinforce: Vec<Detection>,
    /// What the face detector sees.
    pub faces: Vec<Detection>,
}

impl Scene {
    /// The night's samples, one by one.
    pub fn samples(&self) -> Vec<Sample> {
        samples(&self.spans, self.cadence_s)
    }
}

/// The samples of some spans, with their source time.
pub fn samples(spans: &[Span], cadence_s: f64) -> Vec<Sample> {
    spans
        .iter()
        .flat_map(|s| {
            (s.samples.from..=s.samples.to).map(move |n| Sample {
                n,
                source_secs: f64::from(n) * cadence_s,
                detect: bodies(n - s.samples.from, s),
                reinforce: (0..s.reinforcement).map(in_bed).collect(),
                faces: faces(s.faces),
            })
        })
        .collect()
}

/// A 100×200 person box with its corner at `(x0, y0)`.
fn person(x0: f32, y0: f32) -> Detection {
    Detection {
        bbox: Rect::new(x0, y0, x0 + 100.0, y0 + 200.0),
        confidence: 0.80,
        label: Label::Person,
    }
}

/// Body `i` lying in the bed.
pub fn in_bed(i: u32) -> Detection {
    person(150.0 + 180.0 * i as f32, 150.0)
}

/// Body `i` on the floor. The bed grown by the "near bed" margin (0.10 of its
/// width) reaches x=540: near the bed touches it, the door does not.
pub fn on_floor(i: u32, location: Location) -> Detection {
    let near = match location {
        Location::Door => false,
        Location::NearBed => true,
        Location::Mixed { near } => i < near,
    };
    let x0 = if near { 505.0 } else { 560.0 };
    person(x0, 20.0 + 230.0 * i as f32)
}

/// What the main detector sees in sample `offset` of the span: the bed first,
/// then the floor.
fn bodies(offset: u32, s: &Span) -> Vec<Detection> {
    let in_bed_now = match s.bed {
        InBed::Fixed(c) => c,
        InBed::Alternating => offset % 2,
    };
    (0..in_bed_now)
        .map(in_bed)
        .chain((0..s.floor).map(|i| on_floor(i, s.location)))
        .collect()
}

/// The faces: on the head of the resident lying down, at the headboard outside
/// their body, or weak at the headboard.
pub fn faces(faces: Faces) -> Vec<Detection> {
    let face = |x0: f32, y0: f32, confidence: f32| Detection {
        bbox: Rect::new(x0, y0, x0 + 40.0, y0 + 40.0),
        confidence,
        label: Label::Other,
    };
    match faces {
        Faces::None => vec![],
        Faces::OnSubject => vec![face(180.0, 160.0, 0.8)],
        Faces::AtHeadboard => vec![face(280.0, 110.0, 0.8)],
        Faces::Weak => vec![face(280.0, 110.0, 0.1)],
    }
}

/// The fake senses: answer a core probe with what the scene says that sense
/// sees, **inside the requested region**, like the real sense that crops the
/// frame. A box counts if its centre falls in the region. A region without
/// pixels in the frame fails without looking, like the real one. What the
/// scene does not model (pose, depth) answers like a failing sense.
pub fn respond(sample: &Sample, probe: &Probe) -> Result<Reading, Failure> {
    match probe {
        Probe::Detect { region } => {
            crop(&sample.detect, region.unwrap_or_else(whole_frame)).map(Reading::Bodies)
        }
        Probe::Reinforce { region } => crop(&sample.reinforce, *region).map(Reading::Bodies),
        Probe::Faces { region } => crop(&sample.faces, region.rect()).map(Reading::Faces),
        Probe::Pose { .. } | Probe::Depth { .. } => Err(Failure::Model),
    }
}

/// The whole frame.
fn whole_frame() -> Rect {
    Rect::new(0.0, 0.0, FRAME.width as f32, FRAME.height as f32)
}

fn crop(seen: &[Detection], region: Rect) -> Result<Vec<Detection>, Failure> {
    let frame = whole_frame();
    let width = region.x1.min(frame.x1) - region.x0.max(frame.x0);
    let height = region.y1.min(frame.y1) - region.y0.max(frame.y0);
    if width <= 0.0 || height <= 0.0 {
        return Err(Failure::Model);
    }
    Ok(seen
        .iter()
        .filter(|d| {
            let c = d.bbox.centroid();
            c.x >= region.x0 && c.x <= region.x1 && c.y >= region.y0 && c.y <= region.y1
        })
        .copied()
        .collect())
}
