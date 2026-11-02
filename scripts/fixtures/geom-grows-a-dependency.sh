# expect-step: scene-geom is a leaf
# expect-text: scene-geom is not a leaf: it depends on smallvec
# A leaf depends on a closed list (serde, schemars). A dependency with no IO
# passes the core's check, since the core does not name it, and must fail the
# leaf's: it is the leaf's own rule, not the core's.
cargo add -q -p scene-geom smallvec
