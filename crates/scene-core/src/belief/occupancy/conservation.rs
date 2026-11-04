//! The conservation rules (functional memory, §5): people neither appear nor
//! vanish, they move between zones. Two pure functions, shaped as a
//! *decider*: [`settle`] decides which moves explain a change of the stable
//! count, in publishing order; [`evolve`] says how each one moves the counts.
//! No state, no events, no boxes: each rule is tested alone.

use crate::belief::Occupants;
use crate::policy::{BedStep, SideExit};
use crate::zone::Counts;

/// What we believed before the change: `state[t−1]`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Before {
    /// The stable count per zone, unexplained bodies included.
    pub(super) stable: Counts,
    /// How many were retained in bed.
    pub(super) held: u32,
    /// Of the retained, how many were held while one more was seen stepping
    /// down near the bed (rule 1, refined).
    pub(super) held_stepped: u32,
    /// How many bodies in the bed count are not occupants (rule 6).
    pub(super) unexplained: u32,
    pub(super) bed_phase: BedPhase,
    /// Rule 4 bis: how a held body may leave (by the side, seen on the floor
    /// near the bed, or only by reappearing in bed).
    pub(super) held_exit: SideExit,
    /// Rule 1, refined: what counts as the step from the bed to the floor.
    pub(super) bed_step: BedStep,
}

/// The bed phase for rule 6: "the bed without
/// history" and "with history". It is a decision bit, not a record: layer 0
/// keeps no history.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) enum BedPhase {
    /// Until the first climb in, who was there before startup is unknown:
    /// whatever appears in bed is adopted.
    #[default]
    Adopting,
    /// The bed already went up once: whoever appears without coming from the
    /// floor is nobody (unexplained).
    Tracking,
}

/// What we saw: the new count that stabilized, `input[t]`, with what is known
/// of how it was reached.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SeenMark {
    pub(crate) counts: Counts,
    /// How many appeared on the floor far from the bed while it stabilized:
    /// they come from the door, not from the bed (rule 5).
    pub(crate) from_door: u32,
    /// How many extra were seen on the floor while it stabilized: someone who
    /// passed through the floor though the floor count never changed (rule 6).
    pub(crate) passing: u32,
    /// How many more than at rest were seen on the floor near the bed while it
    /// stabilized: who may have stepped down from the bed (rule 1, refined).
    pub(crate) stepped: u32,
}

/// A move of a person (or of a body that is nobody) that explains part of the
/// change. The machine's output: it lives in the transition.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Movement {
    /// Moved from the bed to the floor, seen (rule 1).
    LeftBed,
    /// Stopped being seen in bed without leaving: retained (rule 1).
    Held,
    /// Retained, while one more was seen near the bed: it may have stepped
    /// down (rule 1, refined).
    HeldStepped,
    /// A body retained after a step seen near the bed left the room with
    /// whoever left the floor, without reappearing (rule 1, refined).
    SteppedLeft,
    /// A retained body was seen on the floor near the bed: it got up by the
    /// side (rule 4 bis).
    HeldLeft,
    /// A retained body was seen in bed again (rules 2 and 4).
    Reappeared,
    /// Appeared in bed coming from the floor (rule 6): it left the stable
    /// floor count.
    LayDownFromFloor,
    /// Appeared in bed after passing through the floor without stabilizing
    /// (rule 6).
    LayDownWhilePassing,
    /// Appeared in bed before it had a history: adopted.
    Adopted,
    /// Stopped being seen on the floor: left the room (rule 3).
    LeftRoom,
    /// Appeared on the floor without coming from the bed: entered.
    Entered,
    /// Appeared in bed without coming from the floor: not an occupant (rule 6).
    UnexplainedIn,
    /// An unexplained body stopped being seen: it takes nobody with it.
    UnexplainedOut,
}

