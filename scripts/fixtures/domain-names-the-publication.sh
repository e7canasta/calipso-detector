# The domain speaks its own vocabulary: naming the published scene from attention must fail the check.
cat >> crates/scene-core/src/attention/mod.rs <<'RS'

#[allow(dead_code)]
fn published() -> Option<scene_contract::Scene> {
    None
}
RS
