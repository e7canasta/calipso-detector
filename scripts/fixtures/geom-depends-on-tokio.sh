# expect-step: core dependencies
# expect-text: rule 8: scene-core depends on IO: tokio
# The leaf of the language does IO: scene-geom must not depend on tokio, and the check must fail.
cargo add -q -p scene-geom tokio@1 --no-default-features
