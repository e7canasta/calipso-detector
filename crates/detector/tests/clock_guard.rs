//! The shell's clock guardian (F2; ADR 2026-11-02-reloj-inyectado-en-la-cascara).
//! The shell measures time through its injected `Clock`, and reads the system
//! clock only where the ADR justifies it. This test reads the sources: any
//! `Instant::now()` or `SystemTime::now()` in production code outside the list
//! below fails it. The test modules (`#[cfg(test)]`) are not production code.

use std::fs;
use std::path::{Path, PathBuf};

/// The production files that may read the system clock, each with the reason.
/// Adding one is a decision that goes into the ADR, not into this list alone.
const ALLOWED: &[(&str, &str)] = &[
    (
        "clock.rs",
        "the injected clock itself: SystemClock is the one place that reads the system clock",
    ),
    (
        "source/rtsp.rs",
        "`started` is the arrival stamp of a camera frame and the camera's pacing; both move to the \
         injected clock with F6 (ADR, análisis de `wait_ms`)",
    ),
    (
        "source/frames.rs",
        "the arrival stamp of a frame read from disk: one instant, measured through the injected \
         clock with F6 (ADR: no changes anything measured)",
    ),
    (
        "publish/nats.rs",
        "the shutdown grace of the process: scheduling, not a measurement (out of scope of F6)",
    ),
    (
        "publish.rs",
        "`published_at_ms`: the wall-clock time of a publication, a field of the wire metadata \
         that replay ignores (ADR, sección de `published_at_ms`); SystemTime, not Instant",
    ),
];

/// The production lines of `text`: the `#[cfg(test)]` items (a test module, or any
/// item with a block) are left out, with their braces counted.
fn production_lines(text: &str) -> Vec<(usize, &str)> {
    let mut out = Vec::new();
    let mut depth: i64 = 0;
    let mut in_test = false;
    let mut await_item = false;
    for (n, line) in text.lines().enumerate() {
        let t = line.trim();
        let delta = line.matches('{').count() as i64 - line.matches('}').count() as i64;
        if in_test {
            depth += delta;
            in_test = depth > 0;
            continue;
        }
        if t == "#[cfg(test)]" {
            await_item = true;
            continue;
        }
        if await_item && !t.is_empty() && !t.starts_with("#[") {
            await_item = false;
            if delta > 0 {
                in_test = true;
                depth = delta;
            }
            continue;
        }
        out.push((n + 1, line));
    }
    out
}

fn reads_the_clock(line: &str) -> bool {
    let code = line.trim();
    !code.starts_with("//")
        && (code.contains("Instant::now()") || code.contains("SystemTime::now()"))
}

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

fn relative(root: &Path, file: &Path) -> String {
    file.strip_prefix(root)
        .expect("a file under src")
        .to_string_lossy()
        .replace('\\', "/")
}

#[test]
fn no_production_code_reads_the_system_clock_outside_the_allowed_files() {
    let root = src();
    let mut files = Vec::new();
    rust_files(&root, &mut files);
    let mut offenders = Vec::new();
    for file in files {
        let name = relative(&root, &file);
        if ALLOWED.iter().any(|(allowed, _)| *allowed == name) {
            continue;
        }
        let text = fs::read_to_string(&file).expect("a readable source file");
        for (line_no, line) in production_lines(&text) {
            if reads_the_clock(line) {
                offenders.push(format!("{name}:{line_no}: {}", line.trim()));
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "production code reads the system clock outside the allowed list; measure through the \
         injected Clock, or justify the file in ALLOWED and in the ADR:\n  {}",
        offenders.join("\n  ")
    );
}

#[test]
fn every_allowed_file_exists_and_still_reads_the_clock() {
    let root = src();
    for (name, reason) in ALLOWED {
        assert!(!reason.is_empty(), "{name} has no reason");
        let path = root.join(name);
        let text = fs::read_to_string(&path)
            .unwrap_or_else(|_| panic!("{name} is allowed but does not exist"));
        assert!(
            production_lines(&text)
                .iter()
                .any(|(_, line)| reads_the_clock(line)),
            "{name} is allowed but no longer reads the clock: remove it from ALLOWED"
        );
    }
}

#[test]
fn the_reader_leaves_out_test_modules_and_comments() {
    let text = "fn a() { let t = Instant::now(); }\n\
                /// Instant::now() in a doc comment\n\
                #[cfg(test)]\n\
                mod tests {\n    fn b() { let t = Instant::now(); }\n}\n\
                fn c() {}\n";
    let lines: Vec<&str> = production_lines(text).iter().map(|(_, l)| *l).collect();
    assert!(lines.iter().any(|l| l.contains("fn a()")));
    assert!(lines.iter().any(|l| l.contains("fn c()")));
    assert!(
        !lines.iter().any(|l| l.contains("fn b()")),
        "the test module is left out"
    );
    assert!(!reads_the_clock("/// Instant::now() in a doc comment"));
}
