//! The burst machine: the verification plane's collecting, sending and
//! waiting for a question, as a pure table of events and effects (white book,
//! volume III, ch. 12; ADR 2026-11-02, la ráfaga como máquina pura).
//!
//! `Verification::step(event)` moves the state and returns the effects, in
//! order. It does no IO and reads no clock: time arrives as a `SampleRef`, and
//! every duration is `secs_since` of it. The shell keeps what the machine only
//! asks for: crops (`ProbeKeep`), the request itself (`ProbeVerify`), and the
//! wall-clock deadline of a request, which is not a source-time expiry.
//!
//! Where each event is born in the turn (rule 3: an answer opens no turn):
//!
//! | event | point of the turn | engine operation that emits it |
//! | --- | --- | --- |
//! | `Answer` | between turns, when the verifier answers | `Monitor::verified` → `Engine::answered` |
//! | `Settle`, `Expire` | opening the turn, before anything is looked at | `Engine::settle` → `Verification::open_turn` |
//! | `ObserveHeld` | after believing | `Engine::believe` |
//! | `Ask` | the last attention point of the turn, when a doubt is asked | `Engine::issue` (from `ask_verifier`) |
//! | `KeptCrop`, `KeepFailed` | the reading of the burst's crop (`Keep`) | `Engine::kept` |
//! | `Dispatched` | the reading of the verifier's request (`Verify`) | `Engine::dispatched` |
//! | `Turn` | after `refine`, at the last attention point | `Engine::verifier_turn` |

use super::{Asked, HeldCheck, Settled, Verification, Verifier, decide, holds};
use crate::attention::{SkipReason, VerifyLook};
use crate::belief::bed_posture::BedPostureState;
use crate::geom::Rect;
use crate::look::{Target, Ticket, Verified};
use crate::policy::{Burst, SharpnessPolicy};
use crate::refine::{BodyPlaces, DisparitySide, FeetPlace};
use crate::time::{SampleRef, secs_since};
use crate::units::Seconds;

/// Where the verification plane is: at most one request per camera, either
/// being collected as a burst or in flight.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) enum Phase {
    /// No request is open.
    #[default]
    Idle,
    /// A burst being collected: the shell keeps one crop per `every_secs`.
    Collecting(Collecting),
    /// The request went out at `sent`: it waits for its answer.
    InFlight { ticket: Ticket, sent: SampleRef },
}

/// A burst being collected (white book, volume III, ch. 12).
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Collecting {
    pub(crate) ticket: Ticket,
    pub(crate) look: VerifyLook,
    pub(crate) burst: Burst,
    /// The crops the shell kept, in order.
    pub(crate) crops: Vec<KeptCrop>,
    /// The sample the burst opened at: the longest wait counts from it.
    pub(crate) opened: SampleRef,
    /// The sample the last crop was kept at.
    pub(crate) last: SampleRef,
}

/// A crop kept for a burst: how sharp, and whether it counts.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct KeptCrop {
    pub(crate) sharpness: f32,
    pub(crate) counted: bool,
}

/// What the machine is told. Every time is a `SampleRef`.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Event {
    /// A sample's turn starts: the answers that still apply settle.
    Settle {
        at: SampleRef,
        layer_1: Option<BedPostureState>,
    },
    /// A sample's turn starts: a request past its source-time timeout, or from
    /// another session, expires.
    Expire {
        at: SampleRef,
        timeout: Option<Seconds>,
        layer_1: Option<BedPostureState>,
    },
    /// After believing: the oldest held body, if the policy follows held bodies.
    ObserveHeld {
        at: SampleRef,
        oldest: Option<SampleRef>,
        followed: bool,
    },
    /// The attention decided to ask: sent now, or collected as a burst. The
    /// golden table drives the machine with this event; production calls
    /// [`Verification::ask`] and gets the answer typed.
    #[cfg_attr(not(test), allow(dead_code))]
    Ask {
        at: SampleRef,
        look: VerifyLook,
        burst: Burst,
        judged: bool,
    },
    /// A sample's turn while a burst is being collected: its step. Production
    /// calls [`Verification::turn`].
    #[cfg_attr(not(test), allow(dead_code))]
    Turn {
        at: SampleRef,
        layer_1: Option<BedPostureState>,
        places: Option<BodyPlaces>,
    },
    /// The shell kept the burst's crop, this sharp: `policy` judges it.
    /// Production calls [`Verification::kept_crop`].
    #[cfg_attr(not(test), allow(dead_code))]
    KeptCrop {
        at: SampleRef,
        sharpness: f32,
        policy: Option<SharpnessPolicy>,
    },
    /// The shell could not keep the crop.
    KeepFailed,
    /// The shell took the request (`ok`) or could not.
    Dispatched { ok: bool },
    /// An answer arrived between turns.
    Answer(Verified),
}

