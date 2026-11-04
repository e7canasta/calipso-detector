//! The frame turned as it arrives: a camera that hangs turned is set upright
//! once, right after decoding and before any sense looks at it
//! (`[stream] rotate`). Everything after it, the map included, works on the
//! turned frame.
use super::{Arrival, SampleSource};
use image::DynamicImage;
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// A quarter turn clockwise, as `[stream] rotate` writes it in degrees.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(try_from = "u16", into = "u16")]
pub enum Rotation {
    #[default]
    None,
    Quarter,
    Half,
    ThreeQuarters,
}

impl TryFrom<u16> for Rotation {
    type Error = String;
    fn try_from(degrees: u16) -> Result<Self, Self::Error> {
        match degrees {
            0 => Ok(Self::None),
            90 => Ok(Self::Quarter),
            180 => Ok(Self::Half),
            270 => Ok(Self::ThreeQuarters),
            other => Err(format!(
                "stream.rotate must be 0, 90, 180 or 270 degrees clockwise, not {other}"
            )),
        }
    }
}

impl From<Rotation> for u16 {
    fn from(rotation: Rotation) -> Self {
        match rotation {
            Rotation::None => 0,
            Rotation::Quarter => 90,
            Rotation::Half => 180,
            Rotation::ThreeQuarters => 270,
        }
    }
}

impl Rotation {
    pub fn is_none(&self) -> bool {
        *self == Self::None
    }

    /// The frame, turned.
    pub fn turn(self, frame: DynamicImage) -> DynamicImage {
        match self {
            Self::None => frame,
            Self::Quarter => frame.rotate90(),
            Self::Half => frame.rotate180(),
            Self::ThreeQuarters => frame.rotate270(),
        }
    }
}

/// A source whose frames arrive turned; the rest of what arrives, as it was.
#[derive(Debug)]
pub struct Turned<S> {
    source: S,
    rotation: Rotation,
}

impl<S> Turned<S> {
    pub fn new(source: S, rotation: Rotation) -> Self {
        Self { source, rotation }
    }
}

impl<S: SampleSource<Frame = DynamicImage>> SampleSource for Turned<S> {
    type Frame = DynamicImage;

    fn next(&mut self, wait: Duration) -> Arrival<DynamicImage> {
        match self.source.next(wait) {
            Arrival::Sample {
                at,
                received,
                started,
                frame,
            } => Arrival::Sample {
                at,
                received,
                started,
                frame: frame.map(|frame| frame.map(|frame| self.rotation.turn(frame))),
            },
            other => other,
        }
    }
}
