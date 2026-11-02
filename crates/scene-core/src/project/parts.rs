//! The occupancy of the parts, as every version publishes it: the trama's
//! notation and the scene's block, with the shape of layer 0's room (a state,
//! since when, what is pending and the last cell). It translates, it does not
//! decide.

use crate::belief::parts::{ByPart, Concluded, Look, PartsTransition, Place};
use crate::chronicle::Chronicled;
use crate::time::SampleRef;
use crate::time::secs_since;
use scene_contract::{Parts, PartsPending, Progress};

/// The scene's block of the parts, in this turn.
pub(super) fn parts(
    concluded: &Concluded,
    chronicled: &Chronicled,
    at: SampleRef,
    dwell_secs: f64,
) -> Parts {
    Parts {
        state: state(&concluded.state),
        seen: seen(&concluded.seen),
        since: chronicled.last_parts.map(|t| t.at),
        pending: concluded.pending.map(|(places_now, first)| PartsPending {
            places: places(&places_now),
            duration: Progress {
                have: secs_since(at, first).unwrap_or(0.0),
                need: dwell_secs,
            },
        }),
        last_cell: chronicled.last_parts.as_ref().map(cell),
        reading: concluded.reading,
        reading_since: chronicled.parts_reading.since,
    }
}

/// The cell of a change: `before | seen | after`, the places without HELD.
pub(super) fn cell(t: &PartsTransition) -> String {
    format!(
        "{} | {} | {}",
        places(&t.before),
        seen(&t.seen),
        places(&t.after)
    )
}

/// What a sample saw: `bed IN.OUT : headboard IN.OUT : BFK`, `-` where the
/// sense did not look.
pub(super) fn seen(seen: &ByPart<Look>) -> String {
    let side = |looks: &[Look], pick: fn(bool, bool) -> bool| -> String {
        looks
            .iter()
            .map(|l| match *l {
                Look::NotLooked => '-',
                Look::Looked { inside, outside } => bit(pick(inside, outside)),
            })
            .collect()
    };
    format!(
        "{}.{}:{}.{}:{}",
        side(&seen.bed, |i, _| i),
        side(&seen.bed, |_, o| o),
        side(&seen.headboard, |i, _| i),
        side(&seen.headboard, |_, o| o),
        side(&seen.whole, |i, _| i),
    )
}

/// The places, without HELD: `bed IN.OUT : headboard IN.OUT : BFK`.
pub(super) fn places(places: &ByPart<Place>) -> String {
    format!(
        "{}.{}:{}.{}:{}",
        bits(&places.bed, |p| p.inside),
        bits(&places.bed, |p| p.outside),
        bits(&places.headboard, |p| p.inside),
        bits(&places.headboard, |p| p.outside),
        bits(&places.whole, |p| p.inside),
    )
}

/// The places believed, with HELD: `bed IN.OUT.HELD : headboard IN.OUT.HELD :
/// BFK.HELD`.
fn state(state: &ByPart<Place>) -> String {
    format!(
        "{}.{}.{}:{}.{}.{}:{}.{}",
        bits(&state.bed, |p| p.inside),
        bits(&state.bed, |p| p.outside),
        bits(&state.bed, |p| p.held),
        bits(&state.headboard, |p| p.inside),
        bits(&state.headboard, |p| p.outside),
        bits(&state.headboard, |p| p.held),
        bits(&state.whole, |p| p.inside),
        bits(&state.whole, |p| p.held),
    )
}

fn bits(places: &[Place], pick: fn(Place) -> bool) -> String {
    places.iter().map(|p| bit(pick(*p))).collect()
}

fn bit(b: bool) -> char {
    if b { '1' } else { '0' }
}
