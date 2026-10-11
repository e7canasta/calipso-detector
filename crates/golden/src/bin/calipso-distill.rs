//! Distills a published night (scene.v4 as JSONL) into a golden scene.
//!
//! ```text
//! calipso-distill night.jsonl [--run <run_id>] [--dwell <s>] > golden/night.md
//! calipso-detector --config config/scene.toml --scene golden/night.md
//! ```
use golden::distill::distill;
use golden::language::write;
use std::process::ExitCode;

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let (mut path, mut run, mut dwell) = (None, None, None);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--run" => run = args.next(),
            "--dwell" => match args.next().map(|v| v.parse::<f64>()) {
                Some(Ok(v)) => dwell = Some(v),
                _ => return usage("--dwell needs seconds"),
            },
            "-h" | "--help" => return usage(""),
            other if path.is_none() && !other.starts_with("--") => path = Some(other.to_owned()),
            other => return usage(&format!("unknown argument: {other}")),
        }
    }
    let Some(path) = path else {
        return usage("missing the night file");
    };
    let text = match std::fs::read_to_string(&path) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("read {path}: {e}");
            return ExitCode::FAILURE;
        }
    };
    match distill(&text, run.as_deref(), dwell) {
        Ok(d) => {
            let description = format!(
                "Distilled from run `{}` of camera `{}` ({path}). The expectations are what\nthe engine believed live.",
                d.run_id, d.camera
            );
            print!("{}", write(&d.scene, &description));
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("{path}: {e}");
            ExitCode::FAILURE
        }
    }
}

fn usage(reason: &str) -> ExitCode {
    if !reason.is_empty() {
        eprintln!("{reason}");
    }
    eprintln!("usage: calipso-distill <night.jsonl> [--run <run_id>] [--dwell <s>]");
    if reason.is_empty() {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}
