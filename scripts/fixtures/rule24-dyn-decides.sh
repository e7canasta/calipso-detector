# expect-step: the domain without contract
# expect-text: the_core_does_not_decide_behind_a_dynamic_object
# Rule 24: a decision behind a dynamic object in the core must fail its
# guardian. A `Box<dyn Fn…>` step is not a predicate passed down for one
# computation (`&dyn Fn` is allowed), so the core would be generalising the
# decision.
cat >> crates/scene-core/src/cycle/sequence.rs <<'RS'

#[allow(dead_code)]
fn decide_by(step: Box<dyn Fn() -> u8>) -> u8 {
    step()
}
RS
