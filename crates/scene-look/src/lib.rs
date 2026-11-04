//! The loop's vocabulary: what the core asks to look at and what the shell
//! returns. Never pixels (rule 8).

use scene_contract::{BedPostureState, Keypoint, SampleRef};
use scene_geom::{Point, Quad, Rect};

/// A direction of the image, unit length, that a pose's crop turns to point
/// up (F2.4): the core picks it (the bed's long side, a face in the box); the
/// shell only turns. The vector is normalized on construction.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Up {
    dx: f32,
    dy: f32,
}

impl Up {
    /// The direction of (dx, dy). A zero vector has no direction: `None`.
    pub fn new(dx: f32, dy: f32) -> Option<Self> {
        let length = dx.hypot(dy);
        (length > 0.0).then(|| Self {
            dx: dx / length,
            dy: dy / length,
        })
    }

    /// The unit vector's x component.
    pub fn dx(&self) -> f32 {
        self.dx
    }

    /// The unit vector's y component.
    pub fn dy(&self) -> f32 {
        self.dy
    }
}

/// What the core asks to look at. Whole-frame coordinates.
#[derive(Debug, Clone, PartialEq)]
pub enum Probe {
    /// The main person detector over a region (`None`: the whole frame).
    Detect {
        /// Where; `None`: the whole frame.
        region: Option<Rect>,
    },
    /// The face detector over the subject's box or the headboard.
    Faces {
        /// Where faces are looked for.
        region: FaceRegion,
    },
    /// The finer detector over the expanded bed.
    Reinforce {
        /// The bed grown by the policy's padding.
        region: Rect,
    },
    /// The pose over a body's box: the subject's, or a doubtful body's before
    /// believing (white book, volume VII, ch. 15). The sense does the same.
    Pose {
        /// Where to look: the body's box grown by the pose policy's margin.
        bbox: Rect,
        /// Whose pose it is.
        purpose: PosePurpose,
        /// The direction to turn the crop to, so it points up; `None`: no turn.
        up: Option<Up>,
    },
    /// The depth at these points.
    Depth {
        /// The subject or the empty bed.
        purpose: DepthPurpose,
        /// Where to measure; the reading keeps this order.
        points: Vec<Point>,
        /// What the model sees; `None`: the whole frame. The points stay in
        /// whole-frame coordinates.
        region: Option<Rect>,
    },
    /// Keep this sample's crop for a burst (white book, volume III, ch. 12): the
    /// shell crops it and holds it under the ticket, to send it with the
    /// `Verify` that completes the burst. Nothing is asked yet.
    Keep {
        /// The burst it belongs to.
        ticket: Ticket,
        /// The crop's bounding box, in whole-frame pixels.
        region: Rect,
    },
    /// The slow verifier: a closed question about a crop. With `crops`, the
    /// crops kept for its ticket that go, by the order they were kept; empty,
    /// this sample's own crop. The shell only takes
    /// the request (`Reading::Dispatched`); the answer comes later, through
    /// `Monitor::verified`, as evidence of the
    /// sample it looked at (book 4).
    Verify {
        /// What the answer will be about.
        ticket: Ticket,
        /// The crop's bounding box, in whole-frame pixels (the ledger's region).
        region: Rect,
        /// The bed grown by the crop's margin, in `Quad`'s order (headboard
        /// left, headboard right, foot right, foot left): the shell rectifies
        /// the crop to it, the headboard always up.
        bed: Quad,
        /// The closed question.
        question: Question,
        /// Which kept crops go, by the order they were kept; empty: this
        /// sample's crop.
        crops: Vec<u8>,
    },
}

/// A request to the verifier: which, the sample it looks at and what it asks
/// about. A late answer is applied only if that same thing still holds.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Ticket {
    /// Unique per monitor, in the order they were issued.
    pub id: u64,
    /// The sample whose crop is asked about: the answer is evidence of it.
    pub looked: SampleRef,
    /// What is asked about.
    pub target: Target,
}

