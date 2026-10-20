//! Overmap terrain, and the ids it registers.
//!
//! Reference: `src/omdata.h:372` for the rotation rule, `src/overmap.cpp:262-275`
//! for the line-drawing shapes, and the dump from `--dump-prototype-ids` for the
//! answer. The rule itself was measured and written down before this test existed:
//! [docs/15-overmap-terrain-ids.md](../../docs/15-overmap-terrain-ids.md).

use cdda_content::overmap;
use cdda_json::Value;
use std::collections::BTreeSet;
use std::path::PathBuf;

/// Two types that carry `NO_ROTATE` by inheritance and rotate anyway in the
/// reference's registry. The flag rule explains 5,901 of 5,903 rotatable types;
/// these are the two it does not, and they are named here rather than papered
/// over by weakening the rule for everyone. See docs/15-overmap-terrain-ids.md.
const ROTATION_OUTLIERS: [&str; 2] = [
    "microlab_ratkin_surface_connector",
    "microlab_ratkin_vent_shaft_connector",
];

fn reference_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../references/Cataclysm-DDA")
}

// The corpus is a build artefact of the oracle, so a missing or unparsable one is
// a broken checkout rather than a test finding; failing loudly is the honest
// report, and this file's job is to assert, not to recover.
#[allow(clippy::expect_used, clippy::panic)]
fn reference_ids(registry: &str) -> BTreeSet<String> {
    let corpus = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../corpus/prototypes/ids.json");
    let text = std::fs::read_to_string(&corpus).expect("id corpus");
    let parsed = cdda_json::parse(text.as_bytes()).expect("corpus parses");
    let Value::Object(root) = parsed else {
        panic!("corpus is an object");
    };
    let Some(Value::Array(registries)) = root.get("registries") else {
        panic!("corpus has a registry list");
    };
    for registry_value in registries {
        let Value::Object(entry) = registry_value else {
            continue;
        };
        if entry.get("type") == Some(&Value::Str(registry.to_owned())) {
            let Some(Value::Array(ids)) = entry.get("ids") else {
                panic!("registry {registry} has ids");
            };
            return ids
                .iter()
                .filter_map(|id| match id {
                    Value::Str(text) => Some(text.clone()),
                    _ => None,
                })
                .collect();
        }
    }
    panic!("registry {registry} is in the corpus");
}

#[allow(clippy::expect_used, clippy::panic)]
#[test]
fn the_type_registry_is_the_json_ids_and_nothing_else() {
    let root = reference_root();
    let started = std::time::Instant::now();
    let loaded = overmap::load_paths(&[&root.join("data/core"), &root.join("data/json")])
        .expect("the overmap terrain files load");

    let ours: BTreeSet<String> = loaded.types.iter().map(|kind| kind.id.clone()).collect();
    let theirs = reference_ids("overmap terrain type");

    let missing: Vec<&String> = theirs.difference(&ours).take(10).collect();
    let invented: Vec<&String> = ours.difference(&theirs).take(10).collect();
    println!(
        "overmap terrain types: {} ours, {} reference, {} missing, {} invented in {:.1}s",
        ours.len(),
        theirs.len(),
        theirs.difference(&ours).count(),
        ours.difference(&theirs).count(),
        started.elapsed().as_secs_f64()
    );
    assert!(
        missing.is_empty() && invented.is_empty(),
        "missing {missing:?}, invented {invented:?}"
    );
}

#[allow(clippy::expect_used, clippy::panic)]
#[test]
fn the_terrain_registry_is_the_types_plus_their_variants() {
    let root = reference_root();
    let loaded = overmap::load_paths(&[&root.join("data/core"), &root.join("data/json")])
        .expect("the overmap terrain files load");

    let ours: BTreeSet<String> = loaded.terrains.iter().cloned().collect();
    let theirs = reference_ids("overmap terrain");

    let invented: BTreeSet<&String> = ours.difference(&theirs).collect();
    let unexpected: Vec<&&String> = invented
        .iter()
        .filter(|id| !ROTATION_OUTLIERS.contains(&id.as_str()))
        .collect();
    assert!(
        unexpected.is_empty(),
        "the port made up ids the reference does not have: {unexpected:?}"
    );
    assert_eq!(
        invented.len(),
        ROTATION_OUTLIERS.len(),
        "the outlier list should describe every id the rule cannot explain"
    );

    let missing: BTreeSet<&String> = theirs.difference(&ours).collect();
    println!(
        "overmap terrains: {} ours, {} reference, {} missing",
        ours.len(),
        theirs.len(),
        missing.len()
    );
    // Each outlier registers four variants in the reference and one bare id here,
    // so its eight variants are the whole of what is left.
    assert_eq!(
        missing.len(),
        ROTATION_OUTLIERS.len() * 4,
        "the only ids still missing should be the four variants of each outlier; \
         first: {:?}",
        missing.iter().take(8).collect::<Vec<_>>()
    );
}

/// The reference also mints a `FakeSpecial_<oter>` for every single-tile city
/// building named in a region's building list but not defined as a special
/// (`src/regional_settings.cpp:1242-1262`, `src/overmap.cpp:8048-8065`). Those are
/// not written yet; the count is asserted so the gap cannot drift.
const FAKE_SPECIAL_GAP: usize = 35;

#[allow(clippy::expect_used, clippy::panic)]
#[test]
fn the_special_registry_is_specials_plus_city_buildings() {
    let root = reference_root();
    let loaded = overmap::load_paths(&[&root.join("data/core"), &root.join("data/json")])
        .expect("the overmap files load");

    let theirs = reference_ids("overmap special");
    let invented: Vec<&String> = loaded.specials.difference(&theirs).take(5).collect();
    assert!(invented.is_empty(), "invented specials: {invented:?}");

    let missing: Vec<&String> = theirs.difference(&loaded.specials).collect();
    let fake: Vec<&&String> = missing
        .iter()
        .filter(|id| id.starts_with("FakeSpecial_"))
        .collect();
    println!(
        "overmap specials: {} ours, {} reference, {} missing of which {} are generated",
        loaded.specials.len(),
        theirs.len(),
        missing.len(),
        fake.len()
    );
    assert_eq!(missing.len(), FAKE_SPECIAL_GAP);
    assert_eq!(
        fake.len(),
        FAKE_SPECIAL_GAP,
        "every id still missing should be a generated FakeSpecial"
    );
}
