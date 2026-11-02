#[path = "../support/models.rs"]
pub mod models;
use cucumber::{World, given, then, when};
use detector::VerifierPort;
use image::DynamicImage;
use models::{Call, DepthModel, Model, PoseModel};
use scene_geom::Rect;
use scene_look::{
    Detection, FaceRegion, Failure, Label, Probe, Reading, SenseProbe, Spent, Ticket,
};
use sense::{DetectorPort, ModelError};
use std::time::Duration;

/// The main detector, slowed down by `delay`: the router's clock encloses the
/// answerer's work, so the measured time includes this delay.
#[derive(Debug)]
pub struct Slow {
    pub model: Model,
    pub delay: Duration,
}

impl DetectorPort for Slow {
    fn detect(&mut self, image: &DynamicImage) -> Result<Vec<Detection>, ModelError> {
        std::thread::sleep(self.delay);
        self.model.detect(image)
    }
}

/// The verifier, as a count: how many questions and how many kept crops it took.
#[derive(Debug, Default)]
pub struct Verifier {
    pub asks: usize,
    pub keeps: usize,
}

impl VerifierPort for Verifier {
    type Frame = DynamicImage;
    fn ask(
        &mut self,
        _: &DynamicImage,
        _: Ticket,
        _: Rect,
        _: &scene_geom::Quad,
        _: scene_look::Question,
        _: &[u8],
    ) -> Result<(), Failure> {
        self.asks += 1;
        Ok(())
    }
    fn answers(&mut self) -> Vec<scene_look::Verified> {
        Vec::new()
    }
    fn keep(&mut self, _: &DynamicImage, _: Ticket, _: Rect) -> Result<f32, Failure> {
        self.keeps += 1;
        Ok(3.0)
    }
}

#[derive(Debug, Default, World)]
pub struct Shell {
    pub image: Option<DynamicImage>,
    pub main: Option<Model>,
    pub reinforce: Option<Model>,
    pub faces: Option<Model>,
    pub pose: Option<PoseModel>,
    pub depth: Option<DepthModel>,
    /// How long the main detector takes to answer, in milliseconds.
    pub main_delay_ms: u64,
    /// The probe the scenario's turn asks, and the router's answer to it.
    pub probe: Option<Probe>,
    pub answer: Option<(Result<Reading, Failure>, Spent)>,
    pub verifier: Verifier,
    /// The map the depth model sees, while the scenario builds it.
    pub depth_map: sense::DepthMap,
    pub reading: Option<Result<Reading, Failure>>,
    pub config: Option<Result<detector::Config, String>>,
    pub scenes: std::collections::BTreeMap<u64, serde_json::Value>,
    pub messages: Vec<serde_json::Value>,
    /// The samples the last "When" drove, to replay them with another profile.
    pub samples: (u64, u64),
    pub night: Vec<scene_core::Output>,
    pub distilled: Option<Result<golden::distill::Distilled, golden::language::Error>>,
    pub policy: Option<scene_core::policy::Policy>,
    /// The repository profile a scenario loaded from `config/`, by its path.
    pub loaded_from: Option<String>,
}

