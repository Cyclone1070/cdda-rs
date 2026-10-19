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
        // An exact match wins outright; a tie between two types that merely
        // share one id is decided by the first name, which is arbitrary, so it
        // must not be allowed to hide an exact match further down.
        if theirs == ids {
            return Some((name, theirs, ids.len()));
        }
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
    let root = reference_root();
    let core = root.join("data/core");
    let content = root.join("data/json");
    let ours: BTreeMap<String, BTreeSet<String>> = scan(&[&core, &content])
        .expect("scan")
        .into_iter()
        .map(|registry| (registry.type_name, registry.ids.into_iter().collect()))
        .collect();
    let theirs = expected();
    let mut ours = ours;
    // The enchantment registry holds ids that no file writes: every inline
    // `enchantments` entry becomes a prototype named after its parent
    // (`generated::enchantment_ids`).
    if let Ok(generated) = cdda_content::generated::enchantment_ids(&[&core, &content]) {
        ours.insert("enchantment".to_owned(), generated);
    }
    if let Ok(buffs) = cdda_content::generated::buff_ids(&[&core, &content]) {
        ours.insert("martial art buff".to_owned(), buffs);
    }
    // The turret parts are additions: the registry also holds the parts the
    // files define.
    if let Ok(items) = cdda_content::items::load_paths(&[&core, &content]) {
        ours.entry("vehicle_part".to_owned())
            .or_default()
            .extend(cdda_content::generated::turret_part_ids(&items));
    }
    // Monster types come from a loader rather than from a file scan: the
    // reference adds a null type and a dormant twin for every monster flagged
    // `GEN_DORMANT`, and only the loader knows about them.
    if let Ok(types) = cdda_content::monsters::load_paths(&[&core, &content]) {
        ours.insert(
            "monster type".to_owned(),
            types.into_iter().map(|monster| monster.id).collect(),
        );
    }
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
// The registries the port cannot account for yet, each needing the loader that
// builds its ids: `effect_on_condition` fills from inline slots inside furniture
// and tools, `overmap terrain` and `overmap terrain type` from mapgen, and
// `overmap special` from city-building entries. Their loaders are P1 work.
#[ignore = "86 of 90 registries: the remaining four need the mapgen and inline-EOC loaders"]
fn every_registry_holds_the_same_ids_as_the_reference() {
    let root = reference_root();
    let core = root.join("data/core");
    let content = root.join("data/json");
    let ours: BTreeMap<String, BTreeSet<String>> = scan(&[&core, &content])
        .expect("scan")
        .into_iter()
        .map(|registry| (registry.type_name, registry.ids.into_iter().collect()))
        .collect();
    let theirs = expected();
    let mut ours = ours;
    // The enchantment registry holds ids that no file writes: every inline
    // `enchantments` entry becomes a prototype named after its parent
    // (`generated::enchantment_ids`).
    if let Ok(generated) = cdda_content::generated::enchantment_ids(&[&core, &content]) {
        ours.insert("enchantment".to_owned(), generated);
    }
    if let Ok(buffs) = cdda_content::generated::buff_ids(&[&core, &content]) {
        ours.insert("martial art buff".to_owned(), buffs);
    }
    // The turret parts are additions: the registry also holds the parts the
    // files define.
    if let Ok(items) = cdda_content::items::load_paths(&[&core, &content]) {
        ours.entry("vehicle_part".to_owned())
            .or_default()
            .extend(cdda_content::generated::turret_part_ids(&items));
    }
    // Monster types come from a loader rather than from a file scan: the
    // reference adds a null type and a dormant twin for every monster flagged
    // `GEN_DORMANT`, and only the loader knows about them.
    if let Ok(types) = cdda_content::monsters::load_paths(&[&core, &content]) {
        ours.insert(
            "monster type".to_owned(),
            types.into_iter().map(|monster| monster.id).collect(),
        );
    }
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
