//! The main detector: where it looks (the focus or the whole frame), whether
//! the focus confirmed or the turn widens, and whether a loss in bed is
//! reinforced.

use super::{Consideration, Decision, Glimpse, LatchState, Memory, SkipReason};
use crate::belief::{Alone, Belief, InBed, Room};
use crate::geom::Rect;
use crate::map::{Geometry, Map};
use crate::policy::{FocusPolicy, ReinforcePolicy};
use crate::time::SampleRef;

/// Where the main detector runs, which always runs. While the resident is
/// alone in bed, nothing is stabilizing and the sentinel is not due, at the
/// focus, with more pixels per body: the site map's, or else the bed grown by
/// the focus policy's pad. Otherwise over the whole room (`None`), to count
/// everyone, in bed or not. Without a focus policy or bed geometry there is
/// nothing to decide. It is decided before the sample is seen, with what
/// layer 0 believes.
pub(crate) fn primary_region(
    belief: &Belief,
    memory: &Memory,
    at: SampleRef,
    map: &Map,
    geometry: Geometry,
    focus: Option<&FocusPolicy>,
) -> Option<Rect> {
    let focus = focus?;
    let region = map.focus_region(geometry, focus.pad)?;
    let alone_in_bed = matches!(belief.room(), Room::Alone(Alone::InBed(_)));
    let sentinel = memory.lens.wide_due(at, focus.wide_every_secs);
    (alone_in_bed && belief.settled() && !sentinel).then_some(region)
}

/// The focus saw something other than what is believed: look at the whole
/// frame before believing? The focus only confirms, never refutes: alone in
/// bed and seen, it sees one in bed; held, it sees nobody or the one who
/// reappears; and nobody on the floor. Anything else is asked again over the
/// whole frame, so layer 0 never believes a reading of the focus that did not
/// confirm. A focus that failed proves nothing (rule 1): there is nothing to
/// decide, as with the main detector.
pub(crate) fn widen(g: &Glimpse<'_>, belief: &Belief) -> Consideration<()> {
    let Some(seen) = g.bodies.seen() else {
        return Consideration::Inapplicable;
    };
    let counts = seen.counts();
    let confirms = counts.floor == 0
        && match belief.room() {
            Room::Alone(Alone::InBed(InBed::Seen)) => counts.bed == 1,
            Room::Alone(Alone::InBed(InBed::Held)) => counts.bed <= 1,
            Room::Empty | Room::Accompanied(_) | Room::Alone(Alone::Out) => false,
        };
    if confirms {
        Consideration::Inapplicable
    } else {
        Consideration::Decided(Decision::Ask(()))
    }
}

/// Look at the bed again with the finer detector? There is only something to
/// decide if the main detector sees fewer in bed than were seen: a possible
/// loss. While it sees someone in bed and no fewer than before, nobody was
/// lost.
pub(crate) fn reinforce(
    g: &Glimpse<'_>,
    bed_seen: u32,
    latch: LatchState,
    reinforce: Option<&ReinforcePolicy>,
) -> Consideration<Rect> {
    let Some(policy) = reinforce else {
        return Consideration::Disabled;
    };
    let now = g.bodies.seen().map_or(0, |b| b.counts().bed);
    if now > 0 && now >= bed_seen {
        return Consideration::Inapplicable;
    }
    let Some(bed) = g.map.bed_for(g.geometry) else {
        return Consideration::Decided(Decision::Skip(SkipReason::NoBedGeometry));
    };
    Consideration::Decided(match latch {
        LatchState::Disarmed => Decision::Skip(SkipReason::NoSubjectInBed),
        LatchState::Spent => Decision::Skip(SkipReason::LatchSpent),
        LatchState::Armed => Decision::Ask(bed.bounds().expand(policy.bed_padding)),
    })
}
