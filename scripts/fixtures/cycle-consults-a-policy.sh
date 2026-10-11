# Rule 11: the sequencer asking whether a sense is configured must fail.
cat >> crates/scene-core/src/cycle/waiting.rs <<'RS'

#[allow(dead_code)]
fn face_floor(engine: &super::Engine) -> f32 {
    let Some(policy) = engine.policy.faces else {
        return 0.0;
    };
    policy.min_confidence
}
RS
