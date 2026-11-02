//! The golden table of the burst machine (`contract/verification-burst-table.txt`):
//! every cell of the phase × event × condition table, one row each, with the
//! state it leaves and the effects it causes. The rows are the ones the
//! imperative code produced before the refactor (V19b): the machine must keep
//! every cell. A rule change on purpose prints the new table to copy.

use super::{BurstStep, Effect, Event, Phase, Verification, VerifyRules};
use crate::attention::VerifyLook;
use crate::belief::bed_posture::BedPostureState;
use crate::body::HeadPlaceState;
use crate::geom::{Quad, Rect};
use crate::golden;
use crate::look::{Answer, Question, Target, Verified};
use crate::policy::{Burst, Frames, SharpnessPolicy};
use crate::refine::{BodyPlaces, DisparitySide, FeetPlace};
use crate::time::{SampleRef, SessionId};
use crate::units::Seconds;
use scene_contract::SourceSecs;

fn at(n: u64, secs: f64) -> SampleRef {
    sess(1, n, secs)
}
fn sess(s: u64, n: u64, secs: f64) -> SampleRef {
    SampleRef {
        session: SessionId(s),
        sequence: n,
        source_secs: SourceSecs(secs),
    }
}
fn sec(x: f64) -> Seconds {
    Seconds::new(x).expect("a time")
}
fn held_since() -> SampleRef {
    at(1, 0.0)
}
fn feet_target() -> Target {
    Target::Feet {
        stable: BedPostureState::Lying,
    }
}
fn disparity_target() -> Target {
    Target::Disparity {
        stable: BedPostureState::Lying,
    }
}
fn held_target() -> Target {
    Target::Held {
        since: held_since(),
    }
}
fn places(feet: FeetPlace, disparity: DisparitySide) -> BodyPlaces {
    BodyPlaces {
        head: HeadPlaceState::AtHeadboard,
        feet,
        disparity,
    }
}
fn look(target: Target, question: Question, places: Option<BodyPlaces>) -> VerifyLook {
    let rect = Rect::new(0.0, 0.0, 10.0, 10.0);
    VerifyLook {
        target,
        region: rect,
        bed: Quad::from_rect(rect),
        question,
        places,
    }
}
fn sharp() -> SharpnessPolicy {
    SharpnessPolicy {
        min_ratio: 0.5,
        window: 3,
        max_wait_secs: sec(2.0),
    }
}
fn frames(n: u8) -> Frames {
    let b = Burst {
        frames: n,
        every_secs: sec(1.5),
    };
    Frames {
        held: b,
        sitting: b,
        leaving: b,
    }
}
/// Burst rules: a timeout of 3 s, sharpness judged, `n` frames per question.
fn rules(n: u8, judged: bool) -> VerifyRules {
    VerifyRules {
        held: true,
        timeout_secs: Some(sec(3.0)),
        sharpness: judged.then(sharp),
        frames: Some(frames(n)),
    }
}

fn name(v: &Verification) -> &'static str {
    match v.phase {
        Phase::Idle => "Idle",
        Phase::Collecting(_) => "Collecting",
        Phase::InFlight { .. } => "InFlight",
    }
}

fn held_str(v: &Verification) -> String {
    match v.held {
        None => "—".to_owned(),
        Some(h) => format!(
            "since=s{}n{} at=s{}n{} check={}",
            h.since.session.0,
            h.since.sequence,
            h.held_at.session.0,
            h.held_at.sequence,
            golden::name(h.check.state).unwrap_or_default()
        ),
    }
}

fn row(before: &str, event: &str, cond: &str, v: &Verification, effects: &[Effect]) -> String {
    let effects = if effects.is_empty() {
        "—".to_owned()
    } else {
        effects.iter().map(effect).collect::<Vec<_>>().join("; ")
    };
    format!(
        "{before} | {event} | {cond} | {} asked={} win={:?} held={} | {effects}",
        name(v),
        v.asked.len(),
        v.sharpness.iter().collect::<Vec<_>>(),
        held_str(v),
    )
}

