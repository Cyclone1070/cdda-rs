//! Scripted headless playtests: a world described in a file, played step by step, checked as it goes.
//!
//! A scenario is plain text, one verb per line, and it is deliberately as small as the port is:
//!
//! ```text
//! world 12 flammable
//! seed 7
//! fire 6 6 2 500
//! turn 5
//! expect changed 1
//! expect fire 6 6
//! ```
//!
//! The point is a gate a person can read: when a behaviour changes, the scenario says which turn
//! changed how many squares, and the assertion that fails names the verb that failed. Anything the
//! script does not understand is an error, not a comment - a scenario that silently ignores half its
//! lines is worse than one that refuses to run.

pub mod capture;
pub mod qa;
pub mod png;

use bevy_ecs::prelude::*;

/// One step of a scenario.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Step {
    /// A world of `side` by `side` squares of floor, flammable or bare.
    World {
        /// How many squares on a side.
        side: i32,
        /// Whether the floor feeds a fire.
        flammable: bool,
    },
    /// Installs the game's own vitamin and effect tables from the reference's JSON, as its factories do at start-up.
    Data,
    /// Renames the place for an hour, the way the `place_override` dialogue effect does.
    PlaceOverride(String),
    /// Asserts what the place is called now.
    ExpectPlace(String),
    /// The dice every later draw comes from.
    Seed(u32),
    /// Where the character lays its smell, and how much.
    Scent {
        /// The square's x.
        x: i32,
        /// The square's y.
        y: i32,
    },
    /// A fire, with how hard it burns and how long it has burned.
    Fire {
        /// The square's x.
        x: i32,
        /// The square's y.
        y: i32,
        /// Its strength.
        intensity: i32,
        /// Its age, in the reference's units.
        age: i64,
    },
    /// Take this many turns.
    Turn(u32),
    /// Sets the world's clock, in turns since the epoch, which is how a playtest reaches a rule that waits days.
    Date(i64),
    /// Says the survivor should be wearing an effect, by the data's own id.
    ExpectEffect(String),
    /// Says a monster standing on a square should be wearing an effect.
    ExpectMonsterEffect {
        /// The effect's id, as the data spells it.
        id: String,
        /// The square the monster should be standing on.
        x: i32,
        /// The square the monster should be standing on.
        y: i32,
    },
    /// Says how many monsters in the world are wearing an effect, wherever they have walked to.
    ExpectMonstersWithEffect {
        /// How many monsters should be wearing it.
        count: u32,
        /// The effect's id, as the data spells it.
        id: String,
    },
    /// The last turn changed exactly this many squares.
    ExpectChanged(usize),
    /// A fire stands on this square.
    ExpectFireAt(i32, i32),
    /// Nothing burns on this square.
    ExpectNoFireAt(i32, i32),
    /// Put a survivor on the square, which is who the item steps act through.
    Survivor {
        /// The square's x.
        x: i32,
        /// The square's y.
        y: i32,
    },
    /// Install the game's own item data, so the store knows what things are.
    Items,
    /// Put one of a thing in the survivor's hands.
    Give {
        /// The item's id, as the data spells it.
        item: String,
        /// How many.
        count: i32,
    },
    /// Throws what the survivor carries at a square.
    Throw {
        /// The item's id, as the data spells it.
        item: String,
        /// The square's x.
        x: i32,
        /// The square's y.
        y: i32,
    },
    /// An item of this kind lies on exactly this square.
    ExpectItemAt {
        /// The item's id, as the data spells it.
        item: String,
        /// The square's x.
        x: i32,
        /// The square's y.
        y: i32,
    },
    /// Draws the view to a PNG, so the screen a scenario produces can be diffed.
    Shot(String),
    /// Aims a gun the survivor holds, spending moves to take recoil down.
    ///
    /// Reference: the `AIM` activity's loop over `Character::aim_per_move` (`character.cpp:1146-1210`) - the gain per move, the floor that stops it, and the limit the sights impose.
    Aim {
        /// The gun being aimed, by id.
        gun: String,
        /// How many moves to spend aiming.
        moves: i32,
    },
    /// Fires a burst from a gun the survivor holds: several shots, one after another, each carrying the last one's recoil.
    ///
    /// Reference: `Character::fire_gun`'s `for( curshot < burst_size )` (`src/ranged.cpp:1130-1240`).
    Burst {
        /// The gun's id in the guns table.
        gun: String,
        /// The square's x.
        x: i32,
        /// The square's y.
        y: i32,
        /// How many shots to ask for. A gun that runs dry fires fewer.
        shots: i32,
        /// How many rounds the gun holds when it is handed over.
        rounds: i32,
    },
    /// Shoots a gun the survivor holds at a square, with a number of rounds in it.
    ///
    /// Reference: `Character::fire_gun` (`src/ranged.cpp:1050-1200`) - the shot's price, its own error, the range it is judged at and the blow it does on landing.
    Shoot {
        /// The gun's id in the guns table, such as `ruger_mini`.
        gun: String,
        /// The square's x.
        x: i32,
        /// The square's y.
        y: i32,
        /// How many rounds the gun holds when it is handed over.
        rounds: i32,
    },
    /// Puts an item in the survivor's hands, which is what a blow is built from.
    Wield {
        /// The item's id.
        item: String,
    },
    /// Swings at whatever stands on a square.
    Attack {
        /// The square's x.
        x: i32,
        /// The square's y.
        y: i32,
    },
    /// Tells every monster to walk towards a square, which is what makes it swing at what stands there.
    Goal {
        /// The square's x.
        x: i32,
        /// The square's y.
        y: i32,
    },
    /// Loads the monster definitions the game's data defines.
    Monsters,
    /// Puts a monster on the map, by the name its type has in the data.
    Monster {
        /// The type's id, as the data spells it.
        id: String,
        /// The square's x.
        x: i32,
        /// The square's y.
        y: i32,
    },
    /// Expects the survivor to have lost at least this many hit points.
    ExpectHurt(i32),
    /// Expects some creature to be this close to the survivor, in squares.
    ExpectMonsterNear(i32),
    /// Expects no creature to be standing on this square.
    ExpectNoMonsterAt {
        /// The square's x.
        x: i32,
        /// The square's y.
        y: i32,
    },
    /// Make a recipe, running up to this many turns for it.
    Craft {
        /// The recipe's id, as the data spells it.
        recipe: String,
        /// How many turns to give it.
        turns: u32,
    },
    /// Exactly this many of a thing are lying on the square under the survivor.
    ExpectFloor {
        /// The item's id.
        item: String,
        /// How many.
        count: i32,
    },
    /// The survivor is carrying exactly this many of a thing.
    ExpectCarried {
        /// The item's id.
        item: String,
        /// How many.
        count: i32,
    },
}

/// What running a scenario did.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Outcome {
    /// One line per step that did something worth reading.
    pub lines: Vec<String>,
    /// The assertions that did not hold, in the words of the scenario that made them.
    pub failures: Vec<String>,
}

impl Outcome {
    /// Whether every assertion held.
    #[must_use]
    pub fn passed(&self) -> bool {
        self.failures.is_empty()
    }
}

/// Reads a scenario, refusing anything it does not understand.
///
/// # Errors
///
/// An error naming the line and the word when a line is not a step this runner knows, or when a
/// number is not a number.
/// One `expect` line: a claim about the world that reads a square, a count or an id and nothing else.
///
/// # Errors
///
/// A word that is not a number where a number belongs, or an `expect` family this runner does not know.
fn expect_step(words: &[&str], number: usize) -> Result<Step, String> {
    match words {
        ["effect", id] => Ok(Step::ExpectEffect((*id).to_owned())),
        ["changed", count] => Ok(Step::ExpectChanged(number_of(count, number, "squares")?)),
        ["fire", x, y] => Ok(Step::ExpectFireAt(
            number_of(x, number, "x")?,
            number_of(y, number, "y")?,
        )),
        ["no", "fire", x, y] => Ok(Step::ExpectNoFireAt(
            number_of(x, number, "x")?,
            number_of(y, number, "y")?,
        )),
        ["place", rest @ ..] => Ok(Step::ExpectPlace(rest.join(" "))),
        ["item", item, "at", x, y] => Ok(Step::ExpectItemAt {
            item: (*item).to_owned(),
            x: number_of(x, number, "x")?,
            y: number_of(y, number, "y")?,
        }),
        other => Err(format!("`expect {}` is not a step I know", other.join(" "))),
    }
}

/// Reads a scenario: one step a line, `#` starting a comment, blank lines ignored.
///
/// # Errors
///
/// An error naming the line and the word when a line is not a step this runner knows, or when a number is not a number.
pub fn parse(text: &str) -> Result<Vec<Step>, String> {
    let mut steps = Vec::new();
    for (number, line) in text.lines().enumerate() {
        let line = line.split('#').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }
        let words: Vec<&str> = line.split_whitespace().collect();
        let step = match words.as_slice() {
            ["world", side, "flammable"] => Step::World {
                side: number_of(side, number, "side")?,
                flammable: true,
            },
            ["world", side, "bare"] => Step::World {
                side: number_of(side, number, "side")?,
                flammable: false,
            },
            ["seed", seed] => Step::Seed(number_of(seed, number, "seed")?),
            ["scent", x, y] => Step::Scent {
                x: number_of(x, number, "x")?,
                y: number_of(y, number, "y")?,
            },
            ["fire", x, y, intensity, age] => Step::Fire {
                x: number_of(x, number, "x")?,
                y: number_of(y, number, "y")?,
                intensity: number_of(intensity, number, "intensity")?,
                age: number_of(age, number, "age")?,
            },
            ["turn", count] => Step::Turn(number_of(count, number, "turns")?),
            ["date", turn] => Step::Date(number_of(turn, number, "turn")?),
            ["data"] => Step::Data,
            ["survivor", x, y] => Step::Survivor {
                x: number_of(x, number, "x")?,
                y: number_of(y, number, "y")?,
            },
            ["items"] => Step::Items,
            ["monsters"] => Step::Monsters,
            ["place", rest @ ..] => Step::PlaceOverride(rest.join(" ")),
            ["give", item, count] => Step::Give {
                item: (*item).to_owned(),
                count: number_of(count, number, "count")?,
            },
            ["wield", item] => Step::Wield {
                item: (*item).to_owned(),
            },
            ["attack", x, y] => Step::Attack {
                x: number_of(x, number, "x")?,
                y: number_of(y, number, "y")?,
            },
            ["throw", ..] => throw_step(&words, number)?,
            ["shot", path] => Step::Shot((*path).to_owned()),
            ["aim", ..] => aim_step(&words, number)?,
            ["shoot", ..] => shoot_step(&words)?,
            ["burst", ..] => burst_step(&words)?,
            ["goal", x, y] => Step::Goal {
                x: number_of(x, number, "x")?,
                y: number_of(y, number, "y")?,
            },
            ["monster", id, x, y] => Step::Monster {
                id: (*id).to_owned(),
                x: number_of(x, number, "x")?,
                y: number_of(y, number, "y")?,
            },
            ["expect", "hurt", points] => Step::ExpectHurt(number_of(points, number, "points")?),
            ["expect", "monster", "within", squares] => {
                Step::ExpectMonsterNear(number_of(squares, number, "squares")?)
            },
            ["expect", "monsters", count, "with", "effect", id] => Step::ExpectMonstersWithEffect {
                count: number_of(count, number, "monsters")?,
                id: (*id).to_owned(),
            },
            ["expect", "monster", "effect", id, "at", x, y] => Step::ExpectMonsterEffect {
                id: (*id).to_owned(),
                x: number_of(x, number, "x")?,
                y: number_of(y, number, "y")?,
            },
            ["expect", "no", "monster", "at", x, y] => Step::ExpectNoMonsterAt {
                x: number_of(x, number, "x")?,
                y: number_of(y, number, "y")?,
            },
            ["craft", recipe, turns] => Step::Craft {
                recipe: (*recipe).to_owned(),
                turns: number_of(turns, number, "turns")?,
            },
            ["expect", "floor", item, count] => Step::ExpectFloor {
                item: (*item).to_owned(),
                count: number_of(count, number, "count")?,
            },
            ["expect", "carried", item, count] => Step::ExpectCarried {
                item: (*item).to_owned(),
                count: number_of(count, number, "count")?,
            },
            // The rest of the `expect` family lives in one helper. This arm comes last, after the ones that need more than a square or a count.
            ["expect", rest @ ..] => expect_step(rest, number)?,
            _ => return Err(format!("line {}: `{line}` is not a step I know", number + 1)),
        };
        steps.push(step);
    }
    Ok(steps)
}

