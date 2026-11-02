# expect-step: lints
# expect-text: missing documentation for a function
# The core's public API is documented: an undocumented public item must fail.
cat >> crates/scene-core/src/lib.rs <<'RS'

pub fn undocumented() {}
RS
