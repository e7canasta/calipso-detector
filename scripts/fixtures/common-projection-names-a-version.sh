# C2: the common projection naming a published version must fail.
cat >> crates/scene-core/src/project/mod.rs <<'RS'

#[allow(dead_code)]
fn version() -> scene_contract::SchemaId {
    scene_contract::SchemaId::SceneV4
}
RS