fn number_of<T: std::str::FromStr>(word: &str, line: usize, what: &str) -> Result<T, String> {
    word.parse()
        .map_err(|_| format!("line {}: `{word}` is not a number for {what}", line + 1))
}

/// Plays a scenario from its text.
///
/// # Errors
///
/// Whatever [`parse`] refuses: a scenario that does not parse is not played at all.
pub fn run(text: &str) -> Result<Outcome, String> {
    let steps = parse(text)?;
    let mut world = World::new();
    let mut outcome = Outcome::default();
    let mut last_changed = 0;
    for step in steps {
        match step {
            Step::World { side, flammable } => {
                build_world(&mut world, side, flammable);
                outcome
                    .lines
                    .push(format!("world: {side} by {side}, flammable: {flammable}"));
            }
            Step::Seed(seed) => {
                cdda_sim::turn::set_random(&mut world, cdda_core_types::rng::Rolls::new(seed));
            }
            Step::Scent { x, y } => {
                cdda_sim::turn::set_scent_source(&mut world, (x, y), 0);
            }
            Step::Fire {
                x,
                y,
                intensity,
                age,
            } => {
                light_a_fire(&mut world, x, y, intensity, age);
            }
            Step::Date(turn) => {
                cdda_sim::calendar::set_turn(&mut world, turn);
                outcome.lines.push(format!("date: the clock is at turn {turn}"));
            }
            Step::ExpectMonstersWithEffect { count, id } => {
                let wearing = cdda_sim::creature::creatures(&mut world)
                    .into_iter()
                    .filter(|entity| cdda_sim::applied_effects::has(&world, *entity, id.as_str()))
                    .count();
                if wearing == count as usize {
                    outcome
                        .lines
                        .push(format!("effects: {wearing} monster(s) are {id}"));
                } else {
                    outcome.failures.push(format!(
                        "expected {count} monster(s) to be {id}, and found {wearing}"
                    ));
                }
            }
            Step::ExpectMonsterEffect { id, x, y } => {
                let standing = cdda_sim::creature::creature_at(&world, (x, y, 0))
                    .into_iter()
                    .find(|entity| cdda_sim::applied_effects::has(&world, *entity, id.as_str()));
                if standing.is_some() {
                    outcome.lines.push(format!("effect: something at ({x}, {y}) is {id}"));
                } else {
                    outcome
                        .failures
                        .push(format!("expected something at ({x}, {y}) to be {id}, and nothing is"));
                }
            }
            Step::ExpectEffect(id) => {
                let wearing = survivor(&mut world)
                    .is_some_and(|survivor| cdda_sim::applied_effects::has(&world, survivor, id.as_str()));
                if wearing {
                    outcome.lines.push(format!("effect: the survivor is {id}"));
                } else {
                    outcome
                        .failures
                        .push(format!("expected the survivor to be {id}, and it is not"));
                }
            }
            Step::Turn(count) => {
                for _ in 0..count {
                    last_changed = take_turn(&mut world);
                }
                outcome.lines.push(format!(
                    "turn {count}: {last_changed} square(s) changed in the last of them"
                ));
            }
            Step::ExpectChanged(count) => {
                if last_changed != count {
                    outcome.failures.push(format!(
                        "expected {count} changed square(s), the last turn changed {last_changed}"
                    ));
                }
            }
            Step::ExpectFireAt(x, y) => {
                if cdda_sim::fire_facts::fire_at(&world, (x, y, 0)).is_none() {
                    outcome.failures.push(format!("expected a fire at ({x}, {y})"));
                }
            }
            Step::ExpectNoFireAt(x, y) => {
                if cdda_sim::fire_facts::fire_at(&world, (x, y, 0)).is_some() {
                    outcome.failures.push(format!("expected no fire at ({x}, {y})"));
                }
            }
            other => {
                // The item steps are a group of their own: they all ask the registry, and keeping them in one place is
                // what stops this match from becoming a list of everything the runner can do.
                take_item_step(&mut world, &other, &mut outcome);
            }
        }
    }
    Ok(outcome)
}

/// Runs one of the steps that act on items, answering whether it was one of them.
///
/// Kept apart from [`run`]'s match for one reason worth writing down: those arms all ask the item registry, and a match
/// that grows a dozen item arms stops being readable as the shape of a scenario.
fn take_item_step(world: &mut World, step: &Step, outcome: &mut Outcome) -> bool {
    match step {
        Step::Data => install_the_tables(world, outcome),
        Step::Survivor { x, y } => {
                let survivor = cdda_sim::spawn_player(world, (*x, *y), 80, 0);
                cdda_sim::creature::set_speed(world, survivor, 100, 0);
                // The reference's new character starts at eight in every stat (`newcharacter.cpp:5128-5131`), and a creature with no `Strength` component reads as **zero** - which
                // would make every kick weightless and every hand perfect. The three components are set here, in the scenario's own spawn, rather than defaulted in the sim.
                world
                    .entity_mut(survivor)
                    .insert(cdda_sim::creature::Strength { base: 8 })
                    .insert(cdda_sim::creature::Dexterity { base: 8 })
                    .insert(cdda_sim::creature::Perception { base: 8 });
                outcome.lines.push(format!("survivor: at ({x}, {y})"));
            }
        Step::PlaceOverride(name) => take_place_step(world, name, outcome),
        Step::ExpectPlace(name) => check_place_step(world, name, outcome),
        Step::Items => {
                match install_item_data(world) {
                    Some(kinds) => outcome.lines.push(format!("items: {kinds} kinds")),
                    None => outcome.failures
                        .push("the game's item data is not where the scenario expects it".to_owned()),
                }
            }
        Step::Give { item, count } => {
                let names = cdda_sim::item_registry::snapshot(world);
                let Some(kind) = names.id_of(item) else {
                    outcome.failures.push(format!("no item called {item}"));
                    return true;
                };
                let Some(survivor) = survivor(world) else {
                    outcome.failures.push("there is no survivor".to_owned());
                    return true;
                };
                for _ in 0..*count {
                    let spawned = cdda_sim::drop_item(world, (0, 0), kind, 1);
                    world.entity_mut(spawned).remove::<cdda_sim::Position>();
                    world
                        .entity_mut(spawned)
                        .insert(cdda_sim::creature::Carried(survivor));
                }
            }
        Step::Wield { .. } | Step::Attack { .. } => take_a_swing(world, step, outcome),
        Step::Goal { x, y } => send_the_monsters_to(outcome, world, *x, *y),
        Step::Throw { item, x, y } => throw_a_step(world, outcome, item, *x, *y),
        Step::Shot(path) => take_a_shot(world, outcome, path),
        Step::Shoot { gun, x, y, rounds } => fire_a_step(world, outcome, gun, *x, *y, *rounds),
        Step::Aim { gun, moves } => aim_a_step(world, outcome, gun, *moves),
        Step::Burst { gun, x, y, shots, rounds } => burst_a_step(world, outcome, gun, *x, *y, *shots, *rounds),
        Step::Monsters => match install_monster_data(world) {
            Some(kinds) => outcome.lines.push(format!("monsters: {kinds} kinds")),
            None => outcome
                .failures
                .push("the game's monster data is not where the scenario expects it".to_owned()),
        },
        Step::Monster { id, x, y } => put_a_monster(world, outcome, id, *x, *y),
        Step::ExpectHurt(points) => expect_a_wound(world, outcome, *points),
        Step::ExpectMonsterNear(squares) => expect_a_monster_close(world, outcome, *squares),
        Step::ExpectNoMonsterAt { x, y } => expect_no_monster_at(world, outcome, *x, *y),
        Step::ExpectItemAt { item, x, y } => expect_one_on(world, outcome, item, *x, *y),
        Step::Craft { recipe, turns } => {
                let Some(survivor) = survivor(world) else {
                    outcome.failures.push("there is no survivor".to_owned());
                    return true;
                };
                let allowance = i32::try_from(*turns).unwrap_or(i32::MAX);
                match craft_a_recipe(world, survivor, recipe, allowance) {
                    Some(line) => outcome.lines.push(format!("craft: {line}")),
                    None => outcome
                        .failures
                        .push(format!("no recipe named {recipe}")),
                }
            }
        Step::ExpectFloor { item, count } => {
                let names = cdda_sim::item_registry::snapshot(world);
                let lying = ground_of(world, item, &names);
                if lying != *count {
                    outcome
                        .failures
                        .push(format!("expected {count} {item} on the floor, found {lying}"));
                }
            }
        Step::ExpectCarried { item, count } => {
            let names = cdda_sim::item_registry::snapshot(world);
                let held = survivor(world).map_or(0, |survivor| {
                    cdda_sim::inventory::how_many(world, survivor, item, |kind| names.name_of(kind))
                });
                if held != *count {
                    outcome
                        .failures
                        .push(format!("expected {count} {item} carried, found {held}"));
                }
            }
        _ => return false,
    }
    true
}

/// The survivor a scenario acts through: the marked player, or the first creature when there is none marked.
#[must_use]
pub fn survivor(world: &mut World) -> Option<bevy_ecs::entity::Entity> {
    let marked = {
        let mut query = world.query_filtered::<bevy_ecs::entity::Entity, With<cdda_sim::Player>>();
        query.iter(world).next()
    };
    marked.or_else(|| {
        let mut query =
            world.query_filtered::<bevy_ecs::entity::Entity, With<cdda_sim::creature::Creature>>();
        query.iter(world).next()
    })
}

/// Builds a world of `side` by `side` squares, which is what a scenario starts from.
pub fn build_world(world: &mut World, side: i32, flammable: bool) {
    // A world without a clock is not a world: the reference's calendar always exists, and every rule that reads a turn - a timed event, a scheduled effect, the sleepiness
    // clock - asks it. Scenarios used to start with none, so a step that scheduled something found no clock and quietly scheduled nothing.
    cdda_sim::calendar::set_turn(world, 0);
    let mut level = cdda_core_types::level::Level::new();
    for y in 0..side {
        for x in 0..side {
            level.set_terrain(x, y, 1, 0);
        }
    }
    cdda_sim::load_level(world, &level);
    // What the floor is worth to walk on. Without this a scenario's world has squares with **no move cost at all**, so routing answers "no route" for every step and a
    // monster that hears a noise honestly cannot come. The reference has this every time because terrain is loaded with its `move_cost`; a scenario builds its floor by hand.
    let mut terrain = cdda_sim::terrain_table::TerrainTable::new();
    terrain.insert(1, true);
    terrain.insert_move_cost(1, 2);
    cdda_sim::set_prototypes(
        world,
        cdda_sim::Prototypes {
            terrain,
            furniture: cdda_sim::terrain_table::TerrainTable::new(),
        },
    );
    cdda_sim::terrain_types::set_types(world, [("t_floor".to_owned(), 1u32)]);
    for y in 0..side {
        for x in 0..side {
            cdda_sim::fire_facts::set_fire_facts(
                world,
                (x, y, 0),
                cdda_sim::TerrainFacts {
                    flammable,
                    ..cdda_sim::TerrainFacts::default()
                },
            );
        }
    }
}

/// Puts a fire on a square.
pub fn light_a_fire(world: &mut World, x: i32, y: i32, intensity: i32, age: i64) {
    let mut entry = cdda_core_types::field::FieldEntry::new(3, 1_000_000, false, true);
    entry.set_age(age);
    entry.set_intensity(intensity);
    let entity = world
        .spawn((
            cdda_sim::Field {
                id: cdda_sim::FIRE,
                entry,
                priority: 0,
            },
            cdda_sim::Position { x, y, z: 0 },
        ))
        .id();
    cdda_sim::singletons::index_mut(world).insert(entity, (x, y, 0));
}

