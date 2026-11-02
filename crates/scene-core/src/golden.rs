//! The net of the golden tables (P2): layer 0, the parts and their reading,
//! and layer 1 each print every combination they decide and compare it with
//! their file under `contract/`. Only tests use it: the core does no IO, not
//! even here (rule 8), so each table reads its file at compile time.

use serde::Serialize;

/// A domain value as the contract names it, if it serializes to a name.
pub(crate) fn name<T: Serialize>(value: T) -> Option<String> {
    serde_json::to_value(value)
        .ok()
        .and_then(|v| v.as_str().map(str::to_owned))
}

/// A value as a table writes it: its name in the contract, or nothing if it has
/// none.
pub(crate) fn wire<T: Serialize>(value: T) -> String {
    name(value).unwrap_or_default()
}

/// Fails unless `actual` is `golden`, row by row. The message shows the rows
/// that changed and the whole new table: if a rule changed on purpose, it is
/// copied into `file` and the diff goes into the commit.
#[track_caller]
pub(crate) fn assert_table(file: &str, golden: &str, actual: &str) {
    let different: Vec<_> = golden
        .lines()
        .zip(actual.lines())
        .filter(|(g, a)| g != a)
        .map(|(g, a)| format!("  golden: {g}\n  now:    {a}"))
        .collect();
    assert!(
        different.is_empty() && golden.lines().count() == actual.lines().count(),
        "the table changed; if on purpose, copy it to contract/{file}:\n{}\n\nthe whole new table:\n{actual}",
        different.join("\n")
    );
}
