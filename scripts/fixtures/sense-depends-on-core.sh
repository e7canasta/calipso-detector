# The sense is the language's client, not the core's: it must not depend on scene-core, and the check must fail.
cargo add -q -p sense --path crates/scene-core
