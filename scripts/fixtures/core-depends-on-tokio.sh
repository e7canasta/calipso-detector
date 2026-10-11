# Rule 8: an IO dependency in the core must make the check fail.
cargo add -q -p scene-core tokio@1 --no-default-features
