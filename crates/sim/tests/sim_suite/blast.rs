//! What a blast does to a room.
//!
//! Source: `--dump-blast` from the instrumented reference, which blows up the centre of a sealed room three times and records every square the wavefront bashed, in order,
//! with the force it hit it with.
//!
//! The blast's own dice — one jitter roll per square it pops — are handed in as ones, so the geometry is the unjittered one. That is what makes the answer a fact about the
//! rule rather than about the state of the generator.

#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::cast_possible_truncation
)]

use cdda_content::mapdata::{MapData, load_paths};
use cdda_json::Value;
use cdda_sim::blast::{BlastGrid, blast};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

fn corpus() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/explosions/blast.json")
}

fn number(entry: &Value, key: &str) -> f64 {
    match entry {
        Value::Object(object) => match object.get(key) {
            Some(Value::Int(value)) => f64::from(i32::try_from(*value).expect("small")),
            Some(Value::Float(value)) => *value,
            other => panic!("{key} is a number: {other:?}"),
        },
        other => panic!("a case: {other:?}"),
    }
}

fn flag(entry: &Value, key: &str) -> bool {
    match entry {
        Value::Object(object) => match object.get(key) {
            Some(Value::Bool(value)) => *value,
            other => panic!("{key} is a bool: {other:?}"),
        },
        other => panic!("a case: {other:?}"),
    }
}

/// One recorded case: the room it was blown up in, and the bashes that followed.
struct Case {
    power: f32,
    factor: f32,
    fire: bool,
    /// Bashes on the levels above and below the room, which the fixture does not own and therefore declares rather than records.
    other_levels: usize,
    hits: Vec<(i32, i32, f32)>,
    room: Vec<String>,
    above: Vec<String>,
    below: Vec<String>,
}

fn cases() -> (i32, Vec<Case>) {
    let json = std::fs::read_to_string(corpus()).expect("the blast dump");
    let Value::Object(root) = cdda_json::parse(json.as_bytes()).expect("valid JSON") else {
        panic!("root")
    };
    let Some(Value::Int(half)) = root.get("grid_half") else {
        panic!("grid_half")
    };
    let half = i32::try_from(*half).expect("small");
    let Some(Value::Array(rows)) = root.get("cases") else {
        panic!("cases")
    };
    let cases = rows
        .iter()
        .map(|row| {
            let hits = match row {
                Value::Object(object) => match object.get("bashes") {
                    Some(Value::Array(hits)) => hits,
                    other => panic!("bashes: {other:?}"),
                },
                other => panic!("a case: {other:?}"),
            };
            let hits = hits
                .iter()
                .map(|hit| match hit {
                    Value::Array(parts) => {
                        // The z is recorded and is zero for every hit kept: the dump keeps the room's own level.
                        match &parts[2] {
                            Value::Int(0) => (),
                            other => panic!("a hit off the room's level: {other:?}"),
                        }
                        (
                            i32::try_from(match &parts[0] {
                                Value::Int(value) => *value,
                                other => panic!("x: {other:?}"),
                            })
                            .expect("small"),
                            i32::try_from(match &parts[1] {
                                Value::Int(value) => *value,
                                other => panic!("y: {other:?}"),
                            })
                            .expect("small"),
                            match &parts[3] {
                                Value::Float(value) => {
                                    #[allow(clippy::cast_possible_truncation)]
                                    {
                                        *value as f32
                                    }
                                }
                                other => panic!("force: {other:?}"),
                            },
                        )
                    }
                    other => panic!("a hit: {other:?}"),
                })
                .collect();
            let tiles = |key: &str| -> Vec<String> {
                match row {
                    Value::Object(object) => match object.get(key) {
                        Some(Value::Array(tiles)) => tiles
                            .iter()
                            .map(|tile| match tile {
                                Value::Str(text) => text.clone(),
                                other => panic!("a tile: {other:?}"),
                            })
                            .collect(),
                        other => panic!("{key}: {other:?}"),
                    },
                    other => panic!("a case: {other:?}"),
                }
            };
            #[allow(clippy::cast_possible_truncation)]
            Case {
                power: number(row, "power") as f32,
                factor: number(row, "factor") as f32,
                fire: flag(row, "fire"),
                other_levels: usize::try_from(
                    i32::try_from(number(row, "other_levels").round() as i64).expect("small"),
                )
                .expect("small"),
                hits,
                room: tiles("room"),
                above: tiles("above"),
                below: tiles("below"),
            }
        })
        .collect();
    (half, cases)
}

