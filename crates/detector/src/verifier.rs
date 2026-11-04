//! The slow verifier's port (white book, volume III, I2): outside the turn.
//! The driver hands it each `Probe::Verify` with the looked sample's frame; it
//! crops the bed in memory, asks, and returns at once. The answers come back
//! later, and the driver delivers them to the monitor before the next sample
//! (`Monitor::verified`). Nothing here writes a pixel anywhere (rule 7): the
//! crop goes to the local verifier and is dropped.
use crate::clock::Clock;
use image::{DynamicImage, codecs::jpeg::JpegEncoder};
use scene_geom::{Quad, Rect};
use scene_look::{Failure, Question, Ticket, Verified};
use std::marker::PhantomData;
use std::sync::mpsc;
use std::time::{Duration, Instant};

/// The wire of the protocol (`calipso.verify.v1`): the request's headers and
/// payload, and the reply read as an answer. Documented in
/// `contract/verify-v1.md`.
mod verify_wire;

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
pub(crate) fn bed_in_crop(frame: &DynamicImage, region: Rect, bed: &Quad) -> String {
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
pub(crate) fn sharpness(frame: &DynamicImage, region: Rect) -> Result<f32, Failure> {
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

/// A request on its way to the service: one crop, or a burst in order.
#[derive(Debug)]
struct Request {
    ticket: Ticket,
    question: Question,
    bed: String,
    jpegs: Vec<Vec<u8>>,
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
    /// and the crop as the payload. The wall-clock deadline is not here: it is
    /// [`DeadlineVerifier`]'s, which wraps this port.
    pub fn spawn(
        runtime: &tokio::runtime::Handle,
        url: String,
        subject: String,
        stream: String,
        camera: String,
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
                let (payload, _) = verify_wire::frames_payload(&jpegs);
                let headers = verify_wire::request_headers(
                    camera.as_str(),
                    stream.as_str(),
                    ticket,
                    question,
                    bed.as_str(),
                    &jpegs,
                );
                // No reply is no answer: the core takes it as the request's expiry.
                let answer = match client
                    .request_with_headers(subject.clone(), headers, payload.into())
                    .await
                {
                    Ok(message) => Some(verify_wire::read_answer(&message.payload)),
                    Err(error) => {
                        tracing::warn!(%error, "verifier: no reply");
                        None
                    }
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

/// The wall-clock deadline of a request, kept by the shell (ADR 2026-11-02, la
/// ráfaga como máquina pura, decisión (b)). A request sent through this port
/// that gets no answer within `wall_timeout` is answered with no answer
/// (`Verified { answer: None }`), which the core takes as the expiry of its
/// request, the same as its own source-time timeout. It wraps any verifier, so
/// a fake can test the deadline without a clock that runs.
#[derive(Debug)]
pub struct DeadlineVerifier<V, C> {
    inner: V,
    wall_timeout: Duration,
    clock: C,
    /// The requests sent and not answered yet, with the instant they expire.
    sent: Vec<(Ticket, Instant)>,
}

impl<V, C> DeadlineVerifier<V, C> {
    pub fn new(inner: V, wall_timeout: Duration, clock: C) -> Self {
        Self {
            inner,
            wall_timeout,
            clock,
            sent: Vec::new(),
        }
    }
}

impl<V: VerifierPort, C: Clock> VerifierPort for DeadlineVerifier<V, C> {
    type Frame = V::Frame;
    fn ask(
        &mut self,
        frame: &V::Frame,
        ticket: Ticket,
        region: Rect,
        bed: &Quad,
        question: Question,
        crops: &[u8],
    ) -> Result<(), Failure> {
        self.inner
            .ask(frame, ticket, region, bed, question, crops)?;
        self.sent
            .push((ticket, self.clock.now() + self.wall_timeout));
        Ok(())
    }
    fn answers(&mut self) -> Vec<Verified> {
        // The deadline decides before the answer is read: an answer that arrives
        // within its deadline is delivered as it is; one that arrives after it is
        // dropped, and the expiry below stands (the core has already taken the
        // request as expired).
        let now = self.clock.now();
        let mut answers = Vec::new();
        for verified in self.inner.answers() {
            if let Some(at) = self
                .sent
                .iter()
                .position(|(t, deadline)| t.id == verified.ticket.id && now < *deadline)
            {
                self.sent.remove(at);
                answers.push(verified);
            }
        }
        let mut expired = Vec::new();
        self.sent.retain(|(ticket, deadline)| {
            let due = now >= *deadline;
            if due {
                expired.push(*ticket);
            }
            !due
        });
        answers.extend(expired.into_iter().map(|ticket| Verified {
            ticket,
            answer: None,
        }));
        answers
    }
    fn keep(&mut self, frame: &V::Frame, ticket: Ticket, region: Rect) -> Result<f32, Failure> {
        self.inner.keep(frame, ticket, region)
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
    use scene_look::Answer;

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

    /// A clock the test moves by hand: no real time runs, and every reading is
    /// the one the test set.
    #[derive(Debug)]
    struct ManualClock(std::cell::Cell<Instant>);

    impl Clock for &ManualClock {
        fn now(&self) -> Instant {
            self.0.get()
        }
    }

    /// A verifier that takes every request and answers only what the test
    /// hands it, when the test hands it.
    #[derive(Debug, Default)]
    struct Fake {
        pending: Vec<Verified>,
    }

    impl VerifierPort for Fake {
        type Frame = ();
        fn ask(
            &mut self,
            _: &(),
            _: Ticket,
            _: Rect,
            _: &Quad,
            _: Question,
            _: &[u8],
        ) -> Result<(), Failure> {
            Ok(())
        }
        fn answers(&mut self) -> Vec<Verified> {
            std::mem::take(&mut self.pending)
        }
        fn keep(&mut self, _: &(), _: Ticket, _: Rect) -> Result<f32, Failure> {
            Err(Failure::Model)
        }
    }

    fn ticket(id: u64) -> Ticket {
        let at = scene_contract::SampleRef {
            session: scene_contract::SessionId(1),
            sequence: 12,
            source_secs: scene_contract::SourceSecs(12.0),
        };
        Ticket {
            id,
            looked: at,
            target: scene_look::Target::Held { since: at },
        }
    }

    fn ask(v: &mut DeadlineVerifier<Fake, &ManualClock>, ticket: Ticket) {
        let region = Rect::new(0.0, 0.0, 10.0, 10.0);
        v.ask(
            &(),
            ticket,
            region,
            &Quad::from_rect(region),
            Question::HeldInBed,
            &[],
        )
        .expect("the fake takes the request");
    }

    #[test]
    fn a_request_with_no_answer_within_its_wall_timeout_is_answered_with_no_answer() {
        let clock = ManualClock(std::cell::Cell::new(Instant::now()));
        let mut v = DeadlineVerifier::new(Fake::default(), Duration::from_secs(4), &clock);
        let asked = ticket(1);
        ask(&mut v, asked);
        assert!(v.answers().is_empty(), "nothing is due at the send");
        clock.0.set(clock.0.get() + Duration::from_millis(3_999));
        assert!(v.answers().is_empty(), "nothing is due before the deadline");
        clock.0.set(clock.0.get() + Duration::from_millis(1));
        assert_eq!(
            v.answers(),
            vec![Verified {
                ticket: asked,
                answer: None,
            }],
            "due at the deadline: no answer"
        );
        assert!(v.answers().is_empty(), "delivered once");
    }

    #[test]
    fn a_late_answer_after_its_deadline_is_dropped_and_the_expiry_stands() {
        let clock = ManualClock(std::cell::Cell::new(Instant::now()));
        let mut v = DeadlineVerifier::new(Fake::default(), Duration::from_secs(4), &clock);
        let asked = ticket(1);
        ask(&mut v, asked);
        clock.0.set(clock.0.get() + Duration::from_secs(5));
        v.inner.pending.push(Verified {
            ticket: asked,
            answer: Some(Answer::Yes),
        });
        assert_eq!(
            v.answers(),
            vec![Verified {
                ticket: asked,
                answer: None,
            }],
            "the answer that came after the deadline does not replace the expiry"
        );
    }
}
