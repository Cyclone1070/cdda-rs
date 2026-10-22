//! The prototype digest, checked two ways: the algorithm against the reference's own ids, and the
//! port's loaded registries against the reference's digests.

#![allow(clippy::expect_used, clippy::panic)]

use cdda_content::{digest, fields, starting_kit, terrain, vehicles};
use cdda_json::{Value, parse};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn read(name: &str) -> BTreeMap<String, Value> {
    let text = std::fs::read_to_string(root().join(name)).expect("artifact");
    let value = parse(text.as_bytes()).expect("parses");
    let Value::Object(object) = value else {
        panic!("root")
    };
    let Some(Value::Array(entries)) = object.get("registries") else {
        panic!("registries")
    };
    let mut out = BTreeMap::new();
    for entry in entries {
        let Value::Object(fields) = entry else {
            panic!("entry")
        };
        let Some(Value::Str(name)) = fields.get("type") else {
            panic!("type")
        };
        out.insert(name.clone(), entry.clone());
    }
    out
}

fn ids_of(entry: &Value) -> Vec<String> {
    let Value::Object(fields) = entry else {
        panic!("entry")
    };
    let Some(Value::Array(ids)) = fields.get("ids") else {
        panic!("ids")
    };
    ids.iter()
        .map(|id| match id {
            Value::Str(text) => text.clone(),
            other => panic!("an id that is not a string: {other:?}"),
        })
        .collect()
}

fn digest_of(entry: &Value) -> String {
    let Value::Object(fields) = entry else {
        panic!("entry")
    };
    let Some(Value::Str(text)) = fields.get("digest") else {
        panic!("digest")
    };
    text.clone()
}

#[test]
fn the_algorithm_reproduces_every_digest_the_reference_wrote() {
    let ids = read("corpus/prototypes/ids.json");
    let digests = read("corpus/prototypes/digest.json");
    let mut checked = 0;
    for (name, entry) in &ids {
        let Some(reference) = digests.get(name) else {
            continue;
        };
        let ours = digest::digest_hex(digest::digest_ids(ids_of(entry)));
        assert_eq!(&ours, &digest_of(reference), "registry {name}");
        checked += 1;
    }
    assert_eq!(checked, 90, "every registry in the artifact was checked");
}

#[test]
fn the_order_and_repeats_of_the_input_do_not_matter() {
    let forward = digest::digest_ids(["b", "a", "c", "a"]);
    let backward = digest::digest_ids(["c", "b", "a"]);
    assert_eq!(forward, backward, "sorted and deduplicated first");
    assert_ne!(
        digest::digest_ids(["ab", "c"]),
        digest::digest_ids(["a", "bc"]),
        "the separator is what keeps these apart"
    );
}

#[test]
fn the_ports_own_material_and_trap_ids_match_the_reference_digest() {
    let digests = read("corpus/prototypes/digest.json");
    let loaded_materials = crate::corpus::materials_loaded();
    let ours = digest::digest_hex(digest::digest_ids(
        loaded_materials.iter().map(|material| material.id.as_str()),
    ));
    let reference = digests.get("material").expect("material digest");
    assert_eq!(ours, digest_of(reference), "material ids");

    let loaded_traps = crate::corpus::traps_loaded();
    let ours = digest::digest_hex(digest::digest_ids(
        loaded_traps.iter().map(|trap| trap.id.as_str()),
    ));
    let reference = digests.get("trap").expect("trap digest");
    assert_eq!(ours, digest_of(reference), "trap ids");
}

#[test]
fn the_bigger_registries_match_too() {
    let digests = read("corpus/prototypes/digest.json");

    let loaded_items = crate::corpus::items_loaded();
    let ours = digest::digest_hex(digest::digest_ids(
        loaded_items.iter().map(|item| item.id.as_str()),
    ));
    assert_eq!(
        ours,
        digest_of(digests.get("ITEM").expect("ITEM digest")),
        "item ids"
    );

    let loaded_monsters = crate::corpus::monsters_loaded();
    let ours = digest::digest_hex(digest::digest_ids(
        loaded_monsters.iter().map(|monster| monster.id.as_str()),
    ));
    assert_eq!(
        ours,
        digest_of(digests.get("monster type").expect("monster digest")),
        "monster ids"
    );

    let loaded_overmap = crate::corpus::overmap_loaded();
    let ours = digest::digest_hex(digest::digest_ids(loaded_overmap.terrains.iter()));
    assert_eq!(
        ours,
        digest_of(
            digests
                .get("overmap terrain")
                .expect("overmap terrain digest")
        ),
        "overmap terrain ids"
    );
    let ours = digest::digest_hex(digest::digest_ids(
        loaded_overmap.types.iter().map(|kind| kind.id.as_str()),
    ));
    assert_eq!(
        ours,
        digest_of(
            digests
                .get("overmap terrain type")
                .expect("overmap terrain type digest")
        ),
        "overmap terrain type ids"
    );
}

