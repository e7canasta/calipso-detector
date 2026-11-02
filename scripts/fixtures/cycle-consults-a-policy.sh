# expect-step: cycle only sequences
# expect-text: rule 11: cycle asks which senses are configured
# Rule 11: the sequencer asking whether a sense is configured must fail. The
# engine keeps no policy since M2, so the question is asked of a local value
# shaped like one: the grep reads the text, not the type.
cat >> crates/scene-core/src/cycle/waiting.rs <<'RS'

#[allow(dead_code)]
struct Held {
    policy: Slice,
}

#[allow(dead_code)]
struct Slice {
    focus: Option<u8>,
}

#[allow(dead_code)]
fn asks(h: &Held) -> bool {
    h.policy.focus.is_some()
}
RS
