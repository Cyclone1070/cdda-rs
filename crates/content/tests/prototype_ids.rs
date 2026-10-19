//! The ids each registry holds, compared with what the reference reports.
//!
//! `corpus/prototypes/ids.json` is written by the instrumented reference
//! (`--dump-prototype-ids`) after it loads the core content: ninety registries
//! and every concrete id in each. The port has no merge rules yet, so this is the
//! first thing it can be held to: after reading the same files in the same order,
//! it must arrive at the same set of ids in every registry.

#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::unwrap_used,
    clippy::indexing_slicing
)]

use cdda_content::scan;
use cdda_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

fn reference_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../references/Cataclysm-DDA")
}

/// The reference's own report: registry name to its sorted, deduplicated ids.
fn expected() -> BTreeMap<String, BTreeSet<String>> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/prototypes/ids.json");
    let bytes = std::fs::read(&path).expect("corpus/prototypes/ids.json");
    let value = cdda_json::parse(&bytes).expect("corpus parses");
    let Value::Object(root) = value else {
        panic!("corpus root is not an object")
    };
    let Some(Value::Array(registries)) = root.get("registries") else {
        panic!("corpus has no registries")
    };
    let mut out = BTreeMap::new();
    for registry in registries {
        let Value::Object(registry) = registry else {
            continue;
        };
        let (Some(Value::Str(name)), Some(Value::Array(ids))) =
            (registry.get("type"), registry.get("ids"))
        else {
            continue;
        };
        let ids: BTreeSet<String> = ids
            .iter()
            .filter_map(|id| match id {
                Value::Str(id) => Some(id.clone()),
                _ => None,
            })
            .collect();
        out.insert(name.clone(), ids);
    }
    out
}

/// The best-matching JSON type for a registry, by how many ids they share.
fn pair<'a>(
    ids: &BTreeSet<String>,
    candidates: &'a BTreeMap<String, BTreeSet<String>>,
) -> Option<(&'a String, &'a BTreeSet<String>, usize)> {
    let mut best: Option<(&String, &BTreeSet<String>, usize)> = None;
    for (name, theirs) in candidates {
        let shared = ids.intersection(theirs).count();
        if best.is_none_or(|(_, _, best_shared)| shared > best_shared) {
            best = Some((name, theirs, shared));
        }
    }
    best
}

/// Pairs each reference registry with the JSON type that shares the most ids with
/// it, and says how the two differ. A pair counts as identified when the registry
/// shares at least half its ids with that type; anything less is a registry whose
/// ids are not in the content files at all.
fn differences(
    ours: &BTreeMap<String, BTreeSet<String>>,
    theirs: &BTreeMap<String, BTreeSet<String>>,
) -> Vec<Difference> {
    let mut out = Vec::new();
    for (registry, expected_ids) in theirs {
        let Some((type_name, found, shared)) = pair(expected_ids, ours) else {
            out.push(Difference {
                registry: registry.clone(),
                type_name: None,
                shared: 0,
                missing: expected_ids.clone(),
                extra: BTreeSet::new(),
            });
            continue;
        };
        // A pair is only believed when it is a majority of both sides: a single
        // coincidence between a one-id registry and an eight-id type is not a
        // pairing, and treating it as one would report seven invented ids.
        if shared * 2 < expected_ids.len().max(1) || shared * 2 < found.len().max(1) {
            out.push(Difference {
                registry: registry.clone(),
                type_name: None,
                shared,
                missing: expected_ids.clone(),
                extra: BTreeSet::new(),
            });
            continue;
        }
        out.push(Difference {
            registry: registry.clone(),
            type_name: Some(type_name.clone()),
            shared,
            missing: expected_ids.difference(found).cloned().collect(),
            extra: found.difference(expected_ids).cloned().collect(),
        });
    }
    out
}

struct Difference {
    registry: String,
    type_name: Option<String>,
    shared: usize,
    missing: BTreeSet<String>,
    extra: BTreeSet<String>,
}

#[test]
fn the_port_never_invents_an_id_the_reference_does_not_have() {
    let data = reference_root().join("data/json");
    let ours: BTreeMap<String, BTreeSet<String>> = scan(&data)
        .expect("scan")
        .into_iter()
        .map(|registry| (registry.type_name, registry.ids.into_iter().collect()))
        .collect();
    let theirs = expected();
    let report = differences(&ours, &theirs);

    let mut invented = Vec::new();
    for difference in &report {
        if !difference.extra.is_empty() {
            invented.push(format!(
                "{} <- {}: {} ids the reference does not have, e.g. {:?}",
                difference.registry,
                difference.type_name.as_deref().unwrap_or("(unpaired)"),
                difference.extra.len(),
                difference.extra.iter().take(3).collect::<Vec<_>>()
            ));
        }
    }
    assert!(invented.is_empty(), "{}", invented.join("\n"));
}

#[test]
#[ignore = "M1 gate: needs the loader that creates synthetic and inline prototypes"]
fn every_registry_holds_the_same_ids_as_the_reference() {
    let data = reference_root().join("data/json");
    let ours: BTreeMap<String, BTreeSet<String>> = scan(&data)
        .expect("scan")
        .into_iter()
        .map(|registry| (registry.type_name, registry.ids.into_iter().collect()))
        .collect();
    let theirs = expected();
    let report = differences(&ours, &theirs);

    let exact = report
        .iter()
        .filter(|difference| difference.missing.is_empty() && difference.extra.is_empty())
        .count();
    for difference in &report {
        if difference.missing.is_empty() && difference.extra.is_empty() {
            continue;
        }
        println!(
            "{} <- {}: {} shared, {} missing, {} extra",
            difference.registry,
            difference.type_name.as_deref().unwrap_or("(unpaired)"),
            difference.shared,
            difference.missing.len(),
            difference.extra.len()
        );
    }
    assert_eq!(
        exact,
        theirs.len(),
        "{} of {} registries match",
        exact,
        theirs.len()
    );
}
