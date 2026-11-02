# expect-step: tests
# expect-text: rule 13: the scene schema is not the snapshot
# Rule 13: a contract change without a snapshot must fail. Only the description
# of a published type changes: it compiles and passes clippy, so the only thing
# that can stop it is the schema snapshot.
sed -i "s|^/// The room: the machine's mode, the phase and the last cell\.$|/// The bedroom: the machine's mode, the phase and the last cell.|" crates/scene-contract/src/lib.rs
grep -q 'The bedroom' crates/scene-contract/src/lib.rs || { echo "fixture: the description to change was not found" >&2; exit 1; }
