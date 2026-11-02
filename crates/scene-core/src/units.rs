//! Units in the type (ADR 2026-11-02, unidades-en-el-tipo). A duration in source
//! seconds and a score in `[0, 1]` are different types: one cannot be passed
//! where the other goes, and a `NaN`, a negative time or a confidence of `1.5`
//! cannot enter the core, because the constructors are the only door. Both
//! serialize as the bare number, so the TOML and the published profile keep
//! their form and their hash.

use serde::{Deserialize, Serialize};
use std::fmt;

/// Why a number is not a value of a unit.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum UnitError {
    /// `NaN` or infinite.
    NotFinite,
    /// A time below zero: source time does not run backwards.
    Negative,
    /// A confidence outside `[0, 1]`.
    OutOfRange,
}

impl fmt::Display for UnitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFinite => f.write_str("is not a finite number"),
            Self::Negative => f.write_str("is negative"),
            Self::OutOfRange => f.write_str("is not between 0 and 1"),
        }
    }
}

impl std::error::Error for UnitError {}

/// A duration in source seconds (rule 9): finite and not negative. Zero is
/// valid: a single observation, no wait.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(try_from = "f64", into = "f64")]
pub struct Seconds(f64);

impl Seconds {
    /// No time at all: a single observation, no wait.
    pub const ZERO: Self = Self(0.0);
    /// One source second.
    pub const ONE: Self = Self(1.0);
    /// Two source seconds: the default dwell of a new count.
    pub const TWO: Self = Self(2.0);

    /// Builds a duration. Rejects `NaN`, infinities and negative values.
    pub fn new(seconds: f64) -> Result<Self, UnitError> {
        if !seconds.is_finite() {
            Err(UnitError::NotFinite)
        } else if seconds < 0.0 {
            Err(UnitError::Negative)
        } else {
            Ok(Self(seconds))
        }
    }

    /// Half of it: a unit kept, so it stays a valid duration.
    pub fn half(self) -> Self {
        Self(self.0 / 2.0)
    }

    /// The number of seconds, for the arithmetic of the source time.
    pub fn get(self) -> f64 {
        self.0
    }
}

impl Default for Seconds {
    fn default() -> Self {
        Self::ZERO
    }
}

impl TryFrom<f64> for Seconds {
    type Error = UnitError;

    fn try_from(seconds: f64) -> Result<Self, Self::Error> {
        Self::new(seconds)
    }
}

impl From<Seconds> for f64 {
    fn from(seconds: Seconds) -> Self {
        seconds.0
    }
}

/// A confidence or a threshold on a score: finite and between 0 and 1, both
/// included.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(try_from = "f32", into = "f32")]
pub struct Confidence(f32);

impl Confidence {
    /// No confidence at all.
    pub const ZERO: Self = Self(0.0);
    /// Even odds: the default minimum of a detection.
    pub const HALF: Self = Self(0.5);

    /// Builds a confidence. Rejects `NaN` and anything outside `[0, 1]`.
    pub fn new(value: f32) -> Result<Self, UnitError> {
        if value.is_nan() {
            Err(UnitError::NotFinite)
        } else if !(0.0..=1.0).contains(&value) {
            Err(UnitError::OutOfRange)
        } else {
            Ok(Self(value))
        }
    }

    /// The value, for the comparisons with the scores the models return.
    pub fn get(self) -> f32 {
        self.0
    }
}

impl Default for Confidence {
    fn default() -> Self {
        Self::ZERO
    }
}

impl TryFrom<f32> for Confidence {
    type Error = UnitError;

    fn try_from(value: f32) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl From<Confidence> for f32 {
    fn from(confidence: Confidence) -> Self {
        confidence.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_negative_time_is_refused() {
        assert_eq!(Seconds::new(-1.0), Err(UnitError::Negative));
    }

    #[test]
    fn a_time_that_is_not_a_number_is_refused() {
        assert_eq!(Seconds::new(f64::NAN), Err(UnitError::NotFinite));
        assert_eq!(Seconds::new(f64::INFINITY), Err(UnitError::NotFinite));
    }

    #[test]
    fn zero_and_positive_times_are_accepted() {
        assert_eq!(Seconds::new(0.0).map(Seconds::get), Ok(0.0));
        assert_eq!(Seconds::new(2.5).map(Seconds::get), Ok(2.5));
    }

    #[test]
    fn a_confidence_above_one_is_refused() {
        assert_eq!(Confidence::new(1.5), Err(UnitError::OutOfRange));
        assert_eq!(Confidence::new(-0.1), Err(UnitError::OutOfRange));
    }

    #[test]
    fn a_confidence_that_is_nan_is_refused() {
        assert_eq!(Confidence::new(f32::NAN), Err(UnitError::NotFinite));
    }

    #[test]
    fn a_confidence_between_zero_and_one_is_accepted() {
        assert_eq!(Confidence::new(0.5).map(Confidence::get), Ok(0.5));
        assert_eq!(Confidence::new(0.0), Ok(Confidence::ZERO));
        assert_eq!(Confidence::new(1.0).map(Confidence::get), Ok(1.0));
    }

    #[test]
    fn the_serialized_form_is_the_bare_number() {
        let text = serde_json::to_string(&Seconds::TWO).expect("a number");
        assert_eq!(text, "2.0");
        let back: Seconds = serde_json::from_str(&text).expect("a valid time");
        assert_eq!(back, Seconds::TWO);
        assert!(serde_json::from_str::<Seconds>("-1.0").is_err());
        assert!(serde_json::from_str::<Confidence>("1.5").is_err());
    }
}
