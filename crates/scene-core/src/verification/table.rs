//! The golden table of V's decider: every combination of what a question
//! asked about, what the verifier answered (or `expired`) and whether what it
//! asked about still holds, one row each. If a rule changed on purpose, the
//! test prints the new table to copy into `contract/verification-table.txt`.

use super::{Asked, Settled, decide};
use crate::golden;
use crate::look::Answer;

const GOLDEN: &str = include_str!("../../../../contract/verification-table.txt");

fn settled(s: Settled) -> String {
    match s {
        Settled::Nothing => "—".to_owned(),
        Settled::Feet(feet) => format!("feet {feet:?}"),
        Settled::Disparity(side) => format!("disparity {side:?}"),
        Settled::Held(state) => format!(
            "held {}",
            golden::name(scene_contract::HeldCheckState::from(state)).unwrap_or_default()
        ),
    }
}

fn table() -> String {
    let mut out = String::from("# asked | answer | holds | settled\n");
    for (asked, a) in [
        (Asked::Held, "held"),
        (Asked::Feet, "feet"),
        (Asked::Disparity, "disparity"),
    ] {
        for (answer, n) in [
            (Some(Answer::Yes), "yes"),
            (Some(Answer::No), "no"),
            (Some(Answer::Unsure), "unsure"),
            (None, "expired"),
        ] {
            for holds in [true, false] {
                let h = if holds { "holds" } else { "moved on" };
                out += &format!(
                    "{a} | {n} | {h} | {}\n",
                    settled(decide(asked, answer, holds))
                );
            }
        }
    }
    out
}

#[test]
fn v_decides_as_the_golden_table() {
    golden::assert_table("verification-table.txt", GOLDEN, &table());
}
