//! The domain's guardian (rule 13 and the scene's contract): the domain speaks
//! its own vocabulary and the wire's shared types, and names the published
//! scene and its events only where they are translated (`project`) or
//! sequenced (`cycle`). This test reads the sources, so it needs no tool that a
//! CI may not have.

#![allow(
    clippy::disallowed_methods,
    reason = "a guardian reads the domain's own sources (std::fs), which clippy.toml forbids in the core: this test is the one place that does it"
)]

use std::fs;
use std::path::{Path, PathBuf};

/// The shared core: the wire names the domain may name outside `project/` and
/// `cycle/`. Point 1 of the ADR «el dominio es dueño de lo que decide»; the
/// second test reads it there, so this list and the ADR cannot drift apart.
const SHARED_CORE: &[&str] = &[
    "SampleRef",
    "SessionId",
    "SourceSecs",
    "Zone",
    "Counts",
    "Provenance",
    "Keypoint",
    "Joint",
    "BedSite",
    "SensorState",
    "FrameInfo",
    "Geometry",
    "BedPostureState",
    "Attention",
    "Look",
    "LookOutcome",
    "LookResult",
    "SkipReason",
    "LookKind",
    "Failure",
    "MapState",
    "Calibration",
    "MapShadow",
    "BodyShadow",
    "PlanePoint",
    "ShadowPlane",
    "BodySource",
    "RejectReason",
    "RepublishReason",
];

/// Where the wire may be named in full: the translation (`project`) and the
/// sequencing (`cycle`, including `cycle/engine/` once M4 moves it there).
const EXEMPT: &[&str] = &["project/", "cycle/", "cycle.rs"];

const ADR: &str = "../../../calipso-docs/adr/2026-11-02-el-dominio-es-dueno-de-lo-que-decide.md";

fn src() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("src")
}

fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).expect("the source directory is readable") {
        let path = entry.expect("a directory entry").path();
        if path.is_dir() {
            rust_files(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
}

/// The identifiers a `scene_contract` path names in `text`: the braced list of
/// `scene_contract::{…}` or the single `scene_contract::Name`, in any position
/// (a `use`, or a path written inline).
fn named_from_the_contract(text: &str) -> Vec<String> {
    const PATH: &str = "scene_contract::";
    let mut names = Vec::new();
    let mut rest = text;
    while let Some(at) = rest.find(PATH) {
        let after = &rest[at + PATH.len()..];
        let list = if let Some(braced) = after.strip_prefix('{') {
            // A braced list: up to its matching close (lists here are not nested).
            braced.split('}').next().unwrap_or_default()
        } else {
            after
                .split(|c: char| !(c.is_alphanumeric() || c == '_'))
                .next()
                .unwrap_or_default()
        };
        names.extend(
            list.split(|c: char| !(c.is_alphanumeric() || c == '_'))
                .filter(|word| !word.is_empty())
                .map(str::to_owned),
        );
        rest = after;
    }
    names
}

/// The item a `#[cfg(test)]` gates, from just after the attribute: up to its
/// `;` (`mod tests;`) or the close of its first brace.
fn gated_item_end(after: &str) -> usize {
    let mut depth = 0usize;
    let mut opened = false;
    for (at, c) in after.char_indices() {
        match c {
            ';' if !opened => return at + 1,
            '{' => {
                depth += 1;
                opened = true;
            }
            '}' if opened => {
                depth -= 1;
                if depth == 0 {
                    return at + 1;
                }
            }
            _ => {}
        }
    }
    after.len()
}

/// The production code of a file: its text without the items gated by
/// `#[cfg(test)]`. Test code may name the wire (a golden table prints the
/// contract's names); production code may not, outside the shared core.
/// Braces inside string literals are not counted: a test module with an
/// unbalanced brace in a string would cut the wrong span.
fn production_text(text: &str) -> String {
    const GATE: &str = "#[cfg(test)]";
    let mut out = String::new();
    let mut rest = text;
    while let Some(at) = rest.find(GATE) {
        out.push_str(&rest[..at]);
        let after = &rest[at + GATE.len()..];
        rest = &after[gated_item_end(after)..];
    }
    out.push_str(rest);
    out
}

/// The modules a `#[cfg(test)] mod name;` declares in `text`: their files are
/// test code, whatever they are called (`verification/table.rs` is a golden
/// table's printer, declared by `verification.rs` under the gate).
fn gated_test_modules(text: &str) -> Vec<String> {
    const GATE: &str = "#[cfg(test)]";
    let mut names = Vec::new();
    let mut rest = text;
    while let Some(at) = rest.find(GATE) {
        let mut after = rest[at + GATE.len()..].trim_start();
        while after.starts_with("#[") {
            after = after.split_once(']').map_or("", |(_, r)| r).trim_start();
        }
        if let Some((name, _)) = after
            .strip_prefix("mod ")
            .and_then(|decl| decl.split_once(';'))
        {
            names.push(name.trim().to_owned());
        }
        rest = &rest[at + GATE.len()..];
    }
    names
}

/// The file of a module declared as `name` in the file `relative`.
fn module_file(relative: &str, name: &str) -> String {
    match relative.strip_suffix("/mod.rs") {
        Some(dir) => format!("{dir}/{name}.rs"),
        None => format!("{}/{name}.rs", relative.trim_end_matches(".rs")),
    }
}

/// Whether a file is test code as a whole: a module declared under the test
/// gate, or a file named as a test module.
fn is_a_test_file(relative: &str, test_files: &[String]) -> bool {
    let name = relative.rsplit('/').next().unwrap_or(relative);
    name == "tests.rs"
        || name == "test.rs"
        || name.ends_with("_tests.rs")
        || test_files.iter().any(|t| t == relative)
}

/// Every production file of the core outside `project/` and `cycle/`, as its
/// path relative to `src` and its production text.
fn outside_project_and_cycle() -> Vec<(String, String)> {
    let sources = core_sources();
    let test_files: Vec<String> = sources
        .iter()
        .flat_map(|(relative, text)| {
            gated_test_modules(text)
                .into_iter()
                .map(move |name| module_file(relative, &name))
        })
        .collect();
    sources
        .into_iter()
        .filter(|(relative, _)| {
            !EXEMPT
                .iter()
                .any(|e| relative == e || relative.starts_with(e))
        })
        .filter(|(relative, _)| !is_a_test_file(relative, &test_files))
        .map(|(relative, text)| (relative, production_text(&text)))
        .collect()
}

#[test]
fn the_domain_names_from_the_wire_only_the_shared_core() {
    let mut offenders = Vec::new();
    for (relative, text) in outside_project_and_cycle() {
        for name in named_from_the_contract(&text) {
            if !SHARED_CORE.contains(&name.as_str()) {
                offenders.push(format!("{relative} names scene_contract::{name}"));
            }
        }
    }
    offenders.sort();
    offenders.dedup();
    assert!(
        offenders.is_empty(),
        "production code of the domain names a wire type outside the shared core of the ADR (el dominio es dueño de lo que decide, point 1); move it to the domain or to project/:\n  {}",
        offenders.join("\n  ")
    );
}

#[test]
fn the_shared_core_is_the_list_of_the_adr() {
    let adr = fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join(ADR))
        .expect("the domain's ADR is readable: calipso-docs is next to this repository");
    for name in SHARED_CORE {
        assert!(
            adr.contains(&format!("`{name}`")),
            "{name} is in the guardian's shared core but not in the ADR"
        );
    }
}

#[test]
fn the_guardian_tells_test_code_from_production() {
    let text = "use scene_contract::Verdict;\n\
        #[cfg(test)]\nmod tests {\n    use scene_contract::PartsReading;\n}\n\
        #[cfg(test)] use scene_contract::Head;\n\
        #[cfg(test)]\nmod fixtures;\n\
        fn decide() -> u8 { 1 }\n";
    assert_eq!(
        named_from_the_contract(&production_text(text)),
        ["Verdict"],
        "a test line is not production; a production line is"
    );
    let declared = "use x;\n#[cfg(test)]\nmod table;\n#[cfg(test)]\n#[allow(dead_code)]\nmod burst_table;\nmod machine;\n";
    assert_eq!(gated_test_modules(declared), ["table", "burst_table"]);
    assert_eq!(
        module_file("verification.rs", "table"),
        "verification/table.rs"
    );
    assert_eq!(module_file("belief/mod.rs", "table"), "belief/table.rs");
    let tests = vec!["verification/table.rs".to_owned()];
    assert!(is_a_test_file("verification/table.rs", &tests));
    assert!(!is_a_test_file("verification.rs", &tests));
    assert!(is_a_test_file("refine/tests.rs", &tests));
    assert!(is_a_test_file("belief/parts_tests.rs", &tests));
}

#[test]
fn the_reader_sees_a_braced_list_and_a_single_path() {
    assert_eq!(
        named_from_the_contract("use scene_contract::{Cell, Event,\n    Fv};"),
        ["Cell", "Event", "Fv"]
    );
    assert_eq!(
        named_from_the_contract("let s: scene_contract::Scene;"),
        ["Scene"]
    );
    assert!(named_from_the_contract("use scene_geom::Rect;").is_empty());
}

/// The core's code lines: comments are not decisions, so they are skipped.
fn code_lines(text: &str) -> impl Iterator<Item = &str> {
    text.lines()
        .filter(|line| !line.trim_start().starts_with("//"))
}

/// Whether `line` holds a trait object (`dyn`) that is not a local closure
/// predicate. `&dyn Fn(…)` is allowed: a predicate passed down for one
/// computation. Any other `dyn` (`Box<dyn …>`, `Rc<dyn …>`, `&dyn Opinion`, a
/// `dyn` in a signature) is a decision behind a dynamic object, and breaks
/// rule 24.
fn names_a_trait_object(line: &str) -> bool {
    line.match_indices("dyn ").any(|(at, _)| {
        let word_start = at == 0 || !line.as_bytes()[at - 1].is_ascii_alphanumeric();
        let after = &line[at + "dyn ".len()..];
        let local_predicate = line[..at].ends_with('&')
            && ["Fn(", "FnMut(", "FnOnce("]
                .iter()
                .any(|f| after.starts_with(f));
        word_start && !local_predicate
    })
}

/// Whether `line` declares a map whose key is text (`String`, `&str`,
/// `&'static str`). A policy or evidence keyed by text lets data name the
/// decisions, which rule 24 forbids.
fn keys_a_map_by_text(line: &str) -> bool {
    ["HashMap<", "BTreeMap<", "IndexMap<"].iter().any(|map| {
        line.match_indices(map).any(|(at, m)| {
            let key = line[at + m.len()..].trim_start();
            let key = key.strip_prefix('&').unwrap_or(key).trim_start();
            let key = key
                .strip_prefix("'static ")
                .or_else(|| key.strip_prefix("'a "))
                .unwrap_or(key);
            ["str", "String"].iter().any(|t| {
                key.strip_prefix(t).is_some_and(|rest| {
                    !rest.starts_with(|c: char| c.is_alphanumeric() || c == '_')
                })
            })
        })
    })
}

/// Whether `line` holds a sequence of the round's phases (`Vec<Phase>`,
/// `[Reached; 3]`, `&[Phase]`): the order of the round would then be a value
/// that data can fill, not the match in `next`.
fn sequences_the_round(line: &str) -> bool {
    ["Vec<", "VecDeque<", "[", "&["].iter().any(|open| {
        ["Reached", "Phase"].iter().any(|step| {
            line.match_indices(step).any(|(at, _)| {
                line[..at].ends_with(open)
                    && !line[at + step.len()..]
                        .starts_with(|c: char| c.is_alphanumeric() || c == '_')
            })
        })
    })
}

/// The files of the core as `(path relative to src, text)`.
fn core_sources() -> Vec<(String, String)> {
    let root = src();
    let mut files = Vec::new();
    rust_files(&root, &mut files);
    files
        .into_iter()
        .map(|file| {
            let relative = file
                .strip_prefix(&root)
                .expect("a file under src")
                .to_string_lossy()
                .replace('\\', "/");
            let text = fs::read_to_string(&file).expect("a readable source file");
            (relative, text)
        })
        .collect()
}

#[test]
fn the_core_does_not_decide_behind_a_dynamic_object() {
    let offenders: Vec<String> = core_sources()
        .iter()
        .flat_map(|(relative, text)| {
            code_lines(text)
                .filter(|line| names_a_trait_object(line))
                .map(move |line| format!("{relative}: {}", line.trim()))
        })
        .collect();
    assert!(
        offenders.is_empty(),
        "a decision sits behind a trait object (Box/Rc/Arc/&dyn of a step or opinion); only &dyn Fn predicates are allowed (rule 24):\n  {}",
        offenders.join("\n  ")
    );
}

#[test]
fn the_core_does_not_key_policy_or_evidence_by_text() {
    let offenders: Vec<String> = core_sources()
        .iter()
        .flat_map(|(relative, text)| {
            code_lines(text)
                .filter(|line| keys_a_map_by_text(line))
                .map(move |line| format!("{relative}: {}", line.trim()))
        })
        .collect();
    assert!(
        offenders.is_empty(),
        "a map keyed by String or &str holds the policy or the evidence by name (rule 24):\n  {}",
        offenders.join("\n  ")
    );
}

#[test]
fn the_order_of_the_round_is_not_a_value() {
    let offenders: Vec<String> = core_sources()
        .iter()
        .filter(|(relative, _)| relative.starts_with("cycle/") || relative == "cycle.rs")
        .flat_map(|(relative, text)| {
            code_lines(text)
                .filter(|line| sequences_the_round(line))
                .map(move |line| format!("{relative}: {}", line.trim()))
        })
        .collect();
    assert!(
        offenders.is_empty(),
        "the order of a round is a sequence held as data; it lives in the match of `next` (rule 24):\n  {}",
        offenders.join("\n  ")
    );
}

#[test]
fn the_guardian_tells_a_predicate_from_a_decision() {
    assert!(!names_a_trait_object(
        "fn group(inside: &dyn Fn(Point) -> bool) {"
    ));
    assert!(names_a_trait_object("    steps: Vec<Box<dyn Step>>,"));
    assert!(names_a_trait_object("fn decide(o: &dyn Opinion) {"));
    assert!(names_a_trait_object("let rc: Rc<dyn Rule> = r;"));
    assert!(!keys_a_map_by_text("let by_id: HashMap<u32, Policy> = m;"));
    assert!(keys_a_map_by_text("    rules: HashMap<String, f32>,"));
    assert!(keys_a_map_by_text(
        "    rules: BTreeMap<&'static str, Policy>,"
    ));
    assert!(sequences_the_round("let steps = [Phase::Done];"));
    assert!(sequences_the_round("let order: Vec<Reached> = Vec::new();"));
    assert!(sequences_the_round("const ORDER: &[Phase] = &[];"));
    assert!(!sequences_the_round(
        "fn next(&mut self, reached: Reached) -> Phase {"
    ));
}

/// Whether `line` reads a field of the engine (`.engine.field`, not a method
/// call `.engine.method(…)`) or destructures it (`let Engine { … }`). Rule 11,
/// by Demeter: the sequence asks the engine by name, it does not look inside.
fn reads_an_engine_field(line: &str) -> bool {
    const FIELD: &str = ".engine.";
    let field_read = line.match_indices(FIELD).any(|(at, _)| {
        let after = &line[at + FIELD.len()..];
        let name_len = after
            .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
            .unwrap_or(after.len());
        name_len > 0 && after[name_len..].chars().next().is_some_and(|c| c != '(')
    });
    let destructured = line
        .match_indices("let Engine")
        .any(|(at, m)| line[at + m.len()..].trim_start().starts_with('{'));
    field_read || destructured
}

#[test]
fn the_round_asks_the_engine_by_name_and_does_not_look_inside() {
    let offenders: Vec<String> = core_sources()
        .iter()
        .filter(|(relative, _)| relative.starts_with("cycle/") || relative == "cycle.rs")
        .flat_map(|(relative, text)| {
            text.lines()
                .filter(|line| reads_an_engine_field(line))
                .map(move |line| format!("{relative}: {}", line.trim()))
        })
        .collect();
    assert!(
        offenders.is_empty(),
        "outside cycle/engine.rs the round reads a field of the engine; the engine answers by a named method (rule 11, by Demeter):\n  {}",
        offenders.join("\n  ")
    );
}

#[test]
fn the_guardian_tells_a_field_read_from_a_method_call() {
    assert!(reads_an_engine_field(
        "let f = self.engine.frame.is_some();"
    ));
    assert!(reads_an_engine_field("let f = course.engine.frame;"));
    assert!(reads_an_engine_field("let Engine { belief, .. } = engine;"));
    assert!(reads_an_engine_field("let Engine{ belief, .. } = engine;"));
    assert!(!reads_an_engine_field(
        "let r = self.engine.reinforcement(&primary);"
    ));
    assert!(!reads_an_engine_field("fn new(policy: Policy) -> Engine {"));
    assert!(!reads_an_engine_field("let x = self.engine;"));
}
