//! Who answers each probe, and the one place that measures how long it took
//! (ADR 2026-11-02, quien-responde-cada-probe). A sense probe goes to the
//! senses; a verifier probe goes to the verifier. Nothing else answers, and no
//! other code in the shell reads the clock for a probe.
#![deny(clippy::wildcard_enum_match_arm)]

use crate::senses::SensesPort;
use crate::verifier::VerifierPort;
use scene_core::look::{Failure, Probe, Reading, Route, Spent, VerifierProbe};
use std::time::Instant;

/// Answers `probe` over the sample's `frame`, and measures the answer in wall
/// milliseconds. The answerers return no time: this is the only one.
pub fn answer<S, V>(
    senses: &mut S,
    verifier: &mut V,
    frame: &S::Frame,
    probe: &Probe,
) -> (Result<Reading, Failure>, Spent)
where
    S: SensesPort,
    V: VerifierPort<Frame = S::Frame>,
{
    let started = Instant::now();
    let result = match probe.route() {
        Route::Sense(sense) => senses.look(frame, &sense),
        Route::Verifier(verified) => match verified {
            VerifierProbe::Verify {
                ticket,
                region,
                bed,
                question,
                crops,
            } => verifier
                .ask(frame, ticket, region, &bed, question, crops)
                .map(|()| Reading::Dispatched),
            VerifierProbe::Keep { ticket, region } => verifier
                .keep(frame, ticket, region)
                .map(|sharpness| Reading::Kept { sharpness }),
        },
    };
    (result, Spent(started.elapsed().as_secs_f64() * 1000.0))
}
