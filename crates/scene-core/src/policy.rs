//! The policies the core believes with. They come from the profiles; the core
//! reads them from nowhere (rule 8).

/// Which body counts as evidence, and in which zone.
#[derive(Debug, Clone, PartialEq)]
pub struct AdmissionPolicy {
    /// Minimum confidence of a person detection.
    pub min_confidence: f32,
    /// Two boxes with an IoU from this value up are the same body.
    pub same_body_iou: f32,
    /// A body on the floor is "near the bed" if its box touches the bed grown
    /// by this fraction of its width and height on each side.
    pub near_bed_margin: f32,
}

/// Look at the bed again with a finer detector when someone is lost.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ReinforcePolicy {
    /// How much the bed grows, as a fraction of its width and height, on each side.
    pub bed_padding: f32,
}

/// Look at the bed up close with the main detector while the resident is alone
/// in bed: the same model, more pixels per body.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FocusPolicy {
    /// How much the bed grows, as a fraction of its width and height, on each
    /// side. It is not reinforcement's: the focus must reach whoever stands
    /// next to the bed.
    pub pad: f32,
    /// Source seconds after which the whole frame is looked at again, even if
    /// the focus confirms: whoever comes in far from the bed is seen at most
    /// this late (rule 9). It must be no longer than staff take to walk from
    /// the door to the bed: then they are always seen on the floor first and
    /// layer 0 knows where they came from (rule 6).
    pub wide_every_secs: f64,
}

/// How long a new count must hold for the zones to change: the stabilization
/// against flicker. It is measured in source time, not in samples: the
/// same policy holds at any camera cadence.
#[derive(Debug, Clone, PartialEq)]
pub struct Persistence {
    /// Source seconds the new count must hold, from the first observation that
    /// saw it. Zero: a single observation is enough.
    pub dwell_secs: f64,
}

/// Which face counts as evidence.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FacePolicy {
    /// Below this score a face does not count.
    pub min_confidence: f32,
}

/// The subject's pose and depth, only when worth it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EnrichPolicy {
    /// Centroid shift, as a fraction of the reference box diagonal, from which
    /// the subject moved.
    pub move_threshold: f32,
    /// Is there a depth model?
    pub depth: bool,
    /// Minimum score of a pose keypoint to measure depth at it.
    pub min_keypoint_score: f32,
    /// Relative disparity between top and bottom within which the posture is
    /// "sitting".
    pub posture_tolerance: f32,
    /// Age from which a still pose is requested again, in source seconds.
    /// `None`: while still, it holds forever.
    pub max_pose_age_secs: Option<f64>,
    /// Margin around the subject's box where the pose is looked for, as a
    /// fraction of its width and height on each side: the pose model needs
    /// the head, hands and feet the detector's box cuts. `0`: the box itself.
    pub crop_pad: f32,
}

/// Learn the depth of the empty bed.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CalibrationPolicy {
    /// Delay between failed attempts, in source seconds (rule 9).
    pub retry_secs: f64,
}

/// Follow the resident they left lying: layer 1, the bed posture (R18, 1806).
/// Each state counts once it held its own source time (rule 9); zero, at
/// once. The profile sets them per camera (1804).
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct BedPosturePolicy {
    /// Source seconds lying must hold, to arm it or to come back to it.
    pub dwell_lying_secs: f64,
    /// Source seconds sitting up must hold.
    pub dwell_sitting_up_secs: f64,
    /// Source seconds getting out must hold: the one to report in time.
    pub dwell_getting_out_secs: f64,
}

/// The policies of one camera. Optional senses start switched off.
#[derive(Debug, Clone, PartialEq)]
pub struct Policy {
    /// Which body counts, and where.
    pub admission: AdmissionPolicy,
    /// How long a new count must hold.
    pub persistence: Persistence,
    /// `None`: there is no face detector.
    pub faces: Option<FacePolicy>,
    /// `None`: there is no reinforcement detector.
    pub reinforce: Option<ReinforcePolicy>,
    /// `None`: the main detector always looks at the whole frame.
    pub focus: Option<FocusPolicy>,
    /// `None`: there is no pose model.
    pub enrich: Option<EnrichPolicy>,
    /// `None`: the bed is not calibrated.
    pub calibration: Option<CalibrationPolicy>,
    /// Where depth is measured, for the subject and for the empty bed alike:
    /// `None`, the whole frame; a pad, the bed's box grown by that fraction of
    /// its width and height on each side, centred on the bed. The depth is
    /// relative to what the model sees, so both measure through the same crop.
    pub depth_pad: Option<f32>,
    /// `None`: layer 1 does not exist; nothing follows the bed posture.
    pub bed_posture: Option<BedPosturePolicy>,
    /// `None`: there is no verifier; nothing is verified.
    pub verify: Option<VerifyPolicy>,
}

/// What a camera verifies with the slow verifier (book 4; white book, volume
/// III). Each stream verifies only what matters to it: what is `None` is
/// neither asked nor paid for.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VerifyPolicy {
    /// `None`: a body held in bed is not verified.
    pub held: Option<HeldVerifyPolicy>,
    /// Source seconds a request waits for its answer before it expires and
    /// may be asked again; `None`: it waits as long as it takes. A new
    /// session expires it: source time is not compared across sessions.
    pub timeout_secs: Option<f64>,
    /// How many requests per hour of source time, per camera; `None`: no
    /// limit. The budget restarts with a new session.
    pub max_per_hour: Option<u32>,
}

/// Verify a body held in bed: is the belief still backed by an observation?
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HeldVerifyPolicy {
    /// Source seconds from `held_in_bed` until it is asked about: the cheap
    /// senses (reinforcement, faces) go first.
    pub after_secs: f64,
    /// Source seconds a check holds, from the sample it looked at; then the
    /// held body is asked about again (D5). `None`: it is asked once.
    pub valid_secs: Option<f64>,
}

impl Default for Policy {
    fn default() -> Self {
        Self {
            admission: AdmissionPolicy {
                min_confidence: 0.5,
                same_body_iou: 0.5,
                near_bed_margin: 0.10,
            },
            persistence: Persistence { dwell_secs: 2.0 },
            faces: None,
            reinforce: None,
            focus: None,
            enrich: None,
            calibration: None,
            depth_pad: None,
            bed_posture: None,
            verify: None,
        }
    }
}