/// Takes one turn, and answers how many squares it changed.
pub fn take_turn(world: &mut World) -> usize {
    // A turn is a second of the world's time, and the schedule reads the clock rather than counting its own calls: `focus_stage` asks for every sixtieth turn and
    // `digest_stage` for every three hundredth. **A clock that never moves makes those gates fire once and then never again**, which is a scenario that plays one second
    // three thousand times and reports nothing changed.
    if let Some(calendar) = cdda_sim::calendar::of(world) {
        cdda_sim::calendar::set_turn(world, calendar.turn.saturating_add(1));
    }
    let before = cdda_sim::inspect::world_fingerprints(world);
    cdda_sim::turn::run_turn(world);
    // Reference: the map asks every corpse whether it is ready to get up once a turn (`game::revive_corpse`, `game.cpp:5718-5760`), and the answer places the monster again.
    // The spawn is a closure because a monster type is content: this module loads it, the simulation owns the rule.
    let types = monster_types().unwrap_or_default();
    let _risen = cdda_sim::creature::revive_corpses(world, |world, kind, square| {
        let Some(monster_type) = types.iter().find(|each| {
            cdda_sim::monster_registry::id_of(world, &each.id) == Some(kind)
        }) else {
            return false;
        };
        cdda_sim::creature::spawn_monster_of_type(world, kind, monster_type, square, 100);
        true
    });
    let after = cdda_sim::inspect::world_fingerprints(world);
    cdda_sim::inspect::squares_that_differ(&before, &after).len()
}

/// Puts clothes on the avatar from the game's own item data.
///
/// The reference dresses a character from the item factory, and an item's protection is not a field on the item but
/// a sum over its materials and their thickness (`item::_resist`). That work is `cdda_content::armour`'s; this only
/// has to choose something to wear and hand it to the store as an item with a `WornBy` link, which is then what
/// `creature::dress` reads.
///
/// Returns a line describing what was worn, or nothing when the game data is not where the binary expects it.
/// Installs the game's own item data, so the store can answer what things are, weigh and hold.
///
/// Reference: `item_factory` reading the JSON once at start-up. Everything the item layer does - picking up, stashing,
/// wielding, crafting - asks the registry for names, volumes, weights and pockets, and until this ran a real game world
/// had an empty one: a survivor could carry a hundred anvils and put a rifle in a vest with two litres of pocket.
///
/// Answers how many kinds were installed, or [`None`] when the data is not where the binary expects it.
pub fn install_item_data(world: &mut World) -> Option<usize> {
    let data = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../oracle/src/data/json");
    if !data.is_dir() {
        return None;
    }
    let items = cdda_content::items::load_paths(&[&data]).ok()?;
    cdda_sim::item_data::install(world, &items);
    Some(items.len())
}

/// Loads every monster type the game's data defines, and lets the store answer by name.
///
/// The definitions stay in `cdda-content`: the store is given the two things a scenario needs - which number each name is, and how many hit points it starts with - so a
/// scenario that places `mon_zombie` gets the zombie's own numbers rather than a fixture's.
pub fn install_monster_data(world: &mut World) -> Option<usize> {
    let monsters = monster_types()?;
    // What a corpse is, installed with the monsters that make them: the item the content calls `corpse`, and each monster's own volume, which is what its corpse is measured from
    // (`item::corpse_volume`). A world with no corpse kind sweeps the dead away without leaving anything, which is what every test world before this round did.
    if let (Some(corpse_item), false) = (
        cdda_sim::item_registry::id_of(world, "corpse"),
        monsters.is_empty(),
    ) {
        let volumes = monsters
            .iter()
            .filter_map(|monster| {
                let kind = cdda_sim::monster_registry::id_of(world, &monster.id)?;
                Some((kind, monster.volume))
            })
            .collect();
        cdda_sim::creature::set_corpse_kind(world, corpse_item, volumes);
    }
    let kinds: Vec<cdda_sim::monster_registry::MonsterKind<'_>> = monsters
        .iter()
        .map(|monster| cdda_sim::monster_registry::MonsterKind {
            id: &monster.id,
            hp: monster.hp,
        })
        .collect();
    cdda_sim::monster_registry::set_monster_types(world, &kinds);
    Some(monsters.len())
}

/// Every monster type the game's data defines.
fn monster_types() -> Option<Vec<cdda_content::monsters::MonsterType>> {
    // The monster loader reads the **reference's dump**, not the game's own JSON: the dump states one number per damage type (`armor_bash`, `armor_cut`, `armor_stab`) where the
    // game nests an `armor` object, and it names `melee_sides` where the game's JSON calls it `melee_dice_sides` (`monstergenerator.cpp:813`). Feeding the loader the game's own
    // files therefore gave every monster zero stab armour, zero sides and - once - a `2d0` blow. The dump is the shape this loader speaks, so the dump is what it is fed.
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/monsters/monsters.json");
    let bytes = std::fs::read(path).ok()?;
    let root = cdda_json::parse(&bytes).ok()?;
    let cdda_json::Value::Object(root) = root else {
        return None;
    };
    let Some(cdda_json::Value::Array(prototypes)) = root.get("prototypes") else {
        return None;
    };
    cdda_content::monsters::load_objects(prototypes).ok()
}

/// Renames the place, and checks what the place is called.
///
/// Kept apart from the match for the same reason the item steps are: two arms that only read and write one name would make the scenario interpreter longer without making it
/// clearer, and the scenario's own vocabulary is worth keeping to one screen.
fn take_place_step(world: &mut World, name: &str, outcome: &mut Outcome) {
    let Some(survivor) = survivor(world) else {
        outcome.failures.push("there is no survivor".to_owned());
        return;
    };
    let effect = cdda_content::dialogue::Effect::PlaceOverride {
        place: cdda_content::dialogue::StringSource::Literal(name.to_owned()),
        fallback: None,
        length: cdda_content::dialogue::EffectDuration::Turns(Some(3_600)),
        key: None,
    };
    cdda_sim::dialogue_effects::run_effects(world, survivor, None, &[effect]);
    outcome
        .lines
        .push(format!("place: renamed to {name} for an hour"));
}

/// Checks what the place is called.
///
/// Reference: `display::current_position_text` (`display.cpp:1243-1246`) answers the overriding name **while the event is pending** and the real name after it, which is why
/// this asks the timed event rather than a rename store of its own.
fn check_place_step(world: &World, name: &str, outcome: &mut Outcome) {
    let named = cdda_sim::timed_events::text_of(world, "OVERRIDE_PLACE");
    if named.as_deref() == Some(name) {
        outcome.lines.push(format!("place: {name}"));
    } else {
        outcome
            .failures
            .push(format!("expected the place to be called {name}, and it is {named:?}"));
    }
}

/// Installs the game's own vitamin types, so a body can tell a nutrient from a drug.
///
/// Reference: the `vitamin` factory reading its JSON at start-up (`src/vitamin.cpp`). Until this ran, a real game's body had no table: every level it held was of
/// unknown kind, so **every drug was silently absorbed as nothing** - the same quiet wrongness an uninstalled item registry had before round 764.
///
/// Answers how many types were installed, or [`None`] when the data is not where the binary expects it.
pub fn install_vitamin_data(world: &mut World) -> Option<usize> {
    let data = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../oracle/src/data/json");
    if !data.is_dir() {
        return None;
    }
    let vitamins = cdda_content::vitamins::load_paths(&[&data]).ok()?;
    cdda_sim::vitamin_registry::set_vitamin_types(world, &vitamins);
    Some(vitamins.len())
}

/// Installs the game's own effect types, so a rule that asks what an effect does can answer.
///
/// Reference: `effect_type`'s factory reading its JSON at start-up (`src/effect.cpp`). The store could answer a type by name before this (round 908) but nothing put the
/// game's own types in it, so a real world answered "no such effect" for every one the data defines.
///
/// Answers how many types were installed, or [`None`] when the data is not where the binary expects it.
pub fn install_effect_data(world: &mut World) -> Option<usize> {
    let data = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../oracle/src/data/json");
    if !data.is_dir() {
        return None;
    }
    let effects = cdda_content::effects::load_paths(&[&data]).ok()?;
    let kinds: Vec<cdda_content::effects::EffectType> = effects.into_values().collect();
    let count = kinds.len();
    cdda_sim::effect_registry::set_effect_types(world, &kinds);
    Some(count)
}

/// Installs the field names, and with them the numbers every field rule is keyed to.
///
/// The names come from `corpus/field_types.json` - the reference's `--dump-fields` output, which carries each field's own
/// `int_id` - so the store's idea of "which number is `fd_fire`" is the reference's answer rather than a constant someone
/// transcribed. Returns how many fields were installed.
pub fn install_field_data(world: &mut World) -> Option<usize> {
    let dump = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/field_types.json");
    let text = std::fs::read_to_string(dump).ok()?;
    let value = cdda_json::parse(text.as_bytes()).ok()?;
    let cdda_json::Value::Object(root) = &value else {
        return None;
    };
    let cdda_json::Value::Array(rows) = root.get("prototypes")? else {
        return None;
    };
    let mut kinds: Vec<(String, u32, i32)> = Vec::with_capacity(rows.len());
    for row in rows {
        let cdda_json::Value::Object(object) = row else {
            continue;
        };
        let Some(cdda_json::Value::Str(id)) = object.get("id") else {
            continue;
        };
        let Some(cdda_json::Value::Int(num)) = object.get("num") else {
            continue;
        };
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let number = *num as u32;
        // The type's level count is its maximum intensity (`field_type::get_max_intensity`), and it is what the runtime clamps to.
        let levels = match object.get("levels") {
            Some(cdda_json::Value::Array(levels)) => i32::try_from(levels.len()).unwrap_or(1),
            _ => 1,
        };
        kinds.push((id.clone(), number, levels));
    }
    let named: Vec<(&str, u32, i32)> = kinds
        .iter()
        .map(|(id, num, levels)| (id.as_str(), *num, *levels))
        .collect();
    cdda_sim::field_registry::set_field_names(world, &named);
    Some(named.len())
}

/// Makes a recipe the game's data describes, and says what came of it.
///
/// The recipes come from `corpus/recipes.json`, the reference's own `--dump-recipes` output, because the port's recipe
/// loader reads that dump rather than the raw JSON: the dump is what the crafts are measured against, so a playtest that
/// used a different reader would be testing a different rule.
///
/// The components are put in the avatar's hands first - a survivor who had nothing could not craft anything, and the
/// point of this is to watch the craft itself run: the recipe's own duration, the turns it takes, and what appears at
/// the end.
pub fn craft_a_recipe(
    world: &mut World,
    avatar: bevy_ecs::entity::Entity,
    recipe_id: &str,
    turns: i32,
) -> Option<String> {
    let dump = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/recipes.json");
    let text = std::fs::read_to_string(dump).ok()?;
    let value = cdda_json::parse(text.as_bytes()).ok()?;
    let (recipes, _) = cdda_content::recipes::from_dump(&value)?;
    let recipe = recipes.iter().find(|recipe| recipe.id == recipe_id)?.clone();

    // The QA surface teaches the recipe it is asked to make: `Character::can_make` refuses a recipe the crafter never learned (`crafting.cpp:602-616`), and a
    // scenario that hands over the components without the knowledge would be asking about bookkeeping rather than about making things.
    cdda_sim::recipes_known::learn(world, avatar, recipe_id);
    let names = cdda_sim::item_registry::snapshot(world);
    // A craft already under way is continued rather than begun again: a scenario that splits a long recipe across two
    // lines is asking about the same craft, and `Busy` would make the second line do nothing.
    let already_making = world.get::<cdda_sim::crafting::Making>(avatar).is_some();
    for group in recipe.components.iter().take(usize::from(!already_making)) {
        let Some(component) = group.first() else {
            continue;
        };
        let kind = names.id_of(&component.item)?;
        for _ in 0..component.count.max(1) {
            let item = cdda_sim::drop_item(world, (0, 0), kind, 1);
            world.entity_mut(item).remove::<cdda_sim::Position>();
            world
                .entity_mut(item)
                .insert(cdda_sim::creature::Carried(avatar));
        }
    }

    if !already_making {
        let started = cdda_sim::crafting::begin_craft_from_carried(world, avatar, &recipe, 1, &names);
        if started != cdda_sim::crafting::Craft::Started {
            return Some(format!("{recipe_id} could not be started: {started:?}"));
        }
    }
    let moves = cdda_sim::creature::moves_left(world, avatar);
    let mut spent = 0;
    while spent < turns && world.get::<cdda_sim::crafting::Making>(avatar).is_some() {
        cdda_sim::turn::run_turn(world);
        spent += 1;
    }
    let after = cdda_sim::item_registry::snapshot(world);
    let carried = cdda_sim::inventory::carried_by_id(world, avatar, |kind| after.name_of(kind));
    let made = carried
        .get(&recipe.result)
        .copied()
        .unwrap_or_else(|| ground_of(world, &recipe.result, &after));
    let _ = moves;
    Some(format!(
        "{} -> {} x{} after {spent} turn(s)",
        recipe.id, recipe.result, made
    ))
}