/// Which moves explain the change of the stable count, in publishing order
/// (rule 4): the bed first, then the room.
pub(super) fn settle(before: Before, seen: SeenMark) -> Vec<Movement> {
    use Movement::*;
    let floor_up = seen.counts.floor.saturating_sub(before.stable.floor);
    let floor_down = before.stable.floor.saturating_sub(seen.counts.floor);
    let mut moves = Vec::new();
    let mut push = |m: Movement, n: u32| moves.extend((0..n).map(|_| m));

    if seen.counts.bed < before.stable.bed {
        let gone = before.stable.bed - seen.counts.bed;
        // Rule 1: the bed only goes down through an observed step: the bed goes
        // down and the floor goes up. Rule 5: those who came from the door did
        // not leave the bed. Rule 6: only an occupant leaves the bed.
        let occupants = before.stable.bed.saturating_sub(before.unexplained);
        let risen = floor_up.saturating_sub(seen.from_door);
        // Rule 1, refined: of those seen near the bed while it stabilized,
        // more than are left on the floor means one of them stepped down and
        // left the room in the window. The rest may have stepped down too.
        let (step, maybe) = match before.bed_step {
            BedStep::FloorRises => (risen, 0),
            BedStep::SeenNear => {
                let step = risen.max(seen.stepped.saturating_sub(seen.counts.floor));
                (step, seen.stepped.saturating_sub(step))
            }
        };
        let left_bed = gone.min(step).min(occupants);
        // What did not leave: what was nobody goes first; an occupant stays
        // retained.
        let stayed = gone - left_bed;
        let unexplained_out = stayed.min(before.unexplained);
        let held = stayed - unexplained_out;
        // Retained after a step seen near the bed: it leaves with the next one
        // who leaves the floor, unless it reappears first.
        let held_stepped = held.min(maybe);
        // Whoever stepped down past the floor's rise took the place of someone
        // who left the room meanwhile.
        let through = left_bed.saturating_sub(floor_up);
        push(LeftBed, left_bed);
        push(Held, held - held_stepped);
        push(HeldStepped, held_stepped);
        push(LeftRoom, floor_down + through);
        push(Entered, floor_up - (left_bed - through));
        push(UnexplainedOut, unexplained_out);
    } else {
        let appeared = seen.counts.bed - before.stable.bed;
        // Rule 2: whoever appears in bed is first taken from the retained.
        // Rule 4: a retained body does not leave without reappearing, so this is
        // the only way out for a retained body.
        let reappeared = appeared.min(before.held);
        let new = appeared - reappeared;
        // Rule 6: nobody lies down without having been seen on the floor. Of
        // those who passed through the floor, those still there did not lie down.
        let lay_down = new.min(floor_down + seen.passing.saturating_sub(floor_up));
        // Those who lay down leaving the floor; the rest passed without stabilizing.
        let from_floor = lay_down.min(floor_down);
        let unseen = new - lay_down;
        // Rule 4 bis: of those who appeared on the floor near the bed, not
        // from the door, a retained body that did not reappear got up.
        let held_left = match before.held_exit {
            SideExit::ByTheSide => floor_up
                .saturating_sub(seen.from_door)
                .min(before.held - reappeared),
            SideExit::OnlyReappearing => 0,
        };
        // Rule 1, refined: a body retained after a step seen near the bed left
        // with whoever left the floor, if it did not reappear.
        let left_floor = floor_down - from_floor;
        let stepped_left = before
            .held_stepped
            .min(before.held - reappeared - held_left)
            .min(left_floor);
        push(Reappeared, reappeared);
        push(HeldLeft, held_left);
        push(SteppedLeft, stepped_left);
        push(LayDownFromFloor, from_floor);
        push(LayDownWhilePassing, lay_down - from_floor);
        if before.bed_phase == BedPhase::Adopting {
            push(Adopted, unseen);
        }
        // Whoever lay down from the floor did not leave the room.
        push(LeftRoom, left_floor + stepped_left);
        push(Entered, floor_up - held_left);
        if before.bed_phase == BedPhase::Tracking {
            push(UnexplainedIn, unseen);
        }
    }
    moves
}

