//! Who answers each probe, and the one place that measures how long it took
//! (ADR 2026-11-02, quien-responde-cada-probe). A sense probe goes to the
//! senses; a verifier probe goes to the verifier. Nothing else answers, and no
//! other code in the shell reads the clock for a probe.
#![deny(clippy::wildcard_enum_match_arm)]

use crate::clock::Clock;
use crate::senses::SensesPort;
use crate::verifier::VerifierPort;
use scene_look::{Failure, Probe, Reading, Route, Spent, VerifierProbe};

/// Answers `probe` over the sample's `frame`, and measures the answer in
/// milliseconds of `clock`. The answerers return no time: this is the only one.
pub fn answer<S, V, C>(
    clock: &C,
    senses: &mut S,
    verifier: &mut V,
    frame: &S::Frame,
    probe: &Probe,
) -> (Result<Reading, Failure>, Spent)
where
    S: SensesPort,
    V: VerifierPort<Frame = S::Frame>,
    C: Clock,
{
    let started = clock.now();
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
    let spent = clock.now().saturating_duration_since(started);
    (result, Spent(spent.as_secs_f64() * 1000.0))
}
