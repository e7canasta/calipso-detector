//! The second level and layer 1, on what the turn believed: the refinement reads
//! the body and the places, and the parts and the bed's posture follow it.

use crate::cycle::{Engine, Observed, Refined};
use crate::refine::BodyPlaces;
use crate::room::Room;
use crate::time::SampleRef;

impl Engine {
    /// The second level reads what was believed and delivers its result, with
    /// the digit the verifier filled if an answer arrived (`filled`, from the
    /// turn's opening); layer 1 reads it.
    pub(in crate::cycle) fn refine(
        &mut self,
        at: SampleRef,
        observed: Observed,
        room: Option<&Room>,
        filled: Option<BodyPlaces>,
    ) -> Refined {
        let refinement = crate::refine::refine(
            observed.believed.subject.as_ref().map(|s| s.zone),
            observed.believed.body.as_ref(),
            room,
            self.enrich.as_ref(),
            self.verify_policy.as_ref(),
            filled,
        );
        // The parts, at layer 0's compass: what the sample saw of them.
        let seen = crate::refine::parts::seen(&crate::refine::parts::Sample {
            looked: !matches!(
                observed.evidence.bodies.bodies,
                crate::evidence::BodyEvidence::Unavailable(_)
            ),
            subject: observed.believed.subject.as_ref(),
            body: observed.believed.body.as_ref(),
            faces: &observed.evidence.faces,
            room,
            min_score: self.enrich.as_ref().map(|e| e.min_keypoint_score.get()),
        });
        // Layer 1, on layer 0 and what the second level perceived.
        let (parts, bed_posture) = self.follow(at, seen, refinement.sighting);
        Refined {
            observed,
            refinement,
            bed_posture,
            parts,
        }
    }
}
