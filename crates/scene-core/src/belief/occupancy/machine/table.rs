//! The layer-0 table as a net: every combination of previous
//! belief, evidence and context, up to two people, run through the real
//! `Machine::step` and compared with the golden file. The `RBH | FV` notation
//! lives here: it is presentation, not domain.
//!
//! The core does no IO, not even in its tests (rule 8): the golden file is
//! read at compile time and, if the table changed, the test prints the whole
//! new one. After an intentional rule change, copy it to the golden file and
//! commit the diff:
//! `cargo test -p scene-core table 2>&1 | sed -n '/^# The layer-0 table/,/^# end/p'`

use super::*;
use crate::belief::total;
use crate::evidence::Body;
use crate::evidence::BodySource;
use crate::geom::Rect;
use crate::look::Label;
use crate::policy::{BedStep, FarArrival, Persistence, SideExit};
use crate::room::BodyZone;
use crate::time::SessionId;
use crate::units::Seconds;
use scene_contract::SourceSecs;

const GOLDEN: &str = include_str!("../../../../../../contract/layer-0-table.txt");
const PERSISTENCE: Persistence = Persistence {
    dwell_secs: Seconds::ONE,
};

/// The context a cell is crossed with.
#[derive(Clone, Copy)]
enum Context {
    /// Nobody far, nobody passing, the bed with a history.
    Base,
    /// One of those seen on the floor is far from the bed.
    Far,
    /// Before the final count, one more was seen on the floor.
    Passing,
    /// The bed has no history yet.
    Booting,
    /// As the base, with rule 4 bis: a held body may leave by the side.
    SideExit,
    /// The control of `NearFirst`: the same sequence, without the profile's
    /// `near_first`. The difference between the two columns is the flag alone.
    NearThenFar,
    /// One was seen near the bed first, and one far from it afterwards: rule 5
    /// refined, the profile's `near_first`, does not count the near one as
    /// the door's.
    NearFirst,
    /// As `Passing`, with rule 1 refined, the profile's `step_near`: one more
    /// seen near the bed while the bed went down stepped down from it. The
    /// difference between the two columns is the flag alone.
    StepNear,
}

const CONTEXTS: [(Context, &str); 8] = [
    (Context::Base, "base"),
    (Context::Far, "far"),
    (Context::Passing, "passing"),
    (Context::Booting, "booting"),
    (Context::SideExit, "side exit"),
    (Context::NearThenFar, "near then far"),
    (Context::NearFirst, "near first"),
    (Context::StepNear, "step near"),
];

fn at(secs: f64) -> SampleRef {
    SampleRef {
        session: SessionId(1),
        sequence: 0,
        source_secs: SourceSecs(secs),
    }
}

fn bbox(i: u32) -> Detection {
    let x = 10.0 * f32::from(u16::try_from(i).unwrap());
    Detection {
        bbox: Rect::new(x, 0.0, x + 5.0, 5.0),
        confidence: 0.9,
        label: Label::Person,
    }
}

/// Bodies with `floor` off the bed (`far` of them far from it) and `bed` in it.
fn bodies(floor: u32, far: u32, bed: u32) -> Bodies {
    Bodies {
        bed: (0..bed)
            .map(|i| Body {
                detection: bbox(i),
                source: BodySource::Primary,
                zone: Some(BodyZone::Bed),
                shadow: None,
                parts: None,
                knee_deg: None,
            })
            .collect(),
        floor: (0..floor)
            .map(|i| Body {
                detection: bbox(10 + i),
                source: BodySource::Primary,
                zone: Some(BodyZone::Floor { near_bed: i >= far }),
                shadow: None,
                parts: None,
                knee_deg: None,
            })
            .collect(),
    }
}

/// The belief `RBH` at rest: seen in the stable count, retained, and the bed's
/// history, as if it had been reached by a valid path.
fn machine(r: u32, b: u32, h: u32, history: bool) -> Machine {
    let stable = Counts {
        floor: r - b,
        bed: b - h,
    };
    let rest = bodies(stable.floor, 0, stable.bed);
    let mut z = Machine {
        span: Span::resting(stable, &rest),
        held: (0..h)
            .map(|i| HeldBody {
                detection: bbox(20 + i),
                since: at(0.0),
                stepped: false,
            })
            .collect(),
        bed_phase: if history {
            BedPhase::Tracking
        } else {
            BedPhase::Adopting
        },
        session: Some(at(0.0).session),
        ..Machine::default()
    };
    if stable.bed > 0 {
        z.last_seen_in_bed = Some(Sighting {
            bodies: rest.bed.iter().map(|b| b.detection).collect(),
            at: at(0.0),
        });
    }
    z
}

