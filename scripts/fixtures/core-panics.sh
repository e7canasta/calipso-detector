# expect-step: lints
# expect-text: indexing may panic
# Rule 21: an index that can go out of bounds in the core library must fail.
cat >> crates/scene-core/src/lib.rs <<'RS'

/// The first value.
pub fn first(v: &[f32]) -> f32 {
    v[0]
}
RS
