//! H.264 access unit to RGB conversion with FFmpeg.
//!
//! FFmpeg objects are not `Send`; a decoder lives and dies on the thread that
//! created it.

use crate::IngestError;

type Result<T> = std::result::Result<T, IngestError>;
use image::RgbImage;

use ffmpeg::software::scaling;
use ffmpeg::util::frame::video::Video;

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
    clock_rate: f64,
}

impl FrameDecoder {
    pub fn h264(clock_rate: u32) -> Result<Self> {
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
            clock_rate: f64::from(clock_rate.max(1)),
        })
    }

    /// Decodes a self-contained access unit (a keyframe with parameter sets)
    /// and returns exactly the picture it produces. The decoder is drained and
    /// reset afterwards, so units need not be consecutive.
    pub fn decode_keyframe(&mut self, data: &[u8], pts_ticks: i64) -> Result<Option<Picture>> {
        self.send(data, pts_ticks)?;
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
    pub fn push(&mut self, data: &[u8], pts_ticks: i64) -> Result<Option<Video>> {
        self.send(data, pts_ticks)?;
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

    pub fn picture(&mut self, decoded: &Video) -> Result<Picture> {
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
            source_time_secs: decoded.pts().map(|pts| pts as f64 / self.clock_rate),
        })
    }

    fn send(&mut self, data: &[u8], pts_ticks: i64) -> Result<()> {
        let mut packet = ffmpeg::codec::packet::Packet::copy(data);
        packet.set_pts(Some(pts_ticks));
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
