//! The belief: how many are in the room, who is still in bed even when unseen,
//! who the subject is and, for the one left lying, their bed posture. One
//! aggregate per camera; it never sees a raw reading.

pub(crate) mod bed_posture;
mod occupancy;
pub(crate) mod parts;
mod stabilizer;
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

pub use bed_posture::{BedPosture, BedPostureSeen, BedPostureState};
pub use parts::PartsReading;

pub(crate) use occupancy::{Alone, BedPhase, InBed, Occupancy, Room, Tally, Transition};
pub(crate) use subject::{Enrichment, SubjectBody};

/// The occupants layer 0 believes in, by where they are: off the bed, seen in
/// bed and held in bed. The book's `RBH` is `(total, bed + held, held)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Occupants {
    pub floor: u32,
    pub bed: u32,
    pub held: u32,
}

impl Occupants {
    pub(crate) fn total(self) -> u32 {
        self.floor + self.bed + self.held
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Fact {
    Entered,
    /// Was in bed before the bed had a history: adopted (rule 6).
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
pub(crate) struct Belief {
    occupancy: Machine,
    subject: SubjectMemory,
    /// Layer 1: followed only with its policy.
    bed_posture: bed_posture::Machine,
    /// The occupancy of the subject's parts, at layer 0's compass.
    parts: parts::Machine,
}

/// The subject as layer 0 believes it: the only occupant, where and with what
/// provenance. It knows nothing of the second level; `project` builds the
/// published scene.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Subject {
    pub zone: Zone,
    pub bbox: Rect,
    pub confidence: f32,
    pub provenance: Provenance,
}

/// What the turn concluded, with the layer-0 transition if there was one.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Believed {
    pub occupancy: Occupancy,
    pub subject: Option<Subject>,
    /// The subject's pose and depth, if enriched or kept.
    pub body: Option<SubjectBody>,
    pub transition: Option<Transition>,
}

impl Belief {
    /// The room's mode, as layer 0 believes it.
    pub(crate) fn room(&self) -> Room {
        self.occupancy.room()
    }

    /// No new count is stabilizing: what is believed is not about to change.
    pub(crate) fn settled(&self) -> bool {
        !self.occupancy.changing()
    }

    /// How many are seen in bed, stabilized: if the main detector sees fewer,
    /// it is a possible loss.
    pub(crate) fn bed_seen(&self) -> u32 {
        self.occupancy.seen().bed
    }

    /// Layer 1's stable state, if it is following one.
    pub(crate) fn bed_posture_state(&self) -> Option<bed_posture::BedPostureState> {
        self.bed_posture.stable()
    }

    /// The oldest body held in bed, by the last sample it was seen in.
    pub(crate) fn held_since(&self) -> Option<SampleRef> {
        self.occupancy.held().first().map(|h| h.since)
    }

    /// The box of the subject's last pose: the motion reference.
    pub(crate) fn pose_reference(&self) -> Option<Rect> {
        self.subject.reference()
    }

    /// The sample of the subject's last pose: its age.
    pub(crate) fn pose_since(&self) -> Option<SampleRef> {
        self.subject.since()
    }

    /// Updates the belief with what was seen of the subject's body: pose and
    /// depth (second attention point). Like [`Belief::observe`], but with the
    /// enrichment instead of the bodies.
    pub(crate) fn observe_body(
        &mut self,
        at: SampleRef,
        mut believed: Believed,
        enrichment: Enrichment,
    ) -> Believed {
        believed.body = self
            .subject
            .observe(at, believed.subject.as_ref(), enrichment);
        believed
    }

    /// Layer 1, if its policy follows it: the wire from layer 0 to layer 1.
    /// Its input is layer 0's state now (the resident alone in bed, seen or
    /// held, or nothing) and what the second level perceived. It reads layer
    /// 0 and never changes it.
    pub(crate) fn observe_bed_posture(
        &mut self,
        at: SampleRef,
        sighting: bed_posture::Sighting,
        policy: &Policy,
    ) -> Option<bed_posture::Concluded> {
        let policy = policy.bed_posture.as_ref()?;
        let l0 = match self.occupancy.room() {
            Room::Alone(Alone::InBed(in_bed)) => Some(in_bed),
            Room::Empty | Room::Alone(Alone::Out) | Room::Accompanied(_) => None,
        };
        self.bed_posture.step(at, l0, sighting, policy)
    }

    /// The occupancy of the subject's parts: the wire from layer 0. With a
    /// subject (the only occupant, anywhere) and what the sample saw of its
    /// parts; without one, or without the bed's plane (`None`), there are no
    /// places. Same compass as layer 0: once per sample, its dwell.
    pub(crate) fn observe_parts(
        &mut self,
        at: SampleRef,
        seen: Option<parts::ByPart<parts::Look>>,
        policy: &Policy,
    ) -> Option<parts::Concluded> {
        let subject = seen.is_some() && matches!(self.occupancy.room(), Room::Alone(_));
        self.parts
            .step(at, subject, seen.unwrap_or_default(), &policy.persistence)
    }

    /// Updates the belief with the bodies of a sample. A failure or a sample
    /// without zones proves nothing: they do not change the count (rule 1).
    pub(crate) fn observe(
        &mut self,
        at: SampleRef,
        admission: &Admission,
        policy: &Policy,
    ) -> Believed {
        let current = admission.bodies.seen();
        let transition = current.and_then(|bodies| {
            self.occupancy
                .step(at, bodies, &policy.persistence, policy.held_left)
        });
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

fn subject(zone: Zone, detection: &Detection, provenance: Provenance) -> Subject {
    Subject {
        zone,
        bbox: detection.bbox,
        confidence: detection.confidence,
        provenance,
    }
}
