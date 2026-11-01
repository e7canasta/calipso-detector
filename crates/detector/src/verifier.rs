//! The slow verifier's port (white book, volume III, I2): outside the turn.
//! The driver hands it each `Probe::Verify` with the looked sample's frame; it
//! crops the bed in memory, asks, and returns at once. The answers come back
//! later, and the driver delivers them to the monitor before the next sample
//! (`Monitor::verified`). Nothing here writes a pixel anywhere (rule 7): the
//! crop goes to the local verifier and is dropped.
use image::{DynamicImage, codecs::jpeg::JpegEncoder};
use scene_core::{
    geom::{Quad, Rect},
    look::{Answer, Failure, Question, Ticket, Verified},
};
use std::marker::PhantomData;
use std::sync::mpsc;
use std::time::Duration;

/// Who verifies: it takes a request over the looked frame without blocking
/// the turn, and gives back the answers that arrived since the last call.
pub trait VerifierPort {
    type Frame;
    /// `region` is the crop; `bed` the bed grown by the crop's margin, which
    /// the verifier may rectify the crop to. `crops`: which crops kept for the
    /// ticket go, by the order they were kept; empty, this frame's crop.
    fn ask(
        &mut self,
        frame: &Self::Frame,
        ticket: Ticket,
        region: Rect,
        bed: &Quad,
        question: Question,
        crops: &[u8],
    ) -> Result<(), Failure>;
    fn answers(&mut self) -> Vec<Verified>;
    /// Keeps the crop of `region` for the burst of `ticket` (white book,
    /// volume III, ch. 12): it goes out with the `ask` of the same ticket. A
    /// crop kept for another ticket is dropped: one burst at a time. Returns
    /// how sharp the crop is: the core decides whether it counts.
    fn keep(&mut self, frame: &Self::Frame, ticket: Ticket, region: Rect) -> Result<f32, Failure>;
}

/// No verifier: a profile without `[verify]` never asks; if it were asked, it
/// fails like a missing sense.
#[derive(Debug)]
pub struct NoVerifier<F>(PhantomData<F>);

impl<F> Default for NoVerifier<F> {
    fn default() -> Self {
        Self(PhantomData)
    }
}

impl<F> VerifierPort for NoVerifier<F> {
    type Frame = F;
    fn ask(
        &mut self,
        _: &F,
        _: Ticket,
        _: Rect,
        _: &Quad,
        _: Question,
        _: &[u8],
    ) -> Result<(), Failure> {
        Err(Failure::Model)
    }
    fn answers(&mut self) -> Vec<Verified> {
        Vec::new()
    }
    fn keep(&mut self, _: &F, _: Ticket, _: Rect) -> Result<f32, Failure> {
        Err(Failure::Model)
    }
}

/// The bed's corners in the crop's pixels, as the `Calipso-Bed` header says
/// them: `x,y x,y x,y x,y`, headboard left, headboard right, foot right, foot
/// left. The verifier can rectify the crop to the bed with them; the crop sent
/// stays the natural one.
pub fn bed_in_crop(frame: &DynamicImage, region: Rect, bed: &Quad) -> String {
    let x0 = region.x0.clamp(0.0, frame.width() as f32).floor();
    let y0 = region.y0.clamp(0.0, frame.height() as f32).floor();
    bed.0
        .iter()
        .map(|p| format!("{:.1},{:.1}", p.x - x0, p.y - y0))
        .collect::<Vec<_>>()
        .join(" ")
}

/// The pixels of `region` inside the frame, as `(x, y, width, height)`: wider
/// and taller than `min`, or it fails, like a sense would.
fn pixels(frame: &DynamicImage, region: Rect, min: u32) -> Result<(u32, u32, u32, u32), Failure> {
    let clamp = |v: f32, max: u32| v.clamp(0.0, max as f32) as u32;
    let (x0, y0) = (
        clamp(region.x0, frame.width()),
        clamp(region.y0, frame.height()),
    );
    let (x1, y1) = (
        clamp(region.x1, frame.width()),
        clamp(region.y1, frame.height()),
    );
    if x1 <= x0 + min || y1 <= y0 + min {
        return Err(Failure::Model);
    }
    Ok((x0, y0, x1 - x0, y1 - y0))
}

/// The crop of the frame over `region`, clamped to it, as a JPEG in memory.
/// A region without pixels in the frame fails, like a sense would.
pub fn jpeg_crop(frame: &DynamicImage, region: Rect) -> Result<Vec<u8>, Failure> {
    let (x, y, width, height) = pixels(frame, region, 0)?;
    let crop = frame.crop_imm(x, y, width, height).to_rgb8();
    let mut jpeg = Vec::new();
    JpegEncoder::new_with_quality(&mut jpeg, 85)
        .encode_image(&crop)
        .map_err(|_| Failure::Model)?;
    Ok(jpeg)
}

