//! The compatibility corpus (`tests/compat/`): JSON frozen as it has been
//! published, that must deserialize forever. The wire grows additively and the
//! recordings of the past do not change: every shape here is read again by
//! these tests, so a required field added tomorrow fails the build (rule 13).
//! These files are the promise; regenerating them would rewrite it.

use scene_contract::v5::{Envelope, Event, Scene};

/// A v5 scene in the pre-R16 style: every optional field absent, from the
/// additive ones (`posture`, `bed_posture`, `parts`, the shadow…) to the
/// plain ones. It reads all the same.
#[test]
fn compat_scene_v5_without_additive_fields_still_reads() {
    let json = include_str!("compat/scene_v5_pre_r16.json");
    let envelope: Envelope<Scene> = serde_json::from_str(json).expect("a scene published before");
    assert_eq!(envelope.seq, 18);
    let subject = envelope
        .body
        .subject
        .as_ref()
        .expect("the frozen scene has its subject");
    assert_eq!(subject.zone, scene_contract::Zone::Bed);
    assert!(subject.lying.is_none() && subject.bed_posture.is_none() && subject.parts.is_none());
}

/// `sitting` and `standing`, the values published before 904-5, still read as
/// `upright`: nights recorded with them can be replayed.
#[test]
fn compat_posture_state_sitting_and_standing_still_read() {
    use scene_contract::PostureState;
    let sitting: PostureState =
        serde_json::from_str(include_str!("compat/posture_state_sitting.json"))
            .expect("`sitting` reads");
    let standing: PostureState =
        serde_json::from_str(include_str!("compat/posture_state_standing.json"))
            .expect("`standing` reads");
    assert_eq!(sitting, PostureState::Upright);
    assert_eq!(standing, PostureState::Upright);
}

/// An event with none of the fields added since: an old transition, as the
/// engine published it, frozen here while `contract/examples/` moves on.
#[test]
fn compat_event_without_new_fields_still_reads() {
    let json = include_str!("compat/event_v5_without_new_fields.json");
    let envelope: Envelope<Event> = serde_json::from_str(json).expect("an event published before");
    let Event::Transition(transition) = &envelope.body else {
        panic!("not a transition: {:?}", envelope.body);
    };
    assert_eq!(transition.cell.as_str(), "200:11:210");
}

/// Rejections grouped by reason, the published form: it expands to the flat
/// list the ledger keeps.
#[test]
fn compat_grouped_rejections_still_read() {
    use scene_contract::{LookResult, RejectReason};
    let result: LookResult =
        serde_json::from_str(include_str!("compat/look_result_grouped_rejections.json"))
            .expect("grouped rejections read");
    let LookResult::Read {
        candidates,
        rejected,
    } = &result
    else {
        panic!("not a read: {result:?}");
    };
    assert_eq!(*candidates, 4);
    assert_eq!(
        *rejected,
        vec![
            RejectReason::LowConfidence,
            RejectReason::LowConfidence,
            RejectReason::NotPerson
        ]
    );
}
