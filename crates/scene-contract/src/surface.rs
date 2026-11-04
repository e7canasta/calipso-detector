//! The one policy of the wire's public surface: every published enum is
//! classified in one of two classes and wears its class's shape. Rule 13
//! governs over any guide — an incompatible change to the published contract
//! is a major contract version with an ADR — and the classes state what that
//! means for each vocabulary:
//!
//! - **open** — a status, a reason, a failure or a who/which/how label of an
//!   observation, a look or the reception. Its list can grow within v5
//!   without changing what is already published says about the world, so a
//!   Rust consumer must tolerate unknown ones: `#[non_exhaustive]`.
//! - **closed** — the domain's vocabulary: what the wire says about the world
//!   (states, zones, places, joints, the kinds of facts on the tape). A new
//!   variant is an incompatible change of the contract, versioned with an ADR
//!   (rule 13), so it stays exhaustive: the compiler points at every decision
//!   when that change comes, in the core and in every consumer.
//!
//! `UnitError` is not wire (a Rust error of `scene-core`) and follows the open
//! shape on its own (`scene-core/src/units.rs`).
//!
//! The guardian test below lists every published enum with its class, and
//! fails when one is unclassified (a new one) or wears the wrong shape.

/// The two classes of the policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Class {
    /// A label that can grow within v5 without changing the meaning of what is
    /// published: `#[non_exhaustive]`.
    Open,
    /// Domain vocabulary: a new variant is versioned with an ADR (rule 13), so
    /// it stays exhaustive.
    Closed,
}

/// Every published enum of the wire, with its class. A new enum fails the
/// guardian until it is listed here.
const CLASSES: &[(&str, Class)] = &[
    // Open: statuses, reasons, failures and who/which/how labels.
    ("Provenance", Class::Open),
    ("SensorState", Class::Open),
    ("LookKind", Class::Open),
    ("Failure", Class::Open),
    ("RepublishReason", Class::Open),
    ("SkipReason", Class::Open),
    ("BodySource", Class::Open),
    ("RejectReason", Class::Open),
    ("Cause", Class::Open),
    ("Verifier", Class::Open),
    ("HeadEvidence", Class::Open),
    ("Geometry", Class::Open),
    ("LookResult", Class::Open),
    ("LookOutcome", Class::Open),
    ("FacesStatus", Class::Open),
    ("BodiesStatus", Class::Open),
    ("HeldCheckState", Class::Open),
    ("ShadowPlane", Class::Open),
    ("Calibration", Class::Open),
    // Closed: the domain's vocabulary; growth is versioned (rule 13).
    ("Zone", Class::Closed),
    ("Mode", Class::Closed),
    ("Phase", Class::Closed),
    ("BedState", Class::Closed),
    ("PostureState", Class::Closed),
    ("LyingState", Class::Closed),
    ("PartsReading", Class::Closed),
    ("BedPostureState", Class::Closed),
    ("BedPostureSeen", Class::Closed),
    ("HeadPlaceState", Class::Closed),
    ("TorsoShadow", Class::Closed),
    ("LegsSide", Class::Closed),
    ("PartsStep", Class::Closed),
    ("TurnRule", Class::Closed),
    ("BedSite", Class::Closed),
    ("Joint", Class::Closed),
    ("SchemaId", Class::Closed),
    ("FactBody", Class::Closed),
    ("DerivedBody", Class::Closed),
    ("SensorBody", Class::Closed),
    ("MapBody", Class::Closed),
    ("Event", Class::Closed),
];

#[cfg(test)]
mod tests {
    use super::{CLASSES, Class};
    use std::path::PathBuf;

    /// Every `pub enum` of the crate's source, with whether it wears
    /// `#[non_exhaustive]` (its attribute block is read above the declaration).
    fn published_enums() -> Vec<(String, bool)> {
        let src = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut found = Vec::new();
        let mut files: Vec<PathBuf> = std::fs::read_dir(&src)
            .expect("reads the source directory")
            .map(|e| e.expect("reads an entry").path())
            .filter(|p| p.extension().is_some_and(|e| e == "rs"))
            .collect();
        files.sort();
        for path in files {
            let text = std::fs::read_to_string(&path)
                .unwrap_or_else(|e| panic!("reads {}: {e}", path.display()));
            let lines: Vec<&str> = text.lines().collect();
            for (i, line) in lines.iter().enumerate() {
                let Some(rest) = line.strip_prefix("pub enum ") else {
                    continue;
                };
                let name = rest.trim_end_matches([' ', '{']).to_owned();
                let mut non_exhaustive = false;
                for above in lines[..i].iter().rev() {
                    let above = above.trim();
                    if above == "#[non_exhaustive]" {
                        non_exhaustive = true;
                    } else if !(above.starts_with("#[") || above.starts_with("//")) {
                        break;
                    }
                }
                found.push((name, non_exhaustive));
            }
        }
        found
    }

    /// The policy holds for every published enum, and the list has no stale
    /// entry: a new enum without a class fails here, and so does one wearing
    /// the other class's shape.
    #[test]
    fn every_published_enum_is_classified_and_wears_its_class() {
        for (name, non_exhaustive) in published_enums() {
            let Some((_, class)) = CLASSES.iter().find(|(n, _)| *n == name) else {
                panic!(
                    "{name} is published and unclassified: list it with its class in `surface.rs`"
                );
            };
            match class {
                Class::Open => assert!(
                    non_exhaustive,
                    "{name} is open: it must wear #[non_exhaustive] (rule 13)"
                ),
                Class::Closed => assert!(
                    !non_exhaustive,
                    "{name} is closed: it must stay exhaustive (rule 13)"
                ),
            }
        }
        for (name, _) in CLASSES {
            assert!(
                published_enums().iter().any(|(n, _)| n == name),
                "{name} is classified but is not published any more"
            );
        }
    }
}
