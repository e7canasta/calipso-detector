//! Evidence: which observation counts and how much it is worth. It never
//! decides what is believed or what is looked at. It receives readings already
//! typed: one of another kind than its probe arrives as a failing sense.

use crate::attention::LookResult;
use crate::body::Keypoint;
use crate::estimation;
use crate::geom::{Quad, Rect};
use crate::look::{Detection, Failure, Label};
use crate::policy::{AdmissionPolicy, FacePolicy};
use crate::zone::Counts;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

const NONE: Counts = Counts { floor: 0, bed: 0 };

/// What a sense read, or why it could not.
pub(crate) type Sensed<T> = Result<T, Failure>;

/// An admitted box, keeping which instrument produced it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Body {
    pub detection: Detection,
    pub source: BodySource,
}

/// A body off the bed, and whether it is near it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct FloorBody {
    pub detection: Detection,
    pub near_bed: bool,
    pub source: BodySource,
}

/// The accepted bodies of a sample, per zone, from most to least confident.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct Bodies {
    pub bed: Vec<Body>,
    pub floor: Vec<FloorBody>,
}

impl Bodies {
    /// How many are in each zone.
    pub(crate) fn counts(&self) -> Counts {
        Counts {
            floor: count(self.floor.len()),
            bed: count(self.bed.len()),
        }
    }

    /// How many are on the floor far from the bed: they come from the door, not from the bed.
    pub(crate) fn far(&self) -> u32 {
        count(self.floor.iter().filter(|b| !b.near_bed).count())
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.bed.is_empty() && self.floor.is_empty()
    }

    /// The most confident body, those in bed first.
    pub(crate) fn best(&self) -> Option<&Detection> {
        self.bed
            .first()
            .map(|b| &b.detection)
            .or_else(|| self.floor.first().map(|b| &b.detection))
    }
}

/// What the body detector proves in this sample (rule 1). The belief's `match`
/// must handle every case.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum BodyEvidence {
    /// The instrument failed: it proves nothing.
    Unavailable(Failure),
    /// It ran, but without bed geometry the zone of each body is unknown: it
    /// proves nothing about the zones.
    Unzoned(Vec<Body>),
    /// The accepted bodies, per zone (there may be none).
    Seen(Bodies),
}

impl BodyEvidence {
    /// The bodies seen per zone, if the detector proved them.
    pub(crate) fn seen(&self) -> Option<&Bodies> {
        match self {
            BodyEvidence::Seen(b) => Some(b),
            _ => None,
        }
    }
}

/// The body evidence and what is recorded in the ledger.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Admission {
    pub bodies: BodyEvidence,
    /// What the main detector saw, per zone. Reinforcement does not change it:
    /// it does not re-arm reinforcement and is not a motion reference.
    pub primary: Counts,
    pub result: LookResult,
}

/// All the evidence of a turn, available from before believing until projecting.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Perceived {
    pub bodies: Admission,
    pub faces: FaceEvidence,
}

impl Admission {
    /// Did the main detector see anyone?
    pub(crate) fn primary_seen(&self) -> bool {
        self.primary.bed + self.primary.floor > 0
    }
}

/// What the faces of this sample prove.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) enum FaceEvidence {
    /// Faces were not looked for.
    #[default]
    NotLooked,
    Unavailable(Failure),
    None,
    /// Faces accepted in the requested region.
    Seen(Vec<Detection>),
}

impl FaceEvidence {
    pub(crate) fn seen(&self) -> bool {
        matches!(self, FaceEvidence::Seen(_))
    }
}

/// The face evidence and what is recorded in the ledger.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct FaceAdmission {
    pub faces: FaceEvidence,
    pub result: LookResult,
}