/// What a verification asks about, by its identity.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Target {
    /// A body held in bed, by the last sample it was seen in.
    Held {
        /// The held body's `last_seen`.
        since: SampleRef,
    },
    /// The doubtful feet of the looked sample, while layer 1 holds the same
    /// stable state.
    Feet {
        /// Layer 1's stable state when it was asked.
        stable: BedPostureState,
    },
    /// The doubtful disparity of the looked sample, while layer 1 holds the
    /// same stable state.
    Disparity {
        /// Layer 1's stable state when it was asked.
        stable: BedPostureState,
    },
}

/// A closed question to the verifier: yes, no or unsure. Questions are
/// versioned, like a rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Question {
    /// Is there a person lying in this bed, even if covered?
    HeldInBed,
    /// Is the person in this bed sitting up rather than lying down?
    SittingInBed,
    /// Is the person getting out of the bed, with a leg over the side?
    LeavingBed,
}

impl Question {
    /// Its versioned id, which the verifier and the ledger use.
    pub fn id(self) -> &'static str {
        match self {
            Self::HeldInBed => "held_in_bed@1",
            Self::SittingInBed => "sitting_in_bed@1",
            Self::LeavingBed => "leaving_bed@1",
        }
    }
}

/// The verifier's answer to a closed question.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Answer {
    /// Yes.
    Yes,
    /// No.
    No,
    /// It could not tell.
    Unsure,
}

/// An answer to a ticket, as the shell delivers it. `answer: None` is no answer
/// at all: the shell's wall-clock deadline passed. The core takes it as the
/// source-time expiry of the request (`decide` with no answer), so an answer
/// that the verifier did not give is never read as a «no sé».
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Verified {
    /// The ticket it answers.
    pub ticket: Ticket,
    /// What the verifier said; `None`: nothing came back in time.
    pub answer: Option<Answer>,
}

/// Whose pose is asked for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PosePurpose {
    /// The subject's, after believing: the second level and layer 1.
    Subject,
    /// A doubtful body's, before believing: the room places it with it.
    Body,
}

/// What the depth is measured for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DepthPurpose {
    /// The subject's pose keypoints.
    Subject,
    /// The bed's points, to calibrate it.
    Bed,
}

impl Probe {
    /// The look this probe is recorded as in the ledger. It reads the one
    /// mapping, [`recorded`].
    pub fn kind(&self) -> LookKind {
        recorded(self.route())
    }

    /// The region recorded in the ledger.
    pub fn region(&self) -> Option<Rect> {
        match self {
            Probe::Detect { region } => *region,
            Probe::Faces { region } => Some(region.rect()),
            Probe::Reinforce { region } => Some(*region),
            Probe::Pose { bbox, .. } => Some(*bbox),
            Probe::Depth { region, .. } => *region,
            Probe::Verify { region, .. } | Probe::Keep { region, .. } => Some(*region),
        }
    }

    /// Who answers this probe, and with which view. The one decision of the
    /// protocol: every probe has exactly one route (ADR 2026-11-02,
    /// quien-responde-cada-probe). The match names all seven variants.
    pub fn route(&self) -> Route<'_> {
        match self {
            Probe::Detect { region } => Route::Sense(SenseProbe::Detect { region: *region }),
            Probe::Faces { region } => Route::Sense(SenseProbe::Faces { region: *region }),
            Probe::Reinforce { region } => Route::Sense(SenseProbe::Reinforce { region: *region }),
            Probe::Pose { bbox, purpose, up } => Route::Sense(SenseProbe::Pose {
                bbox: *bbox,
                purpose: *purpose,
                up: *up,
            }),
            Probe::Depth {
                purpose,
                points,
                region,
            } => Route::Sense(SenseProbe::Depth {
                purpose: *purpose,
                points: points.as_slice(),
                region: *region,
            }),
            Probe::Verify {
                ticket,
                region,
                bed,
                question,
                crops,
            } => Route::Verifier(VerifierProbe::Verify {
                ticket: *ticket,
                region: *region,
                bed: *bed,
                question: *question,
                crops: crops.as_slice(),
            }),
            Probe::Keep { ticket, region } => Route::Verifier(VerifierProbe::Keep {
                ticket: *ticket,
                region: *region,
            }),
        }
    }

    /// The sense's view, if a sense answers this probe: a reading of
    /// [`Probe::route`], for the callers that only want the sense side.
    pub fn sense(&self) -> Option<SenseProbe<'_>> {
        match self.route() {
            Route::Sense(sense) => Some(sense),
            Route::Verifier(_) => None,
        }
    }
}

