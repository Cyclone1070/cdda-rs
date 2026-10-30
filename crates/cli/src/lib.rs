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
            ["expect", "changed", count] => Step::ExpectChanged(number_of(count, number, "squares")?),
            ["expect", "fire", x, y] => {
                Step::ExpectFireAt(number_of(x, number, "x")?, number_of(y, number, "y")?)
            }
            ["expect", "no", "fire", x, y] => {
                Step::ExpectNoFireAt(number_of(x, number, "x")?, number_of(y, number, "y")?)
            }
            ["survivor", x, y] => Step::Survivor {
                x: number_of(x, number, "x")?,
                y: number_of(y, number, "y")?,
            },
            ["items"] => Step::Items,
            ["give", item, count] => Step::Give {
                item: (*item).to_owned(),
                count: number_of(count, number, "count")?,
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
        Step::Survivor { x, y } => {
                let survivor = cdda_sim::spawn_player(world, (*x, *y), 80, 0);
                cdda_sim::creature::set_speed(world, survivor, 100, 0);
                outcome.lines.push(format!("survivor: at ({x}, {y})"));
            }
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
    let mut level = cdda_core_types::level::Level::new();
    for y in 0..side {
        for x in 0..side {
            level.set_terrain(x, y, 1, 0);
        }
    }
    cdda_sim::load_level(world, &level);
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
    let before = cdda_sim::inspect::world_fingerprints(world);
    cdda_sim::turn::run_turn(world);
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
    let mut kinds: Vec<(String, u32)> = Vec::with_capacity(rows.len());
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
        kinds.push((id.clone(), number));
    }
    let named: Vec<(&str, u32)> = kinds.iter().map(|(id, num)| (id.as_str(), *num)).collect();
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
    /// `leather` is one fur into one leather and takes 36,000 moves, which is 360 turns at a hundred a turn - so this
    /// also proves the craft is not finishing early.
    #[test]
    fn a_real_recipe_runs_to_its_result() {
        let mut world = World::new();
        cdda_sim::turn::set_random(&mut world, cdda_core_types::rng::Rolls::new(1));
        super::install_item_data(&mut world).expect("the game data is in the repository");
        let avatar = cdda_sim::spawn_player(&mut world, (0, 0), 80, 0);
        cdda_sim::creature::set_speed(&mut world, avatar, 100, 0);

        let line = super::craft_a_recipe(&mut world, avatar, "leather", 400)
            .expect("the data has a leather recipe");

        assert!(line.starts_with("leather -> leather"), "unexpected line: {line}");
        assert!(
            line.contains("x1"),
            "one leather came out of it, not nothing: {line}"
        );
        let turns: i32 = line
            .rsplit("after ")
            .next()
            .and_then(|rest| rest.split_whitespace().next())
            .and_then(|count| count.parse().ok())
            .expect("the line states how many turns it took");
        assert_eq!(turns, 360, "36,000 moves is 360 turns at a hundred a turn");
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
}
