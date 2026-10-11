# C2: the common projection naming a contract version must fail.
cat >> crates/scene-core/src/project/mod.rs <<'RS'

#[allow(dead_code)]
fn version() -> crate::contract::v4::SchemaId {
    crate::contract::v4::SchemaId::SceneV4
}
RS
