# expect-step: no retired v4 in the detector
# expect-text: v4 reappears in the detector
# ADR 2026-11-02-retirar-v4: the detector publishes v5 alone, and v4 does not
# come back in its code. Naming the retired schema fails the check.
cat >> crates/detector/src/lib.rs <<'RS'

#[allow(dead_code)]
const RETIRED: &str = "calipso.scene.v4";
RS
