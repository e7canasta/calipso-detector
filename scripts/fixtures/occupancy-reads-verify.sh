# expect-step: the occupancy names its own slice
# expect-text: the occupancy names a policy of another layer
# The occupancy reading a verification setting (here, the held check's policy) must fail.
cat >> crates/scene-core/src/belief/occupancy/mod.rs <<'RS'

#[allow(dead_code)]
fn held_verify(v: &crate::policy::VerifyPolicy) -> bool {
    v.held.is_some()
}
RS