/// The room as a blast sees it: three recorded levels of "can this be walked through", and nothing else — because the fixture seals the squares above and below the room too,
/// so the wavefront cannot reach a square the dump does not describe.
struct Room {
    half: i32,
    /// Indexed by `z + 1`: the floor below, the room, the sky above.
    levels: [Vec<bool>; 3],
}

impl BlastGrid for Room {
    fn solid(&self, x: i32, y: i32, z: i32) -> bool {
        let width = self.half * 2 + 1;
        if x.abs() > self.half || y.abs() > self.half || !(-1..=1).contains(&z) {
            return true;
        }
        let index = (y + self.half) * width + (x + self.half);
        let level = usize::try_from(z + 1).expect("inside the three levels");
        self.levels[level][usize::try_from(index).expect("inside the room")]
    }

    fn in_bounds(&self, x: i32, y: i32) -> bool {
        x.abs() <= self.half && y.abs() <= self.half
    }
}

/// The three recorded levels of a case, as solidity.
fn room(case: &Case, data: &BTreeMap<String, MapData>, half: i32, width: usize) -> Room {
    let level = |tiles: &[String]| -> Vec<bool> {
        (0..width * width)
            .map(|index| {
                let id = &tiles[index];
                let prototype = data.get(id).unwrap_or_else(|| panic!("{id} is loaded"));
                prototype.move_cost == 0
            })
            .collect()
    };
    Room {
        half,
        levels: [level(&case.below), level(&case.room), level(&case.above)],
    }
}

fn data() -> BTreeMap<String, MapData> {
    let reference =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../references/Cataclysm-DDA/data");
    load_paths(&[&reference.join("core"), &reference.join("json")])
        .expect("the map data loads")
        .into_iter()
        .map(|data| (data.id.clone(), data))
        .collect()
}

/// Compares one recorded blast against the port's, and answers what went wrong and how many squares carry the other one's force.
#[allow(clippy::too_many_lines)]
fn compare(case: &Case, data: &BTreeMap<String, MapData>, half: i32) -> (Vec<String>, usize) {
    // The recorded grid is `(2 * half + 1)` squares across, row-major from the top-left corner.
    let width = usize::try_from(half * 2 + 1).expect("small");
    assert_eq!(case.room.len(), width * width, "the room is square");
    let grid = room(case, data, half, width);
    let outcome = blast(&grid, case.power, case.factor, case.fire, &mut || 1.0);
    // Only the squares inside the room are compared: a blast's reach past a wall it cannot break is real, but it is a function of terrain the fixture does not own.
    let ours: Vec<(i32, i32, f32)> = outcome
        .hits
        .iter()
        .filter(|hit| hit.x.abs() <= half && hit.y.abs() <= half)
        .map(|hit| (hit.x, hit.y, hit.force))
        .collect();
    let mut wrong = Vec::new();
    // **Three claims, and one of them is open.**
    //
    // 1. The set of squares the blast reaches is exact.
    // 2. So is the multiset of forces it hits with.
    // 3. *Which* square gets which force is not yet exact: a square is bashed once, by whichever square reaches it first, and for squares the same distance from the centre
    //    that is a tie in the reference's queue. The count of squares whose force differs is pinned per case so that it can only go down.
    let mut mine_squares: Vec<(i32, i32)> = ours.iter().map(|hit| (hit.0, hit.1)).collect();
    let mut theirs_squares: Vec<(i32, i32)> = case.hits.iter().map(|hit| (hit.0, hit.1)).collect();
    mine_squares.sort_unstable();
    theirs_squares.sort_unstable();
    if mine_squares != theirs_squares {
        let missing: Vec<(i32, i32)> = theirs_squares
            .iter()
            .filter(|square| !mine_squares.contains(square))
            .copied()
            .take(6)
            .collect();
        let extra: Vec<(i32, i32)> = mine_squares
            .iter()
            .filter(|square| !theirs_squares.contains(square))
            .copied()
            .take(6)
            .collect();
        wrong.push(format!(
            "power {} fire {}: missing {missing:?}, extra {extra:?}",
            case.power, case.fire
        ));
        return (wrong, 0);
    }
    let mut mine_forces: Vec<i32> = ours
        .iter()
        .map(|hit| (hit.2 * 1000.0).round() as i32)
        .collect();
    let mut theirs_forces: Vec<i32> = case
        .hits
        .iter()
        .map(|hit| (hit.2 * 1000.0).round() as i32)
        .collect();
    mine_forces.sort_unstable();
    theirs_forces.sort_unstable();
    // A thousandth of a point of slack: the reference accumulates the distance in `float` and truncates a `double` power, which is rounding and not a rule.
    let forces_agree = mine_forces
        .iter()
        .zip(theirs_forces.iter())
        .all(|(left, right)| (left - right).abs() <= 3);
    if !forces_agree {
        wrong.push(format!(
            "power {} fire {}: the forces themselves differ",
            case.power, case.fire
        ));
        return (wrong, 0);
    }
    let swapped = squares_with_the_other_force(&ours, &case.hits);
    if swapped != 0 {
        wrong.push(format!(
            "power {} fire {}: {swapped} squares carry the other one's force",
            case.power, case.fire
        ));
    }
    (wrong, swapped)
}

