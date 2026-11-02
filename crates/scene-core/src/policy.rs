//! The policies the core believes with. They come from the profiles; the core
//! reads them from nowhere (rule 8). Durations are [`Seconds`] and scores that
//! are confidences or thresholds are [`Confidence`] (ADR unidades-en-el-tipo).

use crate::units::{
    Confidence, Degrees, DiagonalFraction, Extension, FeetMargin, Metres, PositiveMetres,
    Proportion, Ratio, RelativeDepth, Seconds,
};

/// Which body counts as evidence, and in which zone.
#[derive(Debug, Clone, PartialEq)]
pub struct AdmissionPolicy {
    /// Minimum confidence of a person detection.
    pub min_confidence: Confidence,
    /// Two boxes with an IoU from this value up are the same body.
    pub same_body_iou: Proportion,
    /// A body on the floor is "near the bed" if its box touches the bed grown
    /// by this fraction of its width and height on each side.
    pub near_bed_margin: Extension,
    /// `Some`: the bed's zones in its own coordinates, for a camera that sees
    /// the bed at an angle. Near the bed is the bed grown by
    /// `near_bed_margin` across and along it, not its bounding box grown; and
    /// the person's box is in bed when at least this share of it falls in the
    /// bed. A body lying on the diagonal of a narrow bed spills its box's
    /// corners far past the rails: corners say nothing there, the share does.
    /// `None`: in the image's axes, the box's four corners in the bed.
    pub bed_frame: Option<Ratio>,
}

/// Look at the bed again with a finer detector when someone is lost.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ReinforcePolicy {
    /// How much the bed grows, as a fraction of its width and height, on each side.
    pub bed_padding: Extension,
}

/// Look at the bed up close with the main detector while the resident is alone
/// in bed: the same model, more pixels per body.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FocusPolicy {
    /// How much the bed grows, as a fraction of its width and height, on each
    /// side. It is not reinforcement's: the focus must reach whoever stands
    /// next to the bed.
    pub pad: Extension,
    /// Source seconds after which the whole frame is looked at again, even if
    /// the focus confirms: whoever comes in far from the bed is seen at most
    /// this late (rule 9). It must be no longer than staff take to walk from
    /// the door to the bed: then they are always seen on the floor first and
    /// layer 0 knows where they came from (rule 6).
    pub wide_every_secs: Seconds,
}

/// How long a new count must hold for the zones to change: the stabilization
/// against flicker. It is measured in source time, not in samples: the
/// same policy holds at any camera cadence.
#[derive(Debug, Clone, PartialEq)]
pub struct Persistence {
    /// Source seconds the new count must hold, from the first observation that
    /// saw it. Zero: a single observation is enough.
    pub dwell_secs: Seconds,
}

/// Which face counts as evidence.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FacePolicy {
    /// Below this score a face does not count.
    pub min_confidence: Confidence,
    /// With nobody in sight, the headboard is grown by this fraction of its
    /// width and height on each side to look for faces: a head that left it
    /// is still found. Where the head is is decided against the exact one.
    pub headboard_pad: Extension,
}

/// The subject's pose and depth, only when worth it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EnrichPolicy {
    /// Centroid shift, as a fraction of the reference box diagonal, from which
    /// the subject moved.
    pub move_threshold: DiagonalFraction,
    /// Is there a depth model?
    pub depth: bool,
    /// Minimum score of a pose keypoint to measure depth at it.
    pub min_keypoint_score: Confidence,
    /// Relative disparity between top and bottom within which the posture is
    /// "sitting".
    pub posture_tolerance: RelativeDepth,
    /// Age from which a still pose is requested again, in source seconds.
    /// `None`: while still, it holds forever.
    pub max_pose_age_secs: Option<Seconds>,
    /// Margin around the subject's box where the pose is looked for, as a
    /// fraction of its width and height on each side: the pose model needs
    /// the head, hands and feet the detector's box cuts. `0`: the box itself.
    pub crop_pad: Extension,
}