/// How many of a named thing are lying on the square under the avatar.
fn ground_of(world: &mut World, name: &str, names: &cdda_sim::item_registry::ItemNames) -> i32 {
    let Some(wanted) = names.id_of(name) else {
        return 0;
    };
    let mut query = world.query::<(&cdda_sim::Position, &cdda_sim::GroundItem)>();
    query
        .iter(world)
        .filter(|(_, item)| item.kind == wanted)
        .map(|(_, item)| item.charges.max(1))
        .sum()
}

/// Plays one `throw` step: hands what the survivor carries to the throwing rules and reports where it stopped.
fn throw_a_step(
    world: &mut World,
    outcome: &mut Outcome,
    item: &str,
    x: i32,
    y: i32,
) {
    let Some(survivor) = survivor(world) else {
        outcome.failures.push("there is no survivor".to_owned());
        return;
    };
    let names = cdda_sim::item_registry::snapshot(world);
    let Some(held) = carried_item_of(world, survivor, item, &names) else {
        outcome
            .failures
            .push(format!("nothing called {item} is in hand to throw"));
        return;
    };
    // What the landing is worth as a noise is part of what the step reports: a thing under half a kilo lands in silence, and a scenario that expects a monster to come
    // needs to see that it never had a sound to come to.
    let weight = cdda_sim::item_registry::snapshot(world)
        .id_of(item)
        .map_or(0, |kind| {
            cdda_sim::item_registry::snapshot(world).weight_of(kind)
        });
    match cdda_sim::throwing::throw_at(world, survivor, held, (x, y, 0)) {
        cdda_sim::throwing::Toss::Landed { at, missed_by, hit } => outcome.lines.push(format!(
            "throw: {item} ({weight} g) landed at ({}, {}, {}) missing by {missed_by:.2}{}",
            at.0,
            at.1,
            at.2,
            match hit {
                Some((_, hurt)) => format!(" and did {hurt} damage"),
                None => String::new(),
            }
        )),
        other => outcome
            .failures
            .push(format!("the throw of {item} did not happen: {other:?}")),
    }
}

/// Plays the `data` step: the tables a game needs before anything asks a question of them.
///
/// What a vitamin is and what an effect does are both the reference's own JSON, read the way its factories read them at start-up.
fn install_the_tables(world: &mut World, outcome: &mut Outcome) {
    let mut parts: Vec<String> = Vec::new();
    match install_vitamin_data(world) {
        Some(count) => parts.push(format!("{count} vitamins")),
        None => outcome
            .failures
            .push("the game's vitamin data is not where the scenario expects it".to_owned()),
    }
    match install_effect_data(world) {
        Some(count) => parts.push(format!("{count} effects")),
        None => outcome
            .failures
            .push("the game's effect data is not where the scenario expects it".to_owned()),
    }
    if !parts.is_empty() {
        outcome.lines.push(format!("data: {}", parts.join(", ")));
    }
}

/// Plays one of the two steps of a fight: taking a thing in hand, and swinging it.
fn take_a_swing(world: &mut World, step: &Step, outcome: &mut Outcome) {
    match step {
        Step::Wield { item } => take_up(world, outcome, item),
        Step::Attack { x, y } => swing_at(world, outcome, *x, *y),
        _ => {}
    }
}

/// Plays one `wield` step: the survivor takes a thing in its hands, which is what its blow is built from.
fn take_up(world: &mut World, outcome: &mut Outcome, item: &str) {
    let names = cdda_sim::item_registry::snapshot(world);
    let Some(kind) = names.id_of(item) else {
        outcome.failures.push(format!("no item called {item}"));
        return;
    };
    let Some(survivor) = survivor(world) else {
        outcome.failures.push("there is no survivor to hold anything".to_owned());
        return;
    };
    world
        .entity_mut(survivor)
        .insert(cdda_sim::creature::Wielding(kind));
    outcome
        .lines
        .push(format!("wield: the survivor holds {item}"));
}

/// Plays one `goal` step: every monster is told where to go, which is how a playtest starts a fight the survivor did not.
fn send_the_monsters_to(outcome: &mut Outcome, world: &mut World, x: i32, y: i32) {
    let mut told = 0;
    for entity in cdda_sim::creature::creatures(world) {
        world
            .entity_mut(entity)
            .insert(cdda_sim::creature::Goal { square: (x, y, 0) });
        told += 1;
    }
    outcome
        .lines
        .push(format!("goal: {told} creature(s) sent to ({x}, {y})"));
}

/// Plays one `attack` step: the survivor swings what it holds at whatever stands on a square.
///
/// The blow is `creature::swing_what_the_character_holds`, the same rule a monster's swing goes through. A square with nothing living on it is a **failure** rather than a
/// no-op: a playtest that swung at air and passed would be a playtest that never fought.
fn swing_at(world: &mut World, outcome: &mut Outcome, x: i32, y: i32) {
    let Some(survivor) = survivor(world) else {
        outcome.failures.push("there is no survivor to swing".to_owned());
        return;
    };
    let victim = cdda_sim::creature::creature_at(world, (x, y, 0))
        .into_iter()
        .find(|entity| *entity != survivor);
    let Some(victim) = victim else {
        outcome
            .failures
            .push(format!("nothing living stands at ({x}, {y}) to hit"));
        return;
    };
    let before = cdda_sim::creature::health_of(world, victim).unwrap_or(0);
    match cdda_sim::creature::swing_what_the_character_holds(world, survivor, victim) {
        None => outcome
            .failures
            .push("the survivor could not swing at all".to_owned()),
        Some(dealt) => {
            let after = cdda_sim::creature::health_of(world, victim).unwrap_or(0);
            outcome.lines.push(format!(
                "attack: {dealt} damage at ({x}, {y}), {before} hit points became {after}"
            ));
        }
    }
}

/// Plays one `monster` step: spawns a monster of the named type and wakes it.
///
/// The type comes from the monster data the scenario loaded, and a name the data does not have is a **failure** rather than a skip: a scenario that quietly spawned nothing
/// would pass every later step by having nothing to fail.
fn put_a_monster(world: &mut World, outcome: &mut Outcome, id: &str, x: i32, y: i32) {
    let Some(kind) = cdda_sim::monster_registry::id_of(world, id) else {
        outcome
            .failures
            .push(format!("the data has no monster called `{id}`"));
        return;
    };
    // The type, not only the registry's two numbers: a monster that arrives without its type's flags is deaf and blind, and every rule that asks what it can do then
    // answers "nothing". The data is read again here because a scenario's steps are independent - the file is a few megabytes and the step runs once.
    let Some(types) = monster_types() else {
        outcome
            .failures
            .push("the game's monster data is not where the scenario expects it".to_owned());
        return;
    };
    let Some(monster_type) = types.iter().find(|monster| monster.id == id) else {
        outcome
            .failures
            .push(format!("the data has no monster called `{id}`"));
        return;
    };
    let monster = cdda_sim::creature::spawn_monster_of_type(world, kind, monster_type, (x, y, 0), 100);
    // A monster a scenario places is one the world will give a turn to, so it is woken the same way a type-spawned one is.
    cdda_sim::creature::set_moves(world, monster, 100);
    let flags = world
        .get::<cdda_sim::creature_flags::CreatureFlags>(monster)
        .map_or(0, |carried| carried.0.len());
    let dice = world
        .get::<cdda_sim::creature::MeleeAttack>(monster)
        .map_or((0, 0, 0), |attack| (attack.0.dice, attack.0.sides, attack.0.accuracy_tenths));
    outcome.lines.push(format!(
        "monster: {id} at ({x}, {y}) with {} hit points, {}d{} at {} tenths and {flags} flags{}",
        monster_type.hp,
        dice.0,
        dice.1,
        dice.2,
        if cdda_sim::creature_flags::has_flag(world, monster, "HEARS") {
            " (it hears)"
        } else {
            " (it is deaf, which is what its type says)"
        }
    ));
}

/// Plays one `expect hurt` step: the survivor must have lost at least this many hit points.
fn expect_a_wound(world: &mut World, outcome: &mut Outcome, points: i32) {
    let Some(survivor) = survivor(world) else {
        outcome.failures.push("there is no survivor to be hurt".to_owned());
        return;
    };
    let Some(health) = world.get::<cdda_sim::creature::Health>(survivor).copied() else {
        outcome.failures.push("the survivor has no hit points".to_owned());
        return;
    };
    let (hp, most) = (health.hp, health.max_hp);
    if hp > most - points {
        outcome.failures.push(format!(
            "the survivor has {hp} of {most} hit points, so fewer than {points} were lost"
        ));
    } else {
        outcome
            .lines
            .push(format!("hurt: the survivor is down to {hp} of {most}"));
    }
}

/// Plays one `expect monster within` step: some creature besides the survivor is this many squares away or fewer.
///
/// This is the step a scenario needs to say **a noise brought something**: the survivor stands still, and after the sound the world has a monster standing near it that was
/// not there before. It counts Chebyshev squares, which is the distance a sound and a walk both use.
fn expect_a_monster_close(world: &mut World, outcome: &mut Outcome, squares: i32) {
    let Some(survivor) = survivor(world) else {
        outcome.failures.push("there is no survivor to stand near".to_owned());
        return;
    };
    let Some(here) = world.get::<cdda_sim::Position>(survivor).map(|place| (place.x, place.y)) else {
        outcome.failures.push("the survivor is nowhere".to_owned());
        return;
    };
    let mut closest: Option<(i32, (i32, i32))> = None;
    // **Creatures only**: an item lying nearby is not a monster arriving, and a check that counted it would pass on the thrown rock itself.
    let mut query = world.query_filtered::<(Entity, &cdda_sim::Position), bevy_ecs::prelude::With<cdda_sim::creature::Creature>>();
    for (entity, place) in query.iter(world) {
        if entity == survivor {
            continue;
        }
        let distance = (place.x - here.0).abs().max((place.y - here.1).abs());
        if closest.is_none_or(|(best, _)| distance < best) {
            closest = Some((distance, (place.x, place.y)));
        }
    }
    match closest {
        Some((distance, at)) if distance <= squares => outcome.lines.push(format!(
            "close: something stands {distance} squares away at ({}, {})",
            at.0, at.1
        )),
        Some((distance, at)) => outcome.failures.push(format!(
            "the nearest thing is {distance} squares away at ({}, {}), not within {squares}",
            at.0, at.1
        )),
        None => outcome
            .failures
            .push("nothing else is in the world at all".to_owned()),
    }
}

/// Plays one `expect no monster at` step: nothing living stands on this square.
///
/// This is how a scenario says a noise **moved** something without pretending to know where it went: a monster that hears a sound walks to a guess up to ten squares out, so
/// "it is no longer where it was" is the only honest claim a playtest can make about an imprecise noise.
fn expect_no_monster_at(world: &mut World, outcome: &mut Outcome, x: i32, y: i32) {
    let player = survivor(world);
    let mut query = world.query_filtered::<(Entity, &cdda_sim::Position), bevy_ecs::prelude::With<cdda_sim::creature::Creature>>();
    for (entity, place) in query.iter(world) {
        if (place.x, place.y) == (x, y) && player != Some(entity) {
            outcome.failures.push(format!(
                "something is still standing at ({x}, {y}), and the scenario said there would be nothing"
            ));
            return;
        }
    }
    outcome
        .lines
        .push(format!("empty: nothing stands at ({x}, {y}) any more"));
}

