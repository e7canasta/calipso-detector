//! The span: since the last stable count, the samples whose
//! raw count has not held yet, with what was seen along the way. It joins what
//! used to be synchronized by hand: when the count changed
//! ([`stabilizer`](crate::belief::stabilizer)) and where whoever changed it came from
//! (the window). It delivers two columns of the table: what we believed we saw
//! and what we saw.

use super::conservation::SeenMark;
use crate::belief::stabilizer::{Candidate, Change, Stabilizer};
use crate::evidence::Bodies;
use crate::policy::Persistence;
use crate::time::SampleRef;
use crate::zone::Counts;

/// A span that held: the stable count it replaces, what was seen and from
/// which sample.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Settled {
    pub(super) from: Counts,
    pub(super) seen: SeenMark,
    pub(super) first: SampleRef,
}
/// What was seen raw while the count stabilized, relative to what was seen
/// with the count at rest: who appeared on the floor far from the bed (rule 5)
/// and how many extra were seen on the floor (rule 6).
#[derive(Debug, Clone, Copy, Default)]
struct Window {
    far_at_rest: u32,
    near_at_rest: u32,
    floor_at_rest: u32,
    from_door: u32,
    passing: u32,
    /// How many extra were already seen near the bed: whoever of them is
    /// later far walked away, and did not come from the door (rule 5, refined).
    walked: u32,
}

impl Window {
    /// The raw count matches the stable one: nobody is on the way.
    fn rest(&mut self, bodies: &Bodies) {
        let floor = bodies.counts().floor;
        *self = Self {
            far_at_rest: bodies.far(),
            near_at_rest: floor - bodies.far(),
            floor_at_rest: floor,
            from_door: 0,
            passing: 0,
            walked: 0,
        };
    }

    /// `near_first`: rule 5, refined (white book, volume VII, ch. 17): those
    /// seen near the bed before are not from the door when they are far.
    fn track(&mut self, bodies: &Bodies, near_first: bool) {
        let extra = |now: u32, rest: u32| now.saturating_sub(rest);
        let far = extra(bodies.far(), self.far_at_rest);
        let walked = if near_first { self.walked } else { 0 };
        self.from_door = self.from_door.max(far.saturating_sub(walked));
        let near = bodies.counts().floor - bodies.far();
        self.walked = self.walked.max(extra(near, self.near_at_rest));
        self.passing = self
            .passing
            .max(extra(bodies.counts().floor, self.floor_at_rest));
    }
}

#[derive(Debug, Clone)]
pub(super) struct Span {
    counts: Stabilizer<Counts>,
    window: Window,
}

impl Span {
    pub(super) fn new(stable: Counts) -> Self {
        Self {
            counts: Stabilizer::new(stable),
            window: Window::default(),
        }
    }

    /// At rest with these bodies: as if the stable count had just been
    /// confirmed with them.
    #[cfg(test)]
    pub(super) fn resting(stable: Counts, bodies: &Bodies) -> Self {
        let mut span = Self::new(stable);
        span.window.rest(bodies);
        span
    }

    pub(super) fn stable(&self) -> Counts {
        self.counts.stable()
    }

    /// The new count that has not held yet, if there is one.
    pub(super) fn pending(&self) -> Option<&Candidate<Counts>> {
        self.counts.pending()
    }

    /// A transition's markers do not cross sessions. The references of the
    /// confirmed count are kept, like the belief.
    pub(super) fn new_session(&mut self) {
        self.window.from_door = 0;
        self.window.passing = 0;
        self.window.walked = 0;
    }

    /// One sample. If the new count held as required, returns the span that
    /// closed, and the span goes back to rest. `near_first`: rule 5, refined.
    pub(super) fn observe(
        &mut self,
        at: SampleRef,
        bodies: &Bodies,
        p: &Persistence,
        near_first: bool,
    ) -> Option<Settled> {
        let raw = bodies.counts();
        if raw == self.counts.stable() {
            self.window.rest(bodies);
        } else {
            self.window.track(bodies, near_first);
        }
        let Change { from, to, first } = self.counts.observe(at, raw, p)?;
        let seen = SeenMark {
            counts: to,
            from_door: self.window.from_door,
            passing: self.window.passing,
        };
        self.window.rest(bodies);
        Some(Settled { from, seen, first })
    }
}
