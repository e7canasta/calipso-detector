//! What layer 1 saw in a sample: the second level reduces the turn to one
//! symbol, the column of the bed-posture table. It reads, never writes the
//! belief.

use crate::belief::{Alone, BedPostureSeen, Believed, InBed, Room};

/// Nothing while layer 0 does not believe the resident alone in bed. Held in
/// bed is held; otherwise, for now, nothing says how they are.
pub(crate) fn seen(believed: &Believed) -> Option<BedPostureSeen> {
    match Room::of(believed.occupancy.occupants()) {
        Room::Alone(Alone::InBed(InBed::Held)) => Some(BedPostureSeen::Held),
        Room::Alone(Alone::InBed(InBed::Seen)) => Some(BedPostureSeen::Unknown),
        Room::Empty | Room::Alone(Alone::Out) | Room::Accompanied(_) => None,
    }
}
