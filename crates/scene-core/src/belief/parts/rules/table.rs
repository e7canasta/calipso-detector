//! The golden table of the parts: every place a position can be in, against
//! everything a sample can say of it, one row each; and the subject that
//! layer 0 lost. If a rule changed on purpose, the test prints the new table
//! to copy into `contract/parts-table.txt`.

use super::super::{ByPart, Look, Place};
use super::{decide, place};

const GOLDEN: &str = include_str!("../../../../../../contract/parts-table.txt");

/// Where a position was: unknown, inside, outside, split; and inside, held.
const BEFORE: [(Place, &str); 5] = [
    (
        Place {
            inside: false,
            outside: false,
            held: false,
        },
        "0.0.0",
    ),
    (
        Place {
            inside: true,
            outside: false,
            held: false,
        },
        "1.0.0",
    ),
    (
        Place {
            inside: false,
            outside: true,
            held: false,
        },
        "0.1.0",
    ),
    (
        Place {
            inside: true,
            outside: true,
            held: false,
        },
        "1.1.0",
    ),
    (
        Place {
            inside: true,
            outside: false,
            held: true,
        },
        "1.0.1",
    ),
];

/// What a sample says: not looked, looked and unseen, inside, outside, split.
const SEEN: [(Look, &str); 5] = [
    (Look::NotLooked, "-.-"),
    (Look::UNSEEN, "0.0"),
    (
        Look::Looked {
            inside: true,
            outside: false,
        },
        "1.0",
    ),
    (
        Look::Looked {
            inside: false,
            outside: true,
        },
        "0.1",
    ),
    (
        Look::Looked {
            inside: true,
            outside: true,
        },
        "1.1",
    ),
];

fn wire(p: Place) -> String {
    let bit = |b| if b { '1' } else { '0' };
    format!("{}.{}.{}", bit(p.inside), bit(p.outside), bit(p.held))
}

fn table() -> String {
    let mut out = String::from(
        "# The parts' table, one position: place before (IN.OUT.HELD) | seen (IN.OUT) | place after.\n\
         # Every position of the trama (cfhsaip · fhs · BFK) follows it; without a subject, none.\n",
    );
    let whole = |p: Place| ByPart {
        bed: [p; 7],
        headboard: [p; 3],
        whole: [p; 3],
    };
    let inside = Place {
        inside: true,
        outside: false,
        held: false,
    };
    let lost = decide(Some(whole(inside)), false, ByPart::default());
    out += &format!(
        "no subject | * | {}\n",
        lost.map_or("—".to_owned(), |p| wire(
            p.bed.first().copied().unwrap_or_default()
        ))
    );
    for (before, b) in BEFORE {
        for (seen, s) in SEEN {
            out += &format!("{b} | {s} | {}\n", wire(place(before, seen)));
        }
    }
    out
}

#[test]
fn the_parts_decide_as_the_golden_table() {
    crate::golden::assert_table("parts-table.txt", GOLDEN, &table());
}
