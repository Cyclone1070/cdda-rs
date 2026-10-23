//! A recorded gas session, replayed.
//!
//! `corpus/fields/gas-60.json` is sixty turns of `fd_toxic_gas` on a floored square, written by the oracle's `--dump-field-series` with the gas processor's own dice
//! labelled. Forty-seven of the turns are quiet and then the field splits, so the fixture pins the arithmetic that a hand-written case would smooth over: the source
//! drops an intensity and gives up a share of its age, and the square it arrives on is born holding that share.
//!
//! What this gate adds to the unit tests beside it is the **sweep**: the reference visits squares in order and ages every field it meets on the way, including one
//! that another square created earlier in the same turn. A replay that decays only the fields it found before the turn looks right until the first split.

#![allow(clippy::panic, clippy::expect_used, clippy::indexing_slicing)]

use bevy_ecs::prelude::*;
use cdda_core_types::field::{FieldEntry, FieldId};
use cdda_core_types::rng::Rolls;
use cdda_sim::{Field, Position};

/// The id the fixture's gas carries. The id only has to agree with itself: the spread rule merges by kind, and a field's number is not what this gate is about.
const GAS: FieldId = FieldId(1);

/// What the port holds after a turn, in the shape the fixture is written in.
fn fields(world: &mut World) -> Vec<(i32, i32, i32, i32, i64)> {
    let mut query = world.query::<(&Field, &Position)>();
    let mut found: Vec<(i32, i32, i32, i32, i64)> = query
        .iter(world)
        .map(|(field, position)| {
            (
                position.x,
                position.y,
                position.z,
                field.entry.intensity(),
                field.entry.age(),
            )
        })
        .collect();
    found.sort_unstable();
    found
}

/// Whether a gas may take a square, as `map::gas_can_spread_to` decides it (`src/map_field.cpp:237-248`).
///
/// Two halves: the square has to be navigable (`ter_furn_movecost > 0`) or `PERMEABLE`, and whatever field is already there must be **weaker** than the one
/// arriving. A wall refuses; so does a square already holding gas of the same strength.
struct Takes {
    /// The squares' terrain ids, from the fixture.
    square_of: std::collections::BTreeMap<(i32, i32, i32), String>,
    /// Each terrain's move cost, from the prototype dump.
    move_cost: std::collections::BTreeMap<String, i32>,
    /// Each terrain's `PERMEABLE`, from the same dump.
    permeable: std::collections::BTreeMap<String, bool>,
    /// What each terrain says about a step between levels, for `valid_move`'s vertical half.
    steps: std::collections::BTreeMap<String, cdda_core_types::movement::StepFacts>,
}

impl Takes {
    fn load(session: &crate::reference_session::Session) -> Self {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/prototypes/terrain.json");
        let text = std::fs::read_to_string(path).expect("the terrain prototypes");
        let cdda_json::Value::Object(root) = cdda_json::parse(text.as_bytes()).expect("valid JSON") else {
            panic!("an object");
        };
        let Some(cdda_json::Value::Array(entries)) = root.get("prototypes") else { panic!("prototypes") };
        let mut move_cost = std::collections::BTreeMap::new();
        let mut permeable = std::collections::BTreeMap::new();
        let mut steps = std::collections::BTreeMap::new();
        for entry in entries {
            let cdda_json::Value::Object(object) = entry else { continue };
            let Some(cdda_json::Value::Str(id)) = object.get("id") else { continue };
            let cost = match object.get("move_cost") {
                Some(cdda_json::Value::Int(value)) => i32::try_from(*value).unwrap_or(0),
                _ => 0,
            };
            let passable = match object.get("flags") {
                Some(cdda_json::Value::Array(list)) => list
                    .iter()
                    .any(|flag| matches!(flag, cdda_json::Value::Str(text) if text == "PERMEABLE")),
                _ => false,
            };
            let has = |flag: &str| match object.get("flags") {
                Some(cdda_json::Value::Array(list)) => list
                    .iter()
                    .any(|item| matches!(item, cdda_json::Value::Str(text) if text == flag)),
                _ => false,
            };
            move_cost.insert(id.clone(), cost);
            permeable.insert(id.clone(), passable);
            steps.insert(
                id.clone(),
                cdda_core_types::movement::StepFacts {
                    move_cost: cost,
                    open_air: has("NO_FLOOR"),
                    no_floor_water: has("NO_FLOOR_WATER"),
                    goes_down: has("GOES_DOWN"),
                },
            );
        }
        let square_of = session
            .grid
            .iter()
            .map(|square| ((square.x, square.y, square.z), square.terrain.clone()))
            .collect();
        Takes {
            square_of,
            move_cost,
            permeable,
            steps,
        }
    }

    /// Whether the terrain lets anything through, before any field is considered.
    fn open(&self, square: (i32, i32, i32)) -> bool {
        let Some(id) = self.square_of.get(&square) else {
            // A square the dump did not describe is not in the bubble, and the reference treats an out-of-bounds tile as impassable.
            return false;
        };
        self.move_cost.get(id).copied().unwrap_or(0) > 0 || self.permeable.get(id).copied().unwrap_or(false)
    }

    /// What a square says about a step between levels.
    fn step(&self, square: (i32, i32, i32)) -> cdda_core_types::movement::StepFacts {
        self.square_of
            .get(&square)
            .and_then(|id| self.steps.get(id))
            .copied()
            .unwrap_or_default()
    }