fn name(h: &Happened) -> &'static str {
    match h.fact {
        Fact::Entered => "entered",
        Fact::LeftRoom => "left_room",
        Fact::Adopted => "lay_down/baseline",
        Fact::LayDown => "lay_down",
        Fact::LeftBed => "left_bed",
        Fact::HeldInBed { .. } => "held_in_bed",
        Fact::Reappeared { .. } => "reappeared",
    }
}

/// `RBH`, plus the unexplained bodies if any: `110+u`.
fn triple(z: &Machine) -> String {
    let t = z.occupants();
    let (r, b, h) = (total(t), t.bed + t.held, t.held);
    let unexplained = match z.unexplained {
        0 => String::new(),
        1 => "+u".into(),
        n => format!("+u{n}"),
    };
    format!("{r}{b}{h}{unexplained}")
}

/// Crosses a cell through `step`: the final belief and the transitions, in order.
fn cross(r: u32, b: u32, h: u32, f: u32, v: u32, context: Context) -> (Machine, Vec<Transition>) {
    let mut z = machine(r, b, h, !matches!(context, Context::Booting));
    let mut transitions = Vec::new();
    let end = match context {
        Context::Far | Context::NearFirst | Context::NearThenFar => bodies(f, f.min(1), v),
        Context::Base
        | Context::Passing
        | Context::Booting
        | Context::SideExit
        | Context::StepNear => bodies(f, 0, v),
    };
    let rules = OccupancyRules {
        persistence: PERSISTENCE,
        held_exit: match context {
            Context::SideExit => SideExit::ByTheSide,
            _ => SideExit::OnlyReappearing,
        },
        far_arrival: match context {
            Context::NearFirst => FarArrival::ThoseNeverNear,
            _ => FarArrival::FromTheDoor,
        },
        bed_step: match context {
            Context::StepNear => BedStep::SeenNear,
            _ => BedStep::FloorRises,
        },
    };
    let mut t = 1.0;
    if matches!(context, Context::Passing | Context::StepNear) {
        let before = (r - b).max(f) + 1;
        transitions.extend(z.step(at(t), &bodies(before, 0, v), &rules));
        t += 0.5;
    }
    if matches!(context, Context::NearFirst | Context::NearThenFar) {
        // One more near the bed than at rest: the one that will walk away.
        let near = (r - b) + 1;
        transitions.extend(z.step(at(t), &bodies(near, 0, v), &rules));
        t += 0.5;
    }
    transitions.extend(z.step(at(t), &end, &rules));
    transitions.extend(z.step(at(t + 1.0), &end, &rules));
    (z, transitions)
}

/// A cell in the book's notation.
fn cell(r: u32, b: u32, h: u32, f: u32, v: u32, context: Context) -> String {
    let (z, transitions) = cross(r, b, h, f, v, context);
    let facts: Vec<_> = facts(&transitions).map(name).collect();
    format!("{} {}", triple(&z), facts.join(","))
        .trim_end()
        .to_string()
}

/// Every cell: `(r, b, h, f, v)`.
fn cells() -> impl Iterator<Item = (u32, u32, u32, u32, u32)> {
    (0..=2).flat_map(|r| {
        (0..=r).flat_map(move |b| {
            (0..=b).flat_map(move |h| {
                (0..=2).flat_map(move |f| (0..=2 - f).map(move |v| (r, b, h, f, v)))
            })
        })
    })
}

/// The mode's name for reading the table: presentation, it lives in the test.
fn mode(room: Room) -> &'static str {
    use super::super::{Alone, InBed};
    match room {
        Room::Empty => "empty",
        Room::Alone(Alone::Out) => "alone, out",
        Room::Alone(Alone::InBed(InBed::Seen)) => "alone, in bed, seen",
        Room::Alone(Alone::InBed(InBed::Held)) => "alone, in bed, held",
        Room::Accompanied(_) => "accompanied",
    }
}