/// A question ready to leave: its ticket, its look and the crops it goes with
/// (none: this sample's).
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Dispatch {
    pub(crate) ticket: Ticket,
    pub(crate) look: VerifyLook,
    pub(crate) crops: Vec<u8>,
}

/// What a question does when it is decided (`Verification::ask`), typed: each
/// variant is the one thing the question can do, so the caller matches it.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum AskOut {
    /// Sent now: one frame, and nothing judges the crops.
    Sent(Dispatch),
    /// Collected as a burst: this sample's crop is kept under the ticket first.
    Collecting { ticket: Ticket, region: Rect },
    /// Refused: another question is in flight or being collected (P4).
    Busy(SkipReason),
    /// Nothing: the question being collected is this same doubt.
    Nothing,
}

/// What a turn does to the burst it is collecting (`Verification::turn`), typed.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum TurnOut {
    /// No burst is being collected: a doubt may ask now.
    Idle,
    /// The burst waits for its next crop.
    Waited,
    /// The burst keeps this sample's crop under the ticket.
    Keep { ticket: Ticket, region: Rect },
    /// The burst was dropped unasked: stale or resolved.
    Dropped(BurstStep),
}

/// What a kept crop does (`Verification::kept_crop`), typed: whether it counted,
/// and, if the burst is now ready, the question that goes out with its crops.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct KeptOut {
    pub(crate) counted: bool,
    pub(crate) ready: Option<Dispatch>,
}

// The effects of each typed output, in the order the table prints them. Only
// `step` uses them (the golden table reads the effects); the turn reads the
// typed outputs.
impl AskOut {
    fn into_effects(self) -> Vec<Effect> {
        match self {
            AskOut::Sent(Dispatch {
                ticket,
                look,
                crops,
            }) => vec![Effect::ProbeVerify {
                ticket,
                look,
                crops,
            }],
            AskOut::Collecting { ticket, region } => vec![Effect::ProbeKeep { ticket, region }],
            AskOut::Busy(reason) => vec![Effect::Skipped(reason)],
            AskOut::Nothing => Vec::new(),
        }
    }
}

impl TurnOut {
    fn into_effects(self) -> Vec<Effect> {
        match self {
            TurnOut::Idle => Vec::new(),
            TurnOut::Waited => vec![Effect::Waited],
            TurnOut::Keep { ticket, region } => vec![Effect::ProbeKeep { ticket, region }],
            TurnOut::Dropped(step) => vec![Effect::Dropped(step)],
        }
    }
}

impl KeptOut {
    fn into_effects(self) -> Vec<Effect> {
        let mut effects = vec![Effect::Kept {
            counted: self.counted,
        }];
        if let Some(Dispatch {
            ticket,
            look,
            crops,
        }) = self.ready
        {
            effects.push(Effect::ProbeVerify {
                ticket,
                look,
                crops,
            });
        }
        effects
    }
}