fn effect(e: &Effect) -> String {
    match e {
        Effect::ProbeKeep { ticket, .. } => format!("keep ticket={}", ticket.id),
        Effect::ProbeVerify { ticket, crops, .. } => {
            format!("verify ticket={} crops={crops:?}", ticket.id)
        }
        Effect::Waited => "waited".to_owned(),
        Effect::Dropped(step) => format!(
            "dropped {}",
            match step {
                BurstStep::Stale => "stale",
                BurstStep::Resolved => "resolved",
                BurstStep::Wait => "wait",
                BurstStep::Keep => "keep",
            }
        ),
        Effect::Kept { counted } => format!("kept counted={counted}"),
        Effect::Settled(settled) => match settled {
            crate::verification::Settled::Nothing => "nothing".to_owned(),
            crate::verification::Settled::Feet(feet) => format!("feet {feet:?}"),
            crate::verification::Settled::Disparity(side) => format!("disparity {side:?}"),
            crate::verification::Settled::Held(state) => {
                format!("held {}", golden::name(*state).unwrap_or_default())
            }
        },
    }
}

/// The ticket a probe carries: the request sent or collected at `ask`.
fn ticket_of(effects: &[Effect]) -> crate::look::Ticket {
    match effects.first() {
        Some(Effect::ProbeVerify { ticket, .. } | Effect::ProbeKeep { ticket, .. }) => *ticket,
        _ => panic!("the ask emits a probe"),
    }
}

fn ask_event(rules: &VerifyRules, at: SampleRef, look: VerifyLook) -> Event {
    Event::Ask {
        at,
        look,
        burst: rules
            .frames
            .map(|f| f.of(look.question))
            .unwrap_or_default(),
        judged: rules.sharpness.is_some(),
    }
}

fn ask(v: &mut Verification, rules: &VerifyRules, at: SampleRef, look: VerifyLook) -> Vec<Effect> {
    v.step(ask_event(rules, at, look))
}

fn turn(
    v: &mut Verification,
    at: SampleRef,
    layer_1: Option<BedPostureState>,
    places: Option<BodyPlaces>,
) -> Vec<Effect> {
    v.step(Event::Turn {
        at,
        layer_1,
        places,
    })
}

fn kept(v: &mut Verification, rules: &VerifyRules, at: SampleRef, sharpness: f32) -> Vec<Effect> {
    v.step(Event::KeptCrop {
        at,
        sharpness,
        policy: rules.sharpness,
    })
}

fn settle(v: &mut Verification, at: SampleRef, layer_1: Option<BedPostureState>) -> Vec<Effect> {
    v.step(Event::Settle { at, layer_1 })
}

fn expire(
    v: &mut Verification,
    rules: &VerifyRules,
    at: SampleRef,
    layer_1: Option<BedPostureState>,
) -> Vec<Effect> {
    v.step(Event::Expire {
        at,
        timeout: rules.timeout_secs,
        layer_1,
    })
}

fn observe(v: &mut Verification, oldest: Option<SampleRef>, at: SampleRef, followed: bool) {
    v.step(Event::ObserveHeld {
        at,
        oldest,
        followed,
    });
}

/// A request in flight for `target`, with the places it fills, sent at 5 s. The
/// held body is followed, as a profile with `[verify.held]` does.
fn in_flight(target: Target, places: Option<BodyPlaces>) -> (Verification, crate::look::Ticket) {
    let mut v = Verification::default();
    observe(&mut v, Some(held_since()), at(1, 0.0), true);
    let question = match target {
        Target::Held { .. } => Question::HeldInBed,
        Target::Feet { .. } => Question::LeavingBed,
        Target::Disparity { .. } => Question::SittingInBed,
    };
    let effects = v.step(Event::Ask {
        at: at(5, 5.0),
        look: look(target, question, places),
        burst: Burst::ONE,
        judged: false,
    });
    (v, ticket_of(&effects))
}

fn answer_name(a: Option<Answer>) -> &'static str {
    match a {
        Some(Answer::Yes) => "yes",
        Some(Answer::No) => "no",
        Some(Answer::Unsure) => "unsure",
        None => "none (wall deadline)",
    }
}

