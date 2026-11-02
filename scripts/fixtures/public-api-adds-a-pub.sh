# expect-step: tests
# expect-text: the public surface of `scene-core` changed
# The public surface of scene-core is frozen (P10): a public item the snapshot
# does not name must fail the tests. The item is documented, so the lints pass.
cat >> crates/scene-core/src/lib.rs <<'RS'

/// Added by the fixture: a public item the snapshot does not name.
pub fn fixture_added() {}
RS
