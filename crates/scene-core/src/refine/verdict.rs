//! The arbiter of the second level: from the heads' opinions to one verdict.
//! A table, like layer 0's (`contract/second-level-table.txt`): a change of
//! rule is a diff of the table. It reads, never writes the belief.
//!
//! For now (R16, 1605, F-1) it rules only in bed: the camera looks down from
//! above, so in bed the head and shoulders farther than the pelvis can only be
//! lying. Off the bed nothing is ruled yet (F-5).

use super::PostureState;
use crate::zone::Zone;

/// A head of the second level: who supports a verdict.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Head {
    /// The depth disparity of head and shoulders against the pelvis (904, 1604).
    Disparity,
}

/// How the resident is, as the arbiter rules it.
// The names are the contract's: the cell of the table is written with them.
#[allow(clippy::enum_variant_names)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum VerdictState {
    /// In bed, the head and shoulders farther from the camera than the pelvis.
    LyingInBed,
    /// In bed, the head and shoulders closer to the camera than the pelvis.
    SittingUpInBed,
    /// In bed, but no head knows how.
    InBed,
}

/// The published verdict: how the resident is, which heads support it and
/// since when it holds (a fold of the chronicle, P4).
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Verdict {
    /// How the resident is.
    pub state: VerdictState,
    /// The heads that support it; empty when none knows.
    pub by: Vec<Head>,
    /// The first sample of this verdict, without interruption.
    pub since: crate::time::SampleRef,
}

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

    const GOLDEN: &str = include_str!("../../../../contract/second-level-table.txt");

    /// Every combination of zone and opinion, one row each.
    fn table() -> String {
        let mut out = String::from("# zone | disparity | verdict | by\n");
        for zone in [Zone::Bed, Zone::Floor] {
            for posture in [
                PostureState::Lying,
                PostureState::Upright,
                PostureState::Unknown,
            ] {
                // The table names the cable's values: their names are the contract's.
                let on_the_wire = |r: &Ruling| {
                    (
                        golden::wire(scene_contract::VerdictState::from(r.state)),
                        r.by.iter()
                            .map(|h| golden::wire(scene_contract::Head::from(*h)))
                            .collect::<Vec<_>>(),
                    )
                };
                let (verdict, by) = match arbitrate(zone, Some(posture)) {
                    None => ("—".to_owned(), "—".to_owned()),
                    Some(r) if r.by.is_empty() => (on_the_wire(&r).0, "—".to_owned()),
                    Some(r) => {
                        let (state, by) = on_the_wire(&r);
                        (state, by.join(", "))
                    }
                };
                out += &format!(
                    "{} | {} | {verdict} | {by}\n",
                    golden::wire(zone),
                    golden::wire(scene_contract::PostureState::from(posture))
                );
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