fn idle_rows(out: &mut Vec<String>) {
    let r1 = rules(1, false);
    let r3 = rules(3, false);
    let rj = rules(1, true);
    let mut v = Verification::default();
    let fx = ask(
        &mut v,
        &r1,
        at(2, 2.0),
        look(
            feet_target(),
            Question::LeavingBed,
            Some(places(FeetPlace::Unseen, DisparitySide::Unknown)),
        ),
    );
    out.push(row("Idle", "ask", "one frame, not judged", &v, &fx));
    let mut v = Verification::default();
    let fx = ask(
        &mut v,
        &r3,
        at(2, 2.0),
        look(feet_target(), Question::LeavingBed, None),
    );
    out.push(row("Idle", "ask", "three frames", &v, &fx));
    let mut v = Verification::default();
    let fx = ask(
        &mut v,
        &rj,
        at(2, 2.0),
        look(feet_target(), Question::LeavingBed, None),
    );
    out.push(row("Idle", "ask", "one frame, judged", &v, &fx));
    let mut v = Verification::default();
    let fx = v.step(Event::Answer(Verified {
        ticket: crate::look::Ticket {
            id: 1,
            looked: at(1, 1.0),
            target: feet_target(),
        },
        answer: Some(Answer::Yes),
    }));
    out.push(row("Idle", "answer", "arrives", &v, &fx));
    let mut v = Verification::default();
    let fx = settle(&mut v, at(2, 2.0), None);
    out.push(row("Idle", "settle", "no answers", &v, &fx));
    let mut v = Verification::default();
    let fx = expire(&mut v, &r1, at(2, 2.0), None);
    out.push(row("Idle", "expire", "no request", &v, &fx));
    let mut v = Verification::default();
    let fx = turn(&mut v, at(2, 2.0), None, None);
    out.push(row("Idle", "turn", "no burst", &v, &fx));
    let mut v = Verification::default();
    let fx = kept(&mut v, &rj, at(2, 2.0), 0.9);
    out.push(row("Idle", "kept crop", "no burst", &v, &fx));
    let mut v = Verification::default();
    let fx = v.step(Event::KeepFailed);
    out.push(row("Idle", "keep failed", "no burst", &v, &fx));
}

fn collecting_rows(out: &mut Vec<String>) {
    let rj = rules(3, true);
    let base = || {
        let mut v = Verification::default();
        ask(
            &mut v,
            &rj,
            at(2, 2.0),
            look(
                feet_target(),
                Question::LeavingBed,
                Some(places(FeetPlace::Unseen, DisparitySide::Unknown)),
            ),
        );
        v
    };
    let mut v = base();
    let fx = turn(&mut v, at(2, 2.5), Some(BedPostureState::Lying), None);
    out.push(row(
        "Collecting",
        "turn",
        "younger than every_secs",
        &v,
        &fx,
    ));
    let mut v = base();
    let fx = turn(&mut v, at(2, 4.0), Some(BedPostureState::Lying), None);
    out.push(row("Collecting", "turn", "every_secs passed", &v, &fx));
    let mut v = base();
    let fx = turn(&mut v, at(2, 4.0), Some(BedPostureState::SittingUp), None);
    out.push(row("Collecting", "turn", "layer 1 moved on", &v, &fx));
    let mut v = base();
    let fx = turn(&mut v, sess(2, 3, 4.0), Some(BedPostureState::Lying), None);
    out.push(row("Collecting", "turn", "another session", &v, &fx));
    let mut v = base();
    let fx = turn(
        &mut v,
        at(2, 2.5),
        Some(BedPostureState::Lying),
        Some(places(FeetPlace::Seen, DisparitySide::Unknown)),
    );
    out.push(row("Collecting", "turn", "doubt cleared", &v, &fx));
    let rh = rules(3, true);
    let mut v = Verification::default();
    observe(&mut v, Some(held_since()), at(1, 0.0), rh.held);
    ask(
        &mut v,
        &rh,
        at(2, 2.0),
        look(held_target(), Question::HeldInBed, None),
    );
    observe(&mut v, None, at(3, 3.0), rh.held);
    let fx = turn(&mut v, at(2, 4.0), Some(BedPostureState::Lying), None);
    out.push(row(
        "Collecting",
        "turn",
        "held body no longer held",
        &v,
        &fx,
    ));
    // Crops: the first always counts, then the median decides.
    let mut v = base();
    let fx = kept(&mut v, &rj, at(2, 2.2), 0.9);
    out.push(row("Collecting", "kept crop", "sharp, first", &v, &fx));
    let mut v = base();
    kept(&mut v, &rj, at(2, 2.2), 0.9);
    let fx = kept(&mut v, &rj, at(2, 2.4), 0.2);
    out.push(row(
        "Collecting",
        "kept crop",
        "blurred, not the last",
        &v,
        &fx,
    ));
    let mut v = base();
    kept(&mut v, &rj, at(2, 2.2), 0.9);
    kept(&mut v, &rj, at(2, 2.3), 0.85);
    let fx = kept(&mut v, &rj, at(2, 2.6), 0.8);
    out.push(row(
        "Collecting",
        "kept crop",
        "third counted: ready",
        &v,
        &fx,
    ));
    let mut v = base();
    kept(&mut v, &rj, at(2, 2.2), 0.9);
    let fx = kept(&mut v, &rj, at(2, 4.5), 0.2);
    out.push(row(
        "Collecting",
        "kept crop",
        "blurred after the longest wait",
        &v,
        &fx,
    ));
    let mut v = base();
    let fx = v.step(Event::KeepFailed);
    out.push(row("Collecting", "keep failed", "", &v, &fx));
    let mut v = base();
    let fx = v.step(Event::Answer(Verified {
        ticket: crate::look::Ticket {
            id: 1,
            looked: at(2, 2.0),
            target: feet_target(),
        },
        answer: Some(Answer::Yes),
    }));
    out.push(row("Collecting", "answer", "for a burst not sent", &v, &fx));
    let mut v = base();
    let fx = settle(&mut v, at(2, 2.5), None);
    out.push(row("Collecting", "settle", "no answers", &v, &fx));
    let mut v = base();
    let fx = expire(&mut v, &rj, at(9, 9.0), None);
    out.push(row("Collecting", "expire", "no request", &v, &fx));
}

