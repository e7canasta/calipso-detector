//! The rules of the parts: a pure decider, like layer 0's `conservation`. The
//! row is the user's, `l0[t] + places[t−1] : seen[t] : places[t]`, and it is
//! decided position by position. Every cell is in the golden table
//! (`contract/parts-table.txt`): a change of rule is a diff of the table.

use super::{ByPart, Look, Place};

/// The places now. `None`: there are no places.
///
/// - Without a subject (layer 0), there are no places: the memory is lost.
/// - A part seen takes where it was seen.
/// - A part not seen, whether its sense did not look or looked and did not
///   find it, keeps where it was last seen, held; never seen, it is unknown.
pub(super) fn decide(
    before: Option<ByPart<Place>>,
    subject: bool,
    seen: ByPart<Look>,
) -> Option<ByPart<Place>> {
    if !subject {
        return None;
    }
    let before = before.unwrap_or_default();
    Some(ByPart {
        bed: each(before.bed, seen.bed),
        headboard: each(before.headboard, seen.headboard),
        whole: each(before.whole, seen.whole),
    })
}

fn each<const N: usize>(before: [Place; N], seen: [Look; N]) -> [Place; N] {
    let mut now = before;
    for (place, look) in now.iter_mut().zip(seen) {
        *place = self::place(*place, look);
    }
    now
}

/// The cell of one position.
pub(super) fn place(before: Place, seen: Look) -> Place {
    match seen {
        Look::Looked { inside, outside } if seen.seen() => Place {
            inside,
            outside,
            held: false,
        },
        _ if before.known() => Place {
            held: true,
            ..before
        },
        _ => Place::default(),
    }
}

#[cfg(test)]
mod table;
