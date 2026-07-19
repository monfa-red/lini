//! Guards for the diagnostic code set [decision 7]: every code is unique, and a
//! snapshot pins each number to its family so a renumber (or a moved code) fails
//! CI. Codes are **stable once assigned** — this snapshot is the contract.

use super::codes::{CATALOG, Code, Phase};

#[test]
fn codes_are_unique() {
    let mut seen = std::collections::HashSet::new();
    for c in CATALOG {
        assert!(
            seen.insert(c.as_str()),
            "duplicate diagnostic code {} (family {})",
            c.as_str(),
            c.family
        );
    }
}

#[test]
fn families_are_unique() {
    let mut seen = std::collections::HashSet::new();
    for c in CATALOG {
        assert!(
            seen.insert(c.family),
            "duplicate diagnostic family '{}'",
            c.family
        );
    }
}

#[test]
fn every_phase_has_a_generic() {
    for phase in [
        Phase::Lex,
        Phase::Parse,
        Phase::Resolve,
        Phase::Validate,
        Phase::Layout,
        Phase::Route,
    ] {
        let g = Code::generic(phase);
        assert_eq!(g.num, 0);
        assert_eq!(g.phase, phase);
    }
}

/// The code → family contract. Renumbering or renaming a family churns this
/// snapshot — codes are stable once assigned.
#[test]
fn catalog_is_pinned() {
    let dump: String = CATALOG
        .iter()
        .map(|c| format!("{} {}\n", c.as_str(), c.family))
        .collect();
    insta::assert_snapshot!("catalog", dump);
}
