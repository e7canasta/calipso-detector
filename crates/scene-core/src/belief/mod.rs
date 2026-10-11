//! The belief: how many are in the room, who is still in bed even when unseen,
//! and who the subject is. One aggregate per camera; it never sees a raw
//! reading (R4 functional memory).

mod occupancy;
mod subject;

use crate::evidence::{Admission, Bodies, BodyEvidence};
use crate::geom::Rect;
use crate::look::Detection;
use crate::policy::Policy;
use crate::provenance::Provenance;
use crate::time::SampleRef;
use crate::zone::Zone;
use occupancy::Machine;
use subject::SubjectMemory;

pub(crate) use occupancy::{Alone, BedPhase, InBed, Occupancy, Room, Tally, Transition};
pub use subject::{Enrichment, SubjectBody};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ZoneCounts {
    pub floor: u32,
    pub bed: u32,
    pub held: u32,
}

impl ZoneCounts {
    pub fn occupants(self) -> u32 {
        self.floor + self.bed + self.held
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Fact {
    Entered,
    /// Was in bed before the bed had a history: adopted (rule 6, R5-3).
    /// Published as `lay_down` with cause `baseline`.
    Adopted,
    LeftRoom,
    LayDown,
    LeftBed,
    HeldInBed {
        last_seen: SampleRef,
    },
    Reappeared {
        held_since: SampleRef,
    },
}

/// What we believe about one camera.
#[derive(Debug, Clone, Default)]
pub struct Belief {
    occupancy: Machine,
    subject: SubjectMemory,
    latch: LatchState,
}

/// The reinforcement memory: "someone was seen in bed". It is belief; firing
/// it is attention.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum LatchState {
    /// Nobody was seen in bed.
    #[default]
    Disarmed,
    /// The main detector saw someone in bed: if lost, it can be reinforced.
    Armed,
    /// This loss was already reinforced.
    Spent,
}

/// The subject as layer 0 believes it: the only occupant, where and with what
/// provenance. It knows nothing of the second level; `project` builds the
/// published scene (spec 1006).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Subject {
    pub zone: Zone,
    pub bbox: Rect,
    pub confidence: f32,
    pub provenance: Provenance,
}

/// What the turn concluded, with the layer-0 transition if there was one.
#[derive(Debug, Clone, PartialEq)]
pub struct Believed {
    pub occupancy: Occupancy,
    pub subject: Option<Subject>,
    /// The subject's pose and depth, if enriched or kept.
    pub body: Option<SubjectBody>,
    pub transition: Option<Transition>,
}

impl Belief {
    pub fn latch(&self) -> LatchState {
        self.latch
    }

    /// How many are seen in bed, stabilized: if the main detector sees fewer,
    /// it is a possible loss.
    pub fn bed_seen(&self) -> u32 {
        self.occupancy.seen().bed
    }

    /// The box of the subject's last pose: the motion reference.
    pub fn pose_reference(&self) -> Option<Rect> {
        self.subject.reference()
    }

    /// The sample of the subject's last pose: its age (903).
    pub fn pose_since(&self) -> Option<SampleRef> {
        self.subject.since()
    }

    /// Updates the belief with what was seen of the subject's body: pose and
    /// depth (second attention point). Like [`Belief::observe`], but with the
    /// enrichment instead of the bodies.
    pub(crate) fn observe_body(
        &mut self,
        at: SampleRef,
        mut b: Believed,
        e: Enrichment,
    ) -> Believed {
        b.body = self.subject.observe(at, b.subject.as_ref(), e);
        b
    }

    /// Reinforcement looked: the latch is spent until the main detector sees
    /// someone in bed again.
    pub(crate) fn reinforced(&mut self) {
        self.latch = LatchState::Spent;
    }

    /// Updates the belief with the bodies of a sample. A failure or a sample
    /// without zones proves nothing: they do not change the count (rule 1).
    pub(crate) fn observe(
        &mut self,
        at: SampleRef,
        admission: &Admission,
        policy: &Policy,
    ) -> Believed {
        let current = admission.bodies.bodies();
        let transition =
            current.and_then(|bodies| self.occupancy.step(at, bodies, &policy.persistence));
        self.rearm(admission.primary.bed);
        let subject = self.subject_now(current);
        if subject.is_none() {
            self.subject.forget();
        }
        let bed_known = !matches!(admission.bodies, BodyEvidence::Unzoned(_));
        Believed {
            occupancy: self.occupancy.snapshot(at, bed_known),
            subject,
            body: None,
            transition,
        }
    }

    /// The main detector saw someone in bed: if lost, it can be reinforced.
    /// With nobody seen in bed, there is no loss to reinforce.
    fn rearm(&mut self, primary_bed: u32) {
        if primary_bed > 0 {
            self.latch = LatchState::Armed;
        } else if self.occupancy.seen().bed == 0 {
            self.latch = LatchState::Disarmed;
        }
    }

    /// The subject: the only occupant, seen in this sample or retained.
    fn subject_now(&self, current: Option<&Bodies>) -> Option<Subject> {
        let (zone, now, last) = match self.occupancy.room() {
            Room::Empty | Room::Accompanied(_) => return None,
            Room::Alone(Alone::InBed(InBed::Seen)) => {
                let now = current.and_then(|b| b.bed.first()).map(|b| &b.detection);
                (Zone::Bed, now, self.occupancy.last_seen_in_bed())
            }
            Room::Alone(Alone::Out) => {
                let now = current.and_then(|b| b.floor.first()).map(|b| &b.detection);
                (Zone::Floor, now, self.occupancy.last_seen_on_floor())
            }
            Room::Alone(Alone::InBed(InBed::Held)) => {
                let kept = self.occupancy.held().first()?;
                let since = kept.since;
                return Some(subject(
                    Zone::Bed,
                    &kept.detection,
                    Provenance::Retained { since },
                ));
            }
        };
        match (now, last) {
            (Some(d), _) => Some(subject(zone, d, Provenance::Current)),
            (None, Some(last)) => last
                .bodies
                .first()
                .map(|d| subject(zone, d, Provenance::Retained { since: last.at })),
            (None, None) => None,
        }
    }
}

fn subject(zone: Zone, d: &Detection, provenance: Provenance) -> Subject {
    Subject {
        zone,
        bbox: d.bbox,
        confidence: d.confidence,
        provenance,
    }
}