/// Accepts the faces with enough confidence inside the requested region.
pub(crate) fn faces(
    reading: Sensed<&[Detection]>,
    region: Rect,
    policy: &FacePolicy,
) -> FaceAdmission {
    let detections = match reading {
        Err(failure) => {
            return FaceAdmission {
                faces: FaceEvidence::Unavailable(failure),
                result: LookResult::failed(failure),
            };
        }
        Ok(d) => d,
    };
    let inside = |d: &Detection| {
        let c = d.bbox.centroid();
        c.x >= region.x0 && c.x <= region.x1 && c.y >= region.y0 && c.y <= region.y1
    };
    let mut accepted = Vec::new();
    let mut rejected = Vec::new();
    for d in detections {
        if d.confidence < policy.min_confidence {
            rejected.push(RejectReason::LowConfidence);
        } else if !inside(d) {
            rejected.push(RejectReason::OutsideRegion);
        } else {
            accepted.push(*d);
        }
    }
    FaceAdmission {
        faces: if accepted.is_empty() {
            FaceEvidence::None
        } else {
            FaceEvidence::Seen(accepted)
        },
        result: LookResult::Read {
            candidates: count(detections.len()),
            rejected,
        },
    }
}

/// Admits the bodies: people with enough confidence, each in its zone.
/// Nobody is rejected for being off the bed.
pub(crate) fn admit(
    reading: Sensed<&[Detection]>,
    bed: Option<&Quad>,
    policy: &AdmissionPolicy,
) -> Admission {
    admit_from(reading, bed, policy, BodySource::Primary)
}

fn admit_from(
    reading: Sensed<&[Detection]>,
    bed: Option<&Quad>,
    policy: &AdmissionPolicy,
    source: BodySource,
) -> Admission {
    let detections = match reading {
        Err(failure) => {
            return Admission {
                bodies: BodyEvidence::Unavailable(failure),
                primary: NONE,
                result: LookResult::failed(failure),
            };
        }
        Ok(d) => d,
    };
    let mut admitted = Vec::new();
    let mut rejected = Vec::new();
    for d in detections {
        match reject(d, policy) {
            Some(reason) => rejected.push(reason),
            None => admitted.push(Body {
                detection: *d,
                source,
            }),
        }
    }
    admitted.sort_by(|a, b| b.detection.confidence.total_cmp(&a.detection.confidence));
    let admitted = one_per_body(admitted, policy.same_body_iou);
    let result = LookResult::Read {
        candidates: count(detections.len()),
        rejected,
    };
    let Some(bed) = bed else {
        return Admission {
            bodies: BodyEvidence::Unzoned(admitted),
            primary: NONE,
            result,
        };
    };
    let near = bed.bounds().expand(policy.near_bed_margin);
    let mut bodies = Bodies::default();
    for d in admitted {
        if bed.contains(d.detection.bbox.centroid()) {
            bodies.bed.push(d);
        } else {
            bodies.floor.push(FloorBody {
                detection: d.detection,
                near_bed: d.detection.bbox.iou(&near) > 0.0,
                source: d.source,
            });
        }
    }
    Admission {
        primary: bodies.counts(),
        bodies: BodyEvidence::Seen(bodies),
        result,
    }
}

/// Consolidates what reinforcement saw with what the main detector saw.
/// Returns the consolidated evidence and the reinforcement result for the
/// ledger. If reinforcement fails, the main detector's evidence holds.
pub(crate) fn consolidate(
    primary: Admission,
    reinforcement: Sensed<&[Detection]>,
    bed: Option<&Quad>,
    policy: &AdmissionPolicy,
) -> (Admission, LookResult) {
    let second = admit_from(reinforcement, bed, policy, BodySource::Reinforcement);
    let bodies = match (primary.bodies, second.bodies) {
        (BodyEvidence::Seen(a), BodyEvidence::Seen(b)) => {
            let mut beds = a.bed;
            beds.extend(b.bed);
            beds.sort_by(|x, y| y.detection.confidence.total_cmp(&x.detection.confidence));
            let mut floor = a.floor;
            for f in b.floor {
                let same = floor
                    .iter()
                    .any(|k| same_body(&k.detection, &f.detection, policy.same_body_iou));
                if !same {
                    floor.push(f);
                }
            }
            BodyEvidence::Seen(Bodies {
                bed: one_per_body(beds, policy.same_body_iou),
                floor,
            })
        }
        (BodyEvidence::Unavailable(_), seen @ BodyEvidence::Seen(_)) => seen,
        (primary, _) => primary,
    };
    (
        Admission {
            bodies,
            primary: primary.primary,
            result: primary.result,
        },
        second.result,
    )
}

