//! The slow verifier's port (white book, volume III, I2): outside the turn.
//! The driver hands it each `Probe::Verify` with the looked sample's frame; it
//! crops the bed in memory, asks, and returns at once. The answers come back
//! later, and the driver delivers them to the monitor before the next sample
//! (`Monitor::verified`). Nothing here writes a pixel anywhere (rule 7): the
//! crop goes to the local verifier and is dropped.
use image::{DynamicImage, codecs::jpeg::JpegEncoder};
use scene_core::{
    geom::Rect,
    look::{Answer, Failure, Question, Ticket, Verified},
};
use std::marker::PhantomData;
use std::sync::mpsc;
use std::time::Duration;

/// Who verifies: it takes a request over the looked frame without blocking
/// the turn, and gives back the answers that arrived since the last call.
pub trait VerifierPort {
    type Frame;
    fn ask(
        &mut self,
        frame: &Self::Frame,
        ticket: Ticket,
        region: Rect,
        question: Question,
    ) -> Result<(), Failure>;
    fn answers(&mut self) -> Vec<Verified>;
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
    fn ask(&mut self, _: &F, _: Ticket, _: Rect, _: Question) -> Result<(), Failure> {
        Err(Failure::Model)
    }
    fn answers(&mut self) -> Vec<Verified> {
        Vec::new()
    }
}

/// The crop of the frame over `region`, clamped to it, as a JPEG in memory.
/// A region without pixels in the frame fails, like a sense would.
pub fn jpeg_crop(frame: &DynamicImage, region: Rect) -> Result<Vec<u8>, Failure> {
    let clamp = |v: f32, max: u32| v.clamp(0.0, max as f32) as u32;
    let (x0, y0) = (
        clamp(region.x0, frame.width()),
        clamp(region.y0, frame.height()),
    );
    let (x1, y1) = (
        clamp(region.x1, frame.width()),
        clamp(region.y1, frame.height()),
    );
    if x1 <= x0 || y1 <= y0 {
        return Err(Failure::Model);
    }
    let crop = frame.crop_imm(x0, y0, x1 - x0, y1 - y0).to_rgb8();
    let mut jpeg = Vec::new();
    JpegEncoder::new_with_quality(&mut jpeg, 85)
        .encode_image(&crop)
        .map_err(|_| Failure::Model)?;
    Ok(jpeg)
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

/// A request on its way to the service.
#[derive(Debug)]
struct Request {
    ticket: Ticket,
    question: Question,
    jpeg: Vec<u8>,
}

/// Live: the local verifier service behind NATS (`calipso.verify.v1`). A
/// background task makes one request/reply at a time; the turn only queues.
#[derive(Debug)]
pub struct NatsVerifier {
    requests: tokio::sync::mpsc::Sender<Request>,
    answers: mpsc::Receiver<Verified>,
}

impl NatsVerifier {
    /// Starts the task on `runtime`, on its own connection to `url`. Each
    /// request carries the camera, the question's versioned id and the ticket
    /// as headers, and the crop as the payload. Without a reply within
    /// `wall_timeout`, the answer is unsure.
    pub fn spawn(
        runtime: &tokio::runtime::Handle,
        url: String,
        subject: String,
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
                jpeg,
            }) = queue.recv().await
            {
                let mut headers = async_nats::HeaderMap::new();
                headers.insert("Calipso-Camera", camera.as_str());
                headers.insert("Calipso-Question", question.id());
                headers.insert("Calipso-Ticket", ticket.id.to_string().as_str());
                let reply = tokio::time::timeout(
                    wall_timeout,
                    client.request_with_headers(subject.clone(), headers, jpeg.into()),
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
        Self { requests, answers }
    }
}

impl VerifierPort for NatsVerifier {
    type Frame = DynamicImage;
    fn ask(
        &mut self,
        frame: &DynamicImage,
        ticket: Ticket,
        region: Rect,
        question: Question,
    ) -> Result<(), Failure> {
        let jpeg = jpeg_crop(frame, region)?;
        self.requests
            .try_send(Request {
                ticket,
                question,
                jpeg,
            })
            .map_err(|_| Failure::Model)
    }
    fn answers(&mut self) -> Vec<Verified> {
        self.answers.try_iter().collect()
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
    fn the_crop_is_a_jpeg_of_the_region_and_fails_without_pixels() {
        let frame = DynamicImage::new_rgb8(64, 48);
        let jpeg = jpeg_crop(&frame, Rect::new(8.0, 8.0, 40.0, 30.0)).unwrap();
        assert_eq!(&jpeg[..2], &[0xFF, 0xD8]);
        let decoded = image::load_from_memory(&jpeg).unwrap();
        assert_eq!((decoded.width(), decoded.height()), (32, 22));
        assert!(jpeg_crop(&frame, Rect::new(70.0, 50.0, 90.0, 60.0)).is_err());
    }
}
