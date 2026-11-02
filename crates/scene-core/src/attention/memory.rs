//! What attention remembers of what it already looked at: the main
//! detector's lens, the reinforcement latch and the last calibration attempt.

use crate::time::SampleRef;
use crate::time::secs_since;

/// What attention remembers of what it already looked at, and nothing else:
/// the main detector's lens, the reinforcement latch and the last time the
/// empty bed was measured. Attention reads it; `cycle` notes each look.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct Memory {
    pub(super) lens: Lens,
    latch: LatchState,
    /// The last calibration attempt, for the retry delay.
    pub(super) calibration_attempt: Option<SampleRef>,
}

/// The reinforcement latch: "someone was seen in bed", so a loss there can
/// be looked at again, once.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) enum LatchState {
    /// Nobody was seen in bed.
    #[default]
    Disarmed,
    /// The main detector saw someone in bed: if lost, it can be reinforced.
    Armed,
    /// This loss was already reinforced.
    Spent,
}

impl Memory {
    /// What a site remembers when the loop starts: the last calibration
    /// attempt, if it was saved.
    pub(crate) fn of(calibration_attempt: Option<SampleRef>) -> Self {
        Self {
            lens: Lens::default(),
            latch: LatchState::default(),
            calibration_attempt,
        }
    }

    pub(crate) fn latch(&self) -> LatchState {
        self.latch
    }

    /// The main detector saw `primary_bed` in bed, and layer 0 has `bed_seen`
    /// seen in bed once it believed: someone seen in bed arms the latch; with
    /// nobody seen in bed, there is no loss to reinforce.
    pub(crate) fn rearm(&mut self, primary_bed: u32, bed_seen: u32) {
        if primary_bed > 0 {
            self.latch = LatchState::Armed;
        } else if bed_seen == 0 {
            self.latch = LatchState::Disarmed;
        }
    }

    /// Reinforcement looked: the latch is spent until the main detector sees
    /// someone in bed again.
    pub(crate) fn reinforced(&mut self) {
        self.latch = LatchState::Spent;
    }

    /// The main detector read the whole frame in this sample.
    pub(crate) fn looked_wide(&mut self, at: SampleRef) {
        self.lens.looked_wide(at);
    }

    /// Calibration was requested: the retry delay runs from here.
    pub(crate) fn calibration_attempted(&mut self, at: SampleRef) {
        self.calibration_attempt = Some(at);
    }
}

/// What the main detector's lens remembers: the last sample it read the whole
/// frame.
#[derive(Debug, Clone, Copy, Default)]
pub(super) struct Lens {
    last_wide: Option<SampleRef>,
}

impl Lens {
    /// The main detector read the whole frame in this sample.
    fn looked_wide(&mut self, at: SampleRef) {
        self.last_wide = Some(at);
    }

    /// Is the whole frame due? Never looked, another session or the period
    /// passed (rule 9).
    pub(super) fn wide_due(&self, now: SampleRef, every_secs: f64) -> bool {
        self.last_wide
            .and_then(|last| secs_since(now, last))
            .is_none_or(|age| age >= every_secs)
    }
}
