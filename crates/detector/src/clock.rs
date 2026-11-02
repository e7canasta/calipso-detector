//! The shell's clock: where the shell reads the instant of a measurement (the
//! time a look took, the timing of a sample, the stall deadline). Production
//! reads the system's clock; replay and the tests read a fixed one, so every
//! measured time is zero and two runs give the same bytes. The core never
//! reads a clock (rule 8). ADR 2026-11-02-reloj-inyectado-en-la-cascara.

use std::time::Instant;

/// Where the shell reads the instant of a measurement.
pub trait Clock {
    fn now(&self) -> Instant;
}

/// The system's monotonic clock: production.
#[derive(Debug, Clone, Copy, Default)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> Instant {
        Instant::now()
    }
}

/// One instant for every reading, so every measured time is zero. The instant
/// is taken when the clock is made: copies share it, so a run that passes the
/// same clock to its driver and to its source measures zero on both sides.
#[derive(Debug, Clone, Copy)]
pub struct FixedClock(Instant);

impl FixedClock {
    pub fn new() -> Self {
        Self(Instant::now())
    }
}

impl Default for FixedClock {
    fn default() -> Self {
        Self::new()
    }
}

impl Clock for FixedClock {
    fn now(&self) -> Instant {
        self.0
    }
}