fn in_flight_rows(out: &mut Vec<String>) {
    let p = Some(places(FeetPlace::Unseen, DisparitySide::Unknown));
    let (mut v, _) = in_flight(feet_target(), p);
    let fx = v.step(Event::Dispatched { ok: false });
    out.push(row("InFlight", "dispatch failed", "feet", &v, &fx));
    let (mut v, _) = in_flight(feet_target(), p);
    let fx = v.step(Event::Dispatched { ok: true });
    out.push(row("InFlight", "dispatch ok", "feet", &v, &fx));
    let (mut v, _) = in_flight(feet_target(), p);
    let fx = turn(&mut v, at(6, 6.0), Some(BedPostureState::Lying), None);
    out.push(row("InFlight", "turn", "feet", &v, &fx));
}

fn settle_rows(out: &mut Vec<String>) {
    for (tn, target) in [
        ("feet", feet_target()),
        ("disparity", disparity_target()),
        ("held", held_target()),
    ] {
        let p = match target {
            Target::Held { .. } => None,
            _ => Some(places(FeetPlace::Unseen, DisparitySide::Unknown)),
        };
        for answer in [
            Some(Answer::Yes),
            Some(Answer::No),
            Some(Answer::Unsure),
            None,
        ] {
            for holds in [true, false] {
                let (mut v, t) = in_flight(target, p);
                if !holds && matches!(target, Target::Held { .. }) {
                    observe(&mut v, None, at(5, 5.0), true);
                }
                let layer = if holds {
                    Some(BedPostureState::Lying)
                } else {
                    Some(BedPostureState::SittingUp)
                };
                let before = name(&v);
                v.step(Event::Answer(Verified { ticket: t, answer }));
                let fx = settle(&mut v, at(6, 6.0), layer);
                let cond = format!(
                    "{tn} · answer {} · {}",
                    answer_name(answer),
                    if holds { "holds" } else { "moved on" }
                );
                out.push(row(before, "answer", &cond, &v, &fx));
            }
        }
        // The answer for another session: discarded, nothing changes.
        let (mut v, t) = in_flight(target, p);
        v.step(Event::Answer(Verified {
            ticket: t,
            answer: Some(Answer::Yes),
        }));
        let fx = settle(&mut v, sess(2, 6, 6.0), Some(BedPostureState::Lying));
        out.push(row(
            "InFlight",
            "answer",
            &format!("{tn} · another session"),
            &v,
            &fx,
        ));
        // The answer for another ticket: consumed, nothing changes.
        let (mut v, mut t) = in_flight(target, p);
        t.id += 100;
        v.step(Event::Answer(Verified {
            ticket: t,
            answer: Some(Answer::Yes),
        }));
        let fx = settle(&mut v, at(6, 6.0), Some(BedPostureState::Lying));
        out.push(row(
            "InFlight",
            "answer",
            &format!("{tn} · another ticket"),
            &v,
            &fx,
        ));
    }
}