/// Plays one `shot` step: draws the view around the survivor and writes it as a PNG.
///
/// The capture is the port's own frame, drawn one cell per square - see [`crate::capture`]. Writing it from a scenario is what makes a **screen** checkable: the same world
/// always produces the same bytes, so a recorded picture is a gate on everything the view reads.
fn take_a_shot(world: &mut World, outcome: &mut Outcome, path: &str) {
    let Some(survivor) = survivor(world) else {
        outcome.failures.push("there is no survivor to draw around".to_owned());
        return;
    };
    let at = world
        .get::<cdda_sim::Position>(survivor)
        .map_or((0, 0), |place| (place.x, place.y));
    let capture = crate::capture::draw(world, at, 15);
    // A scenario names where its picture goes, and a directory that does not exist yet is the scenario's business rather than an error: `target/screens` is created by
    // the first scenario that draws into it.
    if let Some(parent) = std::path::Path::new(path).parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    match std::fs::write(path, capture.image.png()) {
        Ok(()) => outcome.lines.push(format!(
            "shot: {path}, {} squares across, {} of them lit",
            capture.squares_across, capture.lit_squares
        )),
        Err(error) => outcome
            .failures
            .push(format!("cannot write `{path}`: {error}")),
    }
}

/// Plays one `expect item ... at` step: exactly one of that item must lie on that square.
fn expect_one_on(world: &mut World, outcome: &mut Outcome, item: &str, x: i32, y: i32) {
    let names = cdda_sim::item_registry::snapshot(world);
    let here = lying_at(world, x, y, item, &names);
    if here != 1 {
        outcome
            .failures
            .push(format!("expected one {item} on ({x}, {y}), found {here}"));
    }
}

/// How many of an item lie on exactly this square.
///
/// A thrown item is found by the square it stopped on, which is what `expect item <name> at <x> <y>` reads: a port that moved the item somewhere plausible but not where
/// the rule said would pass a "is it on the floor" check and fail this one.
fn lying_at(
    world: &mut World,
    x: i32,
    y: i32,
    name: &str,
    names: &cdda_sim::item_registry::ItemNames,
) -> i32 {
    let Some(wanted) = names.id_of(name) else {
        return 0;
    };
    let mut query = world.query::<(&cdda_sim::Position, &cdda_sim::GroundItem)>();
    query
        .iter(world)
        .filter(|(place, item)| item.kind == wanted && place.x == x && place.y == y)
        .map(|(_, item)| item.charges.max(1))
        .sum()
}

/// The one item of this kind the survivor is holding, if it is holding one.
fn carried_item_of(
    world: &mut World,
    survivor: bevy_ecs::entity::Entity,
    name: &str,
    names: &cdda_sim::item_registry::ItemNames,
) -> Option<bevy_ecs::entity::Entity> {
    let wanted = names.id_of(name)?;
    let mut query = world.query::<(
        bevy_ecs::entity::Entity,
        &cdda_sim::creature::Carried,
        &cdda_sim::GroundItem,
    )>();
    query
        .iter(world)
        .find(|(_, who, item)| who.0 == survivor && item.kind == wanted)
        .map(|(entity, _, _)| entity)
}

/// Puts a garment on the avatar, so the armour rules have something to work on.
///
/// Returns a line describing what was worn, or nothing when the game data is not where the binary expects it.
pub fn dress_the_avatar(world: &mut World, avatar: bevy_ecs::entity::Entity) -> Option<String> {
    let data = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../oracle/src/data/json");
    if !data.is_dir() {
        return None;
    }
    let materials = cdda_content::materials::load_paths(&[&data]).ok()?;
    let items = cdda_content::items::load_paths(&[&data]).ok()?;

    // The garment that covers the most of a body, chosen by id to break ties: a playtest that dressed itself
    // differently each time would be no playtest at all, and "the first item with any torso armour" is a canteen.
    let covering = |item: &cdda_content::items::ItemType| -> std::collections::BTreeSet<String> {
        item.armor_parts
            .iter()
            .filter(|entry| !cdda_content::armour::layers_on(item, &materials, "torso", false).is_empty()
                || entry.thickness > 0.0)
            .flat_map(|entry| entry.covers.clone())
            .collect()
    };
    let candidates: Vec<(std::collections::BTreeSet<String>, &cdda_content::items::ItemType)> = items
        .iter()
        .filter(|item| !cdda_content::armour::layers_on(item, &materials, "torso", false).is_empty())
        .map(|item| (covering(item), item))
        .collect();
    // Among the garments that cover a torso **and** an arm - which rules out a canteen on a strap - take the least
    // protective one: a survivor starts dressed, not armoured. Ties go by id so two runs pick the same thing, and a
    // playtest that dressed itself differently each time would be no playtest at all.
    let protection = |item: &cdda_content::items::ItemType, part: &str| -> f64 {
        cdda_content::armour::resist_of(
            &cdda_content::armour::layers_on(item, &materials, part, false),
            0,
        )
    };
    let mut shirts: Vec<&cdda_content::items::ItemType> = candidates
        .into_iter()
        .filter(|(parts, _)| parts.contains("torso") && (parts.contains("arm_l") || parts.contains("arm_r")))
        .map(|(_, item)| item)
        // A garment whose materials add up to nothing protects nothing, whatever its data claims it covers;
        // wearing it would make the avatar look dressed and be bare.
        .filter(|item| protection(item, "torso") > 0.0)
        .collect();
    // The one closest to a jacket's weight - five points. This is a **demo choice, not a reference rule**: the
    // reference gives a character the clothes its profession names, and no profession has been ported. The least
    // protective garment rounds to nothing and the most protective is heavy plate, so a band in the middle is what
    // leaves a survivor who can be hit and survive it. Ties go by id.
    let weight = |item: &cdda_content::items::ItemType| (protection(item, "torso") - 5.0).abs();
    shirts.sort_by(|left, right| {
        weight(left)
            .partial_cmp(&weight(right))
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| left.id.cmp(&right.id))
    });
    let shirt = shirts.first()?;

    let mut parts: Vec<String> = shirt
        .armor_parts
        .iter()
        .flat_map(|entry| entry.covers.clone())
        .collect();
    parts.sort();
    parts.dedup();
    let mut layers = Vec::new();
    for part in parts {
        for layer in cdda_content::armour::layers_on(shirt, &materials, &part, false) {
            layers.push((part.clone(), layer));
        }
    }
    if layers.is_empty() {
        return None;
    }
    world.spawn((
        cdda_sim::creature::WornBy(avatar),
        cdda_sim::creature::Garment {
            layers: layers.clone(),
        },
        cdda_sim::creature::Toughness::of(&shirt.flags),
    ));
    cdda_sim::creature::dress(world);
    let torso = world
        .get::<cdda_sim::creature::PartArmour>(avatar)
        .and_then(|armour| armour.0.get("torso").copied())
        .unwrap_or(0);
    Some(format!("{} — {torso} point(s) on the torso", shirt.id))
}

/// Finds a spell in the game's dump by id.
///
/// The dump is the oracle's own answer for every `spell_type`, so the game casts the spell the reference would, not a
/// copy of it written out by hand.
#[must_use]
pub fn spell_named(id: &str) -> Option<cdda_content::spells::Spell> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/spells.json");
    let text = std::fs::read_to_string(path).ok()?;
    let value = cdda_json::parse(text.as_bytes()).ok()?;
    let all = cdda_content::spells::load_dump(&value).ok()?;
    all.into_iter().find(|spell| spell.id == id)
}

