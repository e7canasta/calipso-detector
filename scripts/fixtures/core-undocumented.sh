# The core's public API is documented: an undocumented public item must fail.
cat >> crates/scene-core/src/lib.rs <<'RS'

pub fn undocumented() {}
RS
