//! The arbiter of the second level: from the heads' opinions to one verdict.
//! A table, like layer 0's (`contract/second-level-table.txt`): a change of
//! rule is a diff of the table. It reads, never writes the belief.
//!
//! For now (R16, 1605, F-1) it rules only in bed: the camera looks down from
//! above, so in bed the head and shoulders farther than the pelvis can only be
//! lying. Off the bed nothing is ruled yet (F-5).

use super::PostureState;
use crate::zone::Zone;

pub use scene_contract::Head;
pub use scene_contract::{Verdict, VerdictState};
/// The arbiter's ruling: the verdict and the heads that support it.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Ruling {
    pub state: VerdictState,
    pub by: Vec<Head>,
}

/// The table: the subject's zone and the disparity head's opinion. Without a
/// posture there is nothing to rule; off the bed, not yet.
pub(crate) fn arbitrate(zone: Zone, posture: Option<PostureState>) -> Option<Ruling> {
    let posture = posture?;
    let ruling = |state, by: &[Head]| Ruling {
        state,
        by: by.to_vec(),
    };
    match (zone, posture) {
        (Zone::Bed, PostureState::Lying) => {
            Some(ruling(VerdictState::LyingInBed, &[Head::Disparity]))
        }
        (Zone::Bed, PostureState::Upright) => {
            Some(ruling(VerdictState::SittingUpInBed, &[Head::Disparity]))
        }
        (Zone::Bed, PostureState::Unknown) => Some(ruling(VerdictState::InBed, &[])),
        (Zone::Floor, _) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::golden;
    use serde::Serialize;

    const GOLDEN: &str = include_str!("../../../../contract/second-level-table.txt");

    /// A value as the contract names it.
    fn wire<T: Serialize>(value: &T) -> String {
        golden::name(value).unwrap_or_default()
    }

    /// Every combination of zone and opinion, one row each.
    fn table() -> String {
        let mut out = String::from("# zone | disparity | verdict | by\n");
        for zone in [Zone::Bed, Zone::Floor] {
            for posture in [
                PostureState::Lying,
                PostureState::Upright,
                PostureState::Unknown,
            ] {
                let (verdict, by) = match arbitrate(zone, Some(posture)) {
                    None => ("—".to_owned(), "—".to_owned()),
                    Some(r) if r.by.is_empty() => (wire(&r.state), "—".to_owned()),
                    Some(r) => (
                        wire(&r.state),
                        r.by.iter().map(wire).collect::<Vec<_>>().join(", "),
                    ),
                };
                out += &format!("{} | {} | {verdict} | {by}\n", wire(&zone), wire(&posture));
            }
        }
        out
    }

    #[test]
    fn the_arbiter_decides_as_the_golden_table() {
        golden::assert_table("second-level-table.txt", GOLDEN, &table());
    }

    #[test]
    fn without_a_posture_there_is_no_verdict() {
        assert_eq!(arbitrate(Zone::Bed, None), None);
    }
}
