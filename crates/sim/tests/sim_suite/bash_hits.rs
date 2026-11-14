//! What happens when a square is bashed.
//!
//! Source: `--dump-bash` from the instrumented reference (`harness.cpp`), which bashes a fixed set of squares with a fixed set of hits and records the square, the damage it
//! kept and whether anything broke.
//!
//! Two things make this gate worth having over a transcription. First, the **damage** a failed hit leaves behind is state the square carries between hits, so the same hit
//! three times is not three independent answers. Second, `success` is not "something broke": a `destroy` hit breaks the square and reports `success` as **false**, because
//! the reference only sets it from the strength check it skips under `destroy`.

#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation
)]

use cdda_content::mapdata::{MapData, load_paths};
use cdda_json::Value;
use cdda_sim::bash::{BashParams, BashTarget, bash};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

fn corpus() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/bash/hits.json")
}

fn text(entry: &Value, key: &str) -> String {
    match entry {
        Value::Object(object) => match object.get(key) {
            Some(Value::Str(value)) => value.clone(),
            other => panic!("{key} is a string: {other:?}"),
        },
        other => panic!("a hit: {other:?}"),
    }
}

fn flag(entry: &Value, key: &str) -> bool {
    match entry {
        Value::Object(object) => match object.get(key) {
            Some(Value::Bool(value)) => *value,
            other => panic!("{key} is a bool: {other:?}"),
        },
        other => panic!("a hit: {other:?}"),
    }
}

fn number(entry: &Value, key: &str) -> i32 {
    match entry {
        Value::Object(object) => match object.get(key) {
            Some(Value::Int(value)) => i32::try_from(*value).expect("small"),
            other => panic!("{key} is a number: {other:?}"),
        },
        other => panic!("a hit: {other:?}"),
    }
}

fn real(entry: &Value, key: &str) -> f32 {
    match entry {
        Value::Object(object) => match object.get(key) {
            Some(Value::Float(value)) => {
                #[allow(clippy::cast_possible_truncation)]
                {
                    *value as f32
                }
            }
            Some(Value::Int(value)) => f64::from(i32::try_from(*value).expect("small")) as f32,
            other => panic!("{key} is a number: {other:?}"),
        },
        other => panic!("a hit: {other:?}"),
    }
}

/// One recorded hit. The bools are the dump's own columns, kept as they are recorded.
#[allow(clippy::struct_excessive_bools)]
struct Hit {
    terrain: String,
    furniture: String,
    below: String,
    below_before: String,
    strength: i32,
    roll: f32,
    destroy: bool,
    bash_floor: bool,
    step: i32,
    terrain_after: String,
    furniture_after: String,
    damage: i32,
    did_bash: bool,
    success: bool,
    bashed_solid: bool,
    x: i32,
    y: i32,
}

fn hits() -> Vec<Hit> {
    let json = std::fs::read_to_string(corpus()).expect("the bash dump");
    let Value::Object(root) = cdda_json::parse(json.as_bytes()).expect("valid JSON") else {
        panic!("root")
    };
    let Some(Value::Array(rows)) = root.get("hits") else {
        panic!("hits")
    };
    rows.iter()
        .map(|row| Hit {
            terrain: text(row, "ter"),
            furniture: text(row, "furn"),
            below: text(row, "below"),
            below_before: text(row, "below_before"),
            strength: number(row, "str"),
            roll: real(row, "roll"),
            destroy: flag(row, "destroy"),
            bash_floor: flag(row, "bash_floor"),
            step: number(row, "step"),
            terrain_after: text(row, "ter_after"),
            furniture_after: text(row, "furn_after"),
            damage: number(row, "damage"),
            did_bash: flag(row, "did_bash"),
            success: flag(row, "success"),
            bashed_solid: flag(row, "bashed_solid"),
            x: number(row, "x"),
            y: number(row, "y"),
        })
        .collect()
}