/// Casts a spell from the avatar at the nearest creature, and says what happened in a line.
///
/// This is the QA surface's side of P5: the rule lives in `cdda_sim::spellcasting`, and this only has to find a
/// target and report the answer. Returns nothing when there is no such spell or nothing in reach - a cast that
/// silently did nothing would be the hardest kind of bug to see in a playthrough.
pub fn cast_at_nearest(
    world: &mut World,
    avatar: bevy_ecs::entity::Entity,
    spell_id: &str,
    level: i32,
) -> Option<String> {
    let spell = spell_named(spell_id)?;
    let caster = world
        .get::<cdda_sim::Position>(avatar)
        .map(|place| (place.x, place.y, place.z))?;
    let mut nearest: Option<(i64, bevy_ecs::entity::Entity)> = None;
    // Creatures only: every square, wall and dropped rock carries a `Position` too, and the nearest of those would
    // be the floor the avatar is standing on rather than anything worth casting at.
    let mut query = world
        .query_filtered::<(bevy_ecs::entity::Entity, &cdda_sim::Position), With<cdda_sim::creature::Creature>>();
    let mut found = Vec::new();
    for (entity, place) in query.iter(world) {
        if entity == avatar {
            continue;
        }
        found.push((entity, place.x, place.y));
    }
    for (entity, x, y) in found {
        let away = i64::from((x - caster.0).abs().max((y - caster.1).abs()));
        if nearest.is_none_or(|(best, _)| away < best) {
            nearest = Some((away, entity));
        }
    }
    let (_, target) = nearest?;
    // Drawn from the store's own generator, so a playtest seeded the same way casts the same fight.
    let outcome = cdda_sim::spellcasting::cast_at(world, caster, &spell, level, target);
    Some(match outcome {
        cdda_sim::spellcasting::Cast::Hit { part, dealt } => {
            format!("{} hits the {part} for {dealt}", spell.name)
        }
        cdda_sim::spellcasting::Cast::NotAnAttack => {
            format!("{} is not an attack spell", spell.name)
        }
        cdda_sim::spellcasting::Cast::OutOfRange => {
            format!("{} does not reach", spell.name)
        }
        cdda_sim::spellcasting::Cast::NothingToHit => {
            format!("{} has nothing to hit", spell.name)
        }
    })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::panic, clippy::expect_used)]
    use super::*;

    /// A burst step fires several shots, and each one costs moves and is aimed worse than the one before.
    ///
    /// Reference: `ranged.cpp:1130-1240`. The gate plays the step through the scenario machinery rather than calling the sim directly, because the claim is about **the step**: a burst is a
    /// line in a scenario, it spends the survivor's moves shot by shot, and its report lists what each shot did.
    #[test]
    fn a_burst_step_fires_every_shot_and_charges_for_each() {
        let outcome = run(
            "world 12 bare\nsurvivor 0 0\nitems\nmonsters\ndata\nseed 31\nmonster mon_zombie 6 0\nwield ruger_mini\nburst ruger_mini at 6 0 3 3\n",
        )
        .expect("the scenario parses and runs");
        assert!(outcome.failures.is_empty(), "the scenario's own claims held: {:?}", outcome.failures);
        let fired: Vec<&String> = outcome.lines.iter().filter(|line| line.starts_with("burst:")).collect();
        assert_eq!(fired.len(), 1, "one burst report: {fired:?}");
        assert!(fired.first().is_some_and(|line| line.contains("fired 3 of 3")), "and it fired three: {fired:?}");
        let shots: Vec<&String> = outcome.lines.iter().filter(|line| line.trim_start().starts_with("shot:")).collect();
        assert_eq!(shots.len(), 3, "with three shot lines under it: {shots:?}");
        // The dispersion climbs with the recoil, so the misses are not all the same number.
        let misses: Vec<f64> = shots
            .iter()
            .filter_map(|line| line.split("missed by ").nth(1).and_then(|rest| rest.split(',').next()).and_then(|text| text.parse().ok()))
            .collect();
        assert_eq!(misses.len(), 3, "three parsed misses: {misses:?}");
        // **The `seed` line is what puts dice in the world**: without it `take_rolls` answers `None` and every roll is zero, so three shots would report the same 0.000 miss. The
        // first version of this gate did exactly that and the identical misses were the evidence.
        assert!(misses.iter().any(|missed| *missed > 0.0), "the seed line put dice in the world: {misses:?}");
        assert!(
            misses.windows(2).any(|pair| matches!(pair, [a, b] if (a - b).abs() > f64::EPSILON)),
            "and the shots are not identical: {misses:?}"
        );
    }

    /// A gun that names an ammunition type which is not a round id still finds its round.
    ///
    /// The two namespaces are the reference's own (`gun_type::ammo` against `islot_ammo::type`), and the `shoot` step assumed they agreed until a shotgun was tried: a pipe shotgun's `ammo`
    /// is `"shot"`, and the rounds are `shot_bird` and `shot_00`. **This is the gate for the second lookup**, and it is a differential: `223` resolves by id, `shot` by type.
    #[test]
    fn a_gun_that_names_an_ammunition_type_finds_a_round_of_that_type() {
        let read = |name: &str| {
            let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("../../corpus/{name}"));
            let bytes = std::fs::read(path).expect("the corpus file exists");
            cdda_json::parse(&bytes).expect("valid JSON")
        };
        let rounds = cdda_content::ammo::load_dump(&read("items/ammo.json")).expect("the ammo loads");
        let guns = cdda_content::guns::load_dump(&read("items/guns.json")).expect("the guns load");

        let gun = |id: &str| guns.iter().find(|gun| gun.id == id).expect("the gun is in the dump").clone();
        let by_id = gun("ruger_mini").ammo;
        assert_eq!(by_id, "223", "a rifle names a type that is also a round's id");
        assert!(rounds.iter().any(|round| round.id == by_id), "so the id lookup finds it");

        let by_type = gun("pipe_shotgun").ammo;
        assert_eq!(by_type, "shot", "and a shotgun names a type that is no round's id");
        assert!(
            !rounds.iter().any(|round| round.id == by_type),
            "which is what makes the second lookup necessary rather than decorative"
        );
        let found: Vec<&str> = rounds
            .iter()
            .filter(|round| round.ammo_type == by_type)
            .map(|round| round.id.as_str())
            .collect();
        assert!(found.len() > 1, "and there are several rounds of it: {found:?}");
        assert!(found.contains(&"shot_bird"), "birdshot among them: {found:?}");
        assert!(
            round_of_type(&rounds, &by_type).is_some_and(|round| round.ammo_type == by_type),
            "and the resolver the step uses finds one"
        );
    }

    /// Every table the scenarios install is **non-empty**, which is the one check that catches a loader fed the wrong shape.
    ///
    /// Round 1100's defect was a file name: the monster loader was handed the game's own JSON, which names dice faces `melee_dice_sides`, so every monster it loaded had **zero
    /// sides** and the world looked populated. A count of kinds says nothing about that; what says something is asking each installed table for a **specific field** the wrong shape
    /// would leave empty. Three fields, one per loader, chosen because each is the field the wrong shape loses.
    #[test]
    fn every_table_a_scenario_installs_is_filled_from_the_source_its_loader_speaks() {
        let mut world = World::new();
        let items = install_item_data(&mut world).expect("the item data is in the repository");
        assert!(items > 9_000, "every item type: {items}");

        let monsters = install_monster_data(&mut world).expect("the monster data is in the repository");
        assert!(monsters > 1_000, "every monster prototype: {monsters}");

        let Some(types) = monster_types() else {
            panic!("the monster data is in the repository")
        };
        let zombie = types
            .iter()
            .find(|monster| monster.id == "mon_zombie")
            .expect("the data has a zombie");
        // The three fields the wrong shape loses: dice faces, stab armour and the bashing armour that lives in a nested object in the game's JSON.
        assert_eq!(zombie.melee_sides, 3, "a zombie's dice have faces");
        assert!(zombie.melee_dice > 0, "and there is more than none of them");
        assert!(
            types.iter().any(|monster| monster.armor_stab > 0),
            "and some monster resists stabbing at all"
        );
        assert!(
            types.iter().any(|monster| monster.armor_bash > 0),
            "and some monster resists bashing, which the game's JSON nests in an object"
        );
    }

    #[test]
    fn the_avatar_can_be_dressed_from_the_game_s_data() {
        let mut world = World::new();
        let avatar = cdda_sim::spawn_player(&mut world, (0, 0), 80, 0);
        let worn = dress_the_avatar(&mut world, avatar).expect("the game data is in the repository");
        assert!(worn.contains("point"), "the line says what it is worth: {worn}");
        let torso = cdda_sim::creature::armour_on(&world, avatar, "torso");
        assert!(torso >= 1, "a garment that protects nothing is not a garment: {worn}");
        assert_eq!(
            cdda_sim::creature::armour_on(&world, avatar, "foot_l"),
            0,
            "and it says nothing about a part it does not cover"
        );
    }

    #[test]
    fn the_avatar_can_cast_the_taser_from_the_game_s_data_at_what_is_next_to_it() {
        let mut world = World::new();
        let avatar = cdda_sim::spawn_player(&mut world, (0, 0), 80, 0);
        // A cast draws where it lands from the store's generator, so a world without one cannot cast - which is the
        // rule, not an accident: a cast that invented its own randomness could not be replayed.
        cdda_sim::turn::set_random(&mut world, cdda_core_types::rng::Rolls::new(3));
        let torso = cdda_sim::hit_location::BodyPart {
            name: "torso".to_owned(),
            hit_size: 36.0,
            hit_difficulty: 1.0,
            upper: false,
            lower: false,
        };
        let zombie = cdda_sim::spawn_monster(&mut world, (1, 0), cdda_sim::MonsterKindId(0), 30);
        world
            .entity_mut(zombie)
            .insert(cdda_sim::creature::Body { parts: vec![torso] });

        let said = cast_at_nearest(&mut world, avatar, "bio_taser_spell", 0)
            .expect("the dump has the taser and the zombie is next to the avatar");

        assert!(said.contains("torso"), "the line names where it landed: {said}");
        assert_eq!(
            cdda_sim::creature::health_of(&world, zombie),
            Some(15),
            "fifteen off thirty, because the zombie brought no armour"
        );
    }

    #[test]
    fn a_scenario_that_says_what_it_means_runs_and_passes() {
        let outcome = run(
            "world 12 flammable\n\
             seed 7\n\
             fire 6 6 2 500\n\
             turn 3\n\
             expect changed 1\n\
             expect fire 6 6\n",
        )
        .expect("the scenario parses");

        assert!(outcome.passed(), "the assertions held: {:?}", outcome.failures);
        assert!(
            outcome.lines.iter().any(|line| line.contains("1 square(s) changed")),
            "the run reports what happened: {:?}",
            outcome.lines
        );
    }

    #[test]
    fn an_assertion_that_does_not_hold_is_named() {
        let outcome = run("world 12 bare\nfire 6 6 2 500\nturn 1\nexpect no fire 6 6\n")
            .expect("the scenario parses");

        assert!(!outcome.passed(), "the fire is still there after one turn");
        assert_eq!(
            outcome.failures,
            vec!["expected no fire at (6, 6)".to_string()],
            "and the failure says which assertion failed"
        );
    }

    #[test]
    fn every_scenario_in_the_repository_passes() {
        // The directory is the gate: a scenario committed here is one that runs, so a behaviour change
        // that breaks a playtest shows up as a failing test and not as a file nobody executed.
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../scenarios");
        let mut ran = 0;
        let mut failures = Vec::new();
        let entries = std::fs::read_dir(&dir).expect("the scenarios directory");
        for entry in entries {
            let entry = entry.expect("a directory entry");
            let path = entry.path();
            if path.extension().and_then(|kind| kind.to_str()) != Some("txt") {
                continue;
            }
            let text = std::fs::read_to_string(&path).expect("a scenario");
            match run(&text) {
                Ok(outcome) if outcome.passed() => ran += 1,
                Ok(outcome) => failures.push(format!("{}: {:?}", path.display(), outcome.failures)),
                Err(error) => failures.push(format!("{}: {error}", path.display())),
            }
        }
        assert!(ran >= 2, "the gate ran {ran} scenario(s), which is not enough to say anything");
        assert!(failures.is_empty(), "scenarios failed:\n{}", failures.join("\n"));
    }

    #[test]
    fn a_line_nobody_understands_stops_the_run() {
        let error = parse("world 12 flammable\ndance\n").expect_err("dance is not a step");

        assert!(
            error.contains("line 2") && error.contains("dance"),
            "the error names the line and the word: {error}"
        );
    }
}

#[cfg(test)]
mod item_data_tests {
    #![allow(clippy::panic, clippy::expect_used)]
    use bevy_ecs::prelude::*;

    /// A playtest world knows what things are, because it loaded the game's own data.
    ///
    /// Without this the survivor could carry a hundred anvils: every item-layer rule asks the registry, and an empty
    /// registry answers "weightless, takes no room, holds nothing" for everything, which looks like working code.
    #[test]
    fn the_binary_s_world_knows_the_game_s_items() {
        let mut world = World::new();
        let kinds = super::install_item_data(&mut world).expect("the game data is in the repository");
        assert!(kinds > 9_000, "installed {kinds} kinds, which is too few");
        assert_eq!(
            cdda_sim::item_registry::count(&mut world),
            kinds,
            "and the store knows every one of them"
        );
        // A known item, by the name the data uses, weighs something and takes room - which is the whole point.
        let two_by_four = cdda_sim::item_registry::id_of(&mut world, "2x4")
            .expect("the data has a two by four");
        assert!(
            cdda_sim::item_registry::weight_of(&mut world, two_by_four) > 0,
            "a two by four weighs something"
        );
        assert!(
            cdda_sim::item_registry::volume_of(&mut world, two_by_four) > 0,
            "and takes room"
        );
    }
}

#[cfg(test)]
mod craft_tests {
    #![allow(clippy::panic, clippy::expect_used)]
    use bevy_ecs::prelude::*;

    /// A survivor with the ingredients can make a thing the game's data describes, and the turns it takes are the
    /// recipe's own.
    ///
    /// `cordage_36_leather` is six strands into one cord and takes 30,000 moves, which is 300 turns at a hundred a turn - so this also proves the craft is not
    /// finishing early. **The recipe is chosen because it needs no tool and no quality**: the CLI hands a crafter the components and nothing else, and a craft
    /// that refuses for a missing tool is the right answer rather than a bug (see `crafting::begin_craft`).
    #[test]
    fn a_real_recipe_runs_to_its_result() {
        let mut world = World::new();
        cdda_sim::turn::set_random(&mut world, cdda_core_types::rng::Rolls::new(1));
        super::install_item_data(&mut world).expect("the game data is in the repository");
        let avatar = cdda_sim::spawn_player(&mut world, (0, 0), 80, 0);
        cdda_sim::creature::set_speed(&mut world, avatar, 100, 0);

        let line = super::craft_a_recipe(&mut world, avatar, "cordage_36_leather", 400)
            .expect("the data has the cord recipe");

        assert!(
            line.starts_with("cordage_36_leather -> cordage_36_leather"),
            "unexpected line: {line}"
        );
        assert!(
            line.contains("x1"),
            "one cord came out of it, not nothing: {line}"
        );
        let turns: i32 = line
            .rsplit("after ")
            .next()
            .and_then(|rest| rest.split_whitespace().next())
            .and_then(|count| count.parse().ok())
            .expect("the line states how many turns it took");
        assert_eq!(turns, 300, "30,000 moves is 300 turns at a hundred a turn");
    }
}

/// Installs the data's emissions, so an `emit` spell has a table to look its `effect_str` up in.
///
/// The same shape as [`install_field_data`] and the same reason: an effect that reads a table the tests build by hand is right
/// in the tests and empty in the game, which is a difference no behaviour test can see.
pub fn install_emit_data(world: &mut World) -> Option<usize> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../oracle/src/data/json/emit.json");
    let text = std::fs::read_to_string(path).ok()?;
    let value = cdda_json::parse(text.as_bytes()).ok()?;
    let cdda_json::Value::Array(rows) = &value else {
        return None;
    };
    let mut emits: Vec<(String, Vec<cdda_content::emit::Emission>)> = Vec::new();
    for row in rows {
        let Some(emission) = cdda_content::emit::read_emit(row) else {
            continue;
        };
        let cdda_json::Value::Object(object) = row else {
            continue;
        };
        let Some(cdda_json::Value::Str(id)) = object.get("id") else {
            continue;
        };
        match emits.iter_mut().find(|(known, _)| known == id) {
            Some((_, emissions)) => emissions.push(emission),
            None => emits.push((id.clone(), vec![emission])),
        }
    }
    cdda_sim::emit_registry::set_emits(world, &emits);
    Some(emits.len())
}

#[cfg(test)]
#[allow(clippy::panic)]
mod field_data_tests {
    use super::*;
    use bevy_ecs::prelude::World;
    use cdda_core_types::field::FieldId;