fn expire_rows(out: &mut Vec<String>) {
    for (tn, target) in [
        ("feet", feet_target()),
        ("disparity", disparity_target()),
        ("held", held_target()),
    ] {
        let p = match target {
            Target::Held { .. } => None,
            _ => Some(places(FeetPlace::Unseen, DisparitySide::Unknown)),
        };
        let r = rules(1, false);
        for holds in [true, false] {
            let (mut v, _) = in_flight(target, p);
            if !holds && matches!(target, Target::Held { .. }) {
                observe(&mut v, None, at(5, 5.0), true);
            }
            let layer = if holds {
                Some(BedPostureState::Lying)
            } else {
                Some(BedPostureState::SittingUp)
            };
            let before = name(&v);
            let fx = expire(&mut v, &r, at(9, 9.0), layer);
            let cond = format!(
                "{tn} · timeout reached · {}",
                if holds { "holds" } else { "moved on" }
            );
            out.push(row(before, "expire", &cond, &v, &fx));
        }
        let (mut v, _) = in_flight(target, p);
        let fx = expire(&mut v, &r, at(6, 6.0), Some(BedPostureState::Lying));
        out.push(row(
            "InFlight",
            "expire",
            &format!("{tn} · timeout not reached"),
            &v,
            &fx,
        ));
        let (mut v, _) = in_flight(target, p);
        let no_timeout = VerifyRules {
            timeout_secs: None,
            ..rules(1, false)
        };
        let fx = expire(
            &mut v,
            &no_timeout,
            at(9, 9.0),
            Some(BedPostureState::Lying),
        );
        out.push(row(
            "InFlight",
            "expire",
            &format!("{tn} · no timeout"),
            &v,
            &fx,
        ));
        let (mut v, _) = in_flight(target, p);
        let fx = expire(&mut v, &r, sess(2, 9, 9.0), Some(BedPostureState::Lying));
        out.push(row(
            "InFlight",
            "expire",
            &format!("{tn} · another session"),
            &v,
            &fx,
        ));
    }
}

fn held_rows(out: &mut Vec<String>) {
    let mut v = Verification::default();
    observe(&mut v, Some(held_since()), at(1, 1.0), false);
    out.push(row("Idle", "held observe", "not followed", &v, &[]));
    let mut v = Verification::default();
    observe(&mut v, Some(held_since()), at(1, 1.0), true);
    out.push(row("Idle", "held observe", "new body", &v, &[]));
    let mut v = Verification::default();
    observe(&mut v, Some(held_since()), at(1, 1.0), true);
    observe(&mut v, Some(held_since()), at(1, 2.0), true);
    out.push(row(
        "Idle",
        "held observe",
        "same body, same session",
        &v,
        &[],
    ));
    let mut v = Verification::default();
    observe(&mut v, Some(held_since()), at(1, 1.0), true);
    observe(&mut v, Some(held_since()), sess(2, 3, 3.0), true);
    out.push(row(
        "Idle",
        "held observe",
        "same body, new session",
        &v,
        &[],
    ));
    let mut v = Verification::default();
    observe(&mut v, Some(held_since()), at(1, 1.0), true);
    observe(&mut v, Some(at(1, 2.0)), at(1, 5.0), true);
    out.push(row("Idle", "held observe", "another body", &v, &[]));
    let mut v = Verification::default();
    observe(&mut v, Some(held_since()), at(1, 1.0), true);
    observe(&mut v, None, at(1, 5.0), true);
    out.push(row("Idle", "held observe", "no body held", &v, &[]));
}

/// The whole table: one line per cell, in the order the golden file keeps.
pub(super) fn table() -> String {
    let mut out = vec![String::from("# state | event | condition | next | effects")];
    idle_rows(&mut out);
    collecting_rows(&mut out);
    in_flight_rows(&mut out);
    settle_rows(&mut out);
    expire_rows(&mut out);
    held_rows(&mut out);
    out.join("\n") + "\n"
}

#[test]
fn v_bursts_as_the_golden_table() {
    golden::assert_table("verification-burst-table.txt", GOLDEN, &table());
}

const GOLDEN: &str = include_str!("../../../../contract/verification-burst-table.txt");