/// A probe a local sense answers: the five kinds of [`Probe`] that are not the
/// verifier's. A sense port matches it exhaustively, with no wildcard, so a
/// new sense is a new variant the compiler points at (ADR 2026-11-02,
/// quien-responde-cada-probe). The fields are those of the `Probe` it views.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SenseProbe<'a> {
    /// The main person detector over a region.
    Detect {
        /// Where; `None`: the whole frame.
        region: Option<Rect>,
    },
    /// The face detector over the subject's box or the headboard.
    Faces {
        /// Where faces are looked for.
        region: FaceRegion,
    },
    /// The finer detector over the expanded bed.
    Reinforce {
        /// The bed grown by the policy's padding.
        region: Rect,
    },
    /// The pose over a body's box.
    Pose {
        /// Where to look: the body's box grown by the pose policy's margin.
        bbox: Rect,
        /// Whose pose it is.
        purpose: PosePurpose,
        /// The direction to turn the crop to, so it points up; `None`: no turn.
        up: Option<Up>,
    },
    /// The depth at these points.
    Depth {
        /// The subject or the empty bed.
        purpose: DepthPurpose,
        /// Where to measure; the reading keeps this order.
        points: &'a [Point],
        /// What the model sees; `None`: the whole frame.
        region: Option<Rect>,
    },
}

impl SenseProbe<'_> {
    /// The look this probe is recorded as in the ledger, as [`Probe::kind`].
    /// It reads the one mapping, [`recorded`].
    pub fn kind(&self) -> LookKind {
        recorded(Route::Sense(*self))
    }
}

/// The one place a probe's answerer and purpose map to the [`LookKind`] the
/// ledger records the look as (F1c): a sense probe names its sense — the pose
/// and the depth, by their purpose — and a verifier probe names the
/// verifier's two looks. [`Probe::kind`] and [`SenseProbe::kind`] both read
/// it; the match names every variant, with no wildcard (ADR 2026-11-02,
/// quien-responde-cada-probe).
fn recorded(route: Route<'_>) -> LookKind {
    match route {
        Route::Sense(sense) => match sense {
            SenseProbe::Detect { .. } => LookKind::Detect,
            SenseProbe::Faces { .. } => LookKind::Faces,
            SenseProbe::Reinforce { .. } => LookKind::Reinforce,
            SenseProbe::Pose {
                purpose: PosePurpose::Subject,
                ..
            } => LookKind::Pose,
            SenseProbe::Pose {
                purpose: PosePurpose::Body,
                ..
            } => LookKind::PoseBody,
            SenseProbe::Depth {
                purpose: DepthPurpose::Subject,
                ..
            } => LookKind::Depth,
            SenseProbe::Depth {
                purpose: DepthPurpose::Bed,
                ..
            } => LookKind::Calibrate,
        },
        Route::Verifier(verifier) => match verifier {
            VerifierProbe::Verify { .. } => LookKind::Verify,
            VerifierProbe::Keep { .. } => LookKind::Keep,
        },
    }
}

/// A probe the slow verifier answers: the two kinds of [`Probe`] that leave
/// the process. The fields are those of the `Probe` it views; `crops` is the
/// list of kept crops that go, by their order.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum VerifierProbe<'a> {
    /// The slow verifier: a closed question about a crop.
    Verify {
        /// What the answer will be about.
        ticket: Ticket,
        /// The crop's bounding box, in whole-frame pixels.
        region: Rect,
        /// The bed grown by the crop's margin, in `Quad`'s order.
        bed: Quad,
        /// The closed question.
        question: Question,
        /// Which kept crops go, by the order they were kept; empty: this
        /// sample's crop.
        crops: &'a [u8],
    },
    /// Keep this sample's crop for a burst.
    Keep {
        /// The burst it belongs to.
        ticket: Ticket,
        /// The crop's bounding box, in whole-frame pixels.
        region: Rect,
    },
}

