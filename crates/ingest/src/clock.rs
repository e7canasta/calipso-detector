//! The stream stage's clock: where a sample's arrival is stamped (`received_at`,
//! the start of the wait that `wait_ms` measures). The caller chooses it; the
//! shell's clocks (`SystemClock`, `FixedClock`) implement it in
//! `detector::clock`. ADR 2026-11-02 (reloj inyectado en la cáscara).

use std::time::Instant;

/// Where the stream stage reads the instant a sample arrives.
pub trait Clock {
    fn now(&self) -> Instant;
}
