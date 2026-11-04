//! The zone by parts, decided (F4.1; spec `f4-1-la-pieza-decide.md`, the switch
//! `[occupancy.parts] decide`). With the switch on, the cascade and its memory
//! say which bodies are in bed, instead of the box; the bodies move between the
//! bed and the floor lists before anything believes them.
//!
//! The rules, per body, in this order:
//! - a fresh proof decides: the bed by the face at the headboard, the feet on the
//!   mattress or the pelvis at the edge; the floor by a step of the box (past the
//!   near edge, the cut, no touch) or the pelvis high over the feet (standing);
//! - without a fresh proof, the memory keeps the body's zone, if it was matched;
//! - without memory, a body looked at and not proven is on the floor, and a body
//!   whose pose was not looked at stays where the box put it.
//!
//! The memory matches a body to the last turn's by its box, IoU at least the
//! admission's `same_body_iou`. It lives in the engine, never in `room`.

use super::{Body, BodyEvidence, Perceived};
use crate::geom::{Point, Rect};
use crate::room::{BodyZone, Room, Step};

/// What the decision remembers of one body: its box and its zone.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Remembered {
    bbox: Rect,
    in_bed: bool,
}

/// The bodies of the last turn that the parts decided (F4.1).
#[derive(Debug, Default, Clone, PartialEq)]
pub(crate) struct PartsMemory {
    bodies: Vec<Remembered>,
}

impl PartsMemory {
    /// The zone each body of this turn inherits: each remembered body goes to at
    /// most one body, the pairs of highest IoU first, and only from `same_iou` up.
    /// A body with no memory of its own gets `None`.
    fn inherited(&self, boxes: &[Rect], same_iou: f32) -> Vec<Option<bool>> {
        let mut pairs: Vec<(f32, usize, usize)> = boxes
            .iter()
            .enumerate()
            .flat_map(|(i, b)| {
                self.bodies
                    .iter()
                    .enumerate()
                    .map(move |(m, r)| (b.iou(&r.bbox), i, m))
            })
            .filter(|(iou, _, _)| *iou >= same_iou)
            .collect();
        pairs.sort_by(|a, b| b.0.total_cmp(&a.0));
        let mut inherited = vec![None; boxes.len()];
        let mut used = vec![false; self.bodies.len()];
        for (_, i, m) in pairs {
            let free = used.get(m).is_some_and(|u| !u);
            let slot = inherited.get_mut(i);
            if let (true, Some(slot), Some(remembered)) = (free, slot, self.bodies.get(m))
                && slot.is_none()
            {
                *slot = Some(remembered.in_bed);
                if let Some(u) = used.get_mut(m) {
                    *u = true;
                }
            }
        }
        inherited
    }
}

/// The zone the cascade proves, if it proves one: `Some(true)` in bed, `Some(false)`
/// on the floor, `None` when there is no fresh proof.
fn proven(step: Step) -> Option<bool> {
    match step {
        Step::FaceAtHeadboard | Step::FeetOnMattress | Step::PelvisAtEdge => Some(true),
        Step::PastNear | Step::Cut | Step::NoTouch | Step::Standing => Some(false),
        Step::Unproven | Step::PoseNotLooked => None,
    }
}

/// Decide the zone of every body of this sample by its parts, with the memory,
/// and move the bodies to the lists of their zones. Without the parts in the room,
/// nothing changes.
pub(crate) fn decide(
    perceived: &mut Perceived,
    room: &Room,
    memory: &mut PartsMemory,
    same_iou: f32,
) {
    let BodyEvidence::Seen(bodies) = &mut perceived.bodies.bodies else {
        return;
    };
    let beds = bodies.bed.len();
    let boxes: Vec<Rect> = bodies
        .bed
        .iter()
        .map(|b| b.detection.bbox)
        .chain(bodies.floor.iter().map(|b| b.detection.bbox))
        .collect();
    let faces: Vec<Point> = match &perceived.faces {
        crate::evidence::FaceEvidence::Seen(found) => {
            found.iter().map(|f| f.bbox.centroid()).collect()
        }
        _ => Vec::new(),
    };
    let inherited = memory.inherited(&boxes, same_iou);
    let mut in_bed = Vec::with_capacity(boxes.len());
    // The cascade's «near the bed» (the support within near_m of the footprint, F2.2):
    // the floor body's near_bed, not the box's.
    let mut near = Vec::with_capacity(boxes.len());
    for (i, bbox) in boxes.iter().enumerate() {
        let pose = perceived
            .posed
            .as_ref()
            .filter(|p| p.bbox == *bbox)
            .map(|p| p.pose.as_slice());
        let face = Room::face_of(&boxes, &faces, i);
        // Without the parts in the room, the box decides, as before.
        let Some(verdict) = room.parts(*bbox, pose, face) else {
            return;
        };
        near.push(matches!(verdict.zone, BodyZone::Floor { near_bed: true }));
        let remembered = inherited.get(i).copied().flatten();
        let zone = match remembered {
            // In bed by memory, only a step 1 (the box leaves the mattress) releases
            // it: a standing reading (E2) or no fresh proof keeps the bed (F4.1b).
            Some(true) => !matches!(verdict.step, Step::PastNear | Step::Cut | Step::NoTouch),
            _ => proven(verdict.step)
                .or(remembered)
                .unwrap_or(match verdict.step {
                    Step::PoseNotLooked => i < beds,
                    _ => false,
                }),
        };
        in_bed.push(zone);
    }
    // Move the bodies whose zone changed, keeping each one's fields.
    let mut bed_list = Vec::new();
    let mut floor_list = Vec::new();
    for (i, body) in std::mem::take(&mut bodies.bed).into_iter().enumerate() {
        if in_bed.get(i).copied().unwrap_or(false) {
            bed_list.push(body);
        } else {
            if body.source == crate::evidence::BodySource::Primary {
                perceived.bodies.primary.bed = perceived.bodies.primary.bed.saturating_sub(1);
                perceived.bodies.primary.floor += 1;
            }
            floor_list.push(Body {
                zone: Some(BodyZone::Floor {
                    near_bed: near.get(i).copied().unwrap_or(false),
                }),
                ..body
            });
        }
    }
    for (j, body) in std::mem::take(&mut bodies.floor).into_iter().enumerate() {
        if in_bed.get(beds + j).copied().unwrap_or(false) {
            if body.source == crate::evidence::BodySource::Primary {
                perceived.bodies.primary.floor = perceived.bodies.primary.floor.saturating_sub(1);
                perceived.bodies.primary.bed += 1;
            }
            bed_list.push(Body {
                zone: Some(BodyZone::Bed),
                ..body
            });
        } else {
            floor_list.push(Body {
                zone: Some(BodyZone::Floor {
                    near_bed: near.get(beds + j).copied().unwrap_or(false),
                }),
                ..body
            });
        }
    }
    bed_list.sort_by(|a, b| b.detection.confidence.total_cmp(&a.detection.confidence));
    memory.bodies = bed_list
        .iter()
        .map(|b| Remembered {
            bbox: b.detection.bbox,
            in_bed: true,
        })
        .chain(floor_list.iter().map(|b| Remembered {
            bbox: b.detection.bbox,
            in_bed: false,
        }))
        .collect();
    bodies.bed = bed_list;
    bodies.floor = floor_list;
}