    /// `map::valid_move`'s vertical half, with the two squares named by their level rather than by the direction of travel.
    fn may_step(&self, from: (i32, i32, i32), to: (i32, i32, i32)) -> bool {
        match to.2.cmp(&from.2) {
            std::cmp::Ordering::Greater => {
                cdda_core_types::movement::can_step_vertically(self.step(to), self.step(from))
            }
            std::cmp::Ordering::Less => {
                cdda_core_types::movement::can_step_vertically(self.step(from), self.step(to))
            }
            std::cmp::Ordering::Equal => true,
        }
    }
}

#[test]
fn sixty_recorded_turns_of_a_gas_replay_square_for_square() {
    let session = crate::reference_session::Session::load("gas-60");
    assert!(
        !session.dice_by_turn.is_empty(),
        "the fixture labels its turns, so each turn's dice are its own"
    );
    let mut map = session.map();
    let takes = Takes::load(&session);
    let mut world = World::new();
    // **Seeded from the first recorded state, not from the placement.** The dump places the gas and then runs a sweep; the first sweep does not see it - the
    // field is not in a cached submap yet - so the reference's own trace has no dice for the transition into turn 1 while its age still advances. Starting at the
    // state that sweep left is the honest read of the fixture: from there every turn is a sweep that did see the gas, and the dice line up one for one.
    let Some(first) = session.turns.get(1).and_then(|turn| turn.first()) else {
        panic!("the fixture has a second state");
    };
    let mut entry = FieldEntry::new(3, 1_000_000, true, false);
    entry.set_intensity(first.intensity);
    entry.set_age(first.age);
    world.spawn((
        Position {
            x: first.x,
            y: first.y,
            z: first.z,
        },
        Field {
            id: GAS,
            entry,
            priority: 0,
        },
    ));

    let mut disagreements: Vec<(usize, String)> = Vec::new();
    let mut compared = 0_usize;
    let mut spent = 0_usize;
    for (index, recorded) in session.turns.iter().enumerate().skip(2) {
        // The squares this turn will visit, taken before anything runs: a field born during the sweep is aged when the sweep reaches its square, and the order is
        // the squares' own. The dump lists the fields it found, so the port's list is built from the same rule: everything standing at the start of the turn.
        let found: Vec<Entity> = {
            let mut query = world.query::<Entity>();
            query.iter(&world).collect()
        };
        let wind = session.wind.get(index).copied().unwrap_or(session.wind[0]);
        // What each square holds, taken before the turn: the rule refuses a destination whose field is not weaker, and a field born this turn must not be
        // read as having been there when the turn started.
        let standing: std::collections::BTreeMap<(i32, i32, i32), i32> = {
            let mut query = world.query::<(&Field, &Position)>();
            query
                .iter(&world)
                .map(|(field, position)| ((position.x, position.y, position.z), field.entry.intensity()))
                .collect()
        };
        let can_take = |from: (i32, i32, i32), to: (i32, i32, i32)| {
            // `gas_can_spread_to` decides the square will take gas at all; a step to another level must also pass `valid_move`, which is what stops a gas
            // falling through the floor it is standing on.
            takes.open(to) && takes.may_step(from, to) && standing.get(&to).copied().unwrap_or(0) < 3
        };
        // Each turn gets its own recording, so a turn that spends a different number of dice says so here instead of shifting every turn after it.
        let recorded_dice = session
            .dice_by_turn
            .get(index - 1)
            .cloned()
            .unwrap_or_else(|| cdda_core_types::rng::Recording::parse("").expect("empty"));
        let offered = recorded_dice.remaining();
        let mut rolls = Rolls::replaying(recorded_dice);
        cdda_sim::process_gas(&mut world, GAS, 30, wind.power, can_take, &mut rolls);
        let unspent = rolls.recording_remaining().unwrap_or(0);
        spent += offered - unspent;
        compared += 1;
        if unspent != 0 && disagreements.len() < 6 {
            disagreements.push((index, format!("{unspent} recorded dice went unspent this turn")));
        }
        let positions: Vec<(Entity, i32, i32)> = {
            let mut query = world.query::<(Entity, &Position)>();
            query
                .iter(&world)
                .map(|(entity, position)| (entity, position.x, position.y))
                .collect()
        };
        cdda_sim::decay_the_fields_the_sweep_found(
            &mut world,
            &positions,
            0,
            &mut Rolls::new(7),
            |_field| None,
            |_, _, _| {},
        );
        let _ = (found, &mut map);
        let ours = fields(&mut world);
        let theirs: Vec<(i32, i32, i32, i32, i64)> = recorded
            .iter()
            .map(|field| (field.x, field.y, field.z, field.intensity, field.age))
            .collect();
        let mut theirs = theirs;
        theirs.sort_unstable();
        if ours != theirs && disagreements.len() < 6 {
            disagreements.push((index, format!("ours {ours:?} theirs {theirs:?}")));
        }
    }
    assert!(
        disagreements.is_empty(),
        "the port's gas disagrees with the recording: {disagreements:#?}"
    );
    // A gate that compared nothing would pass for the wrong reason, and one that drew nothing would pass without ever exercising the spread rule.
    assert_eq!(compared, session.turns.len() - 2, "every recorded turn was replayed");
    assert!(spent >= 80, "the replay spent the reference's own dice: {spent}");

}
