//! The bodies: which boxes are people, one body per person, in which zone;
//! and the main detector's reading consolidated with the reinforcement's.

use super::{
    Admission, Bodies, Body, BodyEvidence, BodySource, FloorBody, NONE, RejectReason, Sensed, count,
};
use crate::attention::LookResult;
use crate::look::{Detection, Label};
use crate::policy::AdmissionPolicy;
use crate::room::{BodyZone, Room};

/// Admits the bodies: people with enough confidence, each in its zone.
/// Nobody is rejected for being off the bed.
pub(crate) fn admit(
    reading: Sensed<&[Detection]>,
    room: Option<&Room>,
    policy: &AdmissionPolicy,
) -> Admission {
    admit_from(reading, room, policy, BodySource::Primary)
}

fn admit_from(
    reading: Sensed<&[Detection]>,
    room: Option<&Room>,
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
                shadow: None,
                knee_deg: None,
            }),
        }
    }
    admitted.sort_by(|a, b| b.detection.confidence.total_cmp(&a.detection.confidence));
    let admitted = one_per_body(admitted, policy.same_body_iou);
    let result = LookResult::Read {
        candidates: count(detections.len()),
        rejected,
    };
    let Some(room) = room else {
        return Admission {
            bodies: BodyEvidence::Unzoned(admitted),
            primary: NONE,
            result,
        };
    };
    let mut bodies = Bodies::default();
    for d in admitted {
        match room.body_zone(&d.detection.bbox) {
            BodyZone::Bed => bodies.bed.push(d),
            BodyZone::Floor { near_bed } => bodies.floor.push(FloorBody {
                detection: d.detection,
                near_bed,
                source: d.source,
                shadow: None,
                knee_deg: None,
            }),
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
    room: Option<&Room>,
    policy: &AdmissionPolicy,
) -> (Admission, LookResult) {
    let second = admit_from(reinforcement, room, policy, BodySource::Reinforcement);
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
    } else if detection.confidence < policy.min_confidence.get() {
        Some(RejectReason::LowConfidence)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geom::{Point, Quad, Rect};
    use crate::look::Failure;
    use crate::units::Confidence;
    use crate::zone::Counts;

    fn bed() -> Room {
        let bed = Quad::from_rect(Rect::new(100.0, 100.0, 500.0, 400.0));
        Room::new(&bed, None, &POLICY)
    }

    fn person(x0: f32, confidence: f32) -> Detection {
        Detection {
            bbox: Rect::new(x0, 150.0, x0 + 100.0, 350.0),
            confidence,
            label: Label::Person,
        }
    }

    const POLICY: AdmissionPolicy = AdmissionPolicy {
        min_confidence: Confidence::HALF,
        same_body_iou: 0.5,
        near_bed_margin: 0.10,
        bed_frame: None,
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
    fn in_the_bed_frame_near_the_bed_follows_a_bed_seen_on_the_diagonal() {
        // From the corner the bed is a diamond; its bounding box takes in the
        // room's corners.
        let diamond = Quad([
            Point { x: 300.0, y: 100.0 },
            Point { x: 500.0, y: 300.0 },
            Point { x: 300.0, y: 500.0 },
            Point { x: 100.0, y: 300.0 },
        ]);
        let in_the_corner = Detection {
            bbox: Rect::new(105.0, 105.0, 150.0, 170.0),
            confidence: 0.9,
            label: Label::Person,
        };
        let beside = Detection {
            bbox: Rect::new(420.0, 380.0, 470.0, 470.0),
            ..in_the_corner
        };
        let near = |policy: &AdmissionPolicy| {
            let room = Room::new(&diamond, None, policy);
            let b = seen(admit(Ok(&[in_the_corner, beside]), Some(&room), policy));
            (b.floor[0].near_bed, b.floor[1].near_bed)
        };
        assert_eq!(near(&POLICY), (true, true), "image axes: the box grown");
        let bed_frame = AdmissionPolicy {
            bed_frame: Some(0.7),
            ..POLICY
        };
        assert_eq!(
            near(&bed_frame),
            (false, true),
            "the bed grown in its coordinates"
        );
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
}

#[cfg(test)]
mod source_tests {
    use super::*;
    use crate::geom::{Quad, Rect};
    use crate::units::Confidence;
    #[test]
    fn the_winning_box_keeps_its_source_detector() {
        let bed = Quad::from_rect(Rect::new(0.0, 0.0, 100.0, 100.0));
        let p = AdmissionPolicy {
            min_confidence: Confidence::HALF,
            same_body_iou: 0.5,
            near_bed_margin: 0.1,
            bed_frame: None,
        };
        let d = |confidence| Detection {
            bbox: bed.bounds(),
            confidence,
            label: Label::Person,
        };
        for (confidence, source) in [(0.9, BodySource::Reinforcement), (0.7, BodySource::Primary)] {
            let room = Room::new(&bed, None, &p);
            let primary = admit(Ok(&[d(0.8)]), Some(&room), &p);
            let (admission, _) = consolidate(primary, Ok(&[d(confidence)]), Some(&room), &p);
            let bodies = admission.bodies.seen().unwrap();
            assert_eq!(bodies.bed.len(), 1);
            assert_eq!(bodies.bed[0].source, source);
        }
    }
}