fn table() -> String {
    let mut states = std::collections::BTreeSet::new();
    let mut rows = String::new();
    for (r, b, h, f, v) in cells() {
        states.insert((r, b, h));
        rows += &format!("{r}{b}{h} | {f}{v}");
        for (context, label) in CONTEXTS {
            let (z, _) = cross(r, b, h, f, v, context);
            let t = z.occupants();
            states.insert((total(t), t.bed + t.held, t.held));
            rows += &format!(" | {label}: {}", cell(r, b, h, f, v, context));
        }
        rows += "\n";
    }
    let mut out = String::from(
        "# The layer-0 table. Generated by belief/occupancy/machine/table.rs.\n\
         # States: the RBH triple and its mode in the machine (Room).\n",
    );
    for (r, b, h) in states {
        let room = Room::of(Occupants {
            floor: r - b,
            bed: b - h,
            held: h,
        });
        out += &format!("{r}{b}{h} | {}\n", mode(room));
    }
    out += "# Transitions: before RBH | seen FV | context: after RBH [+u unexplained] facts\n";
    out + &rows + "# end\n"
}

#[test]
fn the_layer_0_table_is_the_golden_one() {
    crate::golden::assert_table("layer-0-table.txt", GOLDEN, &table());
}

fn facts(transitions: &[Transition]) -> impl Iterator<Item = &Happened> {
    transitions.iter().flat_map(|t| &t.facts)
}

fn how_many(transitions: &[Transition], which: fn(&Happened) -> bool) -> i64 {
    i64::try_from(facts(transitions).filter(|e| which(e)).count()).unwrap()
}

#[test]
fn the_layer_0_invariants_hold_across_the_table() {
    for (r, b, h, f, v) in cells() {
        for (context, label) in CONTEXTS {
            let (z, events) = cross(r, b, h, f, v, context);
            let cell = format!("{r}{b}{h} | {f}{v} | {label}");
            let after = z.occupants();
            if let Some(last) = facts(&events).last() {
                assert_eq!(
                    last.after.occupants, after,
                    "{cell}: the after of the last fact is the belief"
                );
            }
            // The transition carries its state (P6): what we believed is the
            // cell, what we saw is the raw count and what we believe is the belief.
            if let Some(last) = events.last() {
                assert_eq!(
                    last.after.occupants, after,
                    "{cell}: after of the transition"
                );
                assert_eq!(
                    last.seen.counts,
                    Counts { floor: f, bed: v },
                    "{cell}: what we saw"
                );
            }
            if let Some(first) = events.first() {
                assert_eq!(
                    first.before.occupants,
                    Occupants {
                        floor: r - b,
                        bed: b - h,
                        held: h
                    },
                    "{cell}: before of the transition"
                );
            }
            // Only visibility in bed changes the retained; with the side
            // exit, also being seen on the floor (rule 4 bis).
            let retained = i64::from(after.held) - i64::from(h);
            let held = how_many(&events, |e| matches!(e.fact, Fact::HeldInBed { .. }));
            let reappeared = how_many(&events, |e| matches!(e.fact, Fact::Reappeared { .. }));
            if matches!(context, Context::SideExit) {
                assert!(
                    retained >= held - reappeared - i64::from(f),
                    "{cell}: retained"
                );
            } else {
                assert_eq!(retained, held - reappeared, "{cell}: retained");
            }
            // A retained body does not leave without reappearing (rule 4), or
            // with the side exit, without being seen on the floor (4 bis).
            if reappeared == 0 && !(matches!(context, Context::SideExit) && f > 0) {
                assert!(
                    after.held >= h,
                    "{cell}: a retained body left without reappearing"
                );
            }
            // Only the door changes the total, except for the two named
            // exceptions: adoption at startup and lying down in passing.
            if matches!(context, Context::Base | Context::Far) {
                let total = i64::from(total(after)) - i64::from(r);
                let entered = how_many(&events, |e| e.fact == Fact::Entered);
                let left_room = how_many(&events, |e| e.fact == Fact::LeftRoom);
                assert_eq!(
                    total,
                    entered - left_room,
                    "{cell}: only the door changes the total"
                );
            }
        }
    }
}
