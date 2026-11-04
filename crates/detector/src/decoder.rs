//! Owns the H.264 decoder per session and clock, including its recovery from failures.
use image::DynamicImage;
use ingest::{FrameDecoder, IngestError, Payload, Session};
use std::num::NonZeroU32;

#[derive(Debug)]
struct H264Session {
    session: Session,
    clock_rate: NonZeroU32,
    inner: FrameDecoder,
}

#[derive(Debug, Default)]
pub(crate) struct SessionDecoder {
    current: Option<H264Session>,
}

impl SessionDecoder {
    pub(crate) fn decode(
        &mut self,
        session: Session,
        payload: Payload,
    ) -> Result<Option<DynamicImage>, IngestError> {
        match payload {
            Payload::Picture(picture) => Ok(Some(DynamicImage::ImageRgb8(picture.into_image()))),
            Payload::Keyframe { data, time } => {
                let decoder = match &mut self.current {
                    Some(d) if d.session == session && d.clock_rate == time.clock_rate => d,
                    slot => slot.insert(H264Session {
                        session,
                        clock_rate: time.clock_rate,
                        inner: FrameDecoder::h264(time.clock_rate)?,
                    }),
                };
                match decoder.inner.decode_keyframe(&data, time) {
                    Ok(picture) => Ok(picture.map(|p| DynamicImage::ImageRgb8(p.into_image()))),
                    Err(error) => {
                        self.current = None;
                        Err(error)
                    }
                }
            }
        }
    }
}
