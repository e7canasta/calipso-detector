//! The capability crosses Probe → Reading; the policies are tested in scene-core.
use cucumber::World;
mod depth;
mod distill;
pub mod driver;
mod pose;
mod probe;
mod profile;
mod profile_capability;
#[path = "../support/mod.rs"]
mod support;
mod world;
use world::Shell;

fn main() {
    let features = concat!(env!("CARGO_MANIFEST_DIR"), "/../../features");
    let wip = std::env::var_os("CALIPSO_WIP").is_some();
    // Cucumber's CLI filters replace our own filter; always keep @shell.
    let selected = std::env::var("CALIPSO_SCENARIO").ok();
    futures::executor::block_on(
        Shell::cucumber()
            .fail_on_skipped()
            .with_default_cli()
            .filter_run_and_exit(features, move |feature, rule, scenario| {
                let tags = || {
                    feature
                        .tags
                        .iter()
                        .chain(rule.into_iter().flat_map(|r| &r.tags))
                        .chain(&scenario.tags)
                };
                tags().any(|t| t == "shell")
                    && tags().any(|t| t == "wip") == wip
                    && selected
                        .as_ref()
                        .is_none_or(|name| scenario.name.contains(name))
            }),
    );
}
