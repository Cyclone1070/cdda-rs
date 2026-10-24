//! Can it be built, from a given kit?
//!
//! `corpus/build/check.json` is `--dump-build-check`: four kit - an empty one, one with tools, one with materials, and the two together - and, for each, whether the
//! reference says every construction can be built. Three thousand and twenty-four answers, which is only useful because the kits differ: a construction that answers the
//! same everywhere proves nothing, and the reference's own answers spread across all eight combinations.

#![allow(clippy::expect_used, clippy::panic)]

use cdda_content::constructions;
use cdda_content::items;
use cdda_content::requirements::{Kit, RequirementData};
use cdda_json::{Object, Value};
use std::collections::BTreeMap;

/// One recorded kit: its name and its stacks.
type Recorded = (String, Vec<(String, i32)>);

fn dump() -> (Vec<Recorded>, Vec<(String, String, bool)>) {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let bytes = std::fs::read(root.join("corpus/build/check.json")).expect("the dump");
    let Value::Object(object) = cdda_json::parse(&bytes).expect("valid JSON") else {
        panic!("the dump is an object")
    };
    let text = |object: &Object, key: &str| -> String {
        match object.get(key) {
            Some(Value::Str(value)) => value.clone(),
            other => panic!("{key} is text: {other:?}"),
        }
    };
    let Some(Value::Array(all)) = object.get("inventories") else {
        panic!("the dump holds inventories")
    };
    let mut kits = Vec::new();
    for entry in all {
        let Value::Object(entry) = entry else {
            panic!("a kit")
        };
        let Some(Value::Array(items)) = entry.get("items") else {
            panic!("a kit's items")
        };
        let stacks = items
            .iter()
            .map(|pair| {
                let Value::Array(pair) = pair else { panic!("a stack") };
                let Some(Value::Str(id)) = pair.first() else {
                    panic!("an id")
                };
                let Some(Value::Int(count)) = pair.get(1) else {
                    panic!("a count")
                };
                (id.clone(), i32::try_from(*count).expect("a count"))
            })
            .collect();
        kits.push((text(entry, "name"), stacks));
    }
    let Some(Value::Array(all)) = object.get("rows") else {
        panic!("the dump holds rows")
    };
    let mut rows = Vec::new();
    for entry in all {
        let Value::Object(entry) = entry else {
            panic!("a row")
        };
        let ok = match entry.get("ok") {
            Some(Value::Bool(ok)) => *ok,
            other => panic!("an answer: {other:?}"),
        };
        rows.push((
            text(entry, "construction"),
            text(entry, "inventory"),
            ok,
        ));
    }
    (kits, rows)
}

#[test]
fn every_construction_answers_the_same_way_for_four_kits_as_the_reference_does() {
    let dirs = crate::corpus::dirs();
    let args: Vec<&std::path::Path> = dirs.iter().map(std::path::PathBuf::as_path).collect();
    let loaded = constructions::load_paths(&args).expect("constructions load");
    let groups = cdda_content::requirements::load_paths(&args).expect("requirement groups load");
    let mut subtypes: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let catalog: BTreeMap<String, items::ItemType> = items::load_paths(&args)
        .expect("items load")
        .into_iter()
        .map(|kind| {
            for parent in &kind.subtypes {
                subtypes.entry(parent.clone()).or_default().push(kind.id.clone());
            }
            (kind.id.clone(), kind)
        })
        .collect();

    let (recorded_kits, rows) = dump();
    assert_eq!(recorded_kits.len(), 7);
    assert_eq!(rows.len(), 5_292, "seven kits against 756 constructions");

    // Build the four kits once, keeping each stack's qualities as the reference reads them off the item type.
    let mut kits: BTreeMap<String, Kit> = BTreeMap::new();
    for (name, stacks) in &recorded_kits {
        let mut kit = Kit::default();
        for (id, count) in stacks {
            let Some(kind) = catalog.get(id) else {
                panic!("the kit names {id}, which the port does not know")
            };
            kit.add(id, *count, kind.counts_by_charge(), kind.qualities.clone());
        }
        kits.insert(name.clone(), kit);
    }

    // Resolve each construction's needs once and ask all four kits.
    let mut needs: BTreeMap<&str, RequirementData> = BTreeMap::new();
    for construction in &loaded {
        needs.insert(
            construction.id.as_str(),
            constructions::needs_of(construction, &groups, &subtypes),
        );
    }

    let mut wrong = Vec::new();
    let mut answered = 0_usize;
    for (id, kit_name, expected) in &rows {
        let Some(need) = needs.get(id.as_str()) else {
            continue;
        };
        let Some(kit) = kits.get(kit_name) else {
            panic!("the dump names a kit {kit_name} that is not in it")
        };
        if id == "constr_veh" {
            continue;
        }
        let got = need.can_make(kit);
        if got != *expected {
            wrong.push(format!(
                "{id} with {kit_name}: {got}, the reference says {expected}"
            ));
        }
        answered += 1;
    }
    assert_eq!(answered, 5_285, "every row but the vehicle's seven");
    assert!(
        wrong.is_empty(),
        "{} differences: {:#?}",
        wrong.len(),
        wrong.iter().take(80).collect::<Vec<_>>()
    );
}

#[test]
fn a_tool_with_more_charges_than_needed_still_satisfies_the_requirement() {
    // The reference's `tool_comp::has` compares with `==`, but the number it compares against is `charges_of( type, charges_required )`, which stops counting once it has
    // enough and comes back **clamped** to the requirement (`src/visitable.cpp:916-921`). A planer with four hundred charges therefore satisfies a requirement for four,
    // and a port that compares the raw charge count for equality would refuse every overcharged tool in the game.
    let need = RequirementData {
        tools: vec![vec![("welder".to_owned(), 4, false)]],
        ..RequirementData::default()
    };
    let mut kit = Kit::default();
    kit.add("welder", 400, true, BTreeMap::new());
    assert!(need.can_make(&kit), "four hundred charges are four");

    let mut exact = Kit::default();
    exact.add("welder", 4, true, BTreeMap::new());
    assert!(need.can_make(&exact), "and exactly four are four");

    let mut short = Kit::default();
    short.add("welder", 3, true, BTreeMap::new());
    assert!(!need.can_make(&short), "three are not");
}

#[test]
fn a_held_tool_is_asked_for_in_items_and_a_spent_one_in_charges() {
    // A **negative** count is a tool to hold - any number of charges left in it will do - and a positive one is a number of charges to spend
    // (`tool_comp::by_charges`, `src/requirements.cpp:167-170`). Reading the sign the other way round turns every hammer into a battery.
    let held = RequirementData {
        tools: vec![vec![("hammer".to_owned(), -1, false)]],
        ..RequirementData::default()
    };
    let mut without = Kit::default();
    without.add("welder", 400, true, BTreeMap::new());
    assert!(!held.can_make(&without), "a welder is not a hammer");

    let mut with = Kit::default();
    with.add("hammer", 1, false, BTreeMap::new());
    assert!(held.can_make(&with), "one hammer is one hammer");

    let spent = RequirementData {
        tools: vec![vec![("welder".to_owned(), 4, false)]],
        ..RequirementData::default()
    };
    let mut bare = Kit::default();
    bare.add("welder", 1, false, BTreeMap::new());
    assert!(
        !spent.can_make(&bare),
        "a welder held but not charged cannot spend four charges"
    );
}
