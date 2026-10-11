//! Scripted model per role: what it returns belongs to the image it received.
use image::DynamicImage;
use scene_core::look::Detection;
use sense::{DepthMap, DepthPort, DetectorPort, ModelError, PersonPose, PosePort};
use std::{
    collections::VecDeque,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
};

#[derive(Debug, Clone, PartialEq)]
pub struct Call {
    pub sample: u64,
    pub size: (u32, u32),
}

#[derive(Debug, Default)]
struct State<T> {
    calls: Vec<Call>,
    readings: VecDeque<Result<T, String>>,
    fallback: Fallback<T>,
}

#[derive(Debug, Default)]
struct Fallback<T> {
    reading: T,
    error: Option<String>,
}

/// A scripted model: it returns what was queued and, after that, what it "sees"
/// (or fails), and records each call with the sample and the received image size.
#[derive(Debug)]
pub struct Scripted<T> {
    state: Arc<Mutex<State<T>>>,
    sample: Arc<AtomicU64>,
}

impl<T: Default> Default for Scripted<T> {
    fn default() -> Self {
        Self {
            state: Arc::default(),
            sample: Arc::default(),
        }
    }
}

impl<T> Clone for Scripted<T> {
    fn clone(&self) -> Self {
        Self {
            state: Arc::clone(&self.state),
            sample: Arc::clone(&self.sample),
        }
    }
}

/// Box detector: main, reinforcement or faces.
pub type Model = Scripted<Vec<Detection>>;
/// Pose model: every person in the crop.
pub type PoseModel = Scripted<Vec<PersonPose>>;
/// Depth model: the map of the whole image.
pub type DepthModel = Scripted<DepthMap>;

impl<T: Clone> Scripted<T> {
    pub fn at(&self, sample: u64) {
        self.sample.store(sample, Ordering::Relaxed);
    }
    pub fn sees(&self, reading: T) {
        self.state.lock().unwrap().fallback = Fallback {
            reading,
            error: None,
        };
    }
    pub fn fails(&self) {
        self.state.lock().unwrap().fallback.error = Some("fallo guionado".into());
    }
    pub fn enqueue(&self, reading: Result<T, String>) {
        self.state.lock().unwrap().readings.push_back(reading);
    }
    pub fn calls(&self) -> Vec<Call> {
        self.state.lock().unwrap().calls.clone()
    }
    fn answer(&self, image: &DynamicImage) -> Result<T, ModelError> {
        let mut state = self.state.lock().unwrap();
        state.calls.push(Call {
            sample: self.sample.load(Ordering::Relaxed),
            size: (image.width(), image.height()),
        });
        let result = state
            .readings
            .pop_front()
            .unwrap_or_else(|| match &state.fallback.error {
                Some(error) => Err(error.clone()),
                None => Ok(state.fallback.reading.clone()),
            });
        result.map_err(ModelError::Inference)
    }
}

impl DetectorPort for Model {
    fn detect(&mut self, image: &DynamicImage) -> Result<Vec<Detection>, ModelError> {
        self.answer(image)
    }
}

impl PosePort for PoseModel {
    fn estimate(&mut self, image: &DynamicImage) -> Result<Vec<PersonPose>, ModelError> {
        self.answer(image)
    }
}

impl DepthPort for DepthModel {
    fn estimate(&mut self, image: &DynamicImage) -> Result<DepthMap, ModelError> {
        self.answer(image)
    }
}
