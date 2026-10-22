//! Replaying an environment-only session recorded from the reference.
//!
//! `corpus/fields/fire-floor.json` is a five-turn trace of a single fire on `t_floor`, written by the oracle's `--dump-field-series`. This test
//! replays its **arithmetic**: for each turn it feeds the port the fuel the reference's own trace implies and checks that the port's fire ends
//! where the reference's did - the same intensity and the same age, to the turn.
//!
//! What this test does **not** claim: that the port rolls the same dice. The reference has consumed an unknown number of rolls during world
//! generation before a session begins, so a port that starts a fresh generator cannot be at the same place in the stream. The fuel decision -
//! one in three on hard ground - is therefore an **input** here, and it is pinned separately by `fire_rules::`. Stating that in the test is the
//! difference between a parity claim and a claim about parity.

#![allow(clippy::panic, clippy::expect_used, clippy::indexing_slicing)]

use bevy_ecs::prelude::*;
use cdda_core_types::field::FieldEntry;
use cdda_core_types::fire::Neighbour;
use cdda_core_types::rng::{Recording, Rolls};
use cdda_json::{Value, parse};
use cdda_sim::{FIRE, Field, FireMap, Position, process_fires};

/// One row of the trace: what the reference had after a turn.
#[derive(Debug)]
struct Row {
    turn: i64,
    intensity: i32,
    age: i64,
}

fn trace() -> Vec<Row> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../corpus/fields/fire-floor.json");
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("the fixture at {}: {error}", path.display()));
    let Value::Object(root) = parse(text.as_bytes()).expect("valid json") else {
        panic!("the fixture is an object");
    };
    let Some(Value::Array(turns)) = root.get("turns") else {
        panic!("the fixture has turns");
    };
    let mut rows = Vec::new();
    for turn in turns {
        let Value::Object(turn) = turn else {
            panic!("a turn is an object")
        };
        let number = |key: &str| match turn.get(key) {
            Some(Value::Int(value)) => *value,
            other => panic!("turn field {key} is a number, got {other:?}"),
        };
        let Some(Value::Array(fields)) = turn.get("fields") else {
            panic!("a turn has fields")
        };
        for field in fields {
            let Value::Object(field) = field else {
                panic!("a field is an object")
            };
            let kind = match field.get("type") {
                Some(Value::Str(kind)) => kind.as_str(),
                other => panic!("a field has a type, got {other:?}"),
            };
            if kind != "fd_fire" {
                continue;
            }
            let get = |key: &str| match field.get(key) {
                Some(Value::Int(value)) => i32::try_from(*value).expect("fits"),
                other => panic!("field member {key} is a number, got {other:?}"),
            };
            rows.push(Row {
                turn: number("turn"),
                intensity: get("intensity"),
                age: i64::from(get("age")),
            });
        }
    }
    rows
}

/// A map that answers for `t_floor`: hard ground, no fires beside it, smoke suppressed so the only dice are the fuel's and hot air's.
struct Floor;

impl FireMap for Floor {
    fn adjacent_fires(&self, _square: (i32, i32, i32)) -> i32 {
        0
    }
    fn can_burn(&self, _square: (i32, i32, i32)) -> bool {
        true
    }
    fn ground(&self, _square: (i32, i32, i32)) -> cdda_core_types::fire::Ground {
        // `t_floor` is `FLAMMABLE_HARD` (`corpus/fields/README.md`), so the ground asks `one_in( 3 )` every turn and the recording answers: three dice of
        // "no" and one of "yes". Which turns those are is not this double's business - the dice decide, which is the point.
        cdda_core_types::fire::Ground::FlammableHard
    }
    fn can_spread(&self, _square: (i32, i32, i32)) -> bool {
        false
    }
    fn neighbour(&self, _square: (i32, i32, i32)) -> Neighbour {
        Neighbour::default()
    }
    fn suppress_smoke(&self, _square: (i32, i32, i32)) -> bool {
        true
    }
    fn smoke_produced(&self, _square: (i32, i32, i32)) -> i32 {
        0
    }
}

#[test]
fn the_port_reproduces_the_reference_turn_for_turn() {
    // The trace: turn 0 age 0, turn 1 age 1, turns 2 and 3 age 22 and 43, turn 4 age 42 - the one turn the ground fed the fire. Turn 5 has a
    // second fire (the spread), which this test does not assert: a spread needs the reference's dice, and this test is about arithmetic.
    let rows = trace();
    let arithmetic: Vec<&Row> = rows.iter().filter(|row| row.turn <= 4).collect();
    assert_eq!(arithmetic.len(), 5, "five turns of one fire: {rows:?}");

    let mut world = World::new();
    let mut entry = FieldEntry::new(3, 1_000_000, false, true);
    let first = arithmetic[0];
    entry.set_intensity(first.intensity);
    entry.set_age(first.age);
    let entity = world
        .spawn((
            Field {
                id: FIRE,
                entry,
                priority: 0,
            },
            Position { x: 0, y: 0, z: 0 },
        ))
        .id();

    // The reference's own dice for this session: three dry turns and one that fed, the order it rolled them (`corpus/fields/fire-floor.rolls.txt`, sliced
    // down to the fuel phases). Replaying them is what makes this a parity test rather than an arithmetic one - with invented dice the age would depend on
    // which die I had drawn.
    // The decisions are the reference's - three dry turns and the fourth fed, then the bash die it asks afterwards - and each turn's hot-air gate is added
    // because the port asks it too (the map has no fires beside it, so the gate is `rng( 0, 4 )`). Supplying only the fuel dice would desynchronise the
    // stream at the first hot-air question, which is exactly the failure this arrangement makes visible.
    let mut fuel = Rolls::replaying(
        Recording::parse(
            "int 0 2 1\nint 0 4 0\nint 0 2 1\nint 0 4 0\nint 0 2 0\nint 0 99 11\nint 0 4 0\nint 0 2 1\nint 0 4 0",
        )
        .expect("the fuel and hot air dice"),
    );
    for row in arithmetic.iter().skip(1) {
        process_fires(&mut world, &Floor, 0, &mut fuel);
        // The sweep's order, which `process_fires` deliberately does not duplicate: processors first, then the field's own decay.
        {
            let mut field = world
                .get_mut::<Field>(entity)
                .expect("the fire is still there");
            field.entry.do_decay(row.turn, &mut Rolls::new(7));
        }
        let field = world.get::<Field>(entity).expect("the fire is still there");
        assert_eq!(
            field.entry.intensity(),
            row.intensity,
            "turn {} intensity",
            row.turn
        );
        assert_eq!(field.entry.age(), row.age, "turn {} age", row.turn);
    }
}

#[test]
fn the_trace_really_does_have_a_turn_where_the_age_goes_down() {
    // The property that makes this fixture worth keeping: if a future edit "fixes" the port so ages only ever rise, this test fails and says why.
    let rows = trace();
    let mut ages: Vec<(i64, i64)> = rows
        .iter()
        .filter(|row| row.turn <= 4)
        .map(|row| (row.turn, row.age))
        .collect();
    ages.sort_unstable();
    let fell = ages.windows(2).any(|pair| pair[1].1 < pair[0].1);
    assert!(
        fell,
        "the reference's own trace has one downward step: {ages:?}"
    );
}
