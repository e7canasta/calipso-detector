//! Depth: one value per point of the subject's body, or over the bed.

use super::Sensed;
use crate::attention::LookResult;
use crate::body::BedPoint;
use crate::estimation;
use crate::geom::Point;
use crate::look::DepthTarget;

/// The depth over a set of points: the median of the valid readings.
pub(crate) fn depth(reading: Sensed<&[Option<f32>]>) -> (Option<f32>, LookResult) {
    match reading {
        Err(failure) => (None, LookResult::failed(failure)),
        Ok(values) => (
            estimation::median(values),
            LookResult::read(values.iter().flatten().count()),
        ),
    }
}

/// The depth of each keypoint of a pose of `len` keypoints, and of each bed
/// site: reading `i` goes back to whose `at[i]` is; unmeasured ones and
/// non-finite values stay without depth. If the sense failed, none.
pub(crate) fn depths(
    reading: Sensed<&[Option<f32>]>,
    at: &[(DepthTarget, Point)],
    len: usize,
) -> (Option<SubjectDepths>, LookResult) {
    match reading {
        Err(failure) => (None, LookResult::failed(failure)),
        Ok(values) => {
            let mut depths = SubjectDepths {
                keypoints: vec![None; len],
                bed: Vec::new(),
            };
            for (&(target, p), value) in at.iter().zip(values) {
                let value = value.filter(|d| d.is_finite());
                match target {
                    DepthTarget::Keypoint(i) => {
                        if let Some(slot) = depths.keypoints.get_mut(i) {
                            *slot = value;
                        }
                    }
                    DepthTarget::Bed(name) => depths.bed.push(BedPoint {
                        name,
                        x: p.x,
                        y: p.y,
                        depth: value,
                    }),
                }
            }
            let read = LookResult::read(values.iter().flatten().count());
            (Some(depths), read)
        }
    }
}

/// What one depth look read of the subject: each keypoint's depth, aligned
/// with the pose, and the bed's points read in the same map. It travels whole:
/// what was read in the same look is not split.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct SubjectDepths {
    pub keypoints: Vec<Option<f32>>,
    pub bed: Vec<BedPoint>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn depth_is_the_median_of_the_valid_readings() {
        let (d, r) = depth(Ok(&[Some(2.0), None, Some(4.0), Some(3.0)]));
        assert_eq!(d, Some(3.0));
        assert_eq!(r, LookResult::read(3));
    }
}
