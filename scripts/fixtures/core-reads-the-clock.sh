# Rules 8/9: reading a clock in the core must make clippy fail.
cat >> crates/scene-core/src/lib.rs <<'RS'

pub fn now() -> std::time::Instant {
    std::time::Instant::now()
}
RS
