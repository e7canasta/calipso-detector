# Rule 21: an index that can go out of bounds in the core library must fail.
cat >> crates/scene-core/src/geom.rs <<'RS'

pub fn first(v: &[f32]) -> f32 {
    v[0]
}
RS
