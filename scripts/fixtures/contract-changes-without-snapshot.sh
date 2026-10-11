# Rule 13: a contract change without a snapshot must fail. Only the description
# of a published type changes: it compiles and passes clippy, so the only thing
# that can stop it is the schema snapshot.
sed -i 's|^/// How many people are in the room\.$|/// How many people are in the bedroom.|' crates/scene-core/src/contract/v3.rs
grep -q 'in the bedroom' crates/scene-core/src/contract/v3.rs || { echo "fixture: the description to change was not found" >&2; exit 1; }