impl Shell {
    pub fn parse_config(&mut self, extra: &str) {
        self.config = Some(
            detector::Config::parse(&format!(
                "{}{extra}",
                include_str!("../support/profile.toml")
            ))
            .map_err(|e| e.to_string()),
        );
    }
    pub fn config(&self) -> &detector::Config {
        self.config
            .as_ref()
            .unwrap()
            .as_ref()
            .expect("valid profile")
    }
    pub fn model(&self, role: &str) -> &Model {
        match role {
            "main" => self.main.as_ref(),
            "reinforcement" => self.reinforce.as_ref(),
            "face" => self.faces.as_ref(),
            _ => panic!("unknown role: {role}"),
        }
        .expect("scripted model")
    }
    /// The calls to a model of any role.
    pub fn calls(&self, role: &str) -> Vec<Call> {
        match role {
            "pose" => self.pose.as_ref().expect("scripted model").calls(),
            "depth" => self.depth.as_ref().expect("scripted model").calls(),
            _ => self.model(role).calls(),
        }
    }
    /// Answers a sense probe with the scripted senses, as the router does.
    pub fn look_sense(&self, probe: &Probe) -> Result<Reading, Failure> {
        self.senses().look(
            self.image.as_ref().unwrap(),
            &probe.sense().expect("a sense probe"),
        )
    }
    /// How many times the scripted models were called, in any role.
    pub fn sense_calls(&self) -> usize {
        [
            self.main.as_ref().map(|m| m.calls().len()),
            self.reinforce.as_ref().map(|m| m.calls().len()),
            self.faces.as_ref().map(|m| m.calls().len()),
            self.pose.as_ref().map(|m| m.calls().len()),
            self.depth.as_ref().map(|m| m.calls().len()),
        ]
        .into_iter()
        .flatten()
        .sum()
    }
    /// The shell's senses with the scripted models present.
    pub fn senses(&self) -> sense::Senses {
        let main = Slow {
            model: self.model("main").clone(),
            delay: Duration::from_millis(self.main_delay_ms),
        };
        let mut senses = sense::Senses::new(main);
        if let Some(model) = &self.reinforce {
            senses = senses.with_reinforce(model.clone());
        }
        if let Some(model) = &self.faces {
            senses = senses.with_faces(model.clone());
        }
        if let Some(model) = &self.pose {
            senses = senses.with_pose(model.clone());
        }
        if let Some(model) = &self.depth {
            senses = senses.with_depth(model.clone());
        }
        senses
    }
    pub fn boxes(&self) -> &[Detection] {
        match self.reading.as_ref().unwrap() {
            Ok(Reading::Bodies(boxes) | Reading::Faces(boxes)) => boxes,
            other => panic!("there are no boxes: {other:?}"),
        }
    }
}

#[given(expr = "an image of {int}×{int}")]
fn image(w: &mut Shell, width: u32, height: u32) {
    w.image = Some(DynamicImage::new_rgb8(width, height));
}

#[given(regex = r"^a scripted (main|reinforcement|face) model$")]
fn model(w: &mut Shell, role: String) {
    let port = Some(Model::default());
    match role.as_str() {
        "main" => w.main = port,
        "reinforcement" => w.reinforce = port,
        "face" => w.faces = port,
        _ => unreachable!(),
    }
}

#[given(regex = r"^there is no (reinforcement|face) model$")]
fn absent(w: &mut Shell, role: String) {
    match role.as_str() {
        "reinforcement" => w.reinforce = None,
        "face" => w.faces = None,
        _ => unreachable!(),
    }
}

#[given(regex = r"^the (reinforcement|face) model fails$")]
fn broken(w: &mut Shell, role: String) {
    w.model(&role).fails();
}

#[given(
    regex = r"^the (main|reinforcement|face) model sees (?:a person|a face) at (\d+),(\d+) to (\d+),(\d+) of the crop$"
)]
fn sees(w: &mut Shell, role: String, x0: f32, y0: f32, x1: f32, y1: f32) {
    w.model(&role).sees(vec![Detection {
        bbox: Rect::new(x0, y0, x1, y1),
        confidence: 0.9,
        label: if role != "face" {
            Label::Person
        } else {
            Label::Other
        },
    }]);
}

#[given("the reinforcement model sees a person of confidence 0.10 and a chair")]
fn all_boxes(w: &mut Shell) {
    w.model("reinforcement").sees(vec![
        Detection {
            bbox: Rect::new(0.0, 0.0, 10.0, 10.0),
            confidence: 0.1,
            label: Label::Person,
        },
        Detection {
            bbox: Rect::new(20.0, 20.0, 30.0, 30.0),
            confidence: 0.9,
            label: Label::Other,
        },
    ]);
}

#[given("the face model sees 3 faces, one of confidence 0.05")]
fn all_faces(w: &mut Shell) {
    w.model("face").sees(
        [0.05, 0.3, 0.9]
            .into_iter()
            .map(|confidence| Detection {
                bbox: Rect::new(10.0, 10.0, 20.0, 20.0),
                confidence,
                label: Label::Other,
            })
            .collect(),
    );
}

