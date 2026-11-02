# expect-step: the domain without contract
# expect-text: the_domain_names_from_the_wire_only_the_shared_core
# G3: the domain naming a wire type outside the shared core must fail its
# guardian. `Verifier` is on the wire and not in SHARED_CORE; the re-export is
# unused on purpose, so that only the guardian fails.
cat >> crates/scene-core/src/refine/mod.rs <<'RS'

#[allow(unused_imports)]
pub(crate) use scene_contract::Verifier;
RS