/// What the machine asks the shell to do, or reports it did.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Effect {
    /// Crop this sample for the burst and hold it under the ticket.
    ProbeKeep { ticket: Ticket, region: Rect },
    /// Send the question with these kept crops, in order; none: this sample's.
    ProbeVerify {
        ticket: Ticket,
        look: VerifyLook,
        crops: Vec<u8>,
    },
    /// The burst waits for its next crop.
    Waited,
    /// The burst was dropped unasked: stale or resolved.
    Dropped(BurstStep),
    /// A kept crop counted, or not.
    Kept { counted: bool },
    /// An answer changed what it asked about.
    Settled(Settled),
    /// A new doubt could not be asked: one question at a time per camera (P4).
    Skipped(SkipReason),
}

/// What a burst being collected does at this sample (white book, volume III,
/// ch. 12, V4), in this order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BurstStep {
    /// What it asks about no longer holds: dropped unasked.
    Stale,
    /// The doubtful digit cleared by itself: dropped unasked (V17).
    Resolved,
    /// `every_secs` has not passed since the last kept crop.
    Wait,
    /// Keep this sample's crop: the turn sees how sharp it is and whether the
    /// burst is ready ([`burst_ready`]).
    Keep,
}

/// The next step of a burst being collected. It reads, never writes: the turn
/// applies it to the verification plane. Stale: another session, layer 1 left
/// the stable state it was asked under, or the held body is no longer held.
/// Resolved: this sample saw the doubtful digit clearly (a fresh pose).
fn burst_step(
    at: SampleRef,
    collecting: &Collecting,
    layer_1: Option<BedPostureState>,
    places: Option<BodyPlaces>,
    held: Option<SampleRef>,
) -> BurstStep {
    let Some(age) = secs_since(at, collecting.last) else {
        return BurstStep::Stale;
    };
    if !holds(collecting.ticket.target, layer_1, held) {
        return BurstStep::Stale;
    }
    let resolved = match collecting.ticket.target {
        Target::Feet { .. } => {
            places.is_some_and(|p| matches!(p.feet, FeetPlace::Seen | FeetPlace::PastTheSides))
        }
        Target::Disparity { .. } => places
            .is_some_and(|p| matches!(p.disparity, DisparitySide::Lying | DisparitySide::Upright)),
        Target::Held { .. } => false,
    };
    if resolved {
        return BurstStep::Resolved;
    }
    if age < collecting.burst.every_secs.get() {
        return BurstStep::Wait;
    }
    BurstStep::Keep
}

/// After a crop is kept: which kept crops go, if the burst is ready. Ready when
/// `frames` crops counted (were sharp), and those go; or when the longest wait
/// passed since the first crop, and then the sharpest it has go, in the order
/// they were kept. `None`: keep collecting.
fn burst_ready(
    at: SampleRef,
    collecting: &Collecting,
    policy: Option<&SharpnessPolicy>,
) -> Option<Vec<u8>> {
    let frames = usize::from(collecting.burst.frames.max(1));
    let counted: Vec<u8> = collecting
        .crops
        .iter()
        .enumerate()
        .filter(|(_, c)| c.counted)
        .filter_map(|(i, _)| u8::try_from(i).ok())
        .collect();
    if counted.len() >= frames {
        return Some(counted.into_iter().take(frames).collect());
    }
    let waited = policy.is_some_and(|p| {
        secs_since(at, collecting.opened).is_none_or(|age| age >= p.max_wait_secs.get())
    });
    if !waited || collecting.crops.is_empty() {
        return None;
    }
    let mut sharpest: Vec<(usize, f32)> = collecting
        .crops
        .iter()
        .enumerate()
        .map(|(i, c)| (i, c.sharpness))
        .collect();
    sharpest.sort_by(|a, b| b.1.total_cmp(&a.1));
    let mut chosen: Vec<u8> = sharpest
        .into_iter()
        .take(frames)
        .filter_map(|(i, _)| u8::try_from(i).ok())
        .collect();
    chosen.sort_unstable();
    Some(chosen)
}

