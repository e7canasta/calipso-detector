//! Stabilizing: a new value counts only once it has held (against flicker).
//! It is pure mechanism: it does not know what it counts, only when a value
//! held long enough in source time (rule 9). Layer 0 stabilizes the counts
//! with it and layer 1 the bed posture. Counting samples would make the
//! same policy depend on the camera cadence.

use crate::policy::Persistence;
use crate::time::SampleRef;

/// A new value that repeats, not yet stabilized.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Candidate<T> {
    pub(super) value: T,
    pub(super) first: SampleRef,
}

impl<T> Candidate<T> {
    /// Source seconds since the first observation.
    pub(super) fn elapsed(&self, at: SampleRef) -> f64 {
        at.source_secs.0 - self.first.source_secs.0
    }
}

/// A new value that held: the previous stable one, the one replacing it and
/// the first sample that saw it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Change<T> {
    pub(super) from: T,
    pub(super) to: T,
    pub(super) first: SampleRef,
}

#[derive(Debug, Clone)]
pub(super) struct Stabilizer<T> {
    stable: T,
    pending: Option<Candidate<T>>,
}

impl<T: Copy + Eq> Stabilizer<T> {
    pub(super) fn new(initial: T) -> Self {
        Self {
            stable: initial,
            pending: None,
        }
    }

    pub(super) fn stable(&self) -> T {
        self.stable
    }

    pub(super) fn pending(&self) -> Option<&Candidate<T>> {
        self.pending.as_ref()
    }

    /// One observation. If the new value held as required, returns the change.
    /// Any other value, the stable one included, discards the candidate; and a
    /// new session restarts it: source time is not compared across sessions.
    pub(super) fn observe(&mut self, at: SampleRef, raw: T, p: &Persistence) -> Option<Change<T>> {
        if raw == self.stable {
            self.pending = None;
            return None;
        }
        let candidate = match self.pending {
            Some(c) if c.value == raw && c.first.session == at.session => c,
            _ => Candidate {
                value: raw,
                first: at,
            },
        };
        if candidate.elapsed(at) >= p.dwell_secs {
            let from = std::mem::replace(&mut self.stable, raw);
            self.pending = None;
            Some(Change {
                from,
                to: raw,
                first: candidate.first,
            })
        } else {
            self.pending = Some(candidate);
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::time::{SessionId, SourceSecs};

    fn at(secs: f64) -> SampleRef {
        SampleRef {
            session: SessionId(1),
            sequence: 0,
            source_secs: SourceSecs(secs),
        }
    }

    const TWO: Persistence = Persistence { dwell_secs: 2.0 };

    #[test]
    fn changes_when_it_held_for_the_required_time() {
        let mut s = Stabilizer::new(0);
        assert_eq!(s.observe(at(1.0), 1, &TWO), None);
        assert_eq!(s.observe(at(2.0), 1, &TWO), None);
        assert_eq!(
            s.observe(at(3.0), 1, &TWO),
            Some(Change {
                from: 0,
                to: 1,
                first: at(1.0)
            })
        );
        assert_eq!(s.stable(), 1);
    }

    #[test]
    fn time_rules_not_the_number_of_samples() {
        // One sample every 4 s: the second is already past the required 2 s.
        let mut s = Stabilizer::new(0);
        assert_eq!(s.observe(at(0.0), 1, &TWO), None);
        assert_eq!(
            s.observe(at(4.0), 1, &TWO),
            Some(Change {
                from: 0,
                to: 1,
                first: at(0.0)
            })
        );
        // Ten samples in one second are not enough.
        let mut s = Stabilizer::new(0);
        for i in 0..10 {
            assert_eq!(s.observe(at(f64::from(i) * 0.1), 1, &TWO), None);
        }
    }

    #[test]
    fn without_dwell_the_first_observation_counts() {
        let mut s = Stabilizer::new(0);
        assert_eq!(
            s.observe(at(1.0), 1, &Persistence { dwell_secs: 0.0 }),
            Some(Change {
                from: 0,
                to: 1,
                first: at(1.0)
            })
        );
    }

    #[test]
    fn a_flicker_discards_the_candidate() {
        let mut s = Stabilizer::new(0);
        s.observe(at(1.0), 1, &TWO);
        s.observe(at(2.0), 0, &TWO);
        assert!(s.pending().is_none());
        s.observe(at(3.0), 1, &TWO);
        assert_eq!(s.pending().map(|c| c.first), Some(at(3.0)));
    }

    #[test]
    fn a_candidate_does_not_cross_sessions() {
        // After a reconnection source time starts over: what was gathered in
        // the previous session is neither added nor compared.
        let other = |secs| SampleRef {
            session: SessionId(2),
            sequence: 0,
            source_secs: SourceSecs(secs),
        };
        let mut s = Stabilizer::new(0);
        s.observe(at(100.0), 1, &TWO);
        assert_eq!(s.observe(other(0.0), 1, &TWO), None);
        assert_eq!(s.pending().map(|c| c.first), Some(other(0.0)));
        assert_eq!(
            s.observe(other(2.0), 1, &TWO),
            Some(Change {
                from: 0,
                to: 1,
                first: other(0.0)
            })
        );
    }
}
