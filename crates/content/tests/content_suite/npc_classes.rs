//! The NPC class table: every class the reference defines, and nothing else.
//!
//! `npc_class` decides what an NPC is before they are given a personality - which skills they start with, what they carry,
//! what they wear. The port had no loader for it, so the whole family of NPC-bearing mapgen was untranslatable. This gate is
//! the same one the other eighty-odd registries have: the ids the reference reports, exactly, in both directions.

#![allow(clippy::panic, clippy::expect_used, clippy::indexing_slicing)]

use std::collections::BTreeSet;

fn reference_ids(registry: &str) -> BTreeSet<String> {
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/prototypes/ids.json");
    let text = std::fs::read_to_string(&path).expect("the prototype artifact");
    let value = cdda_json::parse(text.as_bytes()).expect("valid json");
    let cdda_json::Value::Object(root) = value else {
        panic!("the artifact is an object")
    };
    let Some(cdda_json::Value::Array(registries)) = root.get("registries") else {
        panic!("registries")
    };
    for entry in registries {
        let cdda_json::Value::Object(fields) = entry else {
            continue;
        };
        let Some(cdda_json::Value::Str(name)) = fields.get("type") else {
            continue;
        };
        if name != registry {
            continue;
        }
        let Some(cdda_json::Value::Array(ids)) = fields.get("ids") else {
            panic!("ids")
        };
        return ids
            .iter()
            .map(|id| match id {
                cdda_json::Value::Str(text) => text.clone(),
                other => panic!("an id that is not a string: {other:?}"),
            })
            .collect();
    }
    panic!("the artifact has no registry {registry}");
}

#[test]
fn the_class_registry_is_the_json_ids_and_nothing_else() {
    let reference = reference_ids("npc_class");
    assert_eq!(
        reference.len(),
        155,
        "the reference defines this many classes"
    );
    let dirs = [std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../references/Cataclysm-DDA/data/json")];
    let dirs: Vec<&std::path::Path> = dirs.iter().map(std::path::PathBuf::as_path).collect();
    let classes = cdda_content::npc_classes::load_paths(&dirs).expect("the classes load");
    let ours: BTreeSet<String> = classes.iter().map(|class| class.id.clone()).collect();
    let invented: Vec<&String> = ours.difference(&reference).collect();
    let missing: Vec<&String> = reference.difference(&ours).collect();
    assert!(invented.is_empty(), "invented ids: {invented:?}");
    assert!(missing.is_empty(), "missing ids: {missing:?}");
    assert_eq!(ours.len(), reference.len());
}

#[test]
fn a_class_carries_the_name_the_data_gives_it() {
    // The name is a translated string object in the data (`{ "str": "No class" }`), not a bare string, which is the sort of
    // shape that silently loads as empty if the reader only looks for one form.
    let dirs = [std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../references/Cataclysm-DDA/data/json")];
    let dirs: Vec<&std::path::Path> = dirs.iter().map(std::path::PathBuf::as_path).collect();
    let classes = cdda_content::npc_classes::load_paths(&dirs).expect("the classes load");
    let no_class = classes
        .iter()
        .find(|class| class.id == "NC_NONE")
        .expect("the default class");
    assert_eq!(no_class.name, "No class");
    let named = classes
        .iter()
        .filter(|class| !class.name.is_empty())
        .count();
    assert!(
        named > 140,
        "{named} classes of {} have a name",
        classes.len()
    );
}

/// **A shopkeeper's shelf is part of their class.** Reference: `npc_class::load` (`npc_class.cpp:252-268`): a class may name `shopkeeper_item_group`, either as an array of group
/// objects (`group`, `trust`, `strict`, `rigid`) or as **one bare group id**, and a `restock_interval` that defaults to **six days** (`npc_class.h:103`).
///
/// The test loads the real data: at least one class must carry a shop, and the classes that do not must say **six days** rather than zero - a port that left the default out
/// would make every non-shopkeeper restock instantly, and a port that read the string as seconds would make a six-day shop restock every six seconds.
#[test]
fn a_shopkeeper_class_carries_its_shelf_and_its_interval() {
    let roots = [std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../references/Cataclysm-DDA/data/json")];
    let roots: Vec<&std::path::Path> = roots.iter().map(std::path::PathBuf::as_path).collect();
    let classes = cdda_content::npc_classes::load_paths(&roots).expect("the classes load");
    assert!(
        classes.len() > 20,
        "the corpus has classes to read: {}",
        classes.len()
    );
    let with_a_shop = classes
        .iter()
        .filter(|class| !class.shop_item_groups.is_empty())
        .collect::<Vec<_>>();
    assert!(
        !with_a_shop.is_empty(),
        "the data has at least one shopkeeper class"
    );
    let six_days = 6 * 24 * 60 * 60;
    let defaulted = classes
        .iter()
        .filter(|class| class.restock_interval == six_days)
        .count();
    assert!(
        defaulted > 0,
        "a class that names no interval waits six days, and this corpus has a class that names none"
    );
    // **And a class that names one gets exactly what it names.** `NC_LUMBERJACK_MERCHANT` writes `"1 day"`, which is the case that separates a unit table from a default: a
    // port that read the day as one second would leave every defaulted class right and this one six thousand times too fast.
    let named = classes
        .iter()
        .find(|class| class.id == "NC_LUMBERJACK_MERCHANT")
        .expect("the merchant class");
    assert_eq!(
        named.restock_interval,
        24 * 60 * 60,
        "**\"1 day\" is a day, not a second and not six**"
    );
    // **Every group names a group id**, and a rigid one is a fact the data states.
    for class in &with_a_shop {
        for group in &class.shop_item_groups {
            assert!(!group.group.is_empty(), "**{} names its group**", class.id);
        }
    }
    let rigid = with_a_shop
        .iter()
        .flat_map(|class| &class.shop_item_groups)
        .filter(|group| group.rigid)
        .count();
    println!("classes with a shop: {}", with_a_shop.len());
    println!("rigid groups: {rigid}");
}
