//! The senses port: who answers the core's probes over a frame. Live,
//! the `sense` models over an image; in a scene, the scripted world. The core
//! cannot tell: it only sees `Request → Outcome`.
use image::DynamicImage;
use scene_core::{
    FrameInfo,
    look::{Failure, Reading, Request, Spent},
};

/// What can be looked at in a sample. It knows its size: the core needs it to
/// know whether the bed geometry applies.
pub trait Frame {
    fn info(&self) -> FrameInfo;
}

impl Frame for DynamicImage {
    fn info(&self) -> FrameInfo {
        FrameInfo {
            width: self.width(),
            height: self.height(),
        }
    }
}

/// Who looks: it answers each probe over the sample's frame.
pub trait SensesPort {
    type Frame: Frame;
    fn look(&mut self, frame: &Self::Frame, request: &Request)
    -> (Result<Reading, Failure>, Spent);
}

/// Live: the models over the image.
impl SensesPort for sense::Senses {
    type Frame = DynamicImage;
    fn look(
        &mut self,
        frame: &DynamicImage,
        request: &Request,
    ) -> (Result<Reading, Failure>, Spent) {
        sense::Senses::look(self, frame, request)
    }
}

/// A scene sample is seen with the scenes' geometry.
impl Frame for golden::world::Sample {
    fn info(&self) -> FrameInfo {
        golden::world::FRAME
    }
}

/// In a golden scene: what the scene says each sense sees, cropped to the
/// requested region (the only fake, `golden::world::respond`). Looking costs
/// nothing: `Spent(0)`, so a replayed scene always gives the same ledger.
#[derive(Debug, Clone, Copy, Default)]
pub struct Scripted;

impl SensesPort for Scripted {
    type Frame = golden::world::Sample;
    fn look(
        &mut self,
        frame: &golden::world::Sample,
        request: &Request,
    ) -> (Result<Reading, Failure>, Spent) {
        (golden::world::respond(frame, &request.probe), Spent(0.0))
    }
}