/// How many squares carry a different force than the reference gave them, matched by square.
fn squares_with_the_other_force(ours: &[(i32, i32, f32)], theirs: &[(i32, i32, f32)]) -> usize {
    let mut by_square: BTreeMap<(i32, i32), f32> = BTreeMap::new();
    for hit in theirs {
        by_square.insert((hit.0, hit.1), hit.2);
    }
    ours.iter()
        .filter(|hit| {
            by_square
                .get(&(hit.0, hit.1))
                .is_some_and(|force| (force - hit.2).abs() > 2e-3)
        })
        .count()
}

#[test]
fn a_blast_bashes_the_same_squares_with_the_same_force() {
    let (half, cases) = cases();
    assert_eq!(
        half, 11,
        "the recorded grid is the room plus its two-square margin"
    );
    assert_eq!(cases.len(), 3, "three blasts");
    let data = data();
    let mut wrong = Vec::new();
    let mut bashed_total = 0;
    let mut declared_other_levels = 0;
    let mut swaps = 0;
    for case in &cases {
        let (case_wrong, case_swaps) = compare(case, &data, half);
        wrong.extend(case_wrong);
        swaps += case_swaps;
        bashed_total += case.hits.len();
        declared_other_levels += case.other_levels;
    }
    assert!(
        wrong.is_empty(),
        "{} wrong:\n{}",
        wrong.len(),
        wrong.iter().take(8).cloned().collect::<Vec<_>>().join("\n")
    );
    assert_eq!(
        swaps, 0,
        "every square is hit with the force the reference hits it with"
    );
    assert_eq!(
        bashed_total, 812,
        "the three blasts bash this many squares in the room"
    );
    assert_eq!(
        declared_other_levels, 1262,
        "and these many above and below it, which the fixture declares rather than records"
    );
}

/// One square a recorded wavefront popped.
struct RecordedPop {
    x: i32,
    y: i32,
    z: i32,
    distance: f32,
    force: f32,
}

