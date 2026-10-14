//! The reading of the parts: what we believe of the resident from where their
//! parts are, the semantic sub-state of the parts' trama. A pure decision
//! list over six facts of the places, in this order (the user's rules, white
//! book, volume I, ch. 10):
//!
//! - neither the box nor the groin in the bed: out of bed;
//! - the groin in the bed, the box out: on the edge;
//! - the shoulders and the face or the head at the headboard, nothing out of
//!   the bed: lying;
//! - something out of the bed (or a whole box not in it) while some of the
//!   torso is still at the headboard: trying to leave;
//! - everything in the bed, the whole boxes in it and the headboard seen
//!   free: sitting up;
//! - the headboard seen free: sitting in bed;
//! - otherwise: undecided.
//!
//! An arm out is not "something out": reaching is not leaving. A part never
//! seen says nothing: a headboard never seen is not a free headboard. Where occupancy cannot tell (sitting up against the
//! headboard reads lying), the depth decides: that is layer 1, not this.

use super::{ByPart, Place};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// What we believe of the resident from the places of their parts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum PartsReading {
    /// Neither the box nor the groin in the bed.
    OutOfBed,
    /// The groin in the bed, the box out: getting in or out.
    OnTheEdge,
    /// The shoulders and the face or the head at the headboard, nothing out.
    Lying,
    /// Something out of the bed while some of the torso is still at the
    /// headboard: leaving from lying.
    TryingToLeave,
    /// Everything in the bed, the whole boxes in it, nothing at the headboard.
    SittingUp,
    /// Not everything in the bed, nothing at the headboard.
    SittingInBed,
    /// The places match no rule.
    Undecided,
}

/// What is at the headboard, of the face, the head and the shoulders.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Headboard {
    /// None of them was ever seen against it.
    Unseen,
    /// Seen, and none of them at it.
    Free,
    /// Some of them at it, not the shoulders with the face or the head.
    Partly,
    /// The shoulders and the face or the head at it.
    Lying,
}

/// The facts of the places the reading decides on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Facts {
    /// The groin seen in the bed.
    pub groin_in: bool,
    /// The centre of the person's box in the bed.
    pub box_in: bool,
    /// What is at the headboard.
    pub headboard: Headboard,
    /// Some part out of the bed, the arms aside.
    pub out: bool,
    /// The whole boxes seen are in the bed: the person's, and the face's if
    /// it was seen.
    pub boxes_in: bool,
}

impl Facts {
    /// The facts of some places: what was seen, now or retained.
    pub(crate) fn of(places: &ByPart<Place>) -> Self {
        let [centre, face, head, shoulders, _arms, groin, legs] = places.bed;
        let [hb_face, hb_head, hb_shoulders] = places.headboard;
        let [person_box, face_box, _face_at_headboard] = places.whole;
        let headboard = if hb_shoulders.inside && (hb_face.inside || hb_head.inside) {
            Headboard::Lying
        } else if hb_face.inside || hb_head.inside || hb_shoulders.inside {
            Headboard::Partly
        } else if hb_face.known() || hb_head.known() || hb_shoulders.known() {
            Headboard::Free
        } else {
            Headboard::Unseen
        };
        Self {
            groin_in: groin.inside,
            box_in: centre.inside,
            headboard,
            out: [centre, face, head, shoulders, groin, legs]
                .iter()
                .any(|p| p.outside),
            boxes_in: person_box.inside && (face_box.inside || !face_box.known()),
        }
    }
}

/// The reading of some facts: the decision list, in its order.
pub(crate) fn read(f: Facts) -> PartsReading {
    use PartsReading::*;
    let at_headboard = matches!(f.headboard, Headboard::Partly | Headboard::Lying);
    if !f.groin_in && !f.box_in {
        OutOfBed
    } else if f.groin_in && !f.box_in {
        OnTheEdge
    } else if f.headboard == Headboard::Lying && !f.out {
        Lying
    } else if (f.out || !f.boxes_in) && at_headboard {
        TryingToLeave
    } else if !f.out && f.boxes_in && f.headboard == Headboard::Free {
        SittingUp
    } else if f.headboard == Headboard::Free {
        SittingInBed
    } else {
        Undecided
    }
}

/// How long a reading must hold to count: the ones towards leaving, half of
/// layer 0's dwell (a foot out and the torso leaving the headboard can be a
/// single second); the rest, layer 0's.
pub(crate) fn dwell(reading: PartsReading, layer_0: f64) -> f64 {
    match reading {
        PartsReading::TryingToLeave | PartsReading::OnTheEdge => layer_0 / 2.0,
        _ => layer_0,
    }
}

#[cfg(test)]
mod table;
