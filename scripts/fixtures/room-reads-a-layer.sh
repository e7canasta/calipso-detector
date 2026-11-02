# expect-step: the room asks nothing above it
# expect-text: the room depends on a layer above it
# The room reading a layer above it (here, the evidence of the faces) must fail.
cat >> crates/scene-core/src/room/mod.rs <<'RS'

#[allow(dead_code)]
fn faces_seen(faces: &crate::evidence::FaceEvidence) -> bool {
    matches!(faces, crate::evidence::FaceEvidence::Seen(_))
}
RS
