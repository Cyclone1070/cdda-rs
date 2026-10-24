//! The zone manager's answers, square by square.
//!
//! Source: `--dump-zones-probe` from the instrumented reference (`harness.cpp`), which lays down a fixed set of zones and then asks the manager four different questions about
//! every square of a grid.
//!
//! **Order is the point.** The reference holds its zones in one list and reaches into it from both ends: `get_zone_at` walks it forwards and answers with the first zone that
//! contains the square, while `get_bottom_zone` walks it backwards and answers with the last one drawn. A dump that recorded "some zone contains this square" would pass
//! with the list reversed, so the fixture overlaps on purpose and the gate compares the *names*.

#![allow(clippy::expect_used, clippy::panic, clippy::indexing_slicing)]

use cdda_content::zones::{ZoneData, ZoneManager};
use cdda_json::Value;
use std::path::{Path, PathBuf};

fn corpus() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/zones/probe.json")
}

fn array<'a>(value: &'a Value, key: &str) -> &'a [Value] {
    match value {
        Value::Object(root) => match root.get(key) {
            Some(Value::Array(list)) => list,
            other => panic!("{key} is an array: {other:?}"),
        },
        other => panic!("root is an object: {other:?}"),
    }
}

fn text(entry: &Value, key: &str) -> String {
    match entry {
        Value::Object(object) => match object.get(key) {
            Some(Value::Str(value)) => value.clone(),
            other => panic!("{key} is a string: {other:?}"),
        },
        other => panic!("an entry is an object: {other:?}"),
    }
}

fn flag(entry: &Value, key: &str) -> bool {
    match entry {
        Value::Object(object) => match object.get(key) {
            Some(Value::Bool(value)) => *value,
            other => panic!("{key} is a bool: {other:?}"),
        },
        other => panic!("an entry is an object: {other:?}"),
    }
}

fn number(entry: &Value, key: &str) -> i32 {
    match entry {
        Value::Object(object) => match object.get(key) {
            Some(Value::Int(value)) => i32::try_from(*value).expect("small"),
            other => panic!("{key} is a number: {other:?}"),
        },
        other => panic!("an entry is an object: {other:?}"),
    }
}

fn triple(entry: &Value, key: &str) -> (i32, i32, i32) {
    let list = array(entry, key);
    let at = |index: usize| match list.get(index) {
        Some(Value::Int(value)) => i32::try_from(*value).expect("small"),
        other => panic!("a coordinate: {other:?}"),
    };
    (at(0), at(1), at(2))
}

fn zone(entry: &Value) -> ZoneData {
    ZoneData {
        name: text(entry, "name"),
        zone_type: text(entry, "type"),
        faction: text(entry, "faction"),
        invert: flag(entry, "invert"),
        enabled: flag(entry, "enabled"),
        start: triple(entry, "start"),
        end: triple(entry, "end"),
    }
}

fn recorded() -> (Vec<ZoneData>, Vec<ZoneData>) {
    let json = std::fs::read_to_string(corpus()).expect("the zone probe dump");
    let root = cdda_json::parse(json.as_bytes()).expect("valid JSON");
    let owned = array(&root, "zones").iter().map(zone).collect();
    let others = array(&root, "others").iter().map(zone).collect();
    (owned, others)
}

