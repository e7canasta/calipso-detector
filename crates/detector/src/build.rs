//! The binary's identification in the published contract.
use scene_contract::Build;

pub fn current() -> Build {
    Build {
        version: env!("CARGO_PKG_VERSION").to_owned(),
        git: env!("BUILD_GIT_SHA").to_owned(),
    }
}
