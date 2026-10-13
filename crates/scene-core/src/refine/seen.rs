//! What layer 1 saw in a sample: the second level reduces the turn to one
//! symbol, the column of the bed-posture table. It reads, never writes the
//! belief.

use crate::belief::{Alone, BedPostureSeen, Believed, InBed, Room};
use crate::head::HeadPlaceState;
use crate::provenance::Provenance;

/// Nothing while layer 0 does not believe the resident alone in bed. Held in
/// bed is held. Seen, where the head is in a fresh pose: away from the
/// headboard (below it in bed, or anywhere but past the sides) is not lying,
/// by definition. A pose kept from before says nothing new: retained evidence
/// holds, it does not change (SPEC-MULTIHEAD, rule 4).
pub(crate) fn seen(believed: &Believed) -> Option<BedPostureSeen> {
    match Room::of(believed.occupancy.occupants()) {
        Room::Alone(Alone::InBed(InBed::Held)) => Some(BedPostureSeen::Held),
        Room::Alone(Alone::InBed(InBed::Seen)) => Some(by_head(believed)),
        Room::Empty | Room::Alone(Alone::Out) | Room::Accompanied(_) => None,
    }
}

fn by_head(believed: &Believed) -> BedPostureSeen {
    let head = believed
        .body
        .as_ref()
        .filter(|b| b.pose.provenance == Provenance::Current)
        .map(|b| b.head_place.state);
    match head {
        Some(HeadPlaceState::BelowHeadboard | HeadPlaceState::Elsewhere) => {
            BedPostureSeen::HeadAway
        }
        Some(HeadPlaceState::AtHeadboard | HeadPlaceState::AtSide | HeadPlaceState::Unknown)
        | None => BedPostureSeen::Unknown,
    }
}