/// The fixture is only worth gating if it asks the questions the answers depend on.
#[test]
fn the_fixture_overlaps_so_order_matters() {
    let (owned, others) = recorded();
    let manager = {
        let mut manager = ZoneManager::new();
        for zone in &owned {
            manager.add(zone.clone());
        }
        for zone in &others {
            manager.add(zone.clone());
        }
        manager
    };
    let json = std::fs::read_to_string(corpus()).expect("the dump");
    let root = cdda_json::parse(json.as_bytes()).expect("valid JSON");
    let probes = array(&root, "probes");
    assert_eq!(probes.len(), 240, "a grid of 16 by 15 squares");
    let mut first_differs_from_last = 0;
    let mut loot_differs_from_bottom = 0;
    for probe in probes {
        let where_ = (number(probe, "x"), number(probe, "y"), 0);
        let food = manager.zones_at(where_, "LOOT_FOOD", "your_followers");
        if food.len() > 1 {
            first_differs_from_last += 1;
        }
        let bottom = manager.bottom_zone(where_, "your_followers").map(|zone| &zone.name);
        let loot = manager.loot_zone_at(where_, "your_followers").map(|zone| &zone.name);
        if bottom != loot {
            loot_differs_from_bottom += 1;
        }
    }
    assert!(
        first_differs_from_last >= 10,
        "some squares are in more than one food zone: {first_differs_from_last}"
    );
    assert!(
        loot_differs_from_bottom >= 5,
        "the loot-only question has answers the bottom question does not: {loot_differs_from_bottom}"
    );
}

/// Every answer the reference gives about every square.
#[test]
fn every_square_gets_the_manager_s_answer() {
    let (owned, others) = recorded();
    let mut manager = ZoneManager::new();
    for zone in &owned {
        manager.add(zone.clone());
    }
    for zone in &others {
        manager.add(zone.clone());
    }
    let json = std::fs::read_to_string(corpus()).expect("the dump");
    let root = cdda_json::parse(json.as_bytes()).expect("valid JSON");
    let mut wrong = Vec::new();
    for probe in array(&root, "probes") {
        let where_ = (number(probe, "x"), number(probe, "y"), 0);
        let inside: Vec<usize> = match probe {
            Value::Object(object) => match object.get("inside") {
                Some(Value::Array(list)) => list
                    .iter()
                    .map(|value| match value {
                        Value::Int(index) => usize::try_from(*index).expect("small"),
                        other => panic!("an index: {other:?}"),
                    })
                    .collect(),
                other => panic!("inside is an array: {other:?}"),
            },
            other => panic!("a probe: {other:?}"),
        };
        let expected_count = usize::try_from(number(probe, "food_count")).expect("small");
        let expected_first = text(probe, "food_first");
        let expected_bottom = text(probe, "bottom");
        let expected_loot = text(probe, "loot_first");
        let expected_other = usize::try_from(number(probe, "other_inside")).expect("small");

        let ported_inside: Vec<usize> = manager
            .zones()
            .iter()
            .enumerate()
            .filter(|(_, zone)| zone.faction == "your_followers" && zone.has_inside(where_))
            .map(|(index, _)| index)
            .collect();
        let food = manager.zones_at(where_, "LOOT_FOOD", "your_followers");
        let first = manager.zone_at(where_, "LOOT_FOOD", "your_followers");
        let bottom = manager.bottom_zone(where_, "your_followers");
        let loot = manager.loot_zone_at(where_, "your_followers");
        let other = manager.zones_at(where_, "LOOT_FOOD", "no_faction");

        let name = |zone: Option<&ZoneData>| zone.map_or_else(String::new, |zone| zone.name.clone());
        if ported_inside != inside {
            wrong.push(format!("({},{}): inside {ported_inside:?} against {inside:?}", where_.0, where_.1));
        }
        if food.len() != expected_count {
            wrong.push(format!("({},{}): {} food zones against {expected_count}", where_.0, where_.1, food.len()));
        }
        if name(first) != expected_first {
            wrong.push(format!("({},{}): first is {} against {expected_first}", where_.0, where_.1, name(first)));
        }
        if name(bottom) != expected_bottom {
            wrong.push(format!("({},{}): bottom is {} against {expected_bottom}", where_.0, where_.1, name(bottom)));
        }
        if name(loot) != expected_loot {
            wrong.push(format!("({},{}): loot is {} against {expected_loot}", where_.0, where_.1, name(loot)));
        }
        if other.len() != expected_other {
            wrong.push(format!("({},{}): {} other-faction zones against {expected_other}", where_.0, where_.1, other.len()));
        }
    }
    assert!(wrong.is_empty(), "{} wrong of 1,440 answers:\n{}", wrong.len(), wrong.join("\n"));
}
