//! BDD harness (rule 15): runs every `.feature` in `features/` against the loop
//! with scripted senses. An undefined step fails.
//!
//! One scenario: `cargo test -p scene-core --test bdd -- --name "<scenario>"`.
//!
//! Features tagged `@wip` (a spec in progress, still red) do not run in
//! `scripts/check.sh`; `scripts/wip.sh` shows them (`CALIPSO_WIP=1`). Features
//! tagged `@shell` belong to the shell (`features/shell`) and the `detector`
//! harness runs them.

// Tests are shell: the harness reads the environment. Rule 8 governs the core
// library, not its tests.
#![allow(clippy::disallowed_methods)]

mod attention;
mod contract;
#[path = "../equivalence.rs"]
mod equivalence;
mod occupancy;
mod posture;
#[path = "../script.rs"]
mod script;
mod subject;
mod ward;
mod zones;

use cucumber::World;
use ward::Ward;

fn main() {
    let features = concat!(env!("CARGO_MANIFEST_DIR"), "/../../features");
    let wip = std::env::var_os("CALIPSO_WIP").is_some();
    futures::executor::block_on(
        Ward::cucumber()
            .fail_on_skipped()
            .with_default_cli()
            .filter_run_and_exit(features, move |feature, _, scenario| {
                let tags = || feature.tags.iter().chain(&scenario.tags);
                // The shell's capability (models, crops, profile) is checked in
                // its own crate: only the core's policy runs here.
                let shell = tags().any(|t| t == "shell");
                let tagged = tags().any(|t| t == "wip");
                !shell && tagged == wip
            }),
    );
}