/// How a move changes the occupant counts. What is nobody does not count.
pub(super) fn evolve(occupants: Occupants, m: Movement) -> Occupants {
    let Occupants { floor, bed, held } = occupants;
    let (floor, bed, held) = match m {
        Movement::LeftBed => (floor + 1, bed.saturating_sub(1), held),
        Movement::Held | Movement::HeldStepped => (floor, bed.saturating_sub(1), held + 1),
        Movement::HeldLeft | Movement::SteppedLeft => (floor + 1, bed, held.saturating_sub(1)),
        Movement::Reappeared => (floor, bed + 1, held.saturating_sub(1)),
        Movement::LayDownFromFloor => (floor.saturating_sub(1), bed + 1, held),
        Movement::LayDownWhilePassing | Movement::Adopted => (floor, bed + 1, held),
        Movement::LeftRoom => (floor.saturating_sub(1), bed, held),
        Movement::Entered => (floor + 1, bed, held),
        Movement::UnexplainedIn | Movement::UnexplainedOut => (floor, bed, held),
    };
    Occupants { floor, bed, held }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// How many moves of each kind: the rules are asserted by quantities.
    #[derive(Debug, Default)]
    struct Tally {
        left_bed: u32,
        held: u32,
        held_stepped: u32,
        held_left: u32,
        stepped_left: u32,
        reappeared: u32,
        lay_down: u32,
        adopted: u32,
        unexplained_in: u32,
        unexplained_out: u32,
        left_room: u32,
        entered: u32,
    }

    /// A change with everything that decides it, to write each rule in one
    /// line: what we believed and what we saw, together.
    #[derive(Debug, Clone, Copy)]
    struct Change {
        from: Counts,
        to: Counts,
        from_door: u32,
        passing: u32,
        held: u32,
        held_stepped: u32,
        unexplained: u32,
        bed_history: bool,
        held_exit: SideExit,
        stepped: u32,
        bed_step: BedStep,
    }

    fn settle(c: Change) -> Tally {
        let before = Before {
            stable: c.from,
            held: c.held,
            held_stepped: c.held_stepped,
            unexplained: c.unexplained,
            bed_phase: if c.bed_history {
                BedPhase::Tracking
            } else {
                BedPhase::Adopting
            },
            held_exit: c.held_exit,
            bed_step: c.bed_step,
        };
        let seen = SeenMark {
            counts: c.to,
            from_door: c.from_door,
            passing: c.passing,
            stepped: c.stepped,
        };
        let mut t = Tally::default();
        for m in super::settle(before, seen) {
            *match m {
                Movement::LeftBed => &mut t.left_bed,
                Movement::Held => &mut t.held,
                Movement::HeldStepped => &mut t.held_stepped,
                Movement::HeldLeft => &mut t.held_left,
                Movement::SteppedLeft => &mut t.stepped_left,
                Movement::Reappeared => &mut t.reappeared,
                Movement::LayDownFromFloor | Movement::LayDownWhilePassing => &mut t.lay_down,
                Movement::Adopted => &mut t.adopted,
                Movement::UnexplainedIn => &mut t.unexplained_in,
                Movement::UnexplainedOut => &mut t.unexplained_out,
                Movement::LeftRoom => &mut t.left_room,
                Movement::Entered => &mut t.entered,
            } += 1;
        }
        t
    }

    fn c(floor: u32, bed: u32) -> Counts {
        Counts { floor, bed }
    }

    /// A change with a bed that already has a history: every rule applies.
    fn change(from: Counts, to: Counts) -> Change {
        Change {
            from,
            to,
            from_door: 0,
            passing: 0,
            held: 0,
            held_stepped: 0,
            unexplained: 0,
            bed_history: true,
            held_exit: SideExit::OnlyReappearing,
            stepped: 0,
            bed_step: BedStep::FloorRises,
        }
    }

    #[test]
    fn rule_1_the_bed_goes_down_with_an_observed_step() {
        let m = settle(change(c(0, 1), c(1, 0)));
        assert_eq!((m.left_bed, m.held, m.entered), (1, 0, 0));
    }

    #[test]
    fn rule_1_the_bed_goes_down_without_a_step_and_someone_is_retained() {
        let m = settle(change(c(1, 1), c(1, 0)));
        assert_eq!((m.left_bed, m.held), (0, 1));
    }

    #[test]
    fn rule_1_refined_one_more_seen_near_the_bed_is_the_step() {
        let near = |bed_step: BedStep, from: Counts, to: Counts| {
            settle(Change {
                stepped: 1,
                bed_step,
                ..change(from, to)
            })
        };
        // Alone, gets up and goes within one count: fewer left on the floor
        // than were seen near the bed, decided at once.
        let m = near(BedStep::SeenNear, c(0, 1), c(0, 0));
        assert_eq!((m.left_bed, m.held_stepped, m.left_room), (1, 0, 1));
        // With the carer, both gone by the end: both left.
        let m = near(BedStep::SeenNear, c(1, 1), c(0, 0));
        assert_eq!((m.left_bed, m.held_stepped, m.left_room), (1, 0, 2));
        // galia2, turned: one left on the floor, the carer or the resident.
        // Retained, marked as seen stepping.
        let m = near(BedStep::SeenNear, c(1, 1), c(1, 0));
        assert_eq!(
            (m.left_bed, m.held, m.held_stepped, m.left_room),
            (0, 0, 1, 0)
        );
        // The floor rose too: the step is counted once.
        let m = near(BedStep::SeenNear, c(0, 1), c(1, 0));
        assert_eq!((m.left_bed, m.held_stepped, m.entered), (1, 0, 0));
        // Without the rule, as rule 1 always said.
        let m = near(BedStep::FloorRises, c(1, 1), c(1, 0));
        assert_eq!((m.left_bed, m.held, m.held_stepped), (0, 1, 0));
    }

    #[test]
    fn rule_1_refined_the_body_seen_stepping_leaves_with_the_floor() {
        let after = |to: Counts| {
            settle(Change {
                held: 1,
                held_stepped: 1,
                bed_step: BedStep::SeenNear,
                ..change(c(1, 0), to)
            })
        };
        // The last one on the floor leaves: the retained one went too.
        let m = after(c(0, 0));
        assert_eq!((m.stepped_left, m.left_room), (1, 2));
        // It shows in bed again first: it never left.
        let m = after(c(1, 1));
        assert_eq!((m.reappeared, m.stepped_left, m.left_room), (1, 0, 0));
    }

    #[test]
    fn rule_2_whoever_appears_in_bed_reappears_before_being_new() {
        let m = settle(Change {
            held: 1,
            ..change(c(1, 0), c(1, 1))
        });
        assert_eq!((m.reappeared, m.lay_down, m.unexplained_in), (1, 0, 0));
    }

    #[test]
    fn rule_3_on_the_floor_no_longer_being_seen_is_leaving() {
        let m = settle(change(c(2, 0), c(0, 0)));
        assert_eq!(m.left_room, 2);
    }

    #[test]
    fn rule_4_bis_a_retained_body_leaves_by_the_side_seen_near_the_bed() {
        // White book, volume VII, ch. 05: (0,0) with one retained → floor.
        let side = |to: Counts, from_door: u32, held: u32| {
            settle(Change {
                held,
                from_door,
                held_exit: SideExit::ByTheSide,
                ..change(c(0, 0), to)
            })
        };
        let m = side(c(1, 0), 0, 1);
        assert_eq!((m.held_left, m.entered), (1, 0));
        // From the door, someone entered: the retained one stays.
        let m = side(c(1, 0), 1, 1);
        assert_eq!((m.held_left, m.entered), (0, 1));
        let m = side(c(2, 0), 1, 1);
        assert_eq!((m.held_left, m.entered), (1, 1));
        // With nobody retained, as always.
        let m = side(c(1, 0), 0, 0);
        assert_eq!((m.held_left, m.entered), (0, 1));
    }

    #[test]
    fn rule_4_a_retained_body_does_not_leave_without_reappearing() {
        // (0,0) with one retained → (1,0): no step is possible; someone entered.
        let m = settle(Change {
            held: 1,
            ..change(c(0, 0), c(1, 0))
        });
        assert_eq!((m.left_bed, m.entered), (0, 1));
    }

    #[test]
    fn rule_5_whoever_arrives_from_the_door_does_not_leave_the_bed() {
        let m = settle(Change {
            from_door: 1,
            ..change(c(0, 1), c(1, 0))
        });
        assert_eq!((m.left_bed, m.held, m.entered), (0, 1, 1));
    }

    #[test]
    fn rule_6_nobody_lies_down_without_having_been_seen_on_the_floor() {
        let m = settle(change(c(0, 0), c(0, 1)));
        assert_eq!((m.lay_down, m.unexplained_in, m.entered), (0, 1, 0));
        // With someone on the floor who stays there, neither.
        let m = settle(change(c(1, 0), c(1, 1)));
        assert_eq!((m.lay_down, m.unexplained_in), (0, 1));
    }

    #[test]
    fn rule_6_whoever_passed_through_the_floor_without_stabilizing_does_lie_down() {
        let m = settle(Change {
            passing: 1,
            ..change(c(0, 0), c(0, 1))
        });
        assert_eq!((m.lay_down, m.unexplained_in, m.left_room), (1, 0, 0));
    }

    #[test]
    fn rule_6_whoever_passed_through_the_floor_and_stayed_there_did_not_lie_down() {
        // At startup the staff enter and stay next to the bed: they are not who
        // appeared in bed.
        let m = settle(Change {
            passing: 1,
            bed_history: false,
            ..change(c(0, 0), c(1, 1))
        });
        assert_eq!((m.lay_down, m.adopted, m.entered), (0, 1, 1));
    }

    #[test]
    fn rule_6_without_history_the_bed_adopts_what_it_sees() {
        let m = settle(Change {
            bed_history: false,
            ..change(c(1, 0), c(1, 1))
        });
        assert_eq!((m.adopted, m.lay_down, m.unexplained_in), (1, 0, 0));
        // But whoever comes from the floor lies down, even without history.
        let m = settle(Change {
            bed_history: false,
            ..change(c(1, 0), c(0, 1))
        });
        assert_eq!((m.lay_down, m.adopted, m.left_room), (1, 0, 0));
    }

    #[test]
    fn what_was_nobody_leaves_before_an_occupant() {
        // One occupant and one unexplained body; the bed goes down without a
        // step: what was nobody leaves and nobody is retained.
        let m = settle(Change {
            unexplained: 1,
            ..change(c(0, 2), c(0, 1))
        });
        assert_eq!((m.unexplained_out, m.held), (1, 0));
        // With an observed step, the occupant leaves: the unexplained body stays.
        let m = settle(Change {
            unexplained: 1,
            ..change(c(0, 2), c(1, 1))
        });
        assert_eq!((m.left_bed, m.unexplained_out, m.entered), (1, 0, 0));
    }

    #[test]
    fn rule_6_what_was_nobody_does_not_leave_the_bed() {
        // Only unexplained bodies in bed; the bed goes down and the floor goes
        // up: nobody left the bed, someone entered.
        let m = settle(Change {
            unexplained: 1,
            ..change(c(0, 1), c(1, 0))
        });
        assert_eq!((m.left_bed, m.unexplained_out, m.entered), (0, 1, 1));
    }

    #[test]
    fn lying_down_from_the_floor_is_not_leaving_the_room() {
        let m = settle(change(c(2, 0), c(1, 1)));
        assert_eq!((m.lay_down, m.left_room, m.entered), (1, 0, 0));
    }
}
