# expect-step: the domain without contract
# expect-text: C2: the common projection names a retired version
# C2: the common projection naming a published version must fail. The name goes
# before the test module: an item after it fails clippy first, and that is not
# this guardian.
perl -0pi -e 's/(#\[cfg\(test\)\]\nmod tests)/#[allow(dead_code)]\nconst NAMES_A_VERSION: &str = "SceneV4";\n\n$1/' crates/scene-core/src/project/mod.rs
grep -q 'NAMES_A_VERSION' crates/scene-core/src/project/mod.rs || { echo "fixture: the insertion before the test module failed" >&2; exit 1; }
