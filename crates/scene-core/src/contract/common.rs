//! What every contract version shares: the publisher's identity, the profile,
//! the parts of the scene no version changes (the bed, the floor, the subject,
//! the evidence, the republication) and the causes. It also re-exports the
//! domain vocabulary every version publishes. No version depends on another:
//! each re-exports this and defines its own.

// The domain vocabulary every version publishes: its modules own it.
pub use crate::attention::{Attention, Look, LookOutcome, LookResult, SkipReason};
pub use crate::belief::{BedPosture, BedPostureSeen, BedPostureState, PartsReading};
pub use crate::body::{BedPoint, BedSite, BodyPoint, Joint, Keypoint, Pose, SubjectDepth};
pub use crate::body::{HeadEvidence, HeadPlace, HeadPlaceState};
pub use crate::conclusion::RepublishReason;
pub use crate::evidence::{BodySource, RejectReason};
pub use crate::look::{Failure, LookKind};
pub use crate::map::{Calibration, FrameInfo, Geometry, MapState};
pub use crate::provenance::Provenance;
pub use crate::reception::SensorState;
pub use crate::refine::{
    BodyGroup, BodyGroups, Disparities, Head, Lying, LyingState, Posture, PostureState, Verdict,
    VerdictState,
};
pub use crate::room::shadow::{BodyShadow, PlanePoint, ShadowPlane};
pub use crate::time::{SampleRef, SessionId, SourceSecs};
pub use crate::verification::{HeldCheck, HeldCheckState, Verifier};
pub use crate::zone::{Counts, Zone};

pub use scene_contract::Subject;
pub use scene_contract::{Bed, BedState, FaceSeen, FacesEvidence, FacesStatus, Floor, Held, Parts};
pub use scene_contract::{BodiesEvidence, BodiesStatus, BodySeen, Evidence};
pub use scene_contract::{
    Build, LayerRef, Profile, ProfileHash, Progress, SenseProfile, Source, Timing,
};
pub use scene_contract::{Cause, PartsPending, Pending, Republished};