fn recorded_pops(case: &Value) -> Vec<RecordedPop> {
    match case {
        Value::Object(object) => match object.get("pops") {
            Some(Value::Array(pops)) => pops
                .iter()
                .map(|pop| match pop {
                    Value::Array(parts) => {
                        let int = |index: usize| match &parts[index] {
                            Value::Int(value) => i32::try_from(*value).expect("small"),
                            other => panic!("a pop field: {other:?}"),
                        };
                        let real = |index: usize| match &parts[index] {
                            Value::Float(value) => {
                                #[allow(clippy::cast_possible_truncation)]
                                {
                                    *value as f32
                                }
                            }
                            other => panic!("a pop field: {other:?}"),
                        };
                        RecordedPop {
                            x: int(0),
                            y: int(1),
                            z: int(2),
                            distance: real(3),
                            force: real(4),
                        }
                    }
                    other => panic!("a pop: {other:?}"),
                })
                .collect(),
            other => panic!("pops: {other:?}"),
        },
        other => panic!("a case: {other:?}"),
    }
}

/// The wavefront's own order, on its own: which square the queue popped, when, and how far it had travelled to reach it.
///
/// This is the gate the blast's squares-and-forces cannot be: it says whether a disagreement is a **queue** that pops the wrong square or a **bash** that hits it with the
/// wrong force. It needs the fixture's terrain only, and it is compared on the room's own level.
#[test]
fn the_wavefront_pops_the_same_squares_in_the_same_order() {
    let json = std::fs::read_to_string(corpus()).expect("the blast dump");
    let Value::Object(root) = cdda_json::parse(json.as_bytes()).expect("valid JSON") else {
        panic!("root")
    };
    let Some(Value::Array(rows)) = root.get("cases") else {
        panic!("cases")
    };
    let (half, cases) = cases();
    let data = data();
    let mut wrong = Vec::new();
    let mut popped_total = 0;
    for (row, case) in rows.iter().zip(cases.iter()) {
        let recorded = recorded_pops(row);
        let width = usize::try_from(half * 2 + 1).expect("small");
        let grid = room(case, &data, half, width);
        let outcome = blast(&grid, case.power, case.factor, case.fire, &mut || 1.0);
        // Every level the dump records, which is every level the sealed fixture lets the wavefront reach.
        let ours: Vec<(i32, i32, i32, i32, i32)> = outcome
            .pops
            .iter()
            .map(|pop| {
                (
                    pop.x,
                    pop.y,
                    pop.z,
                    (pop.distance * 1000.0).round() as i32,
                    (pop.force * 1000.0).round() as i32,
                )
            })
            .collect();
        let theirs: Vec<(i32, i32, i32, i32, i32)> = recorded
            .iter()
            .map(|pop| {
                (
                    pop.x,
                    pop.y,
                    pop.z,
                    (pop.distance * 1000.0).round() as i32,
                    (pop.force * 1000.0).round() as i32,
                )
            })
            .collect();
        popped_total += theirs.len();
        // Distances and forces to the nearest thousandth, with two thousandths of slack: the reference accumulates the distance in `float` and truncates a `double` power,
        // and two paths to the same square that add up the same distance can still round differently. The square and the order are compared exactly.
        let differs = |left: &(i32, i32, i32, i32, i32), right: &(i32, i32, i32, i32, i32)| {
            left.0 != right.0
                || left.1 != right.1
                || left.2 != right.2
                || (left.3 - right.3).abs() > 2
                || (left.4 - right.4).abs() > 2
        };
        if let Some(index) = ours
            .iter()
            .zip(theirs.iter())
            .position(|(left, right)| differs(left, right))
        {
            wrong.push(format!(
                "power {} fire {}: pop {index} is {:?} against {:?}; ours from {}: {:?}, theirs: {:?}",
                case.power,
                case.fire,
                ours.get(index),
                theirs.get(index),
                index.saturating_sub(2),
                ours[index.saturating_sub(2)..(index + 3).min(ours.len())].to_vec(),
                theirs[index.saturating_sub(2)..(index + 3).min(theirs.len())].to_vec()
            ));
        } else if ours.len() != theirs.len() {
            wrong.push(format!(
                "power {} fire {}: {} pops against {}",
                case.power,
                case.fire,
                ours.len(),
                theirs.len()
            ));
        }
    }
    assert!(popped_total > 0, "the dump records a wavefront");
    assert!(
        wrong.is_empty(),
        "{} wrong:\n{}",
        wrong.len(),
        wrong.iter().take(4).cloned().collect::<Vec<_>>().join("\n")
    );
}
