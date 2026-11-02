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

/// The names of the publication: a domain file that names one of these from
/// `scene_contract` breaks the rule.
const PUBLICATION: &[&str] = &["Scene", "Event", "Envelope", "SchemaId"];

/// Where the publication may be named: the translation and the sequencing.
const EXEMPT: &[&str] = &["project/", "cycle/", "cycle.rs"];

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

#[test]
fn the_domain_does_not_name_the_publication_outside_project_and_cycle() {
    let root = src();
    let mut files = Vec::new();
    rust_files(&root, &mut files);
    let mut offenders = Vec::new();
    for file in files {
        let relative = file
            .strip_prefix(&root)
            .expect("a file under src")
            .to_string_lossy()
            .replace('\\', "/");
        if EXEMPT
            .iter()
            .any(|e| relative == *e || relative.starts_with(e))
        {
            continue;
        }
        let text = fs::read_to_string(&file).expect("a readable source file");
        let names = named_from_the_contract(&text);
        if let Some(name) = names.iter().find(|n| PUBLICATION.contains(&n.as_str())) {
            offenders.push(format!("{relative} names scene_contract::{name}"));
        }
    }
    assert!(
        offenders.is_empty(),
        "the domain names the published scene or events outside project/ and cycle/:\n  {}",
        offenders.join("\n  ")
    );
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