impl Verification {
    /// One event of the burst machine: its state moves and the effects it
    /// causes come back, in order.
    pub(crate) fn step(&mut self, event: Event) -> Vec<Effect> {
        match event {
            Event::Settle { at, layer_1 } => self.settle(at, layer_1),
            Event::Expire {
                at,
                timeout,
                layer_1,
            } => self.expire(at, timeout, layer_1),
            Event::ObserveHeld {
                at,
                oldest,
                followed,
            } => {
                self.observe_held(oldest, at, followed);
                Vec::new()
            }
            Event::Ask {
                at,
                look,
                burst,
                judged,
            } => self.ask(at, look, burst, judged).into_effects(),
            Event::Turn {
                at,
                layer_1,
                places,
            } => self.turn(at, layer_1, places).into_effects(),
            Event::KeptCrop {
                at,
                sharpness,
                policy,
            } => self.kept_crop(at, sharpness, policy).into_effects(),
            Event::KeepFailed | Event::Dispatched { ok: false } => {
                self.phase = Phase::Idle;
                Vec::new()
            }
            Event::Dispatched { ok: true } => Vec::new(),
            Event::Answer(verified) => {
                self.answers.push(verified);
                Vec::new()
            }
        }
    }

    /// A question is decided: sent now when one frame is enough and nothing
    /// judges the crops, collected otherwise.
    pub(crate) fn ask(
        &mut self,
        at: SampleRef,
        look: VerifyLook,
        burst: Burst,
        judged: bool,
    ) -> AskOut {
        // A new doubt does not replace the question being collected or in flight:
        // it waits, and the ledger says so. Attention asks first (the same guard);
        // the machine repeats it, so no path can lose a ticket or a count.
        if let Phase::Collecting(collecting) = &self.phase
            && collecting.ticket.target == look.target
        {
            // The question being collected is not a new doubt: nothing to note.
            return AskOut::Nothing;
        }
        if !matches!(self.phase, Phase::Idle) {
            return AskOut::Busy(SkipReason::VerifyBusy);
        }
        let ticket = self.issue(look.target, at, look.places);
        if burst.frames <= 1 && !judged {
            self.phase = Phase::InFlight { ticket, sent: at };
            AskOut::Sent(Dispatch {
                ticket,
                look,
                crops: Vec::new(),
            })
        } else {
            self.phase = Phase::Collecting(Collecting {
                ticket,
                look,
                burst,
                crops: Vec::new(),
                opened: at,
                last: at,
            });
            AskOut::Collecting {
                ticket,
                region: look.region,
            }
        }
    }

    /// The burst's step at this sample, if one is being collected.
    pub(crate) fn turn(
        &mut self,
        at: SampleRef,
        layer_1: Option<BedPostureState>,
        places: Option<BodyPlaces>,
    ) -> TurnOut {
        let (ticket, region, opened, step) = match &self.phase {
            Phase::Collecting(collecting) => (
                collecting.ticket,
                collecting.look.region,
                collecting.opened,
                burst_step(at, collecting, layer_1, places, self.held.map(|h| h.since)),
            ),
            _ => return TurnOut::Idle,
        };
        match step {
            BurstStep::Stale | BurstStep::Resolved => {
                self.phase = Phase::Idle;
                self.asked_places = None;
                // The question was counted when the burst opened (V19c, ADR la
                // ráfaga como máquina pura): a burst dropped unasked gives it back.
                if let Some(counted) = self.asked.iter().rposition(|a| *a == opened) {
                    self.asked.remove(counted);
                }
                TurnOut::Dropped(step)
            }
            BurstStep::Wait => TurnOut::Waited,
            BurstStep::Keep => TurnOut::Keep { ticket, region },
        }
    }

    /// A crop was kept at `at`: it is judged, and if the burst is ready, the
    /// question goes out with the crops chosen.
    pub(crate) fn kept_crop(
        &mut self,
        at: SampleRef,
        sharpness: f32,
        policy: Option<SharpnessPolicy>,
    ) -> KeptOut {
        let counted = self.judge_crop(at, sharpness, policy.as_ref());
        let mut ready = None;
        if let Phase::Collecting(collecting) = &self.phase
            && let Some(crops) = burst_ready(at, collecting, policy.as_ref())
        {
            let (ticket, look) = (collecting.ticket, collecting.look);
            self.phase = Phase::InFlight { ticket, sent: at };
            ready = Some(Dispatch {
                ticket,
                look,
                crops,
            });
        }
        KeptOut { counted, ready }
    }

