//! Every terrain's and furniture's examine action, and the data behind the four that carry any.
//!
//! `corpus/maps/examine-actions.json` is `--dump-examine-actions`: one entry per terrain or furniture prototype whose `"examine_action"` is not empty, with the actor
//! resolved. 73 of the 77 names in the content are **bare strings** — a hardcoded function with no data at all — and 4 are **objects** that carry members
//! (`src/mapdata.h:500-505`, `src/iexamine_actors.cpp`).
//!
//! Two rules are invisible from the data and only visible in the loader, which is why the gate compares a resolved actor rather than the JSON text: the card reader's
//! `consume_card`, `allow_hacking`, `despawn_monsters` and `query` are all **true** when unstated, its `radius` is **3**, and a reader that states a `mapgen_id` does
//! **not** read the radius at all.

#![allow(clippy::panic, clippy::expect_used, clippy::indexing_slicing)]

use cdda_content::examine::ExamineAction;
use cdda_content::mapdata::{MapData, MapDataKind, load_paths};
use cdda_json::Value;
use std::collections::BTreeMap;

fn recorded() -> Value {
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/maps/examine-actions.json");
    let bytes = std::fs::read(&path).unwrap_or_else(|_| panic!("the recording at {}", path.display()));
    cdda_json::parse(&bytes).expect("valid json")
}

fn loaded() -> BTreeMap<(MapDataKind, String), MapData> {
    let reference =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../references/Cataclysm-DDA/data");
    let dirs = [reference.join("core"), reference.join("json")];
    let paths: Vec<&std::path::Path> = dirs.iter().map(std::path::PathBuf::as_path).collect();
    load_paths(&paths)
        .expect("the map data loads")
        .into_iter()
        .map(|tile| ((tile.kind, tile.id.clone()), tile))
        .collect()
}

fn text(entry: &cdda_json::Object, key: &str) -> String {
    match entry.get(key) {
        Some(Value::Str(text)) => text.clone(),
        other => panic!("{key}: {other:?}"),
    }
}

#[test]
fn every_square_with_an_examine_action_has_the_one_the_reference_has() {
    let Value::Object(root) = recorded() else {
        panic!("the dump is an object")
    };
    let Some(Value::Array(tiles)) = root.get("tiles") else {
        panic!("the dump holds tiles")
    };
    assert_eq!(tiles.len(), 1769, "every terrain and furniture prototype with a function");
    let map = loaded();

    let mut differing: Vec<String> = Vec::new();
    let mut named: BTreeMap<String, usize> = BTreeMap::new();
    let mut objects = 0;
    let mut default_none = 0;
    for entry in tiles {
        let Value::Object(entry) = entry else {
            panic!("a tile")
        };
        let kind = match text(entry, "kind").as_str() {
            "ter" => MapDataKind::Terrain,
            "furn" => MapDataKind::Furniture,
            other => panic!("a kind: {other}"),
        };
        let id = text(entry, "id");
        let action = text(entry, "action");
        // **`none` is the reference's default, not a statement.** A prototype that says nothing gets `iexamine::none`, and one that writes `"examine_action": "none"`
        // gets the same function — so a dump cannot tell them apart, and neither can the port. `none` is therefore counted and skipped rather than compared, and the
        // number is pinned so a loader that stopped reading the member entirely would still fail here.
        if action == "none" {
            default_none += 1;
            continue;
        }
        let Some(tile) = map.get(&(kind, id.clone())) else {
            differing.push(format!("{id} is not loaded"));
            continue;
        };
        let Some(ours) = &tile.examine else {
            differing.push(format!("{id}: the port has no examine action, the reference says {action}"));
            continue;
        };
        // The four object actors are counted separately from the 73 names, because the two halves fail for different reasons.
        let is_object = matches!(
            ours,
            ExamineAction::ApplianceConvert { .. }
                | ExamineAction::CardReader(_)
                | ExamineAction::EffectOnCondition { .. }
                | ExamineAction::Mortar
        );
        if is_object {
            objects += 1;
        } else {
            *named.entry(action.clone()).or_default() += 1;
        }
        if ours.kind() != action {
            differing.push(format!("{id}: the port says {:?}, the reference says {action:?}", ours.kind()));
        }
    }
    // **Non-vacuity, on both halves.** A loader that only kept names would pass a corpus of names, and one that only kept actors would pass on four tiles.
    assert_eq!(default_none, 1208, "the tiles that fall back to `iexamine::none`");
    assert!(named.len() > 60, "most examine actions are bare names: {}", named.len());
    assert!(objects > 70, "and the four object actors cover this many tiles: {objects}");
    assert!(
        differing.is_empty(),
        "{} of {} tiles disagree; the first ten are {:#?}",
        differing.len(),
        tiles.len(),
        differing.get(..10).unwrap_or(&differing)
    );
}

