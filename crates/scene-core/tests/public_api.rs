//! The public surface of `scene-core` is the top-level `pub` list of its `lib.rs`
//! (P10), frozen in `tests/public_api.txt`: changing the API without updating
//! it fails here. Update it on purpose with
//! `CALIPSO_UPDATE_PUBLIC_API=1 cargo test -p scene-core --test public_api`.
//! `PUBLIC_API_LIB` points the test at another `lib.rs`:
//! `scripts/check-public-api.sh` uses it to see the guard fail on an added `pub`.

// Rule 8 bounds the core, not its tests: this one reads the source of lib.rs.
#![allow(clippy::disallowed_methods)]

#[path = "../../../scripts/public_api/extract.rs"]
mod extract;

use std::path::{Path, PathBuf};

#[test]
fn the_public_surface_is_the_snapshot() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let other = std::env::var_os("PUBLIC_API_LIB");
    let lib = other
        .clone()
        .map_or_else(|| manifest.join("src/lib.rs"), PathBuf::from);
    let source =
        std::fs::read_to_string(&lib).unwrap_or_else(|e| panic!("reads {}: {e}", lib.display()));
    let actual = extract::surface(&source);
    let snapshot = manifest.join("tests/public_api.txt");
    if std::env::var_os("CALIPSO_UPDATE_PUBLIC_API").is_some() && other.is_none() {
        std::fs::write(&snapshot, &actual).expect("writes the snapshot");
        return;
    }
    let expected = std::fs::read_to_string(&snapshot).expect("reads the snapshot");
    assert_eq!(
        actual, expected,
        "the public surface of `scene-core` changed; if on purpose: CALIPSO_UPDATE_PUBLIC_API=1 cargo test -p scene-core --test public_api"
    );
}