    /// The window of the camera's last crops judges a crop: it counts if it is
    /// at least `min_ratio` of the median (the first ones always count). It is
    /// kept in the burst being collected, if there is one.
    fn judge_crop(
        &mut self,
        at: SampleRef,
        sharpness: f32,
        policy: Option<&SharpnessPolicy>,
    ) -> bool {
        let counted = match policy {
            None => true,
            Some(p) => {
                let mut recent: Vec<f32> = self.sharpness.iter().copied().collect();
                recent.sort_by(f32::total_cmp);
                let median = recent.get(recent.len() / 2).copied();
                let counted = median.is_none_or(|m| sharpness >= p.min_ratio.get() * m);
                self.sharpness.push_back(sharpness);
                while self.sharpness.len() > usize::from(p.window.max(1)) {
                    self.sharpness.pop_front();
                }
                counted
            }
        };
        if let Phase::Collecting(collecting) = &mut self.phase {
            collecting.crops.push(KeptCrop { sharpness, counted });
            collecting.last = at;
        }
        counted
    }

    /// The answers that arrived since the last turn. One for the request in
    /// flight, in its session and about what still holds, settles what it asked
    /// about; the others are dropped.
    fn settle(&mut self, at: SampleRef, layer_1: Option<BedPostureState>) -> Vec<Effect> {
        let mut effects = Vec::new();
        for Verified { ticket, answer } in std::mem::take(&mut self.answers) {
            if !matches!(&self.phase, Phase::InFlight { ticket: sent, .. } if sent.id == ticket.id)
            {
                continue;
            }
            self.phase = Phase::Idle;
            if ticket.looked.session != at.session {
                continue;
            }
            let holds = holds(ticket.target, layer_1, self.held.map(|h| h.since));
            match decide(Asked::of(ticket.target), answer, holds) {
                Settled::Nothing => {}
                Settled::Feet(feet) => {
                    if let Some(places) = self.asked_places {
                        self.filled = Some(BodyPlaces { feet, ..places });
                        effects.push(Effect::Settled(Settled::Feet(feet)));
                    }
                }
                Settled::Disparity(disparity) => {
                    if let Some(places) = self.asked_places {
                        self.filled = Some(BodyPlaces {
                            disparity,
                            ..places
                        });
                        effects.push(Effect::Settled(Settled::Disparity(disparity)));
                    }
                }
                Settled::Held(state) => {
                    if let Some(held) = self.held.as_mut() {
                        held.check = HeldCheck {
                            state,
                            by: Some(Verifier::Vlm),
                            looked: Some(ticket.looked),
                        };
                        effects.push(Effect::Settled(Settled::Held(state)));
                    }
                }
            }
        }
        effects
    }

