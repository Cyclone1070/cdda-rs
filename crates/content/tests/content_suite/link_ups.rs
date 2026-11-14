//! Every `link_up` use action, resolved: 108 cables.
//!
//! `corpus/items/link-ups.json` is `--dump-link-ups`, from `link_up_actor` (`src/iuse_actor.h:1160-1182`) and its loader (`src/iuse_actor.cpp:4917-4925`). The
//! defaults are what matter here, because 106 of the 108 leave them alone: an efficiency of **0.85** and a move cost of **5**, so a cable nobody described still
//! loses a sixth of its power and still costs a moment to attach.

#![allow(clippy::panic, clippy::expect_used, clippy::indexing_slicing)]

use cdda_content::items::{ItemType, load_paths};
use cdda_content::use_actions::{LinkUp, UseAction};
use cdda_json::Value;
use std::collections::{BTreeMap, BTreeSet};

/// Reads the recorded cables, keyed by the item type that declares them.
fn dump() -> BTreeMap<String, LinkUp> {
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/items/link-ups.json");
    let bytes =
        std::fs::read(&path).unwrap_or_else(|_| panic!("the recording at {}", path.display()));
    let Value::Object(root) = cdda_json::parse(&bytes).expect("valid json") else {
        panic!("the dump is an object")
    };
    let Some(Value::Array(types)) = root.get("types") else {
        panic!("the dump holds types")
    };
    let mut out = BTreeMap::new();
    for entry in types {
        let Value::Object(object) = entry else {
            panic!("a cable")
        };
        let text = |key: &str| match object.get(key) {
            Some(Value::Str(value)) => value.clone(),
            other => panic!("{key} is text: {other:?}"),
        };
        let names = |key: &str| match object.get(key) {
            Some(Value::Array(entries)) => entries
                .iter()
                .map(|entry| match entry {
                    Value::Str(name) => name.clone(),
                    other => panic!("a name: {other:?}"),
                })
                .collect::<BTreeSet<String>>(),
            other => panic!("{key} is a list: {other:?}"),
        };
        out.insert(
            text("id"),
            LinkUp {
                action: text("action"),
                cable_length: match object.get("cable_length") {
                    Some(Value::Int(length)) => i32::try_from(*length).expect("a small number"),
                    other => panic!("cable_length: {other:?}"),
                },
                charge_rate_watts: match object.get("charge_rate_watts") {
                    Some(Value::Int(watts)) => *watts,
                    other => panic!("charge_rate_watts: {other:?}"),
                },
                // `%.9g` writes a whole number without a decimal point, so 0 and 1 arrive as integers.
                efficiency: match object.get("efficiency") {
                    Some(Value::Float(efficiency)) => {
                        #[allow(clippy::cast_possible_truncation)]
                        let efficiency = *efficiency as f32;
                        efficiency
                    }
                    Some(Value::Int(efficiency)) => {
                        #[allow(clippy::cast_precision_loss, clippy::cast_possible_truncation)]
                        let efficiency = *efficiency as f32;
                        efficiency
                    }
                    other => panic!("efficiency: {other:?}"),
                },
                move_cost: match object.get("move_cost") {
                    Some(Value::Int(cost)) => i32::try_from(*cost).expect("a small number"),
                    other => panic!("move_cost: {other:?}"),
                },
                menu_text: text("menu_text"),
                targets: names("targets"),
                can_extend: names("can_extend"),
            },
        );
    }
    out
}

#[test]
fn every_cable_is_read_the_way_the_reference_reads_it() {
    let recorded = dump();
    assert_eq!(recorded.len(), 108, "every cable was recorded");
    let reference = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../references/Cataclysm-DDA/data");
    let dirs = [reference.join("core"), reference.join("json")];
    let paths: Vec<&std::path::Path> = dirs.iter().map(std::path::PathBuf::as_path).collect();
    let loaded: BTreeMap<String, ItemType> = load_paths(&paths)
        .expect("the items load")
        .into_iter()
        .map(|item| (item.id.clone(), item))
        .collect();

    // What the corpus exercises beyond the defaults.
    // Bit comparison on purpose: the question is "did the actor's own default survive", and the default is the exact float the reference initialises the member with.
    // The equality here is against the same `f32`, so it is not a tolerance question at all.
    #[allow(clippy::float_cmp)]
    let is_default = |efficiency: f32| efficiency == 0.85;
    let default_efficiency = recorded
        .values()
        .filter(|cable| is_default(cable.efficiency))
        .count();
    let stated_efficiency = recorded.len() - default_efficiency;
    let extended = recorded
        .values()
        .filter(|cable| !cable.can_extend.is_empty())
        .count();
    let wide = recorded
        .values()
        .filter(|cable| cable.targets.len() > 2)
        .count();
    let powered = recorded
        .values()
        .filter(|cable| cable.charge_rate_watts > 0)
        .count();

    let mut differing: Vec<String> = Vec::new();
    for (id, expected) in &recorded {
        let Some(item) = loaded.get(id) else {
            differing.push(format!("{id} is not loaded"));
            continue;
        };
        let found: Vec<&LinkUp> = item
            .use_actions
            .values()
            .filter_map(|action| match action {
                UseAction::LinkUp(cable) => Some(cable),
                _ => None,
            })
            .collect();
        if found.first().is_none_or(|cable| *cable != expected) {
            differing.push(format!(
                "{id}: the port says {found:?}, the reference says {expected:?}"
            ));
        }
    }
    assert!(
        default_efficiency > 90,
        "the default efficiency has to be exercised: {default_efficiency}"
    );
    assert!(
        stated_efficiency > 0,
        "and so does a cable that states its own"
    );
    assert!(extended > 0, "and the chainable ones: {extended}");
    assert!(
        wide > 0,
        "and a cable with more than the default two targets: {wide}"
    );
    assert!(powered > 90, "and a real charge rate: {powered}");
    assert!(
        differing.is_empty(),
        "{} of {} cables disagree; the first ten are {:#?}",
        differing.len(),
        recorded.len(),
        differing.get(..10).unwrap_or(&differing)
    );
}

#[test]
fn a_power_string_is_read_in_its_own_units() {
    // The actor's `charge_rate` is written `"1 kW"`, not `1000`, so the reader has to apply the suffix — and `"35 W"` has to stay 35.
    assert_eq!(cdda_content::use_actions::parse_watts("1 kW"), 1_000);
    assert_eq!(cdda_content::use_actions::parse_watts("35 W"), 35);
    assert_eq!(cdda_content::use_actions::parse_watts("2 MW"), 2_000_000);
    assert_eq!(cdda_content::use_actions::parse_watts("0.5 kW"), 500);
    assert_eq!(cdda_content::use_actions::parse_watts("nonsense"), 0);
}