/// The pose the sense brought: `None` if it failed or brought no keypoints.
pub(crate) fn pose(reading: Sensed<Option<Vec<Keypoint>>>) -> (Option<Vec<Keypoint>>, LookResult) {
    match reading {
        Err(failure) => (None, LookResult::failed(failure)),
        Ok(Some(kp)) if !kp.is_empty() => {
            let result = LookResult::read(kp.len());
            (Some(kp), result)
        }
        Ok(_) => (None, LookResult::read(0)),
    }
}

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

/// The depth of each keypoint of a pose of `len` keypoints: reading `i` goes
/// back to keypoint `at[i]`; unmeasured ones and non-finite values stay
/// without depth. If the sense failed, none.
pub(crate) fn depths(
    reading: Sensed<&[Option<f32>]>,
    at: &[usize],
    len: usize,
) -> (Option<Vec<Option<f32>>>, LookResult) {
    match reading {
        Err(failure) => (None, LookResult::failed(failure)),
        Ok(values) => {
            let mut depths = vec![None; len];
            for (&i, value) in at.iter().zip(values) {
                if let Some(slot) = depths.get_mut(i) {
                    *slot = value.filter(|d| d.is_finite());
                }
            }
            let read = LookResult::read(values.iter().flatten().count());
            (Some(depths), read)
        }
    }
}

/// One body per person: two boxes are the same if one contains the other or
/// they overlap from `iou` up. The most confident stays (they come sorted).
fn one_per_body(sorted: Vec<Body>, iou: f32) -> Vec<Body> {
    let mut kept: Vec<Body> = Vec::new();
    for d in sorted {
        if !kept
            .iter()
            .any(|k| same_body(&k.detection, &d.detection, iou))
        {
            kept.push(d);
        }
    }
    kept
}

fn same_body(a: &Detection, b: &Detection, iou: f32) -> bool {
    a.bbox.contains_rect(&b.bbox) || b.bbox.contains_rect(&a.bbox) || a.bbox.iou(&b.bbox) >= iou
}

fn reject(detection: &Detection, policy: &AdmissionPolicy) -> Option<RejectReason> {
    if detection.label != Label::Person {
        Some(RejectReason::NotPerson)
    } else if detection.confidence < policy.min_confidence {
        Some(RejectReason::LowConfidence)
    } else {
        None
    }
}

/// Ledger quantities are `u32`; no sense returns that many.
fn count(n: usize) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}

/// Which detector the admitted body came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum BodySource {
    /// The main detector, over the whole frame.
    Primary,
    /// The finer detector, over the bed.
    Reinforcement,
}