    /// The emissions the data defines are installed, and each keeps every row that names it.
    ///
    /// The same gap the field numbers had: the emit table was built by hand in tests, so every `emit` spelling worked in the
    /// test and none in the game. This is the wiring that makes the difference visible.
    #[allow(clippy::panic)]
    #[test]
    fn the_installed_emissions_come_from_the_data() {
        let mut world = World::new();
        let Some(count) = install_emit_data(&mut world) else {
            panic!("the emit data is in the repository");
        };
        // The data defines 61 emission ids, and 48 of them are emissions of **things** - they carry a `qty`, which the port
        // refuses rather than placing one of something where the data says three. So the field emissions the port can run are
        // 13, and the number is the honest size of the `emit` machine as ported.
        assert_eq!(count, 61, "the data defines 61 emission ids, all of them installed");
        let Some(gas) = cdda_sim::emit_registry::emissions_of(&world, "emit_toxic_leak") else {
            panic!("the toxic leak emission is defined");
        };
        assert!(!gas.is_empty(), "and it puts something down");
        // The six `emit` spells name ids that carry a `qty`, which the reference spreads with `map::propagate_field`
        // (`map_field.cpp:2212-2224`); those are installed too, with their quantity kept, and spread by the effect.
        let Some(blast) = cdda_sim::emit_registry::emissions_of(&world, "emit_toxic_blast") else {
            panic!("the toxic blast emission is installed");
        };
        assert!(
            blast.iter().any(|emission| emission.qty > 1),
            "and it carries a quantity, which is what makes a blast a cloud"
        );
        assert!(
            cdda_sim::emit_registry::emissions_of(&world, "emit_nothing_at_all").is_none(),
            "an id nobody defines is absent rather than empty"
        );
    }

    /// The store's field numbers are the reference's, read from its dump rather than transcribed.
    ///
    /// The port kept fourteen field constants and, until `corpus/field_types.json` carried each field's own `int_id`, nothing
    /// could check them. One of them was wrong for two rounds and nothing failed, because a rule keyed to a field that is
    /// never placed on a map has nothing to do. This installs the numbers the reference states and asks the store what it
    /// believes - the constants have to agree with the data.
    #[test]
    fn the_installed_field_names_carry_the_reference_numbers() {
        let mut world = World::new();
        let Some(count) = install_field_data(&mut world) else {
            panic!("the field dump is present");
        };
        assert_eq!(count, 145, "the reference has 145 field types");

        // The level counts come from the dump, not from a constant: `fd_electricity` carries ten intensity levels, so its maximum
        // intensity is ten. Before this round every field clamped at three - right for `fd_blood`, wrong for this one.
        let Some(electricity) = cdda_sim::field_registry::id_of(&mut world, "fd_electricity") else {
            panic!("the field is installed");
        };
        assert_eq!(
            cdda_sim::field_registry::max_intensity_of(&mut world, electricity),
            10,
            "a ten-level field holds ten, which is the data's number"
        );
        let Some(blood) = cdda_sim::field_registry::id_of(&mut world, "fd_blood") else {
            panic!("the field is installed");
        };
        assert_eq!(
            cdda_sim::field_registry::max_intensity_of(&mut world, blood),
            3,
            "and a three-level field still holds three"
        );

        assert_eq!(
            cdda_sim::field_registry::id_of(&mut world, "fd_fire"),
            Some(cdda_sim::FIRE),
            "fire is the constant fire"
        );
        assert_eq!(
            cdda_sim::field_registry::id_of(&mut world, "fd_incendiary"),
            Some(cdda_sim::INCENDIARY),
            "and the incendiary field is the one the rule sweeps for"
        );
        assert_eq!(
            cdda_sim::field_registry::id_of(&mut world, "fd_null"),
            Some(FieldId(0)),
            "null is zero"
        );
        assert_eq!(
            cdda_sim::field_registry::name_of(&mut world, cdda_sim::SMOKE).as_deref(),
            Some("fd_smoke"),
            "and the name comes back for a number"
        );
        // Every constant the port has must be one the data names, and both directions must agree.
        for (name, constant) in [
            ("fd_fire", cdda_sim::FIRE),
            ("fd_smoke", cdda_sim::SMOKE),
            ("fd_extinguisher", cdda_sim::EXTINGUISHER),
            ("fd_flame_burst", cdda_sim::FLAME_BURST),
            ("fd_fire_vent", cdda_sim::FIRE_VENT),
            ("fd_acid_vent", cdda_sim::ACID_VENT),
            ("fd_shock_vent", cdda_sim::SHOCK_VENT),
            ("fd_electricity", cdda_sim::ELECTRICITY),
            ("fd_incendiary", cdda_sim::INCENDIARY),
            ("fd_push_items", cdda_sim::PUSH_ITEMS),
            ("fd_hot_air1", cdda_sim::HOT_AIR_1),
            ("fd_hot_air2", cdda_sim::HOT_AIR_2),
            ("fd_hot_air3", cdda_sim::HOT_AIR_3),
            ("fd_hot_air4", cdda_sim::HOT_AIR_4),
        ] {
            assert_eq!(
                cdda_sim::field_registry::id_of(&mut world, name),
                Some(constant),
                "{name}"
            );
            assert_eq!(
                cdda_sim::field_registry::name_of(&mut world, constant).as_deref(),
                Some(name),
                "and back again for {name}"
            );
        }
    }

    #[allow(clippy::expect_used)]
    /// The `throw` step hands the item to the rules and the `expect item ... at` step reads the square back.
    ///
    /// Reference: the same chain the scenario plays - `Character::throw_item` (`ranged.cpp:1467-1600`) through `projectile_attack_roll` (`ballistics.cpp:204-231`) to the
    /// miss branch of `projectile_attack` (`ballistics.cpp:314-352`). The world here is bare, so there is no item data: the test only proves the **steps** parse and that a
    /// throw with nothing in hand is a failure with a sentence, not a panic.
    #[test]
    fn a_throw_with_nothing_in_hand_says_so() {
        let outcome = crate::run("world 4 bare\nsurvivor 0 0\nthrow rock at 3 0\n").expect("the scenario parses");
        assert!(
            outcome
                .failures
                .iter()
                .any(|line| line.contains("nothing called rock is in hand")),
            "a throw with no item is a sentence: {:?}",
            outcome.failures
        );
    }

    /// An expectation about a square with nothing on it fails, and says what it found.
    #[allow(clippy::expect_used)]
    #[test]
    fn expecting_an_item_on_an_empty_square_fails() {
        let outcome =
            crate::run("world 4 bare\nsurvivor 0 0\nexpect item rock at 2 2\n").expect("the scenario parses");
        assert!(
            outcome
                .failures
                .iter()
                .any(|line| line.contains("expected one rock on (2, 2), found 0")),
            "the square is named in the failure: {:?}",
            outcome.failures
        );
    }

    /// A turn moves the world's clock, which is what every gated stage reads.
    ///
    /// **The schedule does not count its own calls**: `focus_stage` asks for every sixtieth turn of the calendar and `digest_stage` for every three hundredth, so a
    /// clock that never moves runs those rules once and then never again - and a scenario that says `turn 3000` would report the same second three thousand times.
    #[test]
    fn a_turn_advances_the_clock() {
        let mut world = World::new();
        super::build_world(&mut world, 4, false);
        assert_eq!(
            cdda_sim::calendar::of(&world).map(|calendar| calendar.turn),
            Some(0),
            "a fresh world starts at turn zero"
        );
        for _ in 0..5 {
            super::take_turn(&mut world);
        }
        assert_eq!(
            cdda_sim::calendar::of(&world).map(|calendar| calendar.turn),
            Some(5),
            "and five turns are five seconds"
        );
    }

    /// The game's own vitamin and effect tables are installed at start-up, so a body can tell a drug from a nutrient and a
    /// spell can ask what an effect does.
    ///
    /// Without this the tables are empty, and an empty vitamin table is not a neutral default: `update_stomach` asks the type
    /// whether a level skips the guts, so **every drug is absorbed as nothing at all** and no test notices, because a test may
    /// install whatever table it likes.
    #[test]
    fn the_binary_s_world_knows_what_a_vitamin_and_an_effect_are() {
        let mut world = World::new();
        let vitamins = super::install_vitamin_data(&mut world).unwrap_or(0);
        assert!(vitamins > 40, "installed {vitamins} vitamins, which is too few");
        assert_eq!(
            cdda_sim::vitamin_registry::count(&world),
            vitamins,
            "and the store knows every one"
        );
        let effects = super::install_effect_data(&mut world).unwrap_or(0);
        assert!(effects > 100, "installed {effects} effect types, which is too few");
        // A known effect, by the name the data uses, is one the store can answer with its own caps.
        let poison = cdda_sim::effect_registry::kind_of(&mut world, "poison");
        assert!(poison.is_some(), "the data has a poison effect");
        let Some(poison) = poison else {
            return;
        };
        assert_eq!(poison.id, "poison", "and it is the one asked for");
        assert!(
            poison.max_duration > 0,
            "with a duration the rules can count against: {}",
            poison.max_duration
        );
    }
}

#[cfg(test)]
mod grammar_tests {
    #![allow(clippy::panic, clippy::expect_used)]
    use super::parse;

    /// Every line in `docs/cli-steps.md` is a line this runner understands.
    ///
    /// The grammar is the one place a scenario and the code can drift apart without either failing: a step with a handler and **no parser** is invisible to the compiler (round 1120
    /// found `Step::Data` in exactly that state), and a parser arm that is deleted is invisible until a scenario uses it. The document is short enough to read and exact enough to
    /// execute, so it is executed.
    #[test]
    fn every_line_of_the_documented_grammar_parses() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/cli-steps.md");
        let text = std::fs::read_to_string(path).expect("the grammar document is in the repository");
        let mut checked = 0;
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') || line.starts_with("```") || line.contains(" is one of these") {
                continue;
            }
            // Prose lines in the document are skipped; grammar lines all begin with a lowercase keyword followed by a space or nothing.
            let first = line.split_whitespace().next().unwrap_or("");
            if !matches!(
                first,
                "world" | "seed" | "scent" | "fire" | "turn" | "date" | "data" | "survivor" | "items" | "monsters"
                    | "place" | "give" | "wield" | "attack" | "throw" | "shot" | "goal" | "monster" | "craft" | "expect"
            ) {
                continue;
            }
            parse(line).unwrap_or_else(|error| panic!("the document says `{line}`, and the parser says: {error}"));
            checked += 1;
        }
        assert!(checked >= 30, "the grammar document has the whole grammar: {checked} lines");
    }
}

/// Fires a gun the survivor holds, and reports what the shot cost, where it went and what it did.
///
/// Reference: `Character::fire_gun` (`src/ranged.cpp:1050-1200`). The gun and the skills come from the oracle's own dumps - `corpus/items/guns.json` and `corpus/skills.json` - because
/// **the shot's price is the skill's figure and its accuracy is the gun's**, and a scenario that invented either would be testing the scenario.
///
/// The gun is an item entity the survivor wields, with `rounds` in it: the port keeps "how many of these are here" in the item's own charges, so firing is a component write on that entity.
fn fire_a_step(world: &mut World, outcome: &mut Outcome, gun_id: &str, x: i32, y: i32, rounds: i32) {
    let Some(survivor) = survivor(world) else {
        outcome.failures.push("there is no survivor to fire".to_owned());
        return;
    };
    let Some((gun, lines, round)) = fire_tables(gun_id, outcome) else {
        return;
    };
    let names = cdda_sim::item_registry::snapshot(world);
    // The gun is carried: a weapon in the survivor's own hands, which is what `wield` puts there.
    let Some(held) = carried_item_of(world, survivor, gun_id, &names).or_else(|| {
        let kind = cdda_sim::drop_item(world, (0, 0), cdda_sim::ItemTypeId(0), rounds);
        world.entity_mut(kind).insert(cdda_sim::creature::WieldedBy(survivor));
        Some(kind)
    }) else {
        outcome
            .failures
            .push(format!("nothing called {gun_id} is in hand to fire"));
        return;
    };
    // A gun handed over with no rounds is loaded here: the step says how many it holds, and a scenario should not have to. The charges are the store's own count, so this is one write.
    if let Some(mut item) = world.get_mut::<cdda_sim::GroundItem>(held)
        && item.charges == 0
    {
        item.charges = rounds;
    }
    let before = cdda_sim::creature::moves_left(world, survivor);
    let shot = cdda_sim::shot::fire_at(world, survivor, &gun, &lines, (x, y, 0), 18.0, round.as_ref(), Some(held), gun_weight(gun_id), reloads_and_shoots(gun_id), );
    let spent = before - cdda_sim::creature::moves_left(world, survivor);
    outcome.lines.push(format!(
        "fire: {gun_id} {} the shot, {spent} moves, missed by {:.3}, hurt {} {:?}, noise {}",
        if shot.loaded { "made" } else { "could not make" },
        shot.aim.missed_by,
        shot.hurt,
        shot.hit,
        shot.noise
    ));
}