/// How sharp the crop of `region` is: the variance of the Laplacian over the
/// crop in grey, resized to 256 px across, so every crop of a camera is
/// measured on the same scale (white book, volume III, ch. 12). About a
/// millisecond.
pub fn sharpness(frame: &DynamicImage, region: Rect) -> Result<f32, Failure> {
    let (x, y, width, height) = pixels(frame, region, 2)?;
    let grey = frame.crop_imm(x, y, width, height).to_luma8();
    let height = (256 * grey.height() / grey.width().max(1)).max(3);
    let small = image::imageops::resize(&grey, 256, height, image::imageops::FilterType::Triangle);
    let (w, h) = small.dimensions();
    let at = |x: u32, y: u32| f64::from(small.get_pixel(x, y)[0]);
    let mut values = Vec::with_capacity(((w - 2) * (h - 2)) as usize);
    for y in 1..h - 1 {
        for x in 1..w - 1 {
            values.push(at(x - 1, y) + at(x + 1, y) + at(x, y - 1) + at(x, y + 1) - 4.0 * at(x, y));
        }
    }
    let n = values.len().max(1) as f64;
    let mean = values.iter().sum::<f64>() / n;
    let variance = values.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / n;
    Ok(variance as f32)
}

/// What the verifier said, from its reply: `yes`, `no` or anything else, which
/// is unsure. The reply is JSON with an `answer` field, or the bare word.
pub fn read_answer(reply: &[u8]) -> Answer {
    let text = serde_json::from_slice::<serde_json::Value>(reply)
        .ok()
        .and_then(|v| v.get("answer").and_then(|a| a.as_str()).map(str::to_owned))
        .unwrap_or_else(|| String::from_utf8_lossy(reply).into_owned());
    let word: String = text
        .trim()
        .chars()
        .take_while(|c| c.is_alphabetic())
        .collect::<String>()
        .to_lowercase();
    match word.as_str() {
        "yes" | "si" | "sí" => Answer::Yes,
        "no" => Answer::No,
        _ => Answer::Unsure,
    }
}

/// A request on its way to the service: one crop, or a burst in order.
#[derive(Debug)]
struct Request {
    ticket: Ticket,
    question: Question,
    bed: String,
    jpegs: Vec<Vec<u8>>,
}

/// The payload and the `Calipso-Frame-Lengths` header of a request: the crops
/// one after the other, and each one's length in bytes. One crop goes alone
/// and without the header, as before bursts.
pub fn frames_payload(jpegs: &[Vec<u8>]) -> (Vec<u8>, Option<String>) {
    let payload = jpegs.concat();
    let lengths = (jpegs.len() > 1).then(|| {
        jpegs
            .iter()
            .map(|j| j.len().to_string())
            .collect::<Vec<_>>()
            .join(",")
    });
    (payload, lengths)
}

/// Live: the local verifier service behind NATS (`calipso.verify.v1`). A
/// background task makes one request/reply at a time; the turn only queues.
#[derive(Debug)]
pub struct NatsVerifier {
    requests: tokio::sync::mpsc::Sender<Request>,
    answers: mpsc::Receiver<Verified>,
    /// The crops kept for the burst being collected, by its ticket's id.
    kept: Option<(u64, Vec<Vec<u8>>)>,
}

impl NatsVerifier {
    /// Starts the task on `runtime`, on its own connection to `url`. Each
    /// request carries the stream (the NATS prefix), the camera, the
    /// question's versioned id, the ticket and the looked sample as headers,
    /// and the crop as the payload. Without a reply within `wall_timeout`, the
    /// answer is unsure.
    pub fn spawn(
        runtime: &tokio::runtime::Handle,
        url: String,
        subject: String,
        stream: String,
        camera: String,
        wall_timeout: Duration,
    ) -> Self {
        let (requests, mut queue) = tokio::sync::mpsc::channel::<Request>(1);
        let (answered, answers) = mpsc::channel();
        runtime.spawn(async move {
            let client = match async_nats::connect(&url).await {
                Ok(client) => client,
                Err(error) => {
                    tracing::warn!(%error, %url, "verifier: no NATS connection; nothing is verified");
                    return;
                }
            };
            while let Some(Request {
                ticket,
                question,
                bed,
                jpegs,
            }) = queue.recv().await
            {
                let (payload, lengths) = frames_payload(&jpegs);
                let mut headers = async_nats::HeaderMap::new();
                headers.insert("Calipso-Camera", camera.as_str());
                headers.insert("Calipso-Question", question.id());
                headers.insert("Calipso-Ticket", ticket.id.to_string().as_str());
                headers.insert("Calipso-Stream", stream.as_str());
                headers.insert("Calipso-Bed", bed.as_str());
                let looked = ticket.looked;
                let looked = format!(
                    "{}:{}:{}",
                    looked.session.0, looked.sequence, looked.source_secs.0
                );
                headers.insert("Calipso-Looked", looked.as_str());
                if let Some(lengths) = lengths {
                    headers.insert("Calipso-Frames", jpegs.len().to_string().as_str());
                    headers.insert("Calipso-Frame-Lengths", lengths.as_str());
                }
                let reply = tokio::time::timeout(
                    wall_timeout,
                    client.request_with_headers(subject.clone(), headers, payload.into()),
                )
                .await;
                let answer = match reply {
                    Ok(Ok(message)) => read_answer(&message.payload),
                    Ok(Err(error)) => {
                        tracing::warn!(%error, "verifier: request failed");
                        Answer::Unsure
                    }
                    Err(_) => Answer::Unsure,
                };
                if answered.send(Verified { ticket, answer }).is_err() {
                    return;
                }
            }
        });
        Self {
            requests,
            answers,
            kept: None,
        }
    }
}