/// Who answers a probe, with the view of that answerer. Total: every
/// [`Probe`] has exactly one route, and [`Probe::route`] builds it with a
/// match over all seven variants.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Route<'a> {
    /// A local sense answers.
    Sense(SenseProbe<'a>),
    /// The slow verifier answers.
    Verifier(VerifierProbe<'a>),
}

/// Where to look for faces.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FaceRegion {
    /// The subject's box as the detector saw it.
    Subject(Rect),
    /// The bed's headboard, when no subject is in sight.
    Headboard(Rect),
}

impl FaceRegion {
    /// The rectangle, whichever it is.
    pub fn rect(&self) -> Rect {
        match self {
            FaceRegion::Subject(r) | FaceRegion::Headboard(r) => *r,
        }
    }
}

/// Which class the model detected. The sense returns all of them (rule 6).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Label {
    /// A person.
    Person,
    /// Any other class: the policy discards it, with its reason.
    Other,
}

/// A box a model returned, unfiltered.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Detection {
    /// The box, in whole-frame pixels.
    pub bbox: Rect,
    /// The model's score.
    pub confidence: f32,
    /// The class it detected.
    pub label: Label,
}

/// What the shell read. Mirrors `Probe` one to one.
#[derive(Debug, Clone, PartialEq)]
pub enum Reading {
    /// What a person detector saw (`Detect`, `Reinforce`).
    Bodies(Vec<Detection>),
    /// What the face detector saw.
    Faces(Vec<Detection>),
    /// `None`: the model found no pose.
    Pose(Option<Vec<Keypoint>>),
    /// One depth per requested point; `None` where there was no reading.
    Depth(Vec<Option<f32>>),
    /// The verifier took the request (`Verify`): the answer comes later.
    Dispatched,
    /// The shell kept the crop for its burst (`Keep`), and how sharp it is
    /// (any scale, the same for every crop of the camera).
    Kept {
        /// The crop's sharpness.
        sharpness: f32,
    },
}

/// Milliseconds the shell measured. The core only records them (rule 9).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Spent(pub f64);

pub use scene_contract::{Failure, LookKind};
#[cfg(test)]
mod tests {
    use super::*;
    use scene_contract::{SessionId, SourceSecs};

    /// Every kind of probe, once: the views must split them without overlap
    /// (ADR 2026-11-02, quien-responde-cada-probe).
    #[test]
    fn every_probe_is_answered_by_exactly_one_view() {
        let at = SampleRef {
            session: SessionId(1),
            sequence: 1,
            source_secs: SourceSecs(1.0),
        };
        let ticket = Ticket {
            id: 1,
            looked: at,
            target: Target::Held { since: at },
        };
        let rect = Rect::new(0.0, 0.0, 10.0, 10.0);
        let probes = [
            Probe::Detect { region: None },
            Probe::Faces {
                region: FaceRegion::Subject(rect),
            },
            Probe::Reinforce { region: rect },
            Probe::Pose {
                bbox: rect,
                purpose: PosePurpose::Body,
                up: None,
            },
            Probe::Depth {
                purpose: DepthPurpose::Bed,
                points: vec![],
                region: None,
            },
            Probe::Verify {
                ticket,
                region: rect,
                bed: Quad::from_rect(rect),
                question: Question::HeldInBed,
                crops: vec![],
            },
            Probe::Keep {
                ticket,
                region: rect,
            },
        ];
        let routes: Vec<bool> = probes
            .iter()
            .map(|probe| matches!(probe.route(), Route::Sense(_)))
            .collect();
        assert_eq!(
            routes,
            [true, true, true, true, true, false, false],
            "the five senses answer, the verifier answers its two"
        );
        for probe in &probes {
            if let Some(sense) = probe.sense() {
                assert_eq!(sense.kind(), probe.kind());
            }
        }
    }
}