/// The reference checkout's data, which is the copy the remote test machine has.
fn data() -> BTreeMap<String, MapData> {
    let reference =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../references/Cataclysm-DDA/data");
    let dirs = [reference.join("core"), reference.join("json")];
    let args: Vec<&Path> = dirs.iter().map(PathBuf::as_path).collect();
    load_paths(&args)
        .expect("the map data loads")
        .into_iter()
        .map(|data| (data.id.clone(), data))
        .collect()
}

#[test]
fn every_recorded_hit_lands_where_the_reference_lands_it() {
    let hits = hits();
    assert_eq!(hits.len(), 97, "the dump holds 97 hits");
    let data = data();
    assert!(
        data.contains_key("t_floor") && data.contains_key("f_table"),
        "the fixture's own prototypes are loaded: {} types, t_floor {}, f_table {}",
        data.len(),
        data.contains_key("t_floor"),
        data.contains_key("f_table")
    );
    let mut wrong = Vec::new();
    let mut open_air = 0;
    // The hits are grouped by square and strength: the steps of one group share the damage the square has kept, so the test threads it forward instead of resetting it.
    let mut damage = 0;
    let mut previous: Option<(String, String, i32, f32, bool, bool)> = None;
    for hit in &hits {
        let key = (
            hit.terrain.clone(),
            hit.furniture.clone(),
            hit.strength,
            hit.roll,
            hit.destroy,
            hit.bash_floor,
        );
        if previous.as_ref() != Some(&key) {
            damage = 0;
            previous = Some(key);
        }
        let target = BashTarget {
            terrain: hit.terrain.clone(),
            furniture: hit.furniture.clone(),
            below: hit.below.clone(),
            damage,
        };
        let params = BashParams {
            strength: hit.strength,
            silent: true,
            destroy: hit.destroy,
            bash_floor: hit.bash_floor,
            roll: hit.roll,
            bashing_from_above: false,
        };
        let outcome = bash(&target, params, &data);
        damage = outcome.damage;
        // Counted, not skipped: rows that end as a roof are the ones that take the open-air path (`src/map.cpp:4229-4236`) and then get a roof back.
        if hit.terrain_after == "t_open_air" || hit.terrain_after.ends_with("_roof") {
            open_air += 1;
        }
        let where_ = format!(
            "{}/{} at {} (roll {}) step {}",
            hit.terrain, hit.furniture, hit.strength, hit.roll, hit.step
        );
        if outcome.terrain != hit.terrain_after {
            wrong.push(format!(
                "{where_}: terrain {} against {}",
                outcome.terrain, hit.terrain_after
            ));
        }
        if outcome.furniture != hit.furniture_after {
            wrong.push(format!(
                "{where_}: furniture {} against {}",
                outcome.furniture, hit.furniture_after
            ));
        }
        if outcome.damage != hit.damage {
            wrong.push(format!(
                "{where_}: damage {} against {}",
                outcome.damage, hit.damage
            ));
        }
        if outcome.did_bash != hit.did_bash {
            wrong.push(format!(
                "{where_}: did_bash {} against {}",
                outcome.did_bash, hit.did_bash
            ));
        }
        if outcome.success != hit.success {
            wrong.push(format!(
                "{where_}: success {} against {}",
                outcome.success, hit.success
            ));
        }
        if outcome.bashed_solid != hit.bashed_solid {
            wrong.push(format!(
                "{where_}: bashed_solid {} against {}",
                outcome.bashed_solid, hit.bashed_solid
            ));
        }
        assert_eq!(
            hit.below_before, "t_rock_floor",
            "every fixture pins the square below it; a dump recorded without a seed once read a mapgen roof here instead"
        );
        let _ = (hit.x, hit.y);
    }
    // Six of the 97 end as a roof: the two destroyed floors and the four terrain fixtures bashed to nothing, which the reference turns back into `t_rock_roof` from the
    // terrain below. The count is pinned so that a change in the fixtures moves it rather than quietly dropping the roof path out of the gate.
    assert_eq!(open_air, 6, "the dump exercises the roof-regeneration path");
    assert!(
        wrong.is_empty(),
        "{} wrong of 97:\n{}",
        wrong.len(),
        wrong
            .iter()
            .take(14)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n")
    );
}
