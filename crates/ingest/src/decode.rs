//! H.264 access unit to RGB conversion with FFmpeg.
//!
//! FFmpeg objects are not `Send`; a decoder lives and dies on the thread that
//! created it.
//!
//! The work is blocking and runs inside the stream thread's current-thread
//! async runtime: decoding happens in the async context of the RTSP session,
//! so cancelling that future only takes effect between frames — the latency
//! of a cancellation is the duration of the frame being decoded.
//!
//! The consumer of the mailbox may die first. The stream stage reports its
//! state to the shell with events on a channel it sends best-effort (`let _ =
//! …`): a shell that stopped listening loses nothing but those reports. A
//! closed mailbox is different: it means the engine is gone for good and is a
//! fatal stop for the session (`publish` refuses to continue).

use crate::{IngestError, MediaTime};

type Result<T> = std::result::Result<T, IngestError>;
use image::RgbImage;

use ffmpeg::software::scaling;
use ffmpeg::util::frame::video::Video;
use std::num::NonZeroU32;

/// A decoded picture and the source time carried through the decoder.
#[derive(Debug)]
pub struct Picture {
    image: RgbImage,
    /// Source time of the frame that came out of the decoder, which may differ
    /// from the packet just sent when the decoder reorders.
    pub source_time_secs: Option<f64>,
}

pub struct FrameDecoder {
    decoder: ffmpeg::decoder::Video,
    scaler: Option<scaling::Context>,
    clock_rate: NonZeroU32,
}

impl FrameDecoder {
    /// A decoder for the source clock of the timestamps it will receive.
    /// The rate cannot be zero: the type refuses it.
    pub fn h264(clock_rate: NonZeroU32) -> Result<Self> {
        ffmpeg::init()
            .map_err(|e| IngestError::Decode(format!("FFmpeg initialization failed: {e}")))?;
        let codec = ffmpeg::codec::decoder::find(ffmpeg::codec::Id::H264)
            .ok_or_else(|| IngestError::Decode("H264 decoder not found".to_owned()))?;
        let mut context = ffmpeg::codec::Context::new();
        // Single-threaded, low-delay decoding: the picture for a packet must
        // come out for that packet, not one or more packets later.
        context.set_threading(ffmpeg::codec::threading::Config::count(1));
        context.set_flags(ffmpeg::codec::Flags::LOW_DELAY);
        let decoder = ffmpeg::codec::decoder::Decoder(context)
            .open_as(codec)?
            .video()?;
        Ok(Self {
            decoder,
            scaler: None,
            clock_rate,
        })
    }

    /// Decodes a self-contained access unit (a keyframe with parameter sets)
    /// and returns exactly the picture it produces. The decoder is drained and
    /// reset afterwards, so units need not be consecutive.
    pub fn decode_keyframe(&mut self, data: &[u8], time: MediaTime) -> Result<Option<Picture>> {
        self.send(data, time)?;
        self.decoder.send_eof()?;
        let mut last = None;
        let mut decoded = Video::empty();
        while self.decoder.receive_frame(&mut decoded).is_ok() {
            last = Some(self.picture(&decoded)?);
        }
        self.decoder.flush();
        Ok(last)
    }

    /// Feeds one unit of a continuous stream. Returns the newest raw frame
    /// produced, without converting it.
    pub(crate) fn push(&mut self, data: &[u8], time: MediaTime) -> Result<Option<Video>> {
        self.send(data, time)?;
        let mut newest = None;
        loop {
            let mut decoded = Video::empty();
            match self.decoder.receive_frame(&mut decoded) {
                Ok(()) => newest = Some(decoded),
                Err(ffmpeg::Error::Other {
                    errno: ffmpeg::util::error::EAGAIN,
                }) => break,
                Err(error) => return Err(error.into()),
            }
        }
        Ok(newest)
    }

    pub(crate) fn picture(&mut self, decoded: &Video) -> Result<Picture> {
        if let Some(current) = self.scaler.as_ref()
            && (current.input().format != decoded.format()
                || current.input().width != decoded.width()
                || current.input().height != decoded.height())
        {
            self.scaler = None;
        }
        let scaler = match self.scaler.as_mut() {
            Some(scaler) => scaler,
            None => self.scaler.insert(scaling::Context::get(
                decoded.format(),
                decoded.width(),
                decoded.height(),
                ffmpeg::format::Pixel::RGB24,
                decoded.width(),
                decoded.height(),
                scaling::Flags::BILINEAR,
            )?),
        };
        let mut rgb = Video::empty();
        scaler.run(decoded, &mut rgb)?;
        Ok(Picture {
            image: rgb_image(&rgb)?,
            source_time_secs: decoded.pts().map(|pts| {
                MediaTime {
                    ticks: pts,
                    clock_rate: self.clock_rate,
                }
                .as_secs()
            }),
        })
    }

    fn send(&mut self, data: &[u8], time: MediaTime) -> Result<()> {
        let mut packet = ffmpeg::codec::packet::Packet::copy(data);
        packet.set_pts(Some(time.ticks));
        self.decoder.send_packet(&packet)?;
        Ok(())
    }
}

/// Copies an RGB24 frame row by row, dropping the stride padding.
fn rgb_image(frame: &Video) -> Result<RgbImage> {
    let (width, height) = (frame.width() as usize, frame.height() as usize);
    let (data, stride) = (frame.data(0), frame.stride(0));
    let row = width * 3;
    let mut pixels = Vec::with_capacity(row * height);
    for y in 0..height {
        pixels.extend_from_slice(&data[y * stride..y * stride + row]);
    }
    RgbImage::from_raw(width as u32, height as u32, pixels)
        .ok_or_else(|| IngestError::Decode(format!("RGB buffer does not match {width}x{height}")))
}

impl std::fmt::Debug for FrameDecoder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FrameDecoder")
            .field("clock_rate", &self.clock_rate)
            .finish_non_exhaustive()
    }
}

impl Picture {
    pub fn width(&self) -> u32 {
        self.image.width()
    }
    pub fn height(&self) -> u32 {
        self.image.height()
    }
    pub fn image(&self) -> &RgbImage {
        &self.image
    }

    /// Transfers the decoded RGB buffer without copying its pixels.
    pub fn into_image(self) -> RgbImage {
        self.image
    }
}