#[when(
    regex = r"^the core asks for faces (at the headboard|on the subject) (\d+),(\d+) to (\d+),(\d+)$"
)]
fn look_faces(w: &mut Shell, target: String, x0: f32, y0: f32, x1: f32, y1: f32) {
    let mut senses = sense::Senses::new(w.model("main").clone());
    if let Some(model) = &w.faces {
        senses = senses.with_faces(model.clone());
    }
    let rect = Rect::new(x0, y0, x1, y1);
    let region = if target == "at the headboard" {
        FaceRegion::Headboard(rect)
    } else {
        FaceRegion::Subject(rect)
    };
    let reading = senses.look(w.image.as_ref().unwrap(), &SenseProbe::Faces { region });
    if let Ok(reading) = &reading {
        assert!(matches!(reading, Reading::Faces(_)));
    }
    w.reading = Some(reading);
}

#[when(
    regex = r"^the core asks the main detector to look at the region (\d+),(\d+) to (\d+),(\d+)$"
)]
fn detect_region(w: &mut Shell, x0: f32, y0: f32, x1: f32, y1: f32) {
    let mut senses = sense::Senses::new(w.model("main").clone());
    w.reading = Some(senses.look(
        w.image.as_ref().unwrap(),
        &SenseProbe::Detect {
            region: Some(Rect::new(x0, y0, x1, y1)),
        },
    ));
}

#[when(regex = r"^the core asks to reinforce the region (\d+),(\d+) to (\d+),(\d+)$")]
fn reinforce(w: &mut Shell, x0: f32, y0: f32, x1: f32, y1: f32) {
    let mut senses = sense::Senses::new(w.model("main").clone());
    if let Some(port) = &w.reinforce {
        senses = senses.with_reinforce(port.clone());
    }
    w.reading = Some(senses.look(
        w.image.as_ref().unwrap(),
        &SenseProbe::Reinforce {
            region: Rect::new(x0, y0, x1, y1),
        },
    ));
}

#[then(
    regex = r"^the (main|reinforcement|face|pose|depth) model received an image of (\d+)×(\d+)$"
)]
fn received(w: &mut Shell, role: String, width: u32, height: u32) {
    assert_eq!(
        w.calls(&role).iter().map(|c| c.size).collect::<Vec<_>>(),
        vec![(width, height)]
    );
}

#[then(regex = r"^the (main|reinforcement|face|pose) model was not called$")]
fn not_called(w: &mut Shell, role: String) {
    assert!(w.calls(&role).is_empty());
}

#[then(regex = r"^the reading carries (?:a person|a face) at (\d+),(\d+) to (\d+),(\d+)$")]
fn mapped(w: &mut Shell, x0: f32, y0: f32, x1: f32, y1: f32) {
    assert_eq!(w.boxes().len(), 1);
    assert_eq!(w.boxes()[0].bbox, Rect::new(x0, y0, x1, y1));
}

#[then(expr = "the reading carries {int} boxes")]
fn box_count(w: &mut Shell, count: usize) {
    assert_eq!(w.boxes().len(), count);
}

#[then("the reading fails because of the model")]
fn failed(w: &mut Shell) {
    assert_eq!(w.reading, Some(Err(Failure::Model)));
}

#[given(expr = "a profile with {string} of model {string} and padding {float}")]
fn reinforce_profile(w: &mut Shell, section: String, model: String, padding: f32) {
    w.parse_config(&format!(
        "\n{section}\nmodel = \"{model}\"\nbed_padding = {padding}\n"
    ));
}

#[given(
    expr = "a profile with {string} with backend confidence {float} and policy minimum confidence {float}"
)]
fn invalid_reinforce(w: &mut Shell, section: String, backend: f32, minimum: f32) {
    if section == "[senses.detect]" {
        let profile = include_str!("../support/profile.toml").replace(
            "[senses.detect]",
            &format!("[senses.detect]\nbackend_confidence = {backend}"),
        );
        w.config = Some(
            detector::Config::parse(&format!(
                "{profile}\n[policy]\nmin_confidence = {minimum}\n"
            ))
            .map_err(|e| e.to_string()),
        );
        return;
    }
    w.parse_config(&format!("\n[policy]\nmin_confidence = {minimum}\n{section}\nmodel = \"yolo26l.onnx\"\nbackend_confidence = {backend}\nbed_padding = 0.15\n"));
}