    /// A request in flight past its source-time timeout, or from another
    /// session, expires. Feet left without an answer are taken as out, on the
    /// safe side; a held body is not released.
    fn expire(
        &mut self,
        at: SampleRef,
        timeout: Option<Seconds>,
        layer_1: Option<BedPostureState>,
    ) -> Vec<Effect> {
        let Phase::InFlight { ticket, sent } = self.phase else {
            return Vec::new();
        };
        let expired = match secs_since(at, sent) {
            None => true,
            Some(age) => timeout.is_some_and(|max| age >= max.get()),
        };
        if !expired {
            return Vec::new();
        }
        self.phase = Phase::Idle;
        let holds = ticket.looked.session == at.session
            && holds(ticket.target, layer_1, self.held.map(|h| h.since));
        if let Settled::Feet(feet) = decide(Asked::of(ticket.target), None, holds) {
            self.filled = self
                .asked_places
                .map(|places| BodyPlaces { feet, ..places });
            if self.filled.is_some() {
                return vec![Effect::Settled(Settled::Feet(feet))];
            }
        }
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::body::HeadPlaceState;
    use crate::geom::{Quad, Rect};
    use crate::look::Answer;
    use crate::look::Question;
    use crate::time::SessionId;
    use crate::units::Ratio;
    use crate::units::Seconds;
    use scene_contract::SourceSecs;

    fn at(n: u64, secs: f64) -> SampleRef {
        SampleRef {
            session: SessionId(1),
            sequence: n,
            source_secs: SourceSecs(secs),
        }
    }

    fn feet_look(places: Option<BodyPlaces>) -> VerifyLook {
        let rect = Rect::new(0.0, 0.0, 10.0, 10.0);
        VerifyLook {
            target: Target::Feet {
                stable: BedPostureState::Lying,
            },
            region: rect,
            bed: Quad::from_rect(rect),
            question: Question::LeavingBed,
            places,
        }
    }

    fn places(feet: FeetPlace) -> Option<BodyPlaces> {
        Some(BodyPlaces {
            head: HeadPlaceState::AtHeadboard,
            feet,
            disparity: DisparitySide::Unknown,
        })
    }

    fn burst(frames: u8) -> Burst {
        Burst {
            frames,
            every_secs: Seconds::ONE,
        }
    }

    fn sharp() -> Option<SharpnessPolicy> {
        Some(SharpnessPolicy {
            min_ratio: Ratio::new(0.5).expect("a constant ratio"),
            window: 3,
            max_wait_secs: Seconds::TWO,
        })
    }

    /// A question sent and answered before the burst under test: its count stays.
    fn one_answered_question(v: &mut Verification) {
        v.step(Event::Ask {
            at: at(1, 1.0),
            look: feet_look(places(FeetPlace::Unseen)),
            burst: Burst::ONE,
            judged: false,
        });
        v.step(Event::Settle {
            at: at(2, 2.0),
            layer_1: Some(BedPostureState::Lying),
        });
        v.step(Event::Answer(Verified {
            ticket: Ticket {
                id: 1,
                looked: at(1, 1.0),
                target: feet_look(None).target,
            },
            answer: Some(Answer::Yes),
        }));
        v.step(Event::Settle {
            at: at(3, 3.0),
            layer_1: Some(BedPostureState::Lying),
        });
    }

    #[test]
    fn a_burst_dropped_as_stale_gives_its_question_back() {
        let mut v = Verification::default();
        one_answered_question(&mut v);
        let before = v.asked.clone();
        v.step(Event::Ask {
            at: at(4, 4.0),
            look: feet_look(places(FeetPlace::Unseen)),
            burst: burst(3),
            judged: true,
        });
        assert_eq!(
            v.asked.len(),
            before.len() + 1,
            "the burst counts when it opens"
        );
        let fx = v.step(Event::Turn {
            at: at(4, 5.0),
            layer_1: Some(BedPostureState::SittingUp),
            places: None,
        });
        assert_eq!(fx, vec![Effect::Dropped(BurstStep::Stale)]);
        assert_eq!(v.asked, before, "as before the burst opened");
    }

    #[test]
    fn a_burst_dropped_as_resolved_gives_its_question_back() {
        let mut v = Verification::default();
        one_answered_question(&mut v);
        let before = v.asked.clone();
        v.step(Event::Ask {
            at: at(4, 4.0),
            look: feet_look(places(FeetPlace::Unseen)),
            burst: burst(3),
            judged: true,
        });
        let fx = v.step(Event::Turn {
            at: at(4, 4.5),
            layer_1: Some(BedPostureState::Lying),
            places: places(FeetPlace::Seen),
        });
        assert_eq!(fx, vec![Effect::Dropped(BurstStep::Resolved)]);
        assert_eq!(v.asked, before, "as before the burst opened");
    }

    fn ticket_of(v: &Verification) -> Option<Ticket> {
        match &v.phase {
            Phase::Collecting(c) => Some(c.ticket),
            Phase::InFlight { ticket, .. } => Some(*ticket),
            Phase::Idle => None,
        }
    }

    fn disparity_look(places: Option<BodyPlaces>) -> VerifyLook {
        let rect = Rect::new(0.0, 0.0, 10.0, 10.0);
        VerifyLook {
            target: Target::Disparity {
                stable: BedPostureState::Lying,
            },
            region: rect,
            bed: Quad::from_rect(rect),
            question: Question::SittingInBed,
            places,
        }
    }

    #[test]
    fn another_target_while_a_burst_is_collected_is_skipped_and_the_burst_stays() {
        let mut v = Verification::default();
        v.step(Event::Ask {
            at: at(2, 2.0),
            look: feet_look(places(FeetPlace::Unseen)),
            burst: burst(3),
            judged: true,
        });
        let (before, counted) = (v.phase.clone(), v.asked.clone());
        let fx = v.step(Event::Ask {
            at: at(3, 3.0),
            look: disparity_look(places(FeetPlace::Unseen)),
            burst: Burst::ONE,
            judged: false,
        });
        assert_eq!(fx, vec![Effect::Skipped(SkipReason::VerifyBusy)]);
        assert_eq!(
            v.phase, before,
            "the burst is not replaced: same ticket, same look"
        );
        assert_eq!(v.asked, counted, "a skipped doubt is not counted");
    }

    #[test]
    fn the_same_question_while_its_burst_is_collected_is_not_a_new_doubt() {
        let mut v = Verification::default();
        v.step(Event::Ask {
            at: at(2, 2.0),
            look: feet_look(places(FeetPlace::Unseen)),
            burst: burst(3),
            judged: true,
        });
        let (before, counted) = (v.phase.clone(), v.asked.clone());
        let fx = v.step(Event::Ask {
            at: at(3, 3.0),
            look: feet_look(places(FeetPlace::Unseen)),
            burst: Burst::ONE,
            judged: false,
        });
        assert!(fx.is_empty(), "the ledger already has this burst: {fx:?}");
        assert_eq!(v.phase, before, "the burst goes on, the same ticket");
        assert_eq!(
            v.asked, counted,
            "a question already counted is not counted again"
        );
    }

    #[test]
    fn a_new_doubt_while_a_question_is_in_flight_is_skipped_and_the_ticket_stays() {
        let mut v = Verification::default();
        v.step(Event::Ask {
            at: at(2, 2.0),
            look: feet_look(places(FeetPlace::Unseen)),
            burst: Burst::ONE,
            judged: false,
        });
        let ticket = ticket_of(&v);
        let fx = v.step(Event::Ask {
            at: at(3, 3.0),
            look: feet_look(places(FeetPlace::Unseen)),
            burst: Burst::ONE,
            judged: false,
        });
        assert_eq!(fx, vec![Effect::Skipped(SkipReason::VerifyBusy)]);
        assert!(matches!(v.phase, Phase::InFlight { .. }));
        assert_eq!(
            ticket_of(&v),
            ticket,
            "the request in flight keeps its ticket"
        );
    }

    #[test]
    fn a_burst_that_was_sent_keeps_its_count() {
        let mut v = Verification::default();
        one_answered_question(&mut v);
        let counted = v.asked.len();
        v.step(Event::Ask {
            at: at(4, 4.0),
            look: feet_look(places(FeetPlace::Unseen)),
            burst: burst(1),
            judged: true,
        });
        let fx = v.step(Event::KeptCrop {
            at: at(4, 4.0),
            sharpness: 0.9,
            policy: sharp(),
        });
        assert!(
            matches!(fx.last(), Some(Effect::ProbeVerify { .. })),
            "{fx:?}"
        );
        assert!(matches!(v.phase, Phase::InFlight { .. }));
        assert_eq!(
            v.asked.len(),
            counted + 1,
            "sent: the question stays counted"
        );
    }
}
