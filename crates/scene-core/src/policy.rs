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
}

/// Learn the depth of the empty bed.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CalibrationPolicy {
    /// Delay between failed attempts, in source seconds (rule 9).
    pub retry_secs: f64,
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
    /// `None`: there is no pose model.
    pub enrich: Option<EnrichPolicy>,
    /// `None`: the bed is not calibrated.
    pub calibration: Option<CalibrationPolicy>,
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
            enrich: None,
            calibration: None,
        }
    }
}
