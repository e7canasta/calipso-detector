//! The burst machine: the verification plane's collecting, sending and
//! waiting for a question, as a pure table of events and effects (white book,
//! volume III, ch. 12; ADR 2026-11-02, la ráfaga como máquina pura).
//!
//! `Verification::step(event)` moves the state and returns the effects, in
//! order. It does no IO and reads no clock: time arrives as a `SampleRef`, and
//! every duration is `secs_since` of it. The shell keeps what the machine only
//! asks for: crops (`ProbeKeep`), the request itself (`ProbeVerify`), and the
//! wall-clock deadline of a request, which is not a source-time expiry.

use super::{Asked, HeldCheck, Settled, Verification, Verifier, decide, holds};
use crate::attention::VerifyLook;
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
    /// The attention decided to ask: sent now, or collected as a burst.
    Ask {
        at: SampleRef,
        look: VerifyLook,
        burst: Burst,
        judged: bool,
    },
    /// A sample's turn while a burst is being collected: its step.
    Turn {
        at: SampleRef,
        layer_1: Option<BedPostureState>,
        places: Option<BodyPlaces>,
    },
    /// The shell kept the burst's crop, this sharp: `policy` judges it.
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
            } => self.ask(at, look, burst, judged),
            Event::Turn {
                at,
                layer_1,
                places,
            } => self.turn(at, layer_1, places),
            Event::KeptCrop {
                at,
                sharpness,
                policy,
            } => self.kept_crop(at, sharpness, policy),
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
    fn ask(&mut self, at: SampleRef, look: VerifyLook, burst: Burst, judged: bool) -> Vec<Effect> {
        let ticket = self.issue(look.target, at, look.places);
        if burst.frames <= 1 && !judged {
            self.phase = Phase::InFlight { ticket, sent: at };
            vec![Effect::ProbeVerify {
                ticket,
                look,
                crops: Vec::new(),
            }]
        } else {
            self.phase = Phase::Collecting(Collecting {
                ticket,
                look,
                burst,
                crops: Vec::new(),
                opened: at,
                last: at,
            });
            vec![Effect::ProbeKeep {
                ticket,
                region: look.region,
            }]
        }
    }

    /// The burst's step at this sample, if one is being collected.
    fn turn(
        &mut self,
        at: SampleRef,
        layer_1: Option<BedPostureState>,
        places: Option<BodyPlaces>,
    ) -> Vec<Effect> {
        let (ticket, region, step) = match &self.phase {
            Phase::Collecting(collecting) => (
                collecting.ticket,
                collecting.look.region,
                burst_step(at, collecting, layer_1, places, self.held.map(|h| h.since)),
            ),
            _ => return Vec::new(),
        };
        match step {
            BurstStep::Stale | BurstStep::Resolved => {
                self.phase = Phase::Idle;
                self.asked_places = None;
                vec![Effect::Dropped(step)]
            }
            BurstStep::Wait => vec![Effect::Waited],
            BurstStep::Keep => vec![Effect::ProbeKeep { ticket, region }],
        }
    }

    /// A crop was kept at `at`: it is judged, and if the burst is ready, the
    /// question goes out with the crops chosen.
    fn kept_crop(
        &mut self,
        at: SampleRef,
        sharpness: f32,
        policy: Option<SharpnessPolicy>,
    ) -> Vec<Effect> {
        let counted = self.judge_crop(at, sharpness, policy.as_ref());
        let mut effects = vec![Effect::Kept { counted }];
        if let Phase::Collecting(collecting) = &self.phase
            && let Some(crops) = burst_ready(at, collecting, policy.as_ref())
        {
            let (ticket, look) = (collecting.ticket, collecting.look);
            self.phase = Phase::InFlight { ticket, sent: at };
            effects.push(Effect::ProbeVerify {
                ticket,
                look,
                crops,
            });
        }
        effects
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
                let counted = median.is_none_or(|m| sharpness >= p.min_ratio * m);
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
            match decide(Asked::of(ticket.target), Some(answer), holds) {
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