/// Learn the depth of the empty bed.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CalibrationPolicy {
    /// Delay between failed attempts, in source seconds (rule 9).
    pub retry_secs: Seconds,
}

/// Follow the resident they left lying: layer 1, the bed posture (R18, 1806).
/// Each state counts once it held its own source time (rule 9); zero, at
/// once. The profile sets them per camera (1804).
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct BedPosturePolicy {
    /// Source seconds lying must hold, to arm it or to come back to it.
    pub dwell_lying_secs: Seconds,
    /// Source seconds sitting up must hold.
    pub dwell_sitting_up_secs: Seconds,
    /// Source seconds getting out must hold: the one to report in time.
    pub dwell_getting_out_secs: Seconds,
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
    pub depth_pad: Option<Extension>,
    /// `None`: layer 1 does not exist; nothing follows the bed posture.
    pub bed_posture: Option<BedPosturePolicy>,
    /// `None`: there is no verifier; nothing is verified.
    pub verify: Option<VerifyPolicy>,
    /// `None`: no shadow is computed or published.
    pub shadow: Option<ShadowPolicy>,
    /// `Some`: the room in two planes; `None`: frontal, the engine as it was.
    pub two_planes: Option<TwoPlanesPolicy>,
    /// Rule 4 bis (white book, volume VII, ch. 05): with someone held in bed,
    /// a body that appears on the floor near the bed, not from the door, is
    /// the held one who got up. `false`: a held body leaves only reappearing
    /// in bed, as it always did.
    pub held_left: bool,
    /// Rule 5, refined (white book, volume VII, ch. 17): a body that appeared
    /// near the bed and then walked away did not come from the door. `false`:
    /// whoever was seen far while the count stabilized came from the door, as
    /// it always did.
    pub near_first: bool,
}

/// The geometry's shadow (white book, volume VII, ch. 11): estimates of a
/// geometry that does not decide yet, published next to what is decided. It
/// changes no decision.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ShadowPolicy {
    /// Near the bed, in metres: the footprint grown by this much in its own
    /// coordinates. It needs the bed's size.
    pub near_m: PositiveMetres,
    /// A body's support within this many metres of the bed is in bed for the
    /// shadow; past it, on the floor.
    pub support_margin_m: Metres,
}

/// The room measured in two planes, the mattress and the bed's footprint on
/// the floor (white book, volume VII, ch. 14): the profile chooses it for a
/// camera that sees the bed on the diagonal. A box is in bed when enough of
/// it falls over the mattress and its support (the centre of its lower edge)
/// rests close enough to the footprint; on the floor, near the bed when its
/// support is within `near_m`. It needs the footprint and the bed's size.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TwoPlanesPolicy {
    /// The share of the box over the mattress, from this on, to be in bed.
    pub share: Ratio,
    /// The support, at most this many metres from the footprint, to be in bed.
    pub support_m: Metres,
    /// On the floor, the support within this many metres of the footprint is
    /// near the bed.
    pub near_m: PositiveMetres,
    /// `Some`: a doubtful body (on the floor by its support, with enough of
    /// its box over the mattress) has its pose asked before believing, and a
    /// knee bent to this many degrees or less puts it in bed (white book,
    /// volume VII, ch. 15). `None`: the box alone decides.
    pub knee_bent_deg: Option<Degrees>,
}

