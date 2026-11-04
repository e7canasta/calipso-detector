//! The detector shell: it translates the world towards the core.
// What is `pub` is reachable from outside; the rest says `pub(crate)`.
#![warn(unreachable_pub)]
mod build;
mod clock;
mod config;
mod decoder;
mod driver;
mod publish;
mod router;
mod senses;
mod source;
mod stream_watch;
mod verifier;

// The facade: what the outside uses, each item by one path from this root. The
// modules above stay private; a new public item of the crate is exported here
// or it is not public.
pub use build::current as build_info;
pub use clock::{Clock, FixedClock, SystemClock};
pub use config::{Config, NatsConfig};
pub use driver::{Driver, Processed, build_driver};
pub use publish::{CHANNEL_BUFFER, Kind, Publication, PublisherHandle, nats};
pub use router::answer;
pub use senses::{Frame, SensesPort, load as load_senses};
pub use source::{Arrival, FramesSource, RtspSource, SampleDecoder, SampleSource};
pub use verifier::{
    DeadlineVerifier, NatsVerifier, NoVerifier, VerifierEndpoint, VerifierPort, jpeg_crop,
};
