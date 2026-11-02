//! The slow verifier in the turn: its doubts are asked, its bursts are kept, its
//! requests are dispatched, and a turn that reaches it moves a burst on. Each
//! operation answers with what it does, typed: the turn matches on the answer.

use crate::attention::{self, Considered, LookResult, VerifyLook};
use crate::belief::bed_posture;
use crate::cycle::Engine;
use crate::look::Failure;
use crate::look::LookKind;
use crate::refine::BodyPlaces;
use crate::time::SampleRef;
use crate::verification::{AskOut, Event, KeptOut, TurnOut};

impl Engine {
    /// Whether a doubt is worth the slow verifier, over what the turn observed.
    pub(in crate::cycle) fn verify_look(
        &self,
        g: &attention::Glimpse<'_>,
        layer_1: Option<bed_posture::BedPostureState>,
        places: Option<BodyPlaces>,
    ) -> Considered<VerifyLook> {
        Considered::of(
            LookKind::Verify,
            attention::verify(
                g,
                &self.verification,
                layer_1,
                places,
                self.verify_policy.as_ref(),
            ),
        )
    }

    /// A kept crop's reading: the verifier's burst takes it, or it fails. A
    /// failed crop has no burst outcome: there is nothing kept to judge.
    pub(in crate::cycle) fn kept(
        &mut self,
        at: SampleRef,
        kept: Result<f32, Failure>,
    ) -> (LookResult, Option<KeptOut>) {
        match kept {
            Ok(sharpness) => {
                let out = self
                    .verification
                    .kept_crop(at, sharpness, self.verify_rules.sharpness);
                let counted = out.counted;
                (LookResult::Kept { sharpness, counted }, Some(out))
            }
            Err(failure) => {
                self.verification.step(Event::KeepFailed);
                (LookResult::failed(failure), None)
            }
        }
    }

    /// A question to the verifier is decided: sent now, or collected as a burst
    /// (white book, volume III, ch. 12). The answer says which.
    pub(in crate::cycle) fn issue(&mut self, at: SampleRef, look: &VerifyLook) -> AskOut {
        let burst = self
            .verify_rules
            .frames
            .map(|frames| frames.of(look.question))
            .unwrap_or_default();
        self.verification
            .ask(at, *look, burst, self.verify_rules.sharpness.is_some())
    }

    /// The verifier's request left (`ok`) or could not.
    pub(in crate::cycle) fn dispatched(&mut self, ok: bool) {
        self.verification.step(Event::Dispatched { ok });
    }

    /// The turn reaches the verifier's last attention point: a burst being
    /// collected advances with this sample. The answer says what it does.
    pub(in crate::cycle) fn verifier_turn(
        &mut self,
        at: SampleRef,
        layer_1: Option<bed_posture::BedPostureState>,
        places: Option<BodyPlaces>,
    ) -> TurnOut {
        self.verification.turn(at, layer_1, places)
    }
}