/// What a camera verifies with the slow verifier (book 4; white book, volume
/// III). Each stream verifies only what matters to it: what is `None` is
/// neither asked nor paid for.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct VerifyPolicy {
    /// `None`: a body held in bed is not verified.
    pub held: Option<HeldVerifyPolicy>,
    /// Source seconds a request waits for its answer before it expires and
    /// may be asked again; `None`: it waits as long as it takes. A new
    /// session expires it: source time is not compared across sessions.
    pub timeout_secs: Option<Seconds>,
    /// How many requests per hour of source time, per camera; `None`: no
    /// limit. The budget restarts with a new session.
    pub max_per_hour: Option<u32>,
    /// The camera's grey band of the disparity: inside it, lying or upright
    /// is doubtful, layer 1 reads nothing new and the verifier is asked
    /// (P1). `None`: the disparity is never doubtful.
    pub doubtful_disparity: Option<Band>,
    /// The camera's grey band of the feet, as a fraction of the bed's width:
    /// an ankle that close to a side, inside or outside, is doubtful (P2, P3).
    /// Without an answer in time it is taken as out, on the safe side.
    /// `None`: the feet are never doubtful.
    pub doubtful_feet_margin: Option<FeetMargin>,
    /// How many crops each question sends, and how far apart (white book,
    /// volume III, ch. 12). By default one: the looked sample's.
    pub frames: Frames,
    /// Which kept crops are sharp enough to count; `None`: every one counts.
    pub sharpness: Option<SharpnessPolicy>,
}

/// A crop counts for a burst if it is sharp enough, relative to the camera's
/// own recent crops: the same rule serves colour and infrared, day and night
/// (white book, volume III, ch. 12). The shell measures; the core decides.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SharpnessPolicy {
    /// Sharp: at least this fraction of the median of the last crops.
    pub min_ratio: Ratio,
    /// How many of the camera's last kept crops the median looks at.
    pub window: u16,
    /// Source seconds from the burst's first crop: then it asks with the
    /// sharpest it has, rather than wait for sharp ones forever.
    pub max_wait_secs: Seconds,
}

/// A burst: how many crops a question sends and the source seconds between
/// them. The loop decides it, per question; the verifier answers what it is
/// sent (V15).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Burst {
    /// At least one: the sample that opened the doubt.
    pub frames: u8,
    /// Source seconds from one kept crop to the next.
    pub every_secs: Seconds,
}

impl Burst {
    /// One crop, the looked sample's: what a question sent before bursts.
    pub const ONE: Self = Self {
        frames: 1,
        every_secs: Seconds::ZERO,
    };
}

impl Default for Burst {
    fn default() -> Self {
        Self::ONE
    }
}

/// The burst of each question.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Frames {
    /// `held_in_bed`.
    pub held: Burst,
    /// `sitting_in_bed`.
    pub sitting: Burst,
    /// `leaving_bed`.
    pub leaving: Burst,
}

impl Frames {
    /// The burst of a question.
    pub fn of(&self, question: crate::look::Question) -> Burst {
        match question {
            crate::look::Question::HeldInBed => self.held,
            crate::look::Question::SittingInBed => self.sitting,
            crate::look::Question::LeavingBed => self.leaving,
        }
    }
}

/// Two thresholds: a value between them, both included, is doubtful. The
/// quantizer of a grey band (white book, volume III, ch. 10).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Band {
    /// The lower threshold.
    pub lo: RelativeDepth,
    /// The upper threshold.
    pub hi: RelativeDepth,
}

impl Band {
    /// Is the value in the band?
    pub fn contains(self, value: f32) -> bool {
        (self.lo.get()..=self.hi.get()).contains(&value)
    }
}

/// Verify a body held in bed: is the belief still backed by an observation?
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HeldVerifyPolicy {
    /// Source seconds from `held_in_bed` until it is asked about: the cheap
    /// senses (reinforcement, faces) go first.
    pub after_secs: Seconds,
    /// Source seconds a check holds, from the sample it looked at; then the
    /// held body is asked about again (D5). `None`: it is asked once.
    pub valid_secs: Option<Seconds>,
}

impl Default for Policy {
    fn default() -> Self {
        Self {
            admission: AdmissionPolicy {
                min_confidence: Confidence::HALF,
                same_body_iou: Proportion::HALF,
                near_bed_margin: Extension::TENTH,
                bed_frame: None,
            },
            persistence: Persistence {
                dwell_secs: Seconds::TWO,
            },
            faces: None,
            reinforce: None,
            focus: None,
            enrich: None,
            calibration: None,
            depth_pad: None,
            bed_posture: None,
            verify: None,
            shadow: None,
            two_planes: None,
            held_left: false,
            near_first: false,
        }
    }
}
