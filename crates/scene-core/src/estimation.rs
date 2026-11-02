//! Estimates over what was observed: the subject's motion and depth.

use crate::geom::Rect;
use crate::units::DiagonalFraction;

/// Did the subject move? The centroid shifted more than `threshold` times the
/// diagonal of the reference box, or the width or height changed by more than
/// `threshold` proportionally: lying down or turning over in place changes the
/// shape of the box, not its centre (903; v2's criterion, `SubjectMotion`).
pub(crate) fn moved(reference: &Rect, now: &Rect, threshold: DiagonalFraction) -> bool {
    let threshold = threshold.get();
    let (a, b) = (reference.centroid(), now.centroid());
    let shift = (b.x - a.x).hypot(b.y - a.y);
    let (w0, h0) = (reference.x1 - reference.x0, reference.y1 - reference.y0);
    let (w1, h1) = (now.x1 - now.x0, now.y1 - now.y0);
    let changed = |before: f32, now: f32| (now - before).abs() > threshold * before;
    shift > threshold * w0.hypot(h0) || changed(w0, w1) || changed(h0, h1)
}

/// The median of the valid depths; `None` if there are none.
pub(crate) fn median(values: &[Option<f32>]) -> Option<f32> {
    let mut v: Vec<f32> = values
        .iter()
        .flatten()
        .copied()
        .filter(|x| x.is_finite())
        .collect();
    if v.is_empty() {
        return None;
    }
    v.sort_by(f32::total_cmp);
    let m = v.len() / 2;
    if v.len().is_multiple_of(2) {
        Some((v.get(m.checked_sub(1)?)? + v.get(m)?) / 2.0)
    } else {
        v.get(m).copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::units::DiagonalFraction;

    fn frac(value: f32) -> DiagonalFraction {
        DiagonalFraction::new(value).expect("a fraction")
    }

    #[test]
    fn still_and_shifted() {
        let r = Rect::new(200.0, 150.0, 300.0, 350.0);
        assert!(!moved(
            &r,
            &Rect::new(205.0, 150.0, 305.0, 350.0),
            frac(0.10)
        ));
        assert!(moved(
            &r,
            &Rect::new(260.0, 150.0, 360.0, 350.0),
            frac(0.10)
        ));
    }

    #[test]
    fn the_median_ignores_what_was_not_read() {
        assert_eq!(median(&[Some(2.0), None, Some(4.0), Some(3.0)]), Some(3.0));
        assert_eq!(median(&[Some(2.0), Some(4.0)]), Some(3.0));
        assert_eq!(median(&[None, None]), None);
    }
}

#[cfg(test)]
mod tests_moved {
    fn frac(value: f32) -> crate::units::DiagonalFraction {
        crate::units::DiagonalFraction::new(value).expect("a fraction")
    }

    use super::*;

    #[test]
    fn shape_is_motion_too() {
        let before = Rect::new(150.0, 150.0, 250.0, 350.0);
        let same_centre_other_shape = Rect::new(130.0, 170.0, 270.0, 330.0);
        let slightly_shifted = Rect::new(155.0, 150.0, 255.0, 350.0);
        assert!(moved(&before, &same_centre_other_shape, frac(0.1)));
        assert!(!moved(&before, &slightly_shifted, frac(0.1)));
        assert!(!moved(&before, &before, frac(0.1)));
    }
}