/// Why a candidate did not count as evidence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RejectReason {
    /// Another class than a person.
    NotPerson,
    /// Below the policy's minimum confidence.
    LowConfidence,
    /// Outside the region where the look was requested.
    OutsideRegion,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bed() -> Quad {
        Quad::from_rect(Rect::new(100.0, 100.0, 500.0, 400.0))
    }

    fn person(x0: f32, confidence: f32) -> Detection {
        Detection {
            bbox: Rect::new(x0, 150.0, x0 + 100.0, 350.0),
            confidence,
            label: Label::Person,
        }
    }

    const POLICY: AdmissionPolicy = AdmissionPolicy {
        min_confidence: 0.5,
        same_body_iou: 0.5,
        near_bed_margin: 0.10,
    };

    fn seen(a: Admission) -> Bodies {
        match a.bodies {
            BodyEvidence::Seen(b) => b,
            other => panic!("bodies were expected: {other:?}"),
        }
    }

    #[test]
    fn each_body_goes_to_its_zone_and_nobody_is_rejected_for_being_on_the_floor() {
        let b = seen(admit(
            Ok(&[person(200.0, 0.8), person(505.0, 0.9), person(560.0, 0.7)]),
            Some(&bed()),
            &POLICY,
        ));
        assert_eq!(b.counts(), Counts { floor: 2, bed: 1 });
        assert!(b.floor[0].near_bed, "next to the bed");
        assert!(!b.floor[1].near_bed, "at the door");
        assert_eq!(b.far(), 1);
    }

    #[test]
    fn a_box_inside_another_is_the_same_body() {
        let mut inner = person(220.0, 0.6);
        inner.bbox = Rect::new(220.0, 170.0, 280.0, 330.0);
        let b = seen(admit(
            Ok(&[inner, person(200.0, 0.8)]),
            Some(&bed()),
            &POLICY,
        ));
        assert_eq!(b.bed.len(), 1);
        assert_eq!(b.bed[0].detection.confidence, 0.8);
    }

    #[test]
    fn without_geometry_there_are_no_zones() {
        let a = admit(Ok(&[person(200.0, 0.8)]), None, &POLICY);
        assert!(matches!(a.bodies, BodyEvidence::Unzoned(ref b) if b.len() == 1));
    }

    #[test]
    fn reinforcement_seeing_the_same_body_adds_no_body() {
        let primary = admit(Ok(&[person(200.0, 0.8)]), Some(&bed()), &POLICY);
        let (c, _) = consolidate(primary, Ok(&[person(205.0, 0.7)]), Some(&bed()), &POLICY);
        assert_eq!(c.primary, Counts { floor: 0, bed: 1 });
        assert_eq!(seen(c).bed.len(), 1);
    }

    #[test]
    fn if_reinforcement_fails_the_main_detector_holds() {
        let primary = admit(Ok(&[]), Some(&bed()), &POLICY);
        let (c, result) = consolidate(primary, Err(Failure::Model), Some(&bed()), &POLICY);
        assert!(seen(c).is_empty());
        assert!(matches!(result, LookResult::Failed { .. }));
    }

    #[test]
    fn a_failure_proves_nothing() {
        let a = admit(Err(Failure::Cancelled), Some(&bed()), &POLICY);
        assert_eq!(a.bodies, BodyEvidence::Unavailable(Failure::Cancelled));
    }

    #[test]
    fn every_rejection_has_a_reason() {
        let mut other = person(200.0, 0.9);
        other.label = Label::Other;
        let a = admit(Ok(&[other, person(200.0, 0.3)]), Some(&bed()), &POLICY);
        assert_eq!(
            a.result,
            LookResult::Read {
                candidates: 2,
                rejected: vec![RejectReason::NotPerson, RejectReason::LowConfidence],
            }
        );
    }

    #[test]
    fn depth_is_the_median_of_the_valid_readings() {
        let (d, r) = depth(Ok(&[Some(2.0), None, Some(4.0), Some(3.0)]));
        assert_eq!(d, Some(3.0));
        assert_eq!(r, LookResult::read(3));
    }
}

#[cfg(test)]
mod source_tests {
    use super::*;
    #[test]
    fn the_winning_box_keeps_its_source_detector() {
        let bed = Quad::from_rect(Rect::new(0.0, 0.0, 100.0, 100.0));
        let p = AdmissionPolicy {
            min_confidence: 0.5,
            same_body_iou: 0.5,
            near_bed_margin: 0.1,
        };
        let d = |confidence| Detection {
            bbox: bed.bounds(),
            confidence,
            label: Label::Person,
        };
        for (confidence, source) in [(0.9, BodySource::Reinforcement), (0.7, BodySource::Primary)] {
            let primary = admit(Ok(&[d(0.8)]), Some(&bed), &p);
            let (admission, _) = consolidate(primary, Ok(&[d(confidence)]), Some(&bed), &p);
            let bodies = admission.bodies.seen().unwrap();
            assert_eq!(bodies.bed.len(), 1);
            assert_eq!(bodies.bed[0].source, source);
        }
    }
}
