# expect-step: the domain without contract
# expect-text: the_domain_names_from_the_wire_only_the_shared_core
# The domain speaks its own vocabulary: naming the published scene from attention must fail the check.
cat >> crates/scene-core/src/attention/mod.rs <<'RS'

#[allow(dead_code)]
fn published() -> Option<scene_contract::v5::Scene> {
    None
}
RS