#[then("the profile accepts the main detector's threshold")]
fn main_valid(w: &mut Shell) {
    assert_eq!(
        w.config().senses.detect.backend_confidence,
        w.config().policy.min_confidence
    );
}

#[then(expr = "the core policy reinforces the bed grown by {float}")]
fn reinforce_policy(w: &mut Shell, padding: f32) {
    assert_eq!(
        w.config()
            .policy()
            .expect("a valid profile")
            .reinforce
            .unwrap()
            .bed_padding
            .get(),
        padding
    );
}

#[then(expr = "the profile is rejected with a reason that names {string}")]
fn rejected_profile(w: &mut Shell, field: String) {
    let error = w.config.as_ref().unwrap().as_ref().unwrap_err();
    assert!(error.contains(&field), "{error}");
}

#[then("the profile is accepted")]
fn accepted_profile(w: &mut Shell) {
    if let Err(error) = w.config.as_ref().unwrap() {
        panic!("el perfil se rechazo: {error}");
    }
}

#[given(expr = "a profile without {string}")]
fn no_section(w: &mut Shell, _section: String) {
    w.parse_config("");
}

#[then("the profile hash is the baseline one")]
fn legacy_hash(w: &mut Shell) {
    assert_eq!(
        w.config().profile().unwrap().hash.0,
        "sha256:239a5803f0c15901"
    );
}

#[then(expr = "the published profile names the senses {string}")]
fn published_roles(w: &mut Shell, expected: String) {
    let names = w
        .config()
        .profile()
        .unwrap()
        .senses
        .into_iter()
        .map(|s| {
            serde_json::to_value(s.kind)
                .unwrap()
                .as_str()
                .unwrap()
                .to_owned()
        })
        .collect::<Vec<_>>();
    assert_eq!(names.join(", "), expected);
}

#[given(expr = "a profile with {string} of model {string} and minimum confidence {float}")]
fn faces_profile(w: &mut Shell, section: String, model: String, minimum: f32) {
    w.parse_config(&format!(
        "\n{section}\nmodel = \"{model}\"\nmin_confidence = {minimum}\n"
    ));
}

#[given(
    expr = "a profile with {string} of model {string}, minimum confidence {float} and headboard margin {float}"
)]
fn faces_profile_with_margin(
    w: &mut Shell,
    section: String,
    model: String,
    minimum: f32,
    pad: f32,
) {
    w.parse_config(&format!(
        "\n{section}\nmodel = \"{model}\"\nmin_confidence = {minimum}\nheadboard_pad = {pad}\n"
    ));
}

#[given(expr = "a profile with {string} and {string}")]
fn both_profiles(w: &mut Shell, reinforce: String, faces: String) {
    w.parse_config(&format!("\n{reinforce}\nmodel = \"yolo26l.onnx\"\nbed_padding = 0.15\n{faces}\nmodel = \"face/yolov12l-face.onnx\"\nmin_confidence = 0.30\n"));
}

#[given(
    expr = "a profile with {string} with backend confidence {float} and minimum confidence {float}"
)]
fn invalid_faces(w: &mut Shell, section: String, backend: f32, minimum: f32) {
    w.parse_config(&format!("\n{section}\nmodel = \"face/yolov12l-face.onnx\"\nbackend_confidence = {backend}\nmin_confidence = {minimum}\n"));
}

#[then(expr = "the core policy accepts faces from {float}")]
fn faces_policy(w: &mut Shell, minimum: f32) {
    assert_eq!(
        w.config()
            .policy()
            .expect("a valid profile")
            .faces
            .unwrap()
            .min_confidence
            .get(),
        minimum
    );
}

#[then(expr = "the core policy looks for faces at the headboard grown by {float}")]
fn faces_margin(w: &mut Shell, pad: f32) {
    assert_eq!(
        w.config()
            .policy()
            .expect("a valid profile")
            .faces
            .unwrap()
            .headboard_pad
            .get(),
        pad
    );
}