#[test]
fn a_card_reader_that_states_nothing_gets_the_reference_s_defaults() {
    // **The defaults have to come from the loader, not from a tile.** A corpfus of tiles that state every member would let a loader with the wrong defaults pass; the
    // reference's own are `true` for the four switches and **3** for the radius (`src/iexamine_actors.cpp:241-261`).
    let json = br#"{"examine_action": {"type": "cardreader", "flags": ["SCIENCE_CARD"],
        "success_msg": "click", "redundant_msg": "already open"}}"#;
    let value = cdda_json::parse(json).expect("the fixture parses");
    let Value::Object(object) = value else {
        panic!("the fixture is an object")
    };
    let Some(ExamineAction::CardReader(reader)) = cdda_content::examine::read(&object).expect("it reads")
    else {
        panic!("a card reader")
    };
    assert_eq!(reader.flags, ["SCIENCE_CARD"], "the flags are mandatory and kept in order");
    assert!(reader.consume_card, "consume_card defaults to true");
    assert!(reader.allow_hacking, "allow_hacking defaults to true");
    assert!(reader.despawn_monsters, "despawn_monsters defaults to true");
    assert!(reader.query, "query defaults to true");
    assert_eq!(reader.radius, 3, "radius defaults to 3");
    assert!(reader.mapgen_id.is_empty(), "and no mapgen is claimed");
}

#[test]
fn a_card_reader_with_a_mapgen_does_not_read_the_radius() {
    // **The two paths are exclusive in the loader**, not merely in use: a reader that states a `mapgen_id` never reads `radius` or the change maps
    // (`src/iexamine_actors.cpp:248-255`).
    let json = br#"{"examine_action": {"type": "cardreader", "flags": ["SCIENCE_CARD"],
        "success_msg": "click", "redundant_msg": "already open",
        "mapgen_id": "lab_cardreader", "radius": 9, "terrain_changes": {"t_door_locked": "t_door_c"}}}"#;
    let value = cdda_json::parse(json).expect("the fixture parses");
    let Value::Object(object) = value else {
        panic!("the fixture is an object")
    };
    let Some(ExamineAction::CardReader(reader)) = cdda_content::examine::read(&object).expect("it reads")
    else {
        panic!("a card reader")
    };
    assert_eq!(reader.mapgen_id, "lab_cardreader");
    // The port reads the members as the data states them and leaves the choice to the caller, which is what the reference's own loader order amounts to; what this
    // test pins is that **both** are visible here, so a caller cannot silently use a radius the reference would never have read.
    assert_eq!(reader.radius, 9);
    assert_eq!(reader.terrain_changes.len(), 1);
}

/// A stated `examine_action` **replaces** the inherited one, and a child that states nothing keeps it.
///
/// The corpus cannot see this: no prototype writes `"examine_action"` over a parent that has one (the count is **zero**), and `"none"` is the reference's default rather
/// than a statement, so a dump of resolved actions cannot tell "inherited" from "stated". The rule comes from `optional`'s own shape — a member that is present
/// **overwrites** what `copy-from` supplied (`src/generic_factory.h:1168-1192`, and the same rule the port learned on `use_action` in round 629) — and the fixture is
/// the only way to hold the port to it.
#[test]
fn a_stated_examine_action_replaces_the_inherited_one() {
    use cdda_content::mapdata;

    let dir = std::env::temp_dir().join("cdda-examine-inheritance");
    std::fs::create_dir_all(&dir).expect("temp dir");
    std::fs::write(
        dir.join("mapdata.json"),
        r#"[
        { "type": "terrain", "id": "t_test_window", "name": "window", "symbol": "'", "color": "white",
          "move_cost": 0, "examine_action": "curtains", "flags": [ "TRANSPARENT" ] },
        { "type": "terrain", "id": "t_test_blank_window", "name": "window", "copy-from": "t_test_window",
          "examine_action": "none" },
        { "type": "terrain", "id": "t_test_plain_window", "name": "window", "copy-from": "t_test_window" }
        ]"#,
    )
    .expect("fixture");
    let loaded = mapdata::load_paths(&[&dir]).expect("fixture loads");
    let by_id: BTreeMap<&str, &MapData> =
        loaded.iter().map(|tile| (tile.id.as_str(), tile)).collect();
    let examine_of = |id: &str| {
        by_id
            .get(id)
            .unwrap_or_else(|| panic!("{id} is loaded"))
            .examine
            .clone()
    };
    assert_eq!(
        examine_of("t_test_window"),
        Some(ExamineAction::Named("curtains".to_owned())),
        "the parent has curtains"
    );
    assert_eq!(
        examine_of("t_test_blank_window"),
        Some(ExamineAction::Named("none".to_owned())),
        "a child that states its own replaces the parent's rather than keeping it"
    );
    assert_eq!(
        examine_of("t_test_plain_window"),
        Some(ExamineAction::Named("curtains".to_owned())),
        "and a child that states nothing keeps it"
    );
}
