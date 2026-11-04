//! The wire: the vocabulary that `scene.v5` and `events.v5` share, and the
//! versions themselves in `v5`. Types only, with no logic and no domain (ADR
//! 2026-11-02, crates-del-lenguaje). The vocabulary is owned here, by the
//! language crate; `scene-core` re-exports what it decides with, so its paths
//! do not change. Each domain lives in its own module and is re-exported from
//! this root: these re-exports are the public paths, grouped by domain
//! (`tests/public_api.txt` freezes them).
#![warn(missing_docs)]
#![warn(unreachable_pub)]

pub mod v5;

mod bed;
mod body;
mod cell;
mod evidence;
mod look;
mod map;
mod parts;
mod posture;
mod reception;
mod room;
mod run;
mod shadow;
#[cfg(test)]
mod surface;
mod time;
mod zone;

// --- the public surface: one re-export per published name, by domain.

// the domain's only clock: sessions, samples and spans.
pub use time::Progress;
pub use time::SampleRef;
pub use time::SessionId;
pub use time::SourceSecs;
pub use time::Span;
pub use time::Timing;

// who published, and with what.
pub use run::Build;
pub use run::LayerRef;
pub use run::Profile;
pub use run::ProfileHash;
pub use run::SenseProfile;
pub use run::Source;

// the zones and the counts per zone.
pub use zone::Counts;
pub use zone::Zone;

// what the shell reports of the source.
pub use reception::FrameInfo;
pub use reception::SensorState;

// the attention ledger (rule 12).
pub use look::Attention;
pub use look::Failure;
pub use look::Look;
pub use look::LookKind;
pub use look::LookOutcome;
pub use look::LookResult;
pub use look::SkipReason;

// what the sample admitted.
pub use evidence::BodiesEvidence;
pub use evidence::BodiesStatus;
pub use evidence::BodySeen;
pub use evidence::BodySource;
pub use evidence::Evidence;
pub use evidence::FaceSeen;
pub use evidence::FacesEvidence;
pub use evidence::FacesStatus;
pub use evidence::RejectReason;

// the body, its pose and where its claims come from.
pub use body::BodyGroup;
pub use body::BodyGroups;
pub use body::BodyPoint;
pub use body::Disparities;
pub use body::HeadEvidence;
pub use body::HeadPlace;
pub use body::HeadPlaceState;
pub use body::Joint;
pub use body::Keypoint;
pub use body::Pose;
pub use body::Provenance;

// how the body is, by depth.
pub use posture::Lying;
pub use posture::LyingState;
pub use posture::Posture;
pub use posture::PostureState;

// the parts' trama of occupancy.
pub use parts::Parts;
pub use parts::PartsAfter;
pub use parts::PartsBefore;
pub use parts::PartsChange;
pub use parts::PartsPending;
pub use parts::PartsReading;
pub use parts::PartsSeen;

// the bed and layer 1.
pub use bed::Bed;
pub use bed::BedPoint;
pub use bed::BedPosture;
pub use bed::BedPostureBefore;
pub use bed::BedPostureChange;
pub use bed::BedPostureSeen;
pub use bed::BedPostureState;
pub use bed::BedSite;
pub use bed::BedState;
pub use bed::Floor;
pub use bed::Held;
pub use bed::HeldCheck;
pub use bed::HeldCheckState;
pub use bed::Verifier;

// the geometry's shadow over the site.
pub use shadow::BodyShadow;
pub use shadow::LegsSide;
pub use shadow::PartsShadow;
pub use shadow::PartsStep;
pub use shadow::PlanePoint;
pub use shadow::ShadowPlane;
pub use shadow::TorsoShadow;
pub use shadow::TurnRule;

// the site map.
pub use map::Geometry;
pub use map::MapShadow;

// the room's machine and why messages move.
pub use room::Cause;
pub use room::Mode;
pub use room::Pending;
pub use room::Phase;
pub use room::RepublishReason;
pub use room::Republished;
pub use room::Room;

// the key of a row.
pub use cell::Cell;
