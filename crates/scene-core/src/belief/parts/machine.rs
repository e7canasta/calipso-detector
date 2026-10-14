//! The parts' machine: it keeps where each part was last seen while layer 0
//! has a subject, and nothing otherwise. Markov (P1): the retention and the
//! places that are holding, in source time (rule 9), with layer 0's dwell.

use super::reading::{self, Facts};
use super::rules::decide;
use super::{ByPart, Concluded, Look, PartsReading, PartsTransition, Place};
use crate::belief::stabilizer::Stabilizer;
use crate::policy::Persistence;
use crate::time::SampleRef;

/// The places of the subject's parts, while there is a subject.
#[derive(Debug, Clone, Default)]
pub(crate) struct Machine {
    /// Where each part was last seen, held where it was not: the retention.
    last: Option<ByPart<Place>>,
    /// The places without the held marks, stabilized: a part that flickers
    /// between inside and outside does not move them.
    places: Option<Stabilizer<ByPart<Place>>>,
    /// The reading of the places now (retained included), stabilized with its
    /// own dwell: the urgent ones count sooner.
    reading: Option<Stabilizer<PartsReading>>,
}

impl Machine {
    /// One sample, with whether layer 0 has a subject and what was seen. The
    /// rules give the places now; they count once they held layer 0's dwell.
    /// Held marks what was not seen in this sample, not stabilized: it says
    /// where the datum comes from, like a provenance.
    pub(crate) fn step(
        &mut self,
        at: SampleRef,
        subject: bool,
        seen: ByPart<Look>,
        persistence: &Persistence,
    ) -> Option<Concluded> {
        let Some(now) = decide(self.last, subject, seen) else {
            *self = Self::default();
            return None;
        };
        self.last = Some(now);
        let places = self
            .places
            .get_or_insert_with(|| Stabilizer::new(ByPart::default()));
        let change = places.observe(at, unheld(now), persistence);
        let stable = places.stable();
        let read_now = reading::read(Facts::of(&now));
        let read = self
            .reading
            .get_or_insert_with(|| Stabilizer::new(PartsReading::Undecided));
        read.observe(
            at,
            read_now,
            &Persistence {
                dwell_secs: reading::dwell(read_now, persistence.dwell_secs),
            },
        );
        Some(Concluded {
            seen,
            state: held_as(stable, now),
            places: stable,
            pending: places.pending().map(|c| (c.value, c.first)),
            transition: change.map(|c| PartsTransition {
                at,
                before: c.from,
                seen,
                first_seen: c.first,
                after: c.to,
            }),
            reading: read.stable(),
        })
    }
}

fn unheld(places: ByPart<Place>) -> ByPart<Place> {
    let clear = |p: Place| Place { held: false, ..p };
    ByPart {
        bed: places.bed.map(clear),
        headboard: places.headboard.map(clear),
        whole: places.whole.map(clear),
    }
}

/// The stable places, held where this sample did not see the part and there
/// is a place to hold.
fn held_as(stable: ByPart<Place>, now: ByPart<Place>) -> ByPart<Place> {
    fn each<const N: usize>(stable: [Place; N], now: [Place; N]) -> [Place; N] {
        let mut out = stable;
        for (place, now) in out.iter_mut().zip(now) {
            place.held = now.held && place.known();
        }
        out
    }
    ByPart {
        bed: each(stable.bed, now.bed),
        headboard: each(stable.headboard, now.headboard),
        whole: each(stable.whole, now.whole),
    }
}
