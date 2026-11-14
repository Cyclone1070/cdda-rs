//! What an effect does to a character while it lasts, table and arithmetic together.
//!
//! `corpus/effect-mods.json` is `--dump-effect-mods`: for the 180 effect types that have any, every cell of the per-intensity modifier table — effect name, action,
//! reduction level, and the base and scaling the two JSON members state — and the value the reference's arithmetic answers at intensities one to four.
//!
//! The table is `effect_type::load_mod_data` plus `extract_effect` (`src/effect.cpp:287-560`) and the arithmetic is `get_mod_value` (`src/effect.cpp:563-582`). They are
//! recorded together because either alone is easy to get plausibly wrong: a table with the wrong key layout still holds the right numbers, and the right formula over
//! the wrong cells still looks like a formula.

#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    // The data writes whole numbers and the recording spells them `%g`; reading them as `f64` is the point of the comparison.
    clippy::cast_precision_loss
)]

use cdda_content::effects::{ModAction, ModValues};
use cdda_json::Value;
use std::collections::BTreeMap;

fn action(value: i64) -> ModAction {
    match value {
        0 => ModAction::Amount,
        1 => ModAction::ChanceBot,
        2 => ModAction::ChanceTop,
        3 => ModAction::Max,
        4 => ModAction::MaxVal,
        5 => ModAction::Min,
        6 => ModAction::MinVal,
        7 => ModAction::Tick,
        other => panic!("action {other}"),
    }
}

fn number(object: &cdda_json::Object, key: &str) -> f64 {
    match object.get(key) {
        Some(Value::Float(value)) => *value,
        Some(Value::Int(value)) => *value as f64,
        other => panic!("{key}: {other:?}"),
    }
}

fn integer(object: &cdda_json::Object, key: &str) -> i64 {
    match object.get(key) {
        Some(Value::Int(value)) => *value,
        other => panic!("{key}: {other:?}"),
    }
}

fn text(object: &cdda_json::Object, key: &str) -> String {
    match object.get(key) {
        Some(Value::Str(value)) => value.clone(),
        other => panic!("{key}: {other:?}"),
    }
}

#[test]
fn every_modifier_cell_and_value_matches_the_reference() {
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/effect-mods.json");
    let bytes =
        std::fs::read(&path).unwrap_or_else(|_| panic!("the recording at {}", path.display()));
    let value = cdda_json::parse(&bytes).expect("valid json");
    let Value::Object(root) = value else {
        panic!("the dump is an object")
    };
    let Some(Value::Array(types)) = root.get("types") else {
        panic!("types")
    };
    assert_eq!(types.len(), 180, "every effect type with a modifier table");
    assert_eq!(integer(&root, "count"), 1_081, "every recorded cell");

    let data = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../references/Cataclysm-DDA/data");
    let dirs = [data.join("core"), data.join("json")];
    let paths: Vec<&std::path::Path> = dirs.iter().map(std::path::PathBuf::as_path).collect();
    let effects = cdda_content::effects::load_paths(&paths).expect("the effects load");
    assert_eq!(effects.len(), 451, "every effect type the core data states");

    let mut differing: Vec<String> = Vec::new();
    let mut cells = 0;
    let mut scaled = 0;
    let mut levels = std::collections::BTreeSet::new();
    let mut actions = std::collections::BTreeSet::new();
    for recorded in types {
        let Value::Object(recorded) = recorded else {
            panic!("a type")
        };
        let id = text(recorded, "id");
        let Some(ours) = effects.get(&id) else {
            differing.push(format!("{id}: the port does not load it"));
            continue;
        };
        let Some(Value::Array(mods)) = recorded.get("mods") else {
            panic!("mods")
        };
        // The port must hold **exactly** these cells: an extra one would be a key the loader invented, and a missing one a key it dropped.
        assert_eq!(
            ours.mods.values().map(BTreeMap::len).sum::<usize>(),
            mods.len(),
            "{id}: the same number of cells"
        );
        for cell in mods {
            let Value::Object(cell) = cell else {
                panic!("a cell")
            };
            let argument = text(cell, "arg");
            let action = action(integer(cell, "action"));
            let level = u8::try_from(integer(cell, "level")).expect("a level");
            let base = number(cell, "base");
            let scaling = number(cell, "scaling");
            cells += 1;
            levels.insert(level);
            actions.insert(action);
            if scaling != 0.0 {
                scaled += 1;
            }
            let reference = ModValues { base, scaling };
            let ours_cell = ours
                .mods
                .get(&argument)
                .and_then(|group| group.get(&(action, level)));
            if ours_cell != Some(&reference) {
                differing.push(format!(
                    "{id}.{argument}.{action:?}@{level}: the port says {ours_cell:?}, the reference says {reference:?}"
                ));
                continue;
            }
            let Some(Value::Array(values)) = cell.get("values") else {
                panic!("values")
            };
            for (index, recorded_value) in values.iter().enumerate() {
                // `%g` writes a whole number without a point, so a zero comes back as an integer.
                let recorded_value = match recorded_value {
                    Value::Float(value) => *value,
                    Value::Int(value) => *value as f64,
                    other => panic!("a value: {other:?}"),
                };
                let intensity = i32::try_from(index).expect("an intensity") + 1;
                let ours_value = ours.mod_value(&argument, action, level, intensity);
                if (ours_value - recorded_value).abs() > 1e-9 {
                    differing.push(format!(
                        "{id}.{argument}.{action:?}@{level} at intensity {intensity}: the port says {ours_value}, the reference says {recorded_value}"
                    ));
                }
            }
        }
    }
    // **Non-vacuity**: the grid has to hold more than one level, more than one action, and cells that scale rather than stand still.
    assert!(cells == 1_081, "cells compared: {cells}");
    assert_eq!(
        levels.len(),
        2,
        "reduction levels the data actually uses: {levels:?}"
    );
    assert!(
        actions.len() >= 6,
        "actions the data actually sets: {actions:?}"
    );
    assert!(scaled > 100, "cells with a scaling term: {scaled}");
    assert!(
        differing.is_empty(),
        "{} disagreements over {cells} cells; the first twelve are {:#?}",
        differing.len(),
        differing.get(..12).unwrap_or(&differing)
    );
}
