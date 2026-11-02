//! The slow verifier in the turn: its doubts are asked, its bursts are kept, its
//! requests are dispatched, and a turn that reaches it moves a burst on.

use crate::attention::{self, Consideration, LookResult, VerifyLook};
use crate::belief::bed_posture;
use crate::cycle::Engine;
use crate::look::Failure;
use crate::refine::BodyPlaces;
use crate::time::SampleRef;
use crate::verification::{Effect, Event};

impl Engine {
    /// Whether a doubt is worth the slow verifier, over what the turn observed.
    pub(in crate::cycle) fn verify_look(
        &self,
        g: &attention::Glimpse<'_>,
        layer_1: Option<bed_posture::BedPostureState>,
        places: Option<BodyPlaces>,
    ) -> Consideration<VerifyLook> {
        attention::verify(
            g,
            &self.verification,
            layer_1,
            places,
            self.verify_policy.as_ref(),
        )
    }

    /// A kept crop's reading: the verifier's burst takes it, or it fails.
    pub(in crate::cycle) fn kept(
        &mut self,
        at: SampleRef,
        kept: Result<f32, Failure>,
    ) -> (LookResult, Vec<Effect>) {
        match kept {
            Ok(sharpness) => {
                let effects = self.verification.step(Event::KeptCrop {
                    at,
                    sharpness,
                    policy: self.verify_rules.sharpness,
                });
                // A kept crop's first effect is its verdict.
                let counted = matches!(effects.first(), Some(Effect::Kept { counted: true }));
                (LookResult::Kept { sharpness, counted }, effects)
            }
            Err(failure) => {
                self.verification.step(Event::KeepFailed);
                (LookResult::failed(failure), Vec::new())
            }
        }
    }

    /// A question to the verifier is decided: sent now, or collected as a burst
    /// (white book, volume III, ch. 12). The effects say which.
    pub(in crate::cycle) fn issue(&mut self, at: SampleRef, look: &VerifyLook) -> Vec<Effect> {
        let burst = self
            .verify_rules
            .frames
            .map(|frames| frames.of(look.question))
            .unwrap_or_default();
        self.verification.step(Event::Ask {
            at,
            look: *look,
            burst,
            judged: self.verify_rules.sharpness.is_some(),
        })
    }

    /// The verifier's request left (`ok`) or could not.
    pub(in crate::cycle) fn dispatched(&mut self, ok: bool) {
        self.verification.step(Event::Dispatched { ok });
    }

    /// The turn reaches the verifier's last attention point: a burst being
    /// collected advances with this sample. The effects say what it does.
    pub(in crate::cycle) fn verifier_turn(
        &mut self,
        at: SampleRef,
        layer_1: Option<bed_posture::BedPostureState>,
        places: Option<BodyPlaces>,
    ) -> Vec<Effect> {
        self.verification.step(Event::Turn {
            at,
            layer_1,
            places,
        })
    }
}
