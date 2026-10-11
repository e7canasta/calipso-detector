//! Records which source tree built the binary, for `source.build` in the
//! published contract. `BUILD_GIT_SHA` overrides it (container builds without
//! `.git`); otherwise git is asked, and `unknown` is used when it cannot say.
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-env-changed=BUILD_GIT_SHA");
    println!("cargo:rerun-if-changed=../../.git/HEAD");
    println!("cargo:rerun-if-changed=../../.git/index");
    println!("cargo:rerun-if-changed=../../.git/refs/heads/main");
    let sha = std::env::var("BUILD_GIT_SHA").ok().or_else(git_sha);
    println!(
        "cargo:rustc-env=BUILD_GIT_SHA={}",
        sha.as_deref().unwrap_or("unknown")
    );
}

fn git_sha() -> Option<String> {
    let run = |args: &[&str]| {
        Command::new("git")
            .args(args)
            .output()
            .ok()
            .filter(|output| output.status.success())
    };
    let sha = String::from_utf8(run(&["rev-parse", "--short=12", "HEAD"])?.stdout).ok()?;
    let dirty = run(&["status", "--porcelain", "--untracked-files=no"])
        .is_some_and(|output| !output.stdout.is_empty());
    Some(format!(
        "{}{}",
        sha.trim(),
        if dirty { "-dirty" } else { "" }
    ))
}
