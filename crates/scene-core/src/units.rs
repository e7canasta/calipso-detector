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
    /// A measure that must be above zero, and is zero or less.
    NotPositive,
    /// A measure outside its own range, which the error names.
    OutsideRange(&'static str),
}

impl fmt::Display for UnitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFinite => f.write_str("is not a finite number"),
            Self::Negative => f.write_str("is negative"),
            Self::OutOfRange => f.write_str("is not between 0 and 1"),
            Self::NotPositive => f.write_str("is not positive"),
            Self::OutsideRange(range) => write!(f, "is not in {range}"),
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

/// Builds a geometry measure from its check: the same pattern as `Seconds` and
/// `Confidence`. The inner value is private, the constructor is the only door,
/// and the serialized form is the bare number.
macro_rules! measure {
    ($(#[$doc:meta])* $name:ident, $check:expr) => {
        $(#[$doc])*
        #[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Serialize, Deserialize)]
        #[serde(try_from = "f32", into = "f32")]
        pub struct $name(f32);

        impl $name {
            /// Builds the measure. Rejects what its range does not admit.
            pub fn new(value: f32) -> Result<Self, UnitError> {
                let check: fn(f32) -> Result<(), UnitError> = $check;
                check(value)?;
                Ok(Self(value))
            }

            /// The value, for the arithmetic of the geometry.
            pub fn get(self) -> f32 {
                self.0
            }
        }

        impl TryFrom<f32> for $name {
            type Error = UnitError;

            fn try_from(value: f32) -> Result<Self, Self::Error> {
                Self::new(value)
            }
        }

        impl From<$name> for f32 {
            fn from(measure: $name) -> Self {
                measure.0
            }
        }
    };
}

/// Finite: `NaN` and the infinities are not measures.
fn finite(value: f32) -> Result<(), UnitError> {
    if value.is_finite() {
        Ok(())
    } else {
        Err(UnitError::NotFinite)
    }
}

/// Finite and not negative.
fn not_negative(value: f32) -> Result<(), UnitError> {
    finite(value)?;
    if value < 0.0 {
        Err(UnitError::Negative)
    } else {
        Ok(())
    }
}

/// Finite and above zero.
fn positive(value: f32) -> Result<(), UnitError> {
    finite(value)?;
    if value <= 0.0 {
        Err(UnitError::NotPositive)
    } else {
        Ok(())
    }
}

/// Finite and between 0 and 1, both included.
fn unit_interval(value: f32) -> Result<(), UnitError> {
    finite(value)?;
    if (0.0..=1.0).contains(&value) {
        Ok(())
    } else {
        Err(UnitError::OutOfRange)
    }
}

/// Finite, above zero and at most 1: a ratio of the box or of a median.
fn ratio(value: f32) -> Result<(), UnitError> {
    positive(value)?;
    if value <= 1.0 {
        Ok(())
    } else {
        Err(UnitError::OutOfRange)
    }
}

/// Finite, above zero and below half: a margin that covers both sides of the bed.
fn feet_margin(value: f32) -> Result<(), UnitError> {
    positive(value)?;
    if value < 0.5 {
        Ok(())
    } else {
        Err(UnitError::OutsideRange("(0, 0.5)"))
    }
}

/// Finite, above zero and below a half turn.
fn degrees(value: f32) -> Result<(), UnitError> {
    positive(value)?;
    if value < 180.0 {
        Ok(())
    } else {
        Err(UnitError::OutsideRange("(0, 180)"))
    }
}

impl Proportion {
    /// Even overlap: the default of the same-body test.
    pub const HALF: Self = Self(0.5);
}

impl Extension {
    /// A tenth of the reference: the default margin of a near-bed test.
    pub const TENTH: Self = Self(0.1);
    /// The verifier's crop: the bed grown by 15% of its width and height on
    /// each side (white book, volume III, ch. 05).
    pub const VERIFY_CROP: Self = Self(0.15);
}

measure!(
    /// How far a region grows, as a fraction of the width and the height of its
    /// reference, on each side (the bed, the headboard, the subject's box): not
    /// negative, with no upper bound (a pad of 1 doubles the reference).
    Extension,
    not_negative
);

measure!(
    /// A fraction of the reference box's diagonal: not negative, no upper bound.
    DiagonalFraction,
    not_negative
);

measure!(
    /// A distance in metres: finite and not negative.
    Metres,
    not_negative
);

measure!(
    /// A distance in metres that must be above zero (a radius around the bed).
    PositiveMetres,
    positive
);

measure!(
    /// An overlap of two boxes, as a fraction in `[0, 1]`: not a confidence,
    /// though the range is the same.
    Proportion,
    unit_interval
);

measure!(
    /// A ratio in `(0, 1]`: the share of a box over another, or a crop against
    /// the median of the camera's last ones. Zero is not a ratio.
    Ratio,
    ratio
);

measure!(
    /// The grey margin of the feet, as a fraction of the bed's width, on each
    /// side: in `(0, 0.5)`, since two margins cover the whole bed.
    FeetMargin,
    feet_margin
);

measure!(
    /// An angle in degrees, in `(0, 180)`: a bent knee.
    Degrees,
    degrees
);

measure!(
    /// A difference of depth relative to the model's depth: a dimensionless
    /// quantity, finite. The sign is the caller's to judge.
    RelativeDepth,
    finite
);

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

    #[test]
    fn an_extension_is_not_negative_and_has_no_upper_bound() {
        assert_eq!(Extension::new(0.0).map(Extension::get), Ok(0.0));
        assert_eq!(Extension::new(3.0).map(Extension::get), Ok(3.0));
        assert_eq!(Extension::new(-0.1), Err(UnitError::Negative));
        assert_eq!(Extension::new(f32::NAN), Err(UnitError::NotFinite));
        assert_eq!(Extension::new(f32::INFINITY), Err(UnitError::NotFinite));
    }

    #[test]
    fn a_diagonal_fraction_and_metres_are_not_negative() {
        assert!(DiagonalFraction::new(0.0).is_ok());
        assert!(DiagonalFraction::new(1.5).is_ok());
        assert_eq!(DiagonalFraction::new(-0.01), Err(UnitError::Negative));
        assert!(Metres::new(0.0).is_ok());
        assert_eq!(Metres::new(-1.0), Err(UnitError::Negative));
        assert_eq!(Metres::new(f32::NAN), Err(UnitError::NotFinite));
    }

    #[test]
    fn positive_metres_are_above_zero() {
        assert!(PositiveMetres::new(0.4).is_ok());
        assert_eq!(PositiveMetres::new(0.0), Err(UnitError::NotPositive));
        assert_eq!(PositiveMetres::new(-0.4), Err(UnitError::NotPositive));
        assert_eq!(PositiveMetres::new(f32::NAN), Err(UnitError::NotFinite));
    }

    #[test]
    fn a_proportion_is_between_zero_and_one_both_included() {
        assert!(Proportion::new(0.0).is_ok());
        assert!(Proportion::new(1.0).is_ok());
        assert_eq!(Proportion::new(1.01), Err(UnitError::OutOfRange));
        assert_eq!(Proportion::new(-0.01), Err(UnitError::OutOfRange));
    }

    #[test]
    fn a_ratio_is_above_zero_and_at_most_one() {
        assert!(Ratio::new(1.0).is_ok());
        assert!(Ratio::new(0.01).is_ok());
        assert_eq!(Ratio::new(0.0), Err(UnitError::NotPositive));
        assert_eq!(Ratio::new(1.5), Err(UnitError::OutOfRange));
    }

    #[test]
    fn a_feet_margin_is_below_half_the_bed() {
        assert!(FeetMargin::new(0.1).is_ok());
        assert_eq!(FeetMargin::new(0.0), Err(UnitError::NotPositive));
        assert_eq!(
            FeetMargin::new(0.5),
            Err(UnitError::OutsideRange("(0, 0.5)"))
        );
    }

    #[test]
    fn degrees_of_a_bent_knee_are_in_an_open_interval() {
        assert!(Degrees::new(90.0).is_ok());
        assert_eq!(Degrees::new(0.0), Err(UnitError::NotPositive));
        assert_eq!(
            Degrees::new(180.0),
            Err(UnitError::OutsideRange("(0, 180)"))
        );
    }

    #[test]
    fn a_relative_depth_is_finite_of_either_sign() {
        assert!(RelativeDepth::new(-0.05).is_ok());
        assert!(RelativeDepth::new(0.05).is_ok());
        assert_eq!(RelativeDepth::new(f32::NAN), Err(UnitError::NotFinite));
    }

    #[test]
    fn the_measures_round_trip_as_the_bare_number_and_refuse_what_their_range_refuses() {
        let text = serde_json::to_string(&Extension::new(0.25).expect("valid")).expect("number");
        assert_eq!(text, "0.25");
        let back: Extension = serde_json::from_str(&text).expect("valid");
        assert_eq!(back.get(), 0.25);
        assert!(serde_json::from_str::<Extension>("-0.25").is_err());
        assert!(serde_json::from_str::<Ratio>("0.0").is_err());
        assert!(serde_json::from_str::<FeetMargin>("0.5").is_err());
        assert!(serde_json::from_str::<Degrees>("180.0").is_err());
        assert!(serde_json::from_str::<PositiveMetres>("0.0").is_err());
        assert!(serde_json::from_str::<RelativeDepth>("-0.5").is_ok());
    }
}