/// The gun and the skills table, read from the oracle's dumps.
///
/// Both are the reference's resolved data rather than the port's opinion: the guns dump carries the weapon's dispersion and its skill's name, and the skills dump carries the line that prices
/// the shot. A missing file is a scenario failure with the path in it, because a silent zero would be a shot priced at nothing.
fn fire_tables(
    gun_id: &str,
    outcome: &mut Outcome,
) -> Option<(
    cdda_content::guns::GunType,
    Vec<cdda_content::skills::SkillTime>,
    Option<cdda_content::ammo::AmmoType>,
)> {
    let read = |name: &str| -> Option<cdda_json::Value> {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("../../corpus/{name}"));
        let bytes = std::fs::read(&path).ok()?;
        cdda_json::parse(&bytes).ok()
    };
    let Some(guns) = read("items/guns.json").and_then(|value| cdda_content::guns::load_dump(&value).ok()) else {
        outcome.failures.push("the guns dump is not where the scenario expects it".to_owned());
        return None;
    };
    let Some(lines) = read("skills.json").and_then(|value| cdda_content::skills::load_dump(&value).ok()) else {
        outcome.failures.push("the skills dump is not where the scenario expects it".to_owned());
        return None;
    };
    // The round is the gun's own `ammo` field, looked up in the ammo table: **a firearm's blow lives there, not in the gun** (rounds 1146 and 1147), so a shot fired without it does
    // nothing at all - which is what the first run of the shooting scenario showed.
    let rounds = read("items/ammo.json").and_then(|value| cdda_content::ammo::load_dump(&value).ok());
    let Some(gun) = guns.into_iter().find(|gun| gun.id == gun_id) else {
        outcome.failures.push(format!("`{gun_id}` is not a gun in the corpus"));
        return None;
    };
    // Two namespaces, and the port tries both: the round's **id** (which is what `223` is) and, failing that, the **ammunition type** the gun names (which is what `shot` is).
    let round = rounds.and_then(|rounds| {
        rounds
            .iter()
            .find(|round| round.id == gun.ammo)
            .or_else(|| round_of_type(&rounds, &gun.ammo))
            .cloned()
    });
    Some((gun, lines, round))
}

/// `shoot <gun> at <x> <y> <rounds>`: a gun fired at a square with a number of rounds in it.
///
/// # Errors
///
/// When the line is not that shape, or a coordinate is not a number.
fn shoot_step(words: &[&str]) -> Result<Step, String> {
    let ["shoot", gun, "at", x, y, rounds] = words else {
        return Err(format!("`{}` is not a shoot step", words.join(" ")));
    };
    let number = words.len();
    Ok(Step::Shoot {
        gun: (*gun).to_owned(),
        x: number_of(x, number, "x")?,
        y: number_of(y, number, "y")?,
        rounds: number_of(rounds, number, "rounds")?,
    })
}

/// What a gun weighs, in grams, from the reference's own item dump.
///
/// Reference: `item::weight()` (`item.cpp:7256-7261`), which the recoil rule divides by 333 g. The prototypes dump writes `weight_mg` in **milligrams**, and a gun it does not know
/// weighs nothing - which prices its kick as if the weapon were weightless rather than inventing a figure.
fn gun_weight(id: &str) -> f64 {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/prototypes/items.json");
    let Ok(bytes) = std::fs::read(&path) else {
        return 0.0;
    };
    let Ok(value) = cdda_json::parse(&bytes) else {
        return 0.0;
    };
    let cdda_json::Value::Object(root) = &value else {
        return 0.0;
    };
    let Some(cdda_json::Value::Array(items)) = root.get("prototypes") else {
        return 0.0;
    };
    for item in items {
        let cdda_json::Value::Object(object) = item else {
            continue;
        };
        if object.get("id") != Some(&cdda_json::Value::Str(id.to_owned())) {
            continue;
        }
        if let Some(cdda_json::Value::Int(mg)) = object.get("weight_mg") {
            #[allow(clippy::cast_precision_loss)]
            return *mg as f64 / 1000.0;
        }
    }
    0.0
}

/// Whether a weapon reloads as it fires, which the reference calls `RELOAD_AND_SHOOT`.
///
/// Reference: `flag_RELOAD_AND_SHOOT` (`ranged.cpp:1230-1233`), read from the item's own flags in the prototypes dump: a **longbow** and a **compound bow** state it and a rifle states
/// nothing. A weapon the dump does not know answers **no**, which is the cautious answer - it leaves recoil accumulating rather than resetting to the cap.
fn reloads_and_shoots(id: &str) -> bool {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/prototypes/items.json");
    let Ok(bytes) = std::fs::read(&path) else {
        return false;
    };
    let Ok(cdda_json::Value::Object(root)) = cdda_json::parse(&bytes) else {
        return false;
    };
    let Some(cdda_json::Value::Array(items)) = root.get("prototypes") else {
        return false;
    };
    items.iter().any(|item| {
        let cdda_json::Value::Object(object) = item else {
            return false;
        };
        if object.get("id") != Some(&cdda_json::Value::Str(id.to_owned())) {
            return false;
        }
        matches!(object.get("flags"), Some(cdda_json::Value::Array(flags)) if flags.iter().any(
            |flag| *flag == cdda_json::Value::Str("RELOAD_AND_SHOOT".to_owned())
        ))
    })
}

/// Parses the `throw` step: an item thrown at a square.
///
/// # Errors
///
/// When the line is not `throw <item> at <x> <y>`, or a coordinate is not a number.
fn throw_step(words: &[&str], number: usize) -> Result<Step, String> {
    let ["throw", item, "at", x, y] = words else {
        return Err(format!("line {}: expected `throw <item> at <x> <y>`", number + 1));
    };
    Ok(Step::Throw {
        item: (*item).to_owned(),
        x: number_of(x, number, "x")?,
        y: number_of(y, number, "y")?,
    })
}

/// Parses the `aim` step, which is short enough to keep out of `parse`'s own hundred-line budget.
fn aim_step(words: &[&str], number: usize) -> Result<Step, String> {
    let ["aim", gun] = words else {
        let ["aim", gun, moves] = words else {
            return Err(format!("line {}: expected `aim <gun> [moves]`", number + 1));
        };
        return Ok(Step::Aim {
            gun: (*gun).to_owned(),
            moves: number_of(moves, number, "moves")?,
        });
    };
    Ok(Step::Aim {
        gun: (*gun).to_owned(),
        moves: 1,
    })
}

/// Aim a gun for a number of moves, which is the step that spends the recoil a shot leaves behind.
///
/// Reference: the `AIM` activity (`activity_actor.cpp`) looping `Character::aim_per_move` until the gain falls below `MIN_RECOIL_IMPROVEMENT`. The limit is the **point-shooting** limit
/// (`point_shooting_limit`), because the port has no sights to switch to; the sight speed comes from the gun's own sights in the shooter's eyes.
///
/// The moves are spent from the shooter's `Moves`, and the recoil is written back to the shooter's `Recoil` - **one store, one field each**, which is what makes the step a rule rather than a
/// display.
#[allow(clippy::cast_possible_truncation)]
fn aim_a_step(world: &mut World, outcome: &mut Outcome, gun_id: &str, moves: i32) {
    let Some(survivor) = survivor(world) else {
        outcome.failures.push("there is no survivor to aim".to_owned());
        return;
    };
    let Some((gun, lines, _)) = fire_tables(gun_id, outcome) else {
        return;
    };
    let skill = lines
        .iter()
        .find(|line| line.id == gun.skill)
        .map_or(0, |line| cdda_sim::creature::skill_level(world, survivor, &line.id));
    let perception = cdda_sim::creature::effective_perception(world, survivor);
    let archery = gun.skill == "archery";
    let limit = f64::from(cdda_sim::aim::point_shooting_limit(skill, archery));
    let recoil = world.get::<cdda_sim::shot::Recoil>(survivor).map_or(0.0, |recoil| f64::from(recoil.points));
    let (_, sight_speed) = cdda_sim::aim::sight_report(&gun, perception, recoil as i32, false);
    let (after, spent) = cdda_sim::aim::spend_aim(recoil, limit, skill, sight_speed, moves);
    cdda_sim::creature::spend_moves(world, survivor, f64::from(spent));
    if let Some(mut carried) = world.get_mut::<cdda_sim::shot::Recoil>(survivor) {
        let points = after.round() as i32;
        carried.points = points;
    }
    outcome.lines.push(format!(
        "aim: {gun_id} spent {spent} of {moves} moves, recoil {recoil:.1} -> {after:.3}, limit {limit:.0}"
    ));
}

/// The round a gun's ammunition type names, when the round's own id does not.
///
/// Reference: a gun states an **ammunition type** (`gun_type::ammo`, `"shot"`), and the rounds that fit it are separate items keyed by their own ids (`shot_bird`, `shot_00`). The reference
/// picks between them by what is **loaded** (`item::ammo_current`); the port has no magazine entity yet, so it takes the **first round of that type in the table's order** and says so - a
/// deterministic choice from the data rather than an invented favourite. When no round carries the type, the answer is `None` and the step fails rather than firing nothing.
fn round_of_type<'a>(rounds: &'a [cdda_content::ammo::AmmoType], kind: &str) -> Option<&'a cdda_content::ammo::AmmoType> {
    rounds.iter().find(|round| round.ammo_type == kind)
}

/// `burst <gun> at <x> <y> <shots> <rounds>`: several shots in one step.
///
/// # Errors
///
/// When the line is not that shape, or a number is not a number.
fn burst_step(words: &[&str]) -> Result<Step, String> {
    let ["burst", gun, "at", x, y, shots, rounds] = words else {
        return Err(format!("`{}` is not a burst step", words.join(" ")));
    };
    let number = words.len();
    Ok(Step::Burst {
        gun: (*gun).to_owned(),
        x: number_of(x, number, "x")?,
        y: number_of(y, number, "y")?,
        shots: number_of(shots, number, "shots")?,
        rounds: number_of(rounds, number, "rounds")?,
    })
}

/// Fires a burst, and reports each shot the gun actually made.
///
/// Reference: `ranged.cpp:1130-1240` - the loop that spends a round, charges a shot and adds recoil per shot, and stops when the gun will not load.
fn burst_a_step(world: &mut World, outcome: &mut Outcome, gun_id: &str, x: i32, y: i32, shots: i32, rounds: i32) {
    let Some(survivor) = survivor(world) else {
        outcome.failures.push("there is no survivor to fire".to_owned());
        return;
    };
    let Some((gun, lines, round)) = fire_tables(gun_id, outcome) else {
        return;
    };
    let names = cdda_sim::item_registry::snapshot(world);
    // The same rule the single shot uses: a gun in hand, or one put into it by the step itself.
    let Some(held) = carried_item_of(world, survivor, gun_id, &names).or_else(|| {
        let kind = cdda_sim::drop_item(world, (0, 0), cdda_sim::ItemTypeId(0), rounds);
        world.entity_mut(kind).insert(cdda_sim::creature::WieldedBy(survivor));
        Some(kind)
    }) else {
        outcome.lines.push(format!("nothing called {gun_id} is in hand to fire"));
        return;
    };
    if let Some(mut item) = world.get_mut::<cdda_sim::GroundItem>(held)
        && item.charges == 0
    {
        item.charges = rounds;
    }
    let before = cdda_sim::creature::moves_left(world, survivor);
    let made = cdda_sim::shot::fire_burst(
        world,
        survivor,
        &gun,
        &lines,
        (x, y, 0),
        18.0,
        round.as_ref(),
        Some(held),
        gun_weight(gun_id),
        reloads_and_shoots(gun_id),
        shots,
    );
    let spent = before - cdda_sim::creature::moves_left(world, survivor);
    outcome.lines.push(format!("burst: {gun_id} fired {} of {shots} shots for {spent} moves", made.len()));
    for shot in &made {
        outcome.lines.push(format!(
            "  shot: missed by {:.3}, hurt {}, noise {}",
            shot.aim.missed_by, shot.hurt, shot.noise
        ));
    }
    if made.is_empty() {
        outcome.failures.push(format!("{gun_id} fired no shot at all"));
    }
}