impl VerifierPort for NatsVerifier {
    type Frame = DynamicImage;
    fn ask(
        &mut self,
        frame: &DynamicImage,
        ticket: Ticket,
        region: Rect,
        bed: &Quad,
        question: Question,
        crops: &[u8],
    ) -> Result<(), Failure> {
        let kept = match self.kept.take() {
            Some((id, kept)) if id == ticket.id => kept,
            _ => Vec::new(),
        };
        let jpegs = if crops.is_empty() {
            vec![jpeg_crop(frame, region)?]
        } else {
            crops
                .iter()
                .map(|&i| kept.get(usize::from(i)).cloned().ok_or(Failure::Model))
                .collect::<Result<Vec<_>, _>>()?
        };
        self.requests
            .try_send(Request {
                ticket,
                question,
                bed: bed_in_crop(frame, region, bed),
                jpegs,
            })
            .map_err(|_| Failure::Model)
    }
    fn answers(&mut self) -> Vec<Verified> {
        self.answers.try_iter().collect()
    }
    fn keep(&mut self, frame: &DynamicImage, ticket: Ticket, region: Rect) -> Result<f32, Failure> {
        let jpeg = jpeg_crop(frame, region)?;
        let sharp = sharpness(frame, region)?;
        match self.kept.as_mut() {
            Some((id, kept)) if *id == ticket.id => kept.push(jpeg),
            _ => self.kept = Some((ticket.id, vec![jpeg])),
        }
        Ok(sharp)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_reply_is_read_as_yes_no_or_unsure() {
        assert_eq!(read_answer(br#"{"answer":"yes"}"#), Answer::Yes);
        assert_eq!(read_answer(br#"{"answer":"No."}"#), Answer::No);
        assert_eq!(read_answer(b"Yes, the person is sitting up."), Answer::Yes);
        assert_eq!(read_answer(b"I cannot tell"), Answer::Unsure);
        assert_eq!(read_answer(b""), Answer::Unsure);
    }

    #[test]
    fn a_burst_goes_as_its_crops_in_order_with_their_lengths() {
        let one = frames_payload(&[vec![1, 2]]);
        assert_eq!(one, (vec![1, 2], None));
        let three = frames_payload(&[vec![1, 2], vec![3], vec![4, 5, 6]]);
        assert_eq!(three, (vec![1, 2, 3, 4, 5, 6], Some("2,1,3".to_owned())));
    }

    #[test]
    fn a_blurred_crop_is_less_sharp_than_a_sharp_one() {
        let mut sharp = image::RgbImage::new(64, 64);
        for (x, y, p) in sharp.enumerate_pixels_mut() {
            let v = if (x / 4 + y / 4) % 2 == 0 { 255 } else { 0 };
            *p = image::Rgb([v, v, v]);
        }
        let sharp = DynamicImage::ImageRgb8(sharp);
        let blurred = sharp.blur(3.0);
        let region = Rect::new(0.0, 0.0, 64.0, 64.0);
        let (s, b) = (
            sharpness(&sharp, region).unwrap(),
            sharpness(&blurred, region).unwrap(),
        );
        assert!(s > 4.0 * b, "sharp {s}, blurred {b}");
        assert!(sharpness(&sharp, Rect::new(70.0, 70.0, 90.0, 90.0)).is_err());
    }

    #[test]
    fn the_bed_is_said_in_the_crop_s_pixels() {
        let frame = DynamicImage::new_rgb8(64, 48);
        let bed = Quad::from_rect(Rect::new(10.0, 12.0, 40.0, 30.0));
        let said = bed_in_crop(&frame, Rect::new(8.0, 8.0, 42.0, 34.0), &bed);
        assert_eq!(said, "2.0,4.0 32.0,4.0 32.0,22.0 2.0,22.0");
    }

    #[test]
    fn the_crop_is_a_jpeg_of_the_region_and_fails_without_pixels() {
        let frame = DynamicImage::new_rgb8(64, 48);
        let jpeg = jpeg_crop(&frame, Rect::new(8.0, 8.0, 40.0, 30.0)).unwrap();
        assert_eq!(&jpeg[..2], &[0xFF, 0xD8]);
        let decoded = image::load_from_memory(&jpeg).unwrap();
        assert_eq!((decoded.width(), decoded.height()), (32, 22));
        assert!(jpeg_crop(&frame, Rect::new(70.0, 50.0, 90.0, 60.0)).is_err());
    }
}
