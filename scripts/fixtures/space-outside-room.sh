# A layer answering a question of space with the bed's plane must fail.
cat >> crates/scene-core/src/refine/sighting.rs <<'RS'

#[allow(dead_code)]
fn in_bed(bed: &crate::geom::Quad, p: Point) -> bool {
    crate::map::BedPlane::of(bed).is_some_and(|plane| plane.inside(p))
}
RS
