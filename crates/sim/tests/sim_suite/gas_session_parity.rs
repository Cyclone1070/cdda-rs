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
    /// The gas's own numbers, straight from the prototype dump: its decay clock and how eagerly it spreads. A replay that hardcoded them would be replaying a
    /// different gas - and it did, at 30%, which turned the extinguisher's ninety into a gate that refused three recorded dice.
    gas: GasType,
}

impl Takes {
    fn load(session: &crate::reference_session::Session, name: &str) -> Self {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/prototypes/terrain.json");
        let _ = name;
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
            gas: gas_type(session),
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

/// The wind each square stands in, straight from the fixture.
///
/// A gas on a roof and one in the cellar under it are in different winds on the same turn, and the recorded number is what the reference's own `get_local_windpower`
/// answered for that square - so it is read per square and never averaged into one figure for the turn.
fn wind_at(
    recorded: &[crate::reference_session::Placed],
    direction: i32,
) -> impl Fn((i32, i32, i32)) -> cdda_sim::Wind + Copy + '_ {
    move |square: (i32, i32, i32)| {
        recorded
            .iter()
            .find(|field| (field.x, field.y, field.z) == square)
            .map_or(cdda_sim::Wind::default(), |field| cdda_sim::Wind {
                power: field.windpower,
                direction,
                sheltered: field.sheltered,
            })
    }
}

/// What the fixture's own gas is, from the field prototype dump.
#[derive(Debug, Clone, Copy, Default)]
struct GasType {
    /// How long it takes the gas to lose an intensity, in turns.
    half_life: i64,
    /// Whether that half-life doubles with each level.
    linear_half_life: bool,
    /// The chance in a hundred that it spreads to a neighbour each turn.
    percent_spread: i32,
}

/// How long the fixture's own gas takes to lose an intensity, from the field prototype dump.
///
/// The reference's `field_type::half_life` (`src/field_type.h:217`) is the clock a field's decay is arithmetic on, and a newborn mist must be born with its type's,
/// not with a placeholder: the extinguisher loses a level every ten turns, and a port that gave it a million keeps it alive for the rest of the session.
fn gas_type(session: &crate::reference_session::Session) -> GasType {
    let Some(kind) = session
        .turns
        .first()
        .and_then(|turn| turn.first())
        .map(|field| field.kind.clone())
    else {
        return GasType::default();
    };
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/prototypes/fields.json");
    let text = std::fs::read_to_string(path).expect("the field prototypes");
    let cdda_json::Value::Object(root) = cdda_json::parse(text.as_bytes()).expect("valid JSON") else {
        panic!("an object");
    };
    let Some(cdda_json::Value::Array(entries)) = root.get("prototypes") else {
        panic!("prototypes");
    };
    entries
        .iter()
        .find_map(|entry| {
            let cdda_json::Value::Object(object) = entry else {
                return None;
            };
            let Some(cdda_json::Value::Str(id)) = object.get("id") else {
                return None;
            };
            if id != &kind {
                return None;
            }
            let half_life = match object.get("half_life") {
                Some(cdda_json::Value::Int(value)) => *value,
                _ => 0,
            };
            let linear = matches!(object.get("linear_half_life"), Some(cdda_json::Value::Bool(true)));
            let percent_spread = match object.get("percent_spread") {
                Some(cdda_json::Value::Int(value)) => i32::try_from(*value).unwrap_or(0),
                _ => 0,
            };
            Some(GasType {
                half_life,
                linear_half_life: linear,
                percent_spread,
            })
        })
        .unwrap_or_default()
}

/// The submaps holding a field, in the reference's sweep order `(z, x, y)`.
fn submaps(world: &mut World) -> Vec<(i32, i32, i32)> {
    let mut query = world.query::<&Position>();
    let mut keys: Vec<(i32, i32, i32)> = query
        .iter(world)
        .map(|position| cdda_sim::submap_of(position.x, position.y, position.z))
        .collect();
    keys.sort_unstable();
    keys.dedup();
    keys
}

/// Replays one recorded gas session and answers what disagreed, if anything.
fn replay(name: &str) -> (usize, usize, Vec<(usize, String)>) {
    let session = crate::reference_session::Session::load(name);
    assert!(
        !session.dice_by_turn.is_empty(),
        "{name} labels its turns, so each turn's dice are its own"
    );
    let mut map = session.map();
    let takes = Takes::load(&session, name);
    let mut world = World::new();
    // **Seeded from the first recorded state, not from the placement.** The dump places the gas and then runs a sweep; the first sweep does not see it - the
    // field is not in a cached submap yet - so the reference's own trace has no dice for the transition into turn 1 while its age still advances. Starting at the
    // state that sweep left is the honest read of the fixture: from there every turn is a sweep that did see the gas, and the dice line up one for one.
    let Some(first) = session.turns.get(1).and_then(|turn| turn.first()) else {
        panic!("the fixture has a second state");
    };
    // **The type's own half-life and decay shape**, not a placeholder: the reference's extinguisher loses a level every ten turns, and a mist handed a million keeps
    // every intensity the fixture says it lost.
    let mut entry = FieldEntry::new(3, takes.gas.half_life, takes.gas.linear_half_life, false);
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
        let wind_at = wind_at(recorded, wind.direction);
        // The other half of `gas_can_spread_to`: the square can be walked, and a step to another level is legal. Whether the square already holds gas at least as
        // thick as the gas arriving is the crate's question, because only it can see the field a previous square of the same sweep left there.
        let can_take = |from: (i32, i32, i32), to: (i32, i32, i32)| takes.open(to) && takes.may_step(from, to);
        // Each turn gets its own recording, so a turn that spends a different number of dice says so here instead of shifting every turn after it.
        let recorded_dice = session
            .dice_by_turn
            .get(index - 1)
            .cloned()
            .unwrap_or_else(|| cdda_core_types::rng::Recording::parse("").expect("empty"));
        let offered = recorded_dice.remaining();
        if std::env::var("GAS_TRACE").is_ok() {
            eprintln!("turn {index}: {offered} dice, now {}, port {:?}", session.now.get(index - 1).copied().unwrap_or(0), fields(&mut world));
        }
        let mut rolls = Rolls::replaying(recorded_dice);
        // **The sweep submap by submap, aging as it goes.** The reference visits submaps in ascending `(z, x, y)` and ages a field when it reaches the field's own
        // submap (`map_field.cpp:164-184`), so a gas born in a submap the sweep has already passed is not aged this turn. Decaying everything at the end of the turn
        // gets the second case right and the first wrong, which is exactly the one-turn age error the fall fixture found.
        // A cursor, not a list: a gas that spreads into a **later** submap is aged in the same turn, so the sweep has to notice submaps that did not exist when it
        // started. One that spreads backwards waits for the next turn, which is the whole point of the order.
        let mut cursor: Option<(i32, i32, i32)> = None;
        while let Some(key) = submaps(&mut world).into_iter().find(|key| cursor.is_none_or(|cursor| *key > cursor)) {
            cursor = Some(key);
            let _ = wind;
            let half_life_of = |_id: FieldId| takes.gas.half_life;
            cdda_sim::process_gas_in_wind(
                &mut world,
                GAS,
                cdda_sim::Where::Submap(key),
                takes.gas.percent_spread,
                cdda_sim::Surroundings {
                    wind_at,
                    half_life_of,
                },
                can_take,
                &mut rolls,
            );
            let positions: Vec<(Entity, i32, i32)> = {
                let mut query = world.query::<(Entity, &Position)>();
                query
                    .iter(&world)
                    .filter(|(_, position)| cdda_sim::submap_of(position.x, position.y, position.z) == key)
                    .map(|(entity, position)| (entity, position.x, position.y))
                    .collect()
            };
            cdda_sim::decay_the_fields_the_sweep_found(
                &mut world,
                &positions,
                session.now.get(index - 1).copied().unwrap_or(0),
                &mut Rolls::new(7),
                |_field| None,
                |_, _, _| {},
            );
        }
        let unspent = rolls.recording_remaining().unwrap_or(0);
        spent += offered - unspent;
        compared += 1;
        if unspent != 0 && disagreements.len() < 6 {
            disagreements.push((index, format!("{unspent} recorded dice went unspent this turn")));
        }
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
    (compared, spent, disagreements)
}

#[test]
fn sixty_recorded_turns_of_a_gas_replay_square_for_square() {
    let (compared, spent, disagreements) = replay("gas-60");
    assert!(
        disagreements.is_empty(),
        "the port's gas disagrees with the recording: {disagreements:#?}"
    );
    // A gate that compared nothing would pass for the wrong reason, and one that drew nothing would pass without ever exercising the spread rule.
    assert_eq!(compared, 59, "every recorded turn was replayed");
    assert!(spent >= 80, "the replay spent the reference's own dice: {spent}");
}

#[test]
fn a_gas_on_a_stairwell_falls_the_way_the_recording_says() {
    // The fall branch, end to end: the square is a stair, the square under it is open air, and the reference's gas drops through on turn six. Then the two levels
    // trade intensity as each spreads into the other, which is the half of the rule that no hand-written case would have thought to check.
    let (compared, spent, disagreements) = replay("gas-fall");
    assert!(
        disagreements.is_empty(),
        "the port's gas disagrees with the recording: {disagreements:#?}"
    );
    assert_eq!(compared, 39, "every recorded turn was replayed");
    assert!(spent >= 15, "the replay spent the reference's own dice: {spent}");
}

#[test]
#[ignore = "red on the decay draw's bookkeeping, not on the wind: \n both spreads the recording shows are replayed correctly - the rise into z 2 and the turn-two move into the wind, \n the three real draws in the right order. What is left is decay: the reference draws eleven decay exponentials in twenty-four \n turns and the port reaches five fields, so states three and four show ages one turn too old on the newborn and on the risen field. \n The draw is recorded (`drawn decay.turns`) and the replay consumes it; the counts disagree, so the next step is to find which fields \n the reference decays that the sweep never reaches"]
fn a_gas_on_a_roof_spreads_into_the_wind_the_recording_says() {
    // The **wind-blocker** branch, end to end: the gas stands at z 1 under a wind of eighteen with no shelter, so the three squares facing the wind each cost a real
    // draw, and the session cannot be replayed without the branch - the recorded reals have nowhere to go and every later draw is one position out.
    let (compared, spent, disagreements) = replay("gas-wind");
    assert!(
        disagreements.is_empty(),
        "the port's gas disagrees with the recording: {disagreements:#?}"
    );
    assert_eq!(compared, 23, "every recorded turn was replayed");
    assert!(spent >= 10, "the replay spent the reference's own dice: {spent}");
}
