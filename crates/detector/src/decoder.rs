//! Owns the H.264 decoder per session and clock, including its recovery from failures.
use image::DynamicImage;
use ingest::{FrameDecoder, IngestError, Payload};

#[derive(Debug)]
struct H264Session {
    session: u64,
    clock_rate: u32,
    inner: FrameDecoder,
}

#[derive(Debug, Default)]
pub(crate) struct SessionDecoder {
    current: Option<H264Session>,
}

impl SessionDecoder {
    pub fn decode(
        &mut self,
        session: u64,
        payload: Payload,
    ) -> Result<Option<DynamicImage>, IngestError> {
        match payload {
            Payload::Picture(picture) => Ok(Some(DynamicImage::ImageRgb8(picture.into_image()))),
            Payload::Keyframe {
                data,
                pts_ticks,
                clock_rate,
            } => {
                let decoder = match &mut self.current {
                    Some(d) if d.session == session && d.clock_rate == clock_rate => d,
                    slot => slot.insert(H264Session {
                        session,
                        clock_rate,
                        inner: FrameDecoder::h264(clock_rate)?,
                    }),
                };
                match decoder.inner.decode_keyframe(&data, pts_ticks) {
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