#[test]
fn the_registries_that_come_from_dumps_match_as_well() {
    // These two load from an artifact rather than the data files, so a match here says the dump
    // captured the same id set the reference's registry held.
    let digests = read("corpus/prototypes/digest.json");
    let fields_artifact =
        std::fs::read_to_string(root().join("corpus/prototypes/fields.json")).expect("artifact");
    let value = parse(fields_artifact.as_bytes()).expect("parses");
    let loaded = fields::from_dump(&value).expect("fields load");
    let ours = digest::digest_hex(digest::digest_ids(
        loaded.iter().map(|field| field.id.as_str()),
    ));
    assert_eq!(
        ours,
        digest_of(digests.get("field types").expect("field digest")),
        "field ids"
    );

    let terrain_artifact =
        std::fs::read_to_string(root().join("corpus/prototypes/terrain.json")).expect("artifact");
    let value = parse(terrain_artifact.as_bytes()).expect("parses");
    let loaded = terrain::from_dump(&value).expect("terrains load");
    let ours = digest::digest_hex(digest::digest_ids(
        loaded.iter().map(|kind| kind.id.as_str()),
    ));
    assert_eq!(
        ours,
        digest_of(digests.get("terrain").expect("terrain digest")),
        "terrain ids"
    );
}

#[test]
fn furniture_specials_vehicles_and_the_starting_kit_match() {
    let digests = read("corpus/prototypes/digest.json");

    let artifact =
        std::fs::read_to_string(root().join("corpus/prototypes/terrain.json")).expect("artifact");
    let value = parse(artifact.as_bytes()).expect("parses");
    let furniture = terrain::furniture_from_dump(&value).expect("furniture loads");
    let ours = digest::digest_hex(digest::digest_ids(
        furniture.iter().map(|kind| kind.id.as_str()),
    ));
    assert_eq!(
        ours,
        digest_of(digests.get("furniture").expect("furniture digest"))
    );

    let artifact = std::fs::read_to_string(root().join("corpus/vehicles.json")).expect("artifact");
    let value = parse(artifact.as_bytes()).expect("parses");
    let loaded = vehicles::from_dump(&value).expect("vehicles load");
    let ours = digest::digest_hex(digest::digest_ids(loaded.prototypes.iter()));
    assert_eq!(
        ours,
        digest_of(digests.get("vehicle").expect("vehicle digest")),
        "vehicles"
    );
    let ours = digest::digest_hex(digest::digest_ids(loaded.parts.iter()));
    assert_eq!(
        ours,
        digest_of(digests.get("vehicle_part").expect("vehicle part digest")),
        "vehicle parts"
    );

    let artifact =
        std::fs::read_to_string(root().join("corpus/starting_kit.json")).expect("artifact");
    let value = parse(artifact.as_bytes()).expect("parses");
    let kit = starting_kit::from_dump(&value).expect("kit loads");
    let ours = digest::digest_hex(digest::digest_ids(kit.professions.iter()));
    assert_eq!(
        ours,
        digest_of(digests.get("profession").expect("profession digest")),
        "professions"
    );
    let ours = digest::digest_hex(digest::digest_ids(kit.scenarios.iter()));
    assert_eq!(
        ours,
        digest_of(digests.get("scenario").expect("scenario digest")),
        "scenarios"
    );

    let loaded = crate::corpus::overmap_loaded();
    let ours = digest::digest_hex(digest::digest_ids(loaded.specials.iter()));
    assert_eq!(
        ours,
        digest_of(digests.get("overmap special").expect("special digest")),
        "overmap specials"
    );
}

#[test]
fn the_effects_the_harvests_and_the_classes_are_gated_too() {
    // Three registries the port already loaded correctly and nobody had asked it to prove: an effect-on-condition (913, 692 of them defined and 221
    // inline in someone else's slot), a harvest list (366) and an NPC class (155). A loader that a gate does not cover is a loader that drifts, and these
    // three drifted into the corpus long before this test existed.
    let digests = read("corpus/prototypes/digest.json");

    let effects = crate::corpus::eocs_loaded();
    let ours = digest::digest_hex(digest::digest_ids(effects.ids.iter().map(String::as_str)));
    assert_eq!(
        ours,
        digest_of(digests.get("effect_on_condition").expect("effect digest")),
        "effect-on-condition ids"
    );

    let harvests = crate::corpus::harvests_loaded();
    let ours = digest::digest_hex(digest::digest_ids(harvests.keys().map(String::as_str)));
    assert_eq!(
        ours,
        digest_of(digests.get("harvest_list").expect("harvest digest")),
        "harvest list ids"
    );

    let classes = crate::corpus::npc_classes_loaded();
    let ours = digest::digest_hex(digest::digest_ids(
        classes.iter().map(|class| class.id.as_str()),
    ));
    assert_eq!(
        ours,
        digest_of(digests.get("npc_class").expect("class digest")),
        "npc class ids"
    );
}
