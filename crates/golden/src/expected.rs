//! A scene's expectations against what the engine published: at the end of
//! each span, the occupants and the bed state. It is the same definition for
//! the BDD harness and for replaying a scene through the detector.

use crate::language::{Expected, ExpectedBed, Samples, Scene};
use scene_core::contract::{BedState, Scene as PublishedScene};

/// How the published scene differs from what was expected; `None` if it matches.
pub fn difference(expected: &Expected, scene: &PublishedScene) -> Option<String> {
    if let Some(occupants) = expected.occupants
        && scene.room.occupants != occupants
    {
        return Some(format!(
            "occupants: {} and {occupants} were expected",
            scene.room.occupants
        ));
    }
    let bed = expected.bed?;
    let state = scene.bed.state;
    let matches = match bed {
        ExpectedBed::Empty => state == BedState::Empty,
        ExpectedBed::Occupied => matches!(state, BedState::Seen | BedState::Held),
        ExpectedBed::Seen => state == BedState::Seen,
        ExpectedBed::Held => state == BedState::Held,
        ExpectedBed::Unknown => state == BedState::Unknown,
    };
    (!matches).then(|| format!("bed: {state:?} and {bed:?} was expected"))
}

/// The result of a span with an expectation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Outcome {
    pub samples: Samples,
    pub difference: Option<String>,
}

/// Checks a scene's expectations, span by span.
#[derive(Debug, Clone)]
pub struct Verification {
    pending: Vec<(Samples, Expected)>,
    outcomes: Vec<Outcome>,
}

impl Verification {
    pub fn new(scene: &Scene) -> Self {
        Self {
            pending: scene
                .spans
                .iter()
                .filter(|s| s.expected != Expected::default())
                .map(|s| (s.samples, s.expected))
                .collect(),
            outcomes: Vec::new(),
        }
    }

    /// The scene published for sample `n`: if it closes a span with an
    /// expectation, it is compared.
    pub fn observe(&mut self, n: u32, scene: &PublishedScene) {
        for (samples, expected) in &self.pending {
            if samples.to == n {
                self.outcomes.push(Outcome {
                    samples: *samples,
                    difference: difference(expected, scene),
                });
            }
        }
    }

    pub fn outcomes(&self) -> &[Outcome] {
        &self.outcomes
    }

    /// Were all spans checked, and do they all match?
    pub fn matches(&self) -> bool {
        self.outcomes.len() == self.pending.len()
            && self.outcomes.iter().all(|o| o.difference.is_none())
    }
}
