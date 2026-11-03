//! The simulation's store and the systems that run over it.
//!
//! The register is a `bevy_ecs::World`: every thing that exists is an `Entity` with
//! components, reached by id and never by pointer. What a square holds is found through a
//! grid resource rebuilt from the World, never kept as a second authority.

pub mod transform;
pub mod blast;
pub mod bionics;
pub mod calendar;
pub mod cities;
pub mod dialogue_context;
pub mod npc_opinion;
pub mod npcs;
pub mod outdoors;
pub mod proficiencies;
pub mod overmap;
pub mod bash;
pub mod emit_registry;
pub mod falling;
pub mod memory;
pub mod mapgen_place;
pub mod mapgen_update;
pub mod messages;
pub mod missions;
pub mod morale;
pub mod mutation_data;
pub mod mutations;
pub mod seeing;
pub mod snippets;
pub mod field_passage;
pub mod field_registry;
pub mod field_spread;
pub mod field_table;
pub mod flag_registry;
pub mod radiation;
pub mod applied_effects;
pub mod body;
pub mod digest;
pub mod dialogue_effects;
pub mod dialogue_values;
pub mod effect_registry;
pub mod eoc_registry;
pub mod effects;
pub mod singletons;
pub mod square;
pub mod conditions;
pub mod containment;
pub mod craft_roll;
pub mod crafting;
pub mod choices;
pub mod creature;
pub mod creature_flags;
pub mod hit_location;
pub mod melee;
pub mod sounds;
pub mod timed_events;
pub mod traits;
pub mod spell_effects;
pub mod spell_types;
pub mod spellcasting;
pub mod pathfinding;
pub mod routing;
pub mod fire_facts;
pub mod turn;
pub mod inspect;
pub mod inventory;
pub mod item_data;
pub mod item_groups;
pub mod item_registry;
pub mod monster_catalog;
pub mod monster_groups;
pub mod monster_registry;
pub mod monster_spawning;
pub mod pockets;
pub use square::load_level;
pub mod skills;
pub mod skills_of;
pub use singletons::{
    TheFallingQueue, TheFieldRules, TheMemory, TheScent, TheIndex, ThePrototypes, TheSightCasts, SightCasts, TheScentTable, TheClimate, TheFieldTable, TheLight, TheWeather,
    adjust_radiation,
    falling_queue_entity, field_rules, memory, memory_entity, queue_support_loss,
    climate, climate_read, count_sight_cast, field_table, index, index_mut, light,
    light_is_stale, light_mut, light_rebuilds, mark_light_stale,
    prototypes, sight_casts,
    set_field_table,
    set_prototypes, set_weather, scent_table, weather, set_climate, set_field_rules, set_scent_table,
    singleton, support_lost,
};
pub mod food_summary;
pub mod frame;
pub mod friendly;
pub mod furniture_types;
pub mod needs;
pub mod fun_for;
pub mod terrain_table;
pub mod terrain_transform;
pub mod terrain_types;
pub mod noise;
pub mod achievements;
pub mod addictions;
pub mod known_spells;
pub mod math_functions;
pub mod recipes_known;
pub mod revert;
pub mod scheduled;
pub mod scent;
pub mod scent_table;
pub mod temperature;
pub mod weather;
pub mod wetness;
pub mod worn_flags;
pub mod weather_gen;
pub mod vitamin_registry;
pub use weather::{Climate, Forecast, System, Weather, tick_climate, wind_of};

use bevy_ecs::prelude::*;
use cdda_core_types::field::{FieldEntry, FieldId};
use cdda_core_types::line::line_to_3d;
use cdda_core_types::rng::{Rolls, shuffle};
use cdda_core_types::shadowcasting::FourQuadrants;
use std::collections::{BTreeMap, BTreeSet};

/// The level everything stands on until a system learns to climb: the reference's ground floor.
pub const STREET_LEVEL: i32 = 0;

/// Where a thing stands. The only authoritative position, because it lives on the thing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Component)]
pub struct Position {
    /// The square's column.
    pub x: i32,
    /// The square's row.
    pub y: i32,
    /// The level the square is on: negative underground, zero at street level, positive upstairs.
    ///
    /// The reference's maps are three dimensional and most of its rules say so - gas falls before it drifts, smoke rises, a fire on a roof does not warm
    /// the room below. The store carries the level from the start rather than pretending otherwise, even where the systems above it have not learned to
    /// look at it yet.
    pub z: i32,
}

impl Position {
    /// A square at street level, which is where almost every test stands.
    #[must_use]
    pub const fn flat(x: i32, y: i32) -> Self {
        Self { x, y, z: 0 }
    }
}

/// The derived index: which things stand on each square.
///
/// Reference: D2. Rebuilt from the World, never edited directly and never authoritative: if it
/// disagrees with the World, it is wrong.
#[derive(Debug, Default, Component)]
pub struct PositionIndex {
    squares: BTreeMap<(i32, i32, i32), Vec<Entity>>,
}

impl PositionIndex {
    /// Rebuilds the index from the World's `Position` components.
    pub fn rebuild(world: &mut World) {
        let mut squares: BTreeMap<(i32, i32, i32), Vec<Entity>> = BTreeMap::new();
        let mut query = world.query::<(Entity, &Position)>();
        for (entity, position) in query.iter(world) {
            squares
                .entry((position.x, position.y, position.z))
                .or_default()
                .push(entity);
        }
        *singletons::index_mut(world) = Self { squares };
    }

    /// Everything standing on a square at street level, in spawn order.
    #[must_use]
    pub fn at(&self, x: i32, y: i32) -> &[Entity] {
        self.at_level(x, y, 0)
    }

    /// Everything standing on a square on a named level, in spawn order.
    #[must_use]
    pub fn at_level(&self, x: i32, y: i32, z: i32) -> &[Entity] {
        self.squares.get(&(x, y, z)).map_or(&[], Vec::as_slice)
    }

    /// Adds one entity to the index, which is what a change to one square needs.
    ///
    /// The whole index is rebuilt when a world is loaded; a single write is not a load, and
    /// rebuilding for it would make every terrain change cost the level.
    pub fn insert(&mut self, entity: Entity, square: (i32, i32, i32)) {
        let occupants = self.squares.entry(square).or_default();
        if !occupants.contains(&entity) {
            occupants.push(entity);
        }
    }

    /// Takes a thing out of one square, so a move does not leave it in two places at once.
    pub fn remove(&mut self, entity: Entity, square: (i32, i32, i32)) {
        if let Some(occupants) = self.squares.get_mut(&square) {
            occupants.retain(|occupant| *occupant != entity);
            if occupants.is_empty() {
                self.squares.remove(&square);
            }
        }
    }

    /// How many squares hold anything at all.
    #[must_use]
    pub fn occupied_squares(&self) -> usize {
        self.squares.len()
    }
}

/// A patch of something lying on a square: smoke, fire, blood, acid.
///
/// A field is a thing, not a property of the ground: it has a square, a strength and an age,
/// and the field system acts on it every turn. The entry keeps the reference's decay rules
/// (`src/field.cpp`), which were ported and gated before the storage moved here.
#[derive(Debug, Clone, Component)]
pub struct Field {
    /// Which kind of field this is.
    pub id: FieldId,
    /// Its strength and age, with the reference's decay semantics.
    pub entry: FieldEntry,
    /// Draw and merge priority among the fields on one square.
    pub priority: i32,
}

/// Ages every field by one turn and removes the ones that are gone.
///
/// Reference: `map::process_fields_in_submap` (`src/map_field.cpp:160-460`), whose per-field
/// work lives in [`cdda_core_types::field::FieldEntry::do_decay`]. Fields are processed in a
/// fixed order - the reference walks submaps, then squares in x-then-y order, then each
/// square's fields in insertion order - and that order is observable, because a decaying field
/// draws from the same dice as everything else.
pub fn process_fields(world: &mut World, now: i64, rolls: &mut cdda_core_types::rng::Rolls) {
    // The same pass as [`process_field_turns`], with no processors registered: a field ages and dies, and nothing asks it to act.
    // Keeping one implementation rather than two is the point - the two would drift, and the drift would be invisible until a world
    // ran twice and disagreed with itself.
    process_field_turns(world, now, rolls, |_| None, |_, _, _| {});
}

/// Runs one turn of every field, in the reference's order, and lets each take its own turn.
///
/// Reference: `map::process_fields_in_submap` (`src/map_field.cpp:445-499`). Fields are processed squares first, x outside and y
/// inside, then each square's fields in insertion order - and that order is observable, because the turn draws from the same dice
/// as everything else.
///
/// The three steps are the rule, and they are here rather than at the call site:
///
/// 1. a field with no intensity is **gone** before anything runs;
/// 2. a field **created this turn** is spared its processors and only decays, which is what gives a player standing in
///    freshly-lit fire a turn to step out;
/// 3. the decay comes **last**, so a field that spawns this turn is still at full strength while it does.
///
/// `turn_of` says which turn a field takes - in the game it is the turn of the field's type at the field's **live** intensity, so a
/// field that upgrades takes the new level's numbers on its next turn. `on_turn` is where the processors run; the spawn rule itself
/// belongs to the game layer, which owns the group table and the creatures.
pub fn process_field_turns(
    world: &mut World,
    now: i64,
    rolls: &mut cdda_core_types::rng::Rolls,
    turn_of: impl Fn(&Field) -> Option<cdda_core_types::field::FieldTurn>,
    on_turn: impl FnMut(
        &cdda_core_types::field::FieldTurn,
        (i32, i32),
        &mut cdda_core_types::rng::Rolls,
    ),
) {
    let mut present: Vec<(Entity, i32, i32)> = Vec::new();
    let mut query = world.query::<(Entity, &Position)>();
    for (entity, position) in query.iter(world) {
        if world.get::<Field>(entity).is_some() {
            present.push((entity, position.x, position.y));
        }
    }
    decay_the_fields_the_sweep_found(world, &present, now, rolls, turn_of, on_turn);
}

/// The decay half of the sweep, for the squares the sweep **found** when it started.
///
/// Reference: `map::process_fields` (`src/map_field.cpp:164-200`) builds its list of squares before it visits any of them, so a field created while it runs -
/// a fire spreading next door, the hot air a fire leaves - waits until the next turn. That is why this takes the list rather than looking for itself: a
/// caller that runs other systems first (the environment turn does) has to hand over the list it took **before** them, or the newborn loses a turn of its
/// life on the turn it was born.
pub fn decay_the_fields_the_sweep_found(
    world: &mut World,
    present: &[(Entity, i32, i32)],
    now: i64,
    rolls: &mut cdda_core_types::rng::Rolls,
    turn_of: impl Fn(&Field) -> Option<cdda_core_types::field::FieldTurn>,
    mut on_turn: impl FnMut(
        &cdda_core_types::field::FieldTurn,
        (i32, i32),
        &mut cdda_core_types::rng::Rolls,
    ),
) {
    let mut order = present.to_vec();
    order.sort_by_key(|&(entity, x, y)| (x, y, entity.to_bits()));
    for (entity, x, y) in order {
        let Some(field) = world.get::<Field>(entity) else {
            continue;
        };
        if !field.entry.is_alive() {
            world.despawn(entity);
            continue;
        }
        let age = field.entry.age();
        let turn = turn_of(field);
        if age == 0 || turn.is_none() {
            // Spared its processors; it still ages, which is the whole of what happens to it.
            let died = world.get_mut::<Field>(entity).is_some_and(|mut field| {
                field.entry.do_decay(now, rolls);
                !field.entry.is_alive()
            });
            if died {
                world.despawn(entity);
            }
            continue;
        }
        let Some(turn) = turn else {
            continue;
        };
        // The spawn itself is the game layer's: it owns the group table and the creatures. The sim says only that the turn came,
        // in the order the reference takes it.
        on_turn(&turn, (x, y), rolls);
        let died = world.get_mut::<Field>(entity).is_some_and(|mut field| {
            field.entry.do_decay(now, rolls);
            !field.entry.is_alive()
        });
        if died {
            world.despawn(entity);
        }
    }
}

/// What the map tells a fire about the world around it.
///
/// The fire processor asks its surroundings seven different questions (`src/map_field.cpp:945-1425`), and they all belong to the map rather
/// than to the fire. Gathering them behind one trait keeps the fire system readable and lets a test answer them one at a time.
pub trait FireMap {
    /// How many fires touch this square.
    fn adjacent_fires(&self, square: (i32, i32, i32)) -> i32;
    /// Whether the ground here can burn.
    fn can_burn(&self, square: (i32, i32, i32)) -> bool;
    /// What the fire is standing on, as far as eating it goes.
    ///
    /// The reference reads the terrain and furniture flags here (`map_field.cpp:1050`); the fuel rule that turns those flags into turns of life rolls dice, so it
    /// belongs to the fire's turn rather than to the map's answers.
    fn ground(&self, square: (i32, i32, i32)) -> cdda_core_types::fire::Ground;
    /// Whether the fire is somewhere it can spread from, rather than a brazier or an oven.
    fn can_spread(&self, square: (i32, i32, i32)) -> bool;
    /// What one of the eight neighbouring squares looks like.
    fn neighbour(&self, square: (i32, i32, i32)) -> cdda_core_types::fire::Neighbour;
    /// Whether smoke is suppressed here, as a sealed room does.
    fn suppress_smoke(&self, square: (i32, i32, i32)) -> bool;
    /// Whether the square has **no** floor, so anything left on it falls (`TFLAG_NO_FLOOR`, `map::has_floor_or_water`, `src/map.cpp:2651-2660`).
    ///
    /// The default answer is **no**: a map that has not been asked cannot promise a hole, and the reference's own fallback for a square it cannot read is to hold things up.
    fn no_floor(&self, square: (i32, i32, i32)) -> bool {
        let _ = square;
        false
    }
    /// How much smoke the fire's fuel made this turn.
    /// Whether the smoke may rise through the square overhead - `TFLAG_NO_FLOOR` up there, an open shaft or a hole in a roof.
    ///
    /// The default answer is **no**, because a map that has not looked upward cannot promise a way through, and the reference's own fallback is to leave
    /// the smoke where it was made.
    fn smoke_ceiling_open(&self, _square: (i32, i32, i32)) -> bool {
        false
    }
}

/// The handful of terrain facts the fire chain asks about, read from a loaded prototype.
///
/// The reference asks the terrain and furniture objects themselves (`ter_furn_has_flag`), which is fine because they are right there. The port keeps the
/// *answers* rather than the objects, so `cdda-sim` does not have to know what a `MapData` is - and so a test can describe a square in six booleans.
// Six independent answers, because the reference asks six independent flags; grouping them into enums would invent relationships the game does not have
// (a floor can be `FLAMMABLE_HARD` and `NO_FLOOR` at once).
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TerrainFacts {
    /// `FLAMMABLE`: feeds a fire every turn.
    pub flammable: bool,
    /// `FLAMMABLE_ASH`: the same, and leaves ash behind.
    pub flammable_ash: bool,
    /// `FLAMMABLE_HARD`: feeds a fire on a die.
    pub flammable_hard: bool,
    /// `FIRE_CONTAINER`: a brazier holds its fire and does not feed it.
    pub fire_container: bool,
    /// `NO_FLOOR`: open air here, so nothing above a fire is holding its smoke down.
    pub no_floor: bool,
    /// `SUPPRESS_SMOKE`: a sealed room.
    pub suppress_smoke: bool,
}

impl TerrainFacts {
    /// Whether this square is a fuel the fire can eat, as the reference defines it.
    ///
    /// Reference: `can_burn = ( ter.is_flammable() || frn.is_flammable() ) && !has_flag( FIRE_CONTAINER )` (`src/map_field.cpp:1050`). The container rule
    /// is the surprising half: a brazier is flammable and must not feed what is burning in it.
    #[must_use]
    pub fn can_burn(&self) -> bool {
        (self.flammable || self.flammable_ash || self.flammable_hard) && !self.fire_container
    }
}

/// The squares whose support was destroyed in the last turn.
///
/// Reference: `map::support_cache_dirty` (`src/map.h:1637`) filled by `map::support_dirty` (`src/map.cpp:3018-3035`). A square whose support goes away is queued, and the
/// next turn's `process_falling` takes the queue **whole** (`std::set<tripoint_bub_ms> last_cache = std::move( support_cache_dirty )`) and clears it - so a square that
/// falls is asked once, and a square that keeps falling is re-queued by whatever breaks under it next.
///
/// A component, on the one entity [`TheFallingQueue`] marks: the architecture is one store, and a
/// queue is a thing like any other. The reference keeps it as a member of the map, which is the
/// structure this port deliberately does not copy.
#[derive(Debug, Clone, Default, PartialEq, Eq, Component)]
pub struct SupportLost {
    /// The squares waiting to be asked whether anything on them still has a floor.
    pub squares: BTreeSet<(i32, i32, i32)>,
}

impl SupportLost {
    /// Queues one square, as `map::support_dirty` does when a tile loses its support.
    pub fn queue(&mut self, square: (i32, i32, i32)) {
        self.squares.insert(square);
    }
}

/// The map as the fire chain sees it: terrain answers per square, and which squares are on fire.
///
/// A read model, not a second authority. It is built from the world at the start of a turn (`note_fire` for each fire, `set_terrain` for each square the game
/// has loaded), and thrown away afterwards. Keeping it separate from the store is what lets the fire rules be asked about a square without a `&World`.
#[derive(Debug, Clone, Default)]
pub struct MapFacts {
    terrain: BTreeMap<(i32, i32, i32), TerrainFacts>,
    furniture: BTreeMap<(i32, i32, i32), TerrainFacts>,
    items: BTreeMap<(i32, i32, i32), bool>,
    fires: BTreeMap<(i32, i32, i32), (i32, i64)>,
}

impl MapFacts {
    /// An empty map: no terrain, no fires.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Says what is underfoot at a square.
    pub fn set_terrain(&mut self, square: (i32, i32, i32), facts: TerrainFacts) {
        self.terrain.insert(square, facts);
    }

    /// Says what is standing on a square. The reference asks `ter_furn_has_flag( ter, frn, flag )`, which reads both, so a square's answers are its
    /// terrain's and its furniture's together - a bench on a tiled floor burns like a bench, and a locker on one does not burn at all.
    pub fn set_furniture(&mut self, square: (i32, i32, i32), facts: TerrainFacts) {
        self.furniture.insert(square, facts);
    }

    /// Says whether something burnable is lying on a square - the spread rule's last way to catch, and the one a fire uses to cross a fireproof floor.
    pub fn set_items(&mut self, square: (i32, i32, i32), burnable: bool) {
        self.items.insert(square, burnable);
    }

    /// Records that a square holds a fire of this intensity and age, for [`FireMap::adjacent_fires`] and the flashpoint rule.
    pub fn note_fire(&mut self, square: (i32, i32, i32), intensity: i32, age: i64) {
        self.fires.insert(square, (intensity, age));
    }

    /// What is underfoot and what stands on it, taken together, or nothing at all for a square nobody described.
    #[must_use]
    pub fn terrain_at(&self, square: (i32, i32, i32)) -> TerrainFacts {
        let under = self.terrain.get(&square).copied().unwrap_or_default();
        match self.furniture.get(&square) {
            Some(standing) => TerrainFacts {
                flammable: under.flammable || standing.flammable,
                flammable_ash: under.flammable_ash || standing.flammable_ash,
                flammable_hard: under.flammable_hard || standing.flammable_hard,
                fire_container: under.fire_container || standing.fire_container,
                no_floor: under.no_floor || standing.no_floor,
                suppress_smoke: under.suppress_smoke || standing.suppress_smoke,
            },
            None => under,
        }
    }
}

impl MapFacts {
    /// Reads the world into a map the fire rules can be asked about.
    ///
    /// The store is the authority; this is the question sheet for one turn, built from it and
    /// thrown away afterwards. Everything it holds was in the world: the ground's answers from the
    /// `FireFacts` components, the items from the ground items, the fires from the fire fields.
    #[must_use]
    pub fn from_world(world: &bevy_ecs::prelude::World) -> Self {
        let mut map = Self::new();
        let mut ground = world.try_query::<(&Position, &fire_facts::FireFacts)>();
        if let Some(query) = ground.as_mut() {
            for (position, held) in query.iter(world) {
                map.set_terrain((position.x, position.y, position.z), held.facts);
            }
        }
        let mut items = world.try_query::<(&Position, &GroundItem)>();
        if let Some(query) = items.as_mut() {
            for (position, item) in query.iter(world) {
                if item.burn.is_some() {
                    map.set_items((position.x, position.y, position.z), true);
                }
            }
        }
        let mut fires = world.try_query::<(&Position, &Field)>();
        if let Some(query) = fires.as_mut() {
            for (position, field) in query.iter(world) {
                if field.id == FIRE {
                    map.note_fire(
                        (position.x, position.y, position.z),
                        field.entry.intensity(),
                        field.entry.age(),
                    );
                }
            }
        }
        map
    }
}

impl FireMap for MapFacts {
    fn adjacent_fires(&self, square: (i32, i32, i32)) -> i32 {
        let mut count = 0;
        for (index, _) in EIGHT_NEIGHBOURS.iter().enumerate() {
            if let Some(offset) = EIGHT_NEIGHBOURS.get(index) {
                let beside = (square.0 + offset.0, square.1 + offset.1, square.2);
                if self.fires.contains_key(&beside) {
                    count += 1;
                }
            }
        }
        count
    }

    fn can_burn(&self, square: (i32, i32, i32)) -> bool {
        self.terrain_at(square).can_burn()
    }

    fn ground(&self, square: (i32, i32, i32)) -> cdda_core_types::fire::Ground {
        let facts = self.terrain_at(square);
        if facts.fire_container {
            // A container holds its fire and does not feed it, so as fuel it is nothing at all.
            return cdda_core_types::fire::Ground::Inert;
        }
        // The order is the reference's (`map_field.cpp:1054-1104`) and it decides which rule runs: plain `FLAMMABLE` first and without a die, then
        // `FLAMMABLE_HARD` behind `one_in( 3 )`, then `FLAMMABLE_ASH`. A bench on a tiled floor is therefore *hard* ground, not ash - the harder flag wins
        // because it is asked first, which is exactly the kind of precedence a state dump never shows and a dice replay does.
        if facts.flammable {
            cdda_core_types::fire::Ground::Flammable
        } else if facts.flammable_hard {
            cdda_core_types::fire::Ground::FlammableHard
        } else if facts.flammable_ash {
            cdda_core_types::fire::Ground::FlammableAsh
        } else {
            cdda_core_types::fire::Ground::Inert
        }
    }

    fn can_spread(&self, square: (i32, i32, i32)) -> bool {
        // Whether the fire may spread from here is about what it is: a brazier or an oven does not spill onto the floor.
        !self.terrain_at(square).fire_container
    }

    fn neighbour(&self, square: (i32, i32, i32)) -> cdda_core_types::fire::Neighbour {
        let facts = self.terrain_at(square);
        let fire = self.fires.get(&square).copied().unwrap_or((0, 0));
        cdda_core_types::fire::Neighbour {
            has_fire: self.fires.contains_key(&square),
            fire_intensity: fire.0,
            fire_age: fire.1,
            items: self.items.get(&square).copied().unwrap_or(false),
            web: false,
            // Three separate questions with three separate dice: plain `FLAMMABLE` takes no roll to catch, `FLAMMABLE_ASH` rolls `one_in( 2 )` and
            // `FLAMMABLE_HARD` rolls `one_in( 5 )`. Folding ash into `flammable` here cost a die per bench - the port asked twice where the reference asked
            // once, and the recording said so within four draws.
            flammable: facts.flammable,
            flammable_ash: facts.flammable_ash,
            flammable_hard: facts.flammable_hard,
        }
    }

    fn suppress_smoke(&self, square: (i32, i32, i32)) -> bool {
        self.terrain_at(square).suppress_smoke
    }

    fn smoke_ceiling_open(&self, square: (i32, i32, i32)) -> bool {
        self.terrain_at((square.0, square.1, square.2 + 1)).no_floor
    }
}

/// Gives every fire on the level its turn: it spends or gains fuel, and grows if it has earned it.
///
/// Reference: the fire processor inside the per-turn field sweep (`src/map_field.cpp:945-1360`), whose arithmetic is
/// [`cdda_core_types::fire::fire_turn`]. Two of its inputs belong to the map rather than to the fire - how many fires touch it and whether
/// the ground under it can burn - so they arrive as answers the environment layer supplies, and how much the fire ate this turn arrives the
/// same way.
///
/// A fire whose turn leaves it dead - intensity gone, or its age run out - is taken off the store, which is what the sweep does to any
/// field that does not survive its turn.
/// Lights the fires the spread rule asked for.
///
/// One fire per square - a square is a column **and** a floor - and each is born with two minutes of burning already behind it, which is what makes a
/// newborn weak and short-lived rather than fuelled (`map_field.cpp:1122`, "Lower age is a longer lasting fire").
fn light_the_newborn(world: &mut World, newborn: Vec<(i32, i32, i32)>) {
    for (x, y, z) in newborn {
        let mut query = world.query::<(&Field, &Position)>();
        let taken = query.iter(world).any(|(field, position)| {
            (position.x, position.y, position.z) == (x, y, z) && field.id == FIRE
        });
        if taken {
            continue;
        }
        let mut entry = cdda_core_types::field::FieldEntry::new(3, 1_000_000, false, true);
        entry.set_age(120);
        entry.set_intensity(1);
        world.spawn((
            Field {
                id: FIRE,
                entry,
                priority: 0,
            },
            Position { x, y, z },
        ));
    }
}

/// One turn of every fire in the store, in the reference's order.
///
/// The order is the rule: the fires are visited in the sweep's own order (x, then y, then z), each one burns the **items on its square** before it eats the
/// ground (`map_field.cpp:976` then `:1050`), and the fires its turn lights are only added after the whole sweep has finished - a newborn is spared the
/// processors of the turn that made it, and the turn after that too, because it is listed before it exists.
pub fn process_fires(
    world: &mut World,
    map: &impl FireMap,
    windpower: i32,
    rolls: &mut cdda_core_types::rng::Rolls,
) {
    process_fires_in_submap(world, map, windpower, Where::Anywhere, rolls);
}

/// Every incendiary field on the squares of one submap, in the sweep's order.
///
/// Reference: `field_processor_fd_incendiary` is registered against `fd_incendiary` (`field_type.cpp:350`), so it runs for that field and no other. It is called here
/// rather than from a general dispatch table because the table does not exist yet and a processor nothing calls is a library, not a port.
pub fn process_incendiaries_in_submap(
    world: &mut World,
    scope: Where,
    facts: impl Fn(i32, i32, i32) -> TerrainFacts,
    rolls: &mut cdda_core_types::rng::Rolls,
) {
    let mut order: Vec<(i32, i32, i32, Entity)> = Vec::new();
    let mut query = world.query::<(Entity, &Position, &Field)>();
    for (entity, position, field) in query.iter(world) {
        if !scope.holds((position.x, position.y, position.z)) {
            continue;
        }
        if field.id == INCENDIARY {
            order.push((position.x, position.y, position.z, entity));
        }
    }
    order.sort_by_key(|&(x, y, z, entity)| (x, y, z, entity.to_bits()));
    for (_, _, _, entity) in order {
        process_incendiary(world, entity, &facts, rolls);
    }
}

/// Fires on the squares of one submap, for a caller walking the reference's sweep submap by submap.
///
/// See [`process_gas_in_submap`] for why the sweep is spatial: the reference ages a field when its submap comes up, so a fire born in a submap the sweep has already
/// passed keeps the age it was born with.
pub fn process_fires_in_submap(
    world: &mut World,
    map: &impl FireMap,
    windpower: i32,
    scope: Where,
    rolls: &mut cdda_core_types::rng::Rolls,
) {
    let mut order: Vec<(i32, i32, i32, Entity)> = Vec::new();
    let mut query = world.query::<(Entity, &Position, &Field)>();
    for (entity, position, field) in query.iter(world) {
        if !scope.holds((position.x, position.y, position.z)) {
            continue;
        }
        if field.id == FIRE {
            order.push((position.x, position.y, position.z, entity));
        }
    }
    order.sort_by_key(|&(x, y, z, entity)| (x, y, z, entity.to_bits()));
    let mut newborn: Vec<(i32, i32, i32)> = Vec::new();
    for (x, y, z, entity) in order {
        let square = (x, y, z);
        let Some((alive, age, field_intensity)) = world.get::<Field>(entity).map(|field| {
            (
                field.entry.is_alive(),
                field.entry.age(),
                field.entry.intensity(),
            )
        }) else {
            continue;
        };
        // A fire whose turn begins with nothing left is gone before the rules run, which is the sweep's own order for any field.
        if !alive {
            world.despawn(entity);
            continue;
        }
        // A field that has just been created is **spared its processors** and only decays (`map_field.cpp:445-499`). For a fire that means
        // its first turn on the map costs it a turn of age and nothing else - no fuel, no growth, no smoke. The recording caught this: the
        // reference's own trace goes 0, 1, 22, and a port that ran the rules on the first turn went 0, 21.
        if age == 0 {
            continue;
        }
        // **The items on the square are burned first** (`map_field.cpp:976-1048`): the block assigns `time_added` and `smoke` before the ground block adds to
        // them, so a fire standing on a plank and a tiled floor gets the plank's fraction of a turn *plus* what the floor gives.
        let burned = burn_items_at(
            world,
            square,
            field_intensity,
            !map.can_spread(square),
            rolls,
        );
        let fuel = cdda_core_types::fire::ground_fuel(
            map.ground(square),
            field_intensity,
            map.can_burn(square),
            windpower,
            rolls,
        );
        let turn = cdda_core_types::fire::fire_turn(
            age,
            field_intensity,
            map.adjacent_fires(square),
            map.can_burn(square),
            fuel.time_added + burned.time_added,
        );
        let intensity = turn.intensity;
        let age = turn.age;
        let spreads = map.can_spread(square);
        let dead = world.get_mut::<Field>(entity).is_some_and(|mut field| {
            field.entry.set_age(age);
            field.entry.set_intensity(intensity);
            !field.entry.is_alive()
        });
        if dead {
            world.despawn(entity);
            continue;
        }
        // The eight squares around it, in the reference's `eight_horizontal_neighbors` order, each described by what the map says.
        let mut neighbours = [cdda_core_types::fire::Neighbour::default(); 8];
        for (index, offset) in EIGHT_NEIGHBOURS.iter().enumerate() {
            if let Some(slot) = neighbours.get_mut(index) {
                *slot = map.neighbour((x + offset.0, y + offset.1, z));
            }
        }
        let burning = Burning {
            entity,
            square,
            intensity,
            age,
            spreads,
        };
        let adjacent = spread_and_flash(world, &burning, &neighbours, rolls, &mut newborn);
        // The neighbour count comes from the phase, because that is where the reference counts it: a fire that may not spread never looks, and reports
        // none.
        fire_discharge(
            world,
            square,
            &Discharging {
                intensity,
                adjacent_fires: adjacent,
                smoke_made: fuel.smoke + burned.smoke,
                windpower,
            },
            map,
            rolls,
        );
    }
    light_the_newborn(world, newborn);
}

/// The spread phase and the spread loop, in the reference's order, for one fire.
///
/// Reference: `map_field.cpp:1160-1400`. This is split out of [`process_fires`] only because the whole turn had grown past the point a reader can hold at
/// once; the order below **is** the rule:
///
/// 1. the wind-blocker scan start is drawn even though the port does not model wind blockers - skip it and every later die of the turn is one out;
/// 2. the flashpoint die, and whichever of its two branches it picks (see [`cdda_core_types::fire::spread_phase`]);
/// 3. the spread loop, which draws its own start.
///
/// Returns the number of neighbouring fires the phase counted, which the smoke and hot air gates use.
struct Burning {
    entity: Entity,
    square: (i32, i32, i32),
    intensity: i32,
    age: i64,
    spreads: bool,
}

fn spread_and_flash(
    world: &mut World,
    fire: &Burning,
    neighbours: &[cdda_core_types::fire::Neighbour; 8],
    rolls: &mut cdda_core_types::rng::Rolls,
    newborn: &mut Vec<(i32, i32, i32)>,
) -> i32 {
    let Burning {
        entity,
        square,
        intensity,
        age,
        spreads,
    } = *fire;
    let (x, y, z) = square;
    let _wind_scan = rolls.rng(0, 7);
    let phase = cdda_core_types::fire::spread_phase(intensity, age, spreads, neighbours, rolls);
    for index in &phase.boosted {
        if let Some(offset) = EIGHT_NEIGHBOURS.get(*index) {
            let target = (x + offset.0, y + offset.1, z);
            let boosted = {
                let mut query = world.query::<(Entity, &Position, &Field)>();
                query
                    .iter(world)
                    .find(|(_, position, field)| {
                        (position.x, position.y, position.z) == target && field.id == FIRE
                    })
                    .map(|(entity, _, _)| entity)
            };
            if let Some(boosted) = boosted
                && let Some(mut field) = world.get_mut::<Field>(boosted)
            {
                // `mod_field_intensity( 1 )` only while it is below two: a fire that is already raging gains nothing but the fuel.
                let hotter = if field.entry.intensity() < 2 {
                    field.entry.intensity() + 1
                } else {
                    field.entry.intensity()
                };
                let fed = field.entry.age() - 5 * 60;
                field.entry.set_intensity(hotter);
                field.entry.set_age(fed);
            }
        }
    }
    if (phase.spent != 0 || phase.grown.is_some())
        && let Some(mut field) = world.get_mut::<Field>(entity)
    {
        if let Some((grown, grown_age)) = phase.grown {
            field.entry.set_intensity(grown);
            field.entry.set_age(grown_age);
        } else {
            let spent = field.entry.age() + phase.spent;
            field.entry.set_age(spent);
        }
    }
    let caught = if spreads {
        cdda_core_types::fire::spread_fire(rolls, intensity, spreads, neighbours)
    } else {
        Vec::new()
    };
    let did_spread = !caught.is_empty();
    for index in caught {
        if let Some(offset) = EIGHT_NEIGHBOURS.get(index) {
            newborn.push((x + offset.0, y + offset.1, z));
        }
    }
    if did_spread {
        // Spreading costs the fire a minute of its own life.
        if let Some(mut field) = world.get_mut::<Field>(entity) {
            let spent = field.entry.age() + 60;
            field.entry.set_age(spent);
        }
    }
    phase.adjacent
}

/// The eight squares around one, in the reference's order.
///
/// The order is `eight_horizontal_neighbors` (`point.h:450`), and it is **not** a clockwise walk: west comes before east, and the southern row runs
/// south-west, south, south-east. I had it as a plain clockwise ring for four rounds, which cost nothing while only the fire's own square mattered and cost
/// exactly one square as soon as the parity test could see where a fire spread: the dice agreed, the square was one step round the ring.
pub const EIGHT_NEIGHBOURS: [(i32, i32); 8] = [
    (-1, -1),
    (0, -1),
    (1, -1),
    (-1, 0),
    (1, 0),
    (-1, 1),
    (0, 1),
    (1, 1),
];
/// Gives every square holding this gas its turn, moving gas to neighbours that will take it.
///
/// Reference: the field sweep's call to `spread_gas` (`src/map_field.cpp:582`), which runs once per gas per turn and is what makes hot air, smoke and tear
/// gas move. Whether a field is a gas is `percent_spread` in its data, not a processor of its own, so this system takes the id and the rate rather than
/// hard-coding either.
///
/// The order is the store's, square by square, and a gas that moves is recorded by [`cdda_core_types::fire::spread_gas`]'s arithmetic: the source keeps
/// `age - age / intensity` and the destination receives `age / intensity`, thickening if it already held the gas. A square whose intensity reaches zero
/// leaves the store, which is the sweep's behaviour for any field that does not survive its turn.
///
/// The store is two dimensional, so the two vertical directions the rule names have nowhere to go: they are offered as `false` and skipped, which keeps the
/// vertical half of the rule a wiring change rather than a rewrite.
pub fn process_gas(
    world: &mut World,
    id: FieldId,
    percent_spread: i32,
    windpower: i32,
    can_take: impl Fn((i32, i32, i32), (i32, i32, i32)) -> bool + Copy,
    rolls: &mut cdda_core_types::rng::Rolls,
) {
    process_gas_in_submap(world, id, Where::Anywhere, percent_spread, windpower, can_take, rolls);
}

/// One gas, on the squares of one submap, for a caller walking the reference's sweep submap by submap.
///
/// The reference's `process_fields` visits submaps in ascending `(z, x, y)` and ages a field when it reaches the field's own submap, so a gas that falls into a submap
/// the sweep has already passed keeps the age it was born with while one that spreads into a later submap is aged in the same turn. A caller that processed every gas
/// and aged every field at the end of the turn would get the second case right and the first wrong.
/// The wind at a square: what the spread rule reads, and nothing about the weather beyond it.
///
/// Reference: `map::spread_gas` (`src/map_field.cpp:279-284`) asks `get_local_windpower` about the square the gas stands on and `g->is_sheltered` there, so the wind is a
/// property of a **square**, not of a turn: a gas on a roof and one in the cellar under it stand in different winds on the same turn.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Wind {
    /// The local wind power at the square, which sets the spread chance's ceiling and the blockers' resistance.
    pub power: i32,
    /// Which way the wind blows, in degrees.
    pub direction: i32,
    /// Whether the square is under a roof. A sheltered gas never feels the wind, however hard it blows.
    pub sheltered: bool,
}

/// Every gas of one kind on the squares of one submap, with the wind read per square.
///
/// The sweep's door: `run_environment_turn` walks submaps in the reference's order and hands this the weather, a fixture replay hands it the recorded wind of each
/// square, and both get the same rule.
/// What a caller knows about a gas's world that the rule cannot see for itself.
#[derive(Debug, Clone, Copy)]
pub struct Surroundings<W, T> {
    /// The wind at a square.
    pub wind_at: W,
    /// The half-life of a field type, which a newborn must be born with.
    pub half_life_of: T,
}

/// Every gas of one kind on the squares of one submap, with the wind read **per square** and newborns born on their type's clock.
///
/// The sweep's door. `run_environment_turn` hands it the weather and the loaded field data; a fixture replay hands it the recorded wind of each square and the
/// prototype dump's half-life, and both get the same rule.
pub fn process_gas_in_wind(
    world: &mut World,
    id: FieldId,
    scope: Where,
    percent_spread: i32,
    surroundings: Surroundings<impl Fn((i32, i32, i32)) -> Wind + Copy, impl Fn(FieldId) -> i64 + Copy>,
    can_take: impl Fn((i32, i32, i32), (i32, i32, i32)) -> bool + Copy,
    rolls: &mut cdda_core_types::rng::Rolls,
) {
    let Surroundings {
        wind_at,
        half_life_of,
    } = surroundings;
    let mut order: Vec<(i32, i32, i32, Entity)> = Vec::new();
    let mut query = world.query::<(Entity, &Position, &Field)>();
    for (entity, position, field) in query.iter(world) {
        if !scope.holds((position.x, position.y, position.z)) {
            continue;
        }
        if field.id == id {
            order.push((position.x, position.y, position.z, entity));
        }
    }
    order.sort_by_key(|&(x, y, z, entity)| (x, y, z, entity.to_bits()));
    for (x, y, z, entity) in order {
        process_one_gas(
            world,
            entity,
            percent_spread,
            wind_at((x, y, z)),
            half_life_of(id),
            can_take,
            rolls,
        );
    }
}

/// Every gas of one kind on the squares of one submap, with one wind for all of them.
///
/// The older door, kept for callers that have a single wind to give - a smoke test, an old per-field turn. The sweep uses [`process_gas_in_wind`], because a gas on a
/// roof and one in the cellar under it stand in different winds on the same turn.
pub fn process_gas_in_submap(
    world: &mut World,
    id: FieldId,
    scope: Where,
    percent_spread: i32,
    windpower: i32,
    can_take: impl Fn((i32, i32, i32), (i32, i32, i32)) -> bool + Copy,
    rolls: &mut cdda_core_types::rng::Rolls,
) {
    let mut order: Vec<(i32, i32, i32, Entity)> = Vec::new();
    let mut query = world.query::<(Entity, &Position, &Field)>();
    for (entity, position, field) in query.iter(world) {
        if !scope.holds((position.x, position.y, position.z)) {
            continue;
        }
        if field.id == id {
            order.push((position.x, position.y, position.z, entity));
        }
    }
    order.sort_by_key(|&(x, y, z, entity)| (x, y, z, entity.to_bits()));
    for (_, _, _, entity) in order {
        let wind = Wind {
            power: windpower,
            direction: 0,
            sheltered: false,
        };
        process_one_gas(world, entity, percent_spread, wind, 1_000_000, can_take, rolls);
    }
}

/// One field's turn of spreading: the whole rule, for a single entity.
///
/// The single source of the gas rule, called both by the sweep and by the older per-field door, because two implementations that disagree are worse than one that is
/// wrong: the session gates were green while the older path silently dropped every fall.
fn process_one_gas(
    world: &mut World,
    entity: Entity,
    percent_spread: i32,
    wind: Wind,
    half_life: i64,
    can_take: impl Fn((i32, i32, i32), (i32, i32, i32)) -> bool,
    rolls: &mut cdda_core_types::rng::Rolls,
) {
    let Some(field) = world.get::<Field>(entity) else {
        return;
    };
    let intensity = field.entry.intensity();
    let age = field.entry.age();
    let id = field.id;
    let Some(position) = world.get::<Position>(entity).copied() else {
        return;
    };
    let (x, y, z) = (position.x, position.y, position.z);
    {
        // **What the squares hold right now**, which is not what they held when the turn began: an earlier square in the same sweep may have moved gas into or out of
        // one of them, and the reference reads the live field (`gas_can_spread_to`, `src/map_field.cpp:237-248`). Read once per source square, because nothing
        // changes between these ten questions and this square's own move.
        let standing = field_strengths(world, id);
        // The half of `gas_can_spread_to` that is about fields: a square takes gas only when it holds none, or holds a **weaker** one of the same kind. The caller
        // answers the other half - whether the square can be walked, and whether a step to another level is legal - because that is a question about terrain.
        let takes = |to: (i32, i32, i32)| {
            can_take((x, y, z), to) && standing.get(&to).copied().unwrap_or(0) < intensity
        };
        // Below first, above last: the answers are collected here in the order `spread_gas` tries them.
        let mut beside = [false; 8];
        for (index, offset) in EIGHT_NEIGHBOURS.iter().enumerate() {
            if let Some(slot) = beside.get_mut(index) {
                *slot = takes((x + offset.0, y + offset.1, z));
            }
        }
        let around = cdda_core_types::fire::Around {
            below: takes((x, y, z - 1)),
            beside,
            above: takes((x, y, z + 1)),
            // The caller owns the weather; this rule owns what to do with it.
            sheltered: wind.sheltered,
            windpower: wind.power,
            wind_direction: wind.direction,
        };
        let movement = cdda_core_types::fire::spread_gas(intensity, age, percent_spread, &around, rolls);
        let destination = match movement.direction {
            cdda_core_types::fire::GasDirection::Side(index) => neighbour_of(x, y, z, index),
            cdda_core_types::fire::GasDirection::Down => Some((x, y, z - 1)),
            cdda_core_types::fire::GasDirection::Up => Some((x, y, z + 1)),
            cdda_core_types::fire::GasDirection::None => None,
        };
        let Some(destination) = destination else {
            return;
        };
        apply_gas_spread(world, entity, destination, id, &movement, intensity, half_life);

    }
}

/// Moves one gas: the square it reaches gains an intensity and the share of the age it was given, and the square it left loses both.
///
/// Split out of [`process_gas_in_submap`] because one half is arithmetic and the other touches the world: the rule decides **where** the gas goes, this decides what
/// each end looks like afterwards.
fn apply_gas_spread(
    world: &mut World,
    entity: Entity,
    destination: (i32, i32, i32),
    id: FieldId,
    movement: &cdda_core_types::fire::GasSpread,
    intensity: i32,
    half_life: i64,
) {
    let existing = {
        let mut query = world.query::<(Entity, &Position, &Field)>();
        query
            .iter(world)
            .find(|(_, position, field)| {
                (position.x, position.y, position.z) == destination && field.id == id
            })
            .map(|(entity, _, field)| (entity, field.entry.intensity(), field.entry.age()))
    };
    if let Some((taken, thick, taken_age)) = existing {
        if let Some(mut field) = world.get_mut::<Field>(taken) {
            field.entry.set_intensity(thick + 1);
            field.entry.set_age(taken_age + movement.transferred_age);
        }
    } else {
        // The newborn carries the **type's own** half-life: a mist born from a spreading mist must decay on the same clock as its parent, and a placeholder here
        // is a field that outlives the reference's by however much the placeholder is larger.
        let mut entry = cdda_core_types::field::FieldEntry::new(intensity.max(1), half_life, false, false);
        entry.set_intensity(1);
        entry.set_age(movement.transferred_age);
        world.spawn((
            Field {
                id,
                entry,
                priority: 0,
            },
            Position {
                x: destination.0,
                y: destination.1,
                z: destination.2,
            },
        ));
    }
    let died = world.get_mut::<Field>(entity).is_some_and(|mut field| {
        field.entry.set_intensity(movement.source_intensity);
        field.entry.set_age(movement.source_age);
        !field.entry.is_alive()
    });
    if died {
        world.despawn(entity);
    }
}

/// Which squares a sweep step should look at.
///
/// The reference's sweep is nested square by square inside a submap and decays a field **as it reaches its square** (`map_field.cpp:164-184` plus the per-square body
/// of `process_fields_in_submap`), so a newborn is aged this turn only if the sweep has not yet passed the square it was born on. A scope is how the rules keep that
/// option open: the sweep asks for one square at a time, and older doors ask for a whole submap or for everywhere.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Where {
    /// Every field of the kind, wherever it stands.
    Anywhere,
    /// Only the fields standing on the squares of one submap.
    Submap((i32, i32, i32)),
    /// Only the fields on one square.
    Square((i32, i32, i32)),
}

impl Where {
    /// Whether a field standing at this square is in scope.
    #[must_use]
    pub fn holds(self, square: (i32, i32, i32)) -> bool {
        match self {
            Where::Anywhere => true,
            Where::Submap(key) => submap_of(square.0, square.1, square.2) == key,
            Where::Square(only) => square == only,
        }
    }
}

/// What each square holds of one gas, right now.
fn field_strengths(
    world: &mut World,
    id: FieldId,
) -> std::collections::BTreeMap<(i32, i32, i32), i32> {
    let mut query = world.query::<(&Position, &Field)>();
    query
        .iter(world)
        .filter(|(_, field)| field.id == id && field.entry.is_alive())
        .map(|(position, field)| ((position.x, position.y, position.z), field.entry.intensity()))
        .collect()
}

/// Which submap a square belongs to: the reference's `my_MAPSIZE` grid, twelve squares to a side (`map::process_fields`, `src/map_field.cpp:164-184`).
///
/// The sweep walks these in ascending `(z, x, y)` order and ages a field when it reaches the field's own submap, which is why a gas that **falls** into the submap the
/// sweep has already passed keeps the age it was born with, while one that spreads into a later submap is aged in the same turn.
#[must_use]
pub fn submap_of(x: i32, y: i32, z: i32) -> (i32, i32, i32) {
    (z, x.div_euclid(12), y.div_euclid(12))
}

/// The submaps holding at least one field, in the sweep's own order.
///
/// Rebuilt at every step of the walk rather than taken once, because a field born in a **later** submap is visited by the same sweep.
#[must_use]
pub fn submaps_with_fields(world: &mut World) -> Vec<(i32, i32, i32)> {
    let mut query = world.query::<(&Position, &Field)>();
    let mut keys: Vec<(i32, i32, i32)> = query
        .iter(world)
        .map(|(position, _)| submap_of(position.x, position.y, position.z))
        .collect();
    keys.sort_unstable();
    keys.dedup();
    keys
}

/// The square one step away at a neighbour index on the same floor, in this project's neighbour order.
fn neighbour_of(x: i32, y: i32, z: i32, index: usize) -> Option<(i32, i32, i32)> {
    EIGHT_NEIGHBOURS
        .get(index)
        .map(|offset| (x + offset.0, y + offset.1, z))
}

/// A thing that gives off light: a torch, a fire, a lamp.
#[derive(Debug, Clone, Copy, Component)]
pub struct LightSource {
    /// How bright it is, in the reference's luminance units.
    pub luminance: f32,
}

/// The brightness of each square, in the reference's four quarters.
///
/// An answer, not a thing: written by the lighting system whenever something that casts light
/// has moved or died, and thrown away when it is stale.
#[derive(Debug, Clone, Default, Component)]
pub struct Light {
    squares: BTreeMap<(i32, i32), FourQuadrants>,
    /// Whether something has changed that the answer does not account for yet.
    stale: bool,
    /// How many times the answer has been built, which is the light's own receipt.
    rebuilds: u32,
}

impl Light {
    /// Whether the answer has to be rebuilt before it can be trusted.
    #[must_use]
    pub fn is_stale(&self) -> bool {
        self.stale
    }

    /// Notes that something changed which the answer does not account for.
    ///
    /// Reference: the weather's sight penalty dirties every level's light (`weather.cpp:938-941`).
    pub fn mark_stale(&mut self) {
        self.stale = true;
    }

    /// Records that the answer was just built.
    pub fn note_build(&mut self) {
        self.stale = false;
        self.rebuilds = self.rebuilds.saturating_add(1);
    }

    /// How many times the answer has been built.
    #[must_use]
    pub fn rebuilds(&self) -> u32 {
        self.rebuilds
    }

    /// The quarters of a square, if anything lights it.
    #[must_use]
    pub fn at(&self, x: i32, y: i32) -> Option<FourQuadrants> {
        self.squares.get(&(x, y)).copied()
    }

    /// How bright a square is at its brightest corner.
    #[must_use]
    pub fn max_at(&self, x: i32, y: i32) -> f32 {
        self.squares.get(&(x, y)).map_or(0.0, FourQuadrants::max)
    }

    /// The map the rule functions read.
    ///
    /// The rules were written against a `BTreeMap` of quarters because that is what a cast produces; the resource is the same thing with a name, so it hands over its own map
    /// rather than a copy per square.
    #[must_use]
    pub fn squares_for_the_rules(&self) -> BTreeMap<(i32, i32), FourQuadrants> {
        self.squares.clone()
    }

    /// How many squares anything lights.
    #[must_use]
    pub fn lit_squares(&self) -> usize {
        self.squares.len()
    }

    /// Raises every square by what another answer says, taking the brighter corner.
    ///
    /// Light sources and the sun do not add up into something brighter than either: the
    /// reference keeps the maximum per quarter (`update_light_quadrants`,
    /// `src/shadowcasting.h:56-62`), so a torch in daylight does not make a square "more than
    /// daytime".
    fn merge(&mut self, other: BTreeMap<(i32, i32), FourQuadrants>) {
        for (square, quarters) in other {
            match self.squares.get(&square) {
                Some(existing) => {
                    self.squares.insert(square, existing.max_with(quarters));
                }
                None => {
                    self.squares.insert(square, quarters);
                }
            }
        }
    }
}

/// Reference: `map::generate_lightmap` (`src/lightmap.cpp:443-560`), which clears the map,
/// casts every source and applies the sun. The store is the authority for what is where: the
/// terrain comes from the level resource, and smoke dims a square by being an entity standing
/// on it, not by being a copy of the level's own map.
/// Applies what a weather update reported, and answers whether the light was flushed.
///
/// Reference: `weather.cpp:938-941`. The condition is the **sight penalty**, not the weather:
/// `sunny` to `clear` changes the sky and costs nothing, while `clear` to `fog` dims every ray and
/// has to dirty the light.
pub fn apply_weather_change(
    world: &mut bevy_ecs::prelude::World,
    change: cdda_content::weather::Change,
) -> bool {
    match change {
        cdda_content::weather::Change::Weather { sight_changed: true } => {
            singletons::mark_light_stale(world);
            true
        }
        _ => false,
    }
}

/// Terrain that light cannot pass.
pub const SOLID_TERRAIN: u32 = 0;

/// The loaded prototypes the map asks about: one table per layer.
///
/// Two instances of the same type rather than one, because terrain and furniture are separate id spaces and code three means two different things. Keeping them in one
/// resource also means a query does not have to be handed the tables it needs - the world already carries them, the way the reference's world carries its prototype
/// registry.
#[derive(Debug, Clone, Default, Component)]
pub struct Prototypes {
    /// What each terrain code says about light.
    pub terrain: terrain_table::TerrainTable,
    /// What each furniture code says about light.
    pub furniture: terrain_table::TerrainTable,
}

/// Whether one square can see another, with both layers of every square and the fields counted.
///
/// Reference: `map::sees` (`src/map.cpp:7329-7362`) over `map::is_transparent`, which reads the transparency cache **with** the fields in it. Prefer
/// [`sees_through_fields`] for the reference's other question, the one that ignores them.
#[must_use]
pub fn sees(world: &World, from: (i32, i32), to: (i32, i32), range: i32) -> bool {
    sees_through_fields(world, from, to, range, true)
}

/// Whether one square can see another, choosing whether the fields count.
///
/// Reference: the reference keeps two casts, `is_transparent` and `is_transparent_wo_fields`, and picks between them with a member pointer at the top of `map::sees`
/// (`map.cpp:7337-7338`). This is the same choice, made by the caller.
#[must_use]
pub fn sees_through_fields(
    world: &World,
    from: (i32, i32),
    to: (i32, i32),
    range: i32,
    with_fields: bool,
) -> bool {
    let prototypes = singletons::prototypes(world);
    let (terrain, furniture) = match prototypes {
        Some(prototypes) => (&prototypes.terrain, &prototypes.furniture),
        // Nothing loaded: every square is air, which is the honest answer for a world with no prototypes rather than a reason to refuse to look.
        None => {
            return cdda_core_types::vision::sees_on_one_level(
                from,
                to,
                range,
                |_x, _y| true,
                |_, _| true,
            );
        }
    };
    let fields = if with_fields {
        Some(field_opacity(world))
    } else {
        None
    };
    cdda_core_types::vision::sees_on_one_level(
        from,
        to,
        range,
        |x, y| {
            let terrain_code = crate::square::terrain_at(world, (x, y, 0)).unwrap_or(0);
            let furniture_code = crate::square::furniture_at(world, (x, y, 0)).unwrap_or(0);
            let mut transparency = terrain
                .transparency(terrain_code)
                .min(furniture.transparency(furniture_code));
            if let Some(fields) = &fields {
                transparency *= fields.multiplier_at(x, y);
            }
            transparency > 0.0
        },
        |x, y| crate::square::square_at(world, (x, y, 0)).is_some(),
    )
}

/// How much each square's fields multiply its transparency by.
///
/// Reference: the field half of `calc_transp` (`lightmap.cpp:155-165`), which **multiplies** a square's transparency by every field that is not transparent. The numbers
/// read backwards from what they look like: `TRANSPARENCY_OPEN_AIR` is about `0.032` and smoke at its second intensity is `10`, so a room with smoke in it is ten times
/// as absorbing as one without, and smoke at its third intensity is `0`, which blinds the line completely.
///
/// A square with no fields multiplies by one.
pub(crate) fn field_opacity(world: &World) -> FieldOpacityGrid {
    let mut squares: BTreeMap<(i32, i32), f32> = BTreeMap::new();
    let table = singletons::field_table(world);
    let mut query = world.try_query::<(Entity, &Position, &Field)>();
    if let Some(query) = query.as_mut() {
        for (_entity, position, field) in query.iter(world) {
            let intensity = field.entry.intensity();
            let translucency = table.map_or(0.0, |table| table.translucency(field.id.0, intensity));
            let entry = squares.entry((position.x, position.y)).or_insert(1.0);
            *entry *= translucency;
        }
    }
    FieldOpacityGrid { squares }
}

/// The strongest field on each square, for the questions that ask whether a square is see-through.
struct FieldOpacityGrid {
    squares: BTreeMap<(i32, i32), f32>,
}

impl FieldOpacityGrid {
    /// What the square's fields multiply its transparency by, one when it has none.
    fn multiplier_at(&self, x: i32, y: i32) -> f32 {
        self.squares.get(&(x, y)).copied().unwrap_or(1.0)
    }
}

/// Works out the light from what the world has loaded: both layers of every square, the fields, and the weather.
///
/// Reference: `map::build_map_cache`'s order (`map.cpp:9556-9557`) - the sky is decided **first**, because the transparency rule charges the weather for the squares that are
/// in it. `sight_penalty` is the weather's cost: `1` for a clear sky, above one for rain, fog or snow (the data gives rain 1.1, snow 1.3 and fog 1.7).
///
/// The prototypes come from the [`Prototypes`] resource and the fields from [`field_table::FieldTable`], so there is one source for what a code means and no way to cast
/// with a table the world does not have.
pub fn cast_lights_with(world: &mut World) {
    let sight_penalty = singletons::weather(world).unwrap_or_default().sight_penalty();
    // The sky map: an `INDOORS` flag on either layer covers the three-by-three block around it (`build_outside_cache`).
    let (terrain, furniture) = singletons::prototypes(world).map_or_else(
        || {
            (
                terrain_table::TerrainTable::new(),
                terrain_table::TerrainTable::new(),
            )
        },
        |prototypes| (prototypes.terrain.clone(), prototypes.furniture.clone()),
    );
    let terrain_codes = crate::square::terrain_by_square(world);
    let furniture_codes = crate::square::furniture_by_square(world);
    let side = crate::square::extent(world);
    let mut indoors = vec![false; side * side];
    for y in 0..side {
        for x in 0..side {
            let terrain_code = terrain_codes
                .get(&(as_i32(x), as_i32(y)))
                .copied()
                .unwrap_or(0);
            let furniture_code = furniture_codes
                .get(&(as_i32(x), as_i32(y)))
                .copied()
                .unwrap_or(0);
            if let Some(slot) = indoors.get_mut(x * side + y) {
                *slot = terrain.is_indoors(terrain_code) || furniture.is_indoors(furniture_code);
            }
        }
    }
    let outside = cdda_core_types::outside_cache::outside_of(
        STREET_LEVEL,
        &cdda_core_types::outside_cache::OutsideFacts {
            side,
            indoors: indoors.clone(),
        },
    );
    let fields = field_opacity(world);
    cast_lights_inner(
        world,
        |code| terrain.transparency(code),
        |code| furniture.transparency(code),
        |x, y| {
            let (Ok(xu), Ok(yu)) = (usize::try_from(x), usize::try_from(y)) else {
                return false;
            };
            if xu >= side || yu >= side {
                // Off the level the world has not made: not under a roof we know of.
                return true;
            }
            outside.get(xu * side + yu).copied().unwrap_or(true)
        },
        &fields,
        sight_penalty,
    );
}

/// A square and how brightly it shines.
type Emitter = (i32, i32, f32);

/// The reference's two lists of lights: what the map's own data emits, which is buffered, and what a caller placed, which is not.
type LightSources = (Vec<Emitter>, Vec<Emitter>);

/// Every light the world holds: what a caller placed, and what the terrain, the furniture and the fields emit themselves.
///
/// Reference: `map::generate_lightmap`'s gathering loop (`lightmap.cpp:525-547`), which walks every square and adds a source for each terrain, furniture and field whose own
/// data says it emits one, on top of the items and creatures it also reads. The port has no items or creatures yet; the three that come from the loaded prototypes do.
fn light_sources(world: &mut World) -> LightSources {
    // What a caller placed - a torch, a component someone spawned - is the port's stand-in for the reference's items and creatures: it casts immediately and is not buffered.
    let mut placed: Vec<Emitter> = Vec::new();
    let mut source_query = world.query::<(&Position, &LightSource)>();
    for (position, source) in source_query.iter(world) {
        placed.push((position.x, position.y, source.luminance));
    }
    // The world's own emitters, gathered the way `map::generate_lightmap` gathers them (`lightmap.cpp:525-547`): a burning square is a light, a lamp terrain is a light,
    // and the number is each one's own data rather than something a caller placed.
    let mut emitters: BTreeMap<(i32, i32), f32> = BTreeMap::new();
    {
        // **The squares that exist, not a rectangle around them.** A level is a set of squares, and
        // walking the box they fit in would ask about every hole - and ask the world for the
        // prototypes once per hole, which is how a bulk loop turns into a stall. The tables come out
        // once, outside the walk.
        let terrain_codes = crate::square::terrain_by_square(world);
        let furniture_codes = crate::square::furniture_by_square(world);
        let (terrain, furniture) = singletons::prototypes(world).map_or_else(
            || {
                (
                    terrain_table::TerrainTable::new(),
                    terrain_table::TerrainTable::new(),
                )
            },
            |prototypes| (prototypes.terrain.clone(), prototypes.furniture.clone()),
        );
        for (square, code) in &terrain_codes {
            let furniture_code = furniture_codes.get(square).copied().unwrap_or(0);
            let emitted = terrain.light(*code).max(furniture.light(furniture_code));
            if emitted > 0.0 {
                emitters.insert(*square, emitted);
            }
        }
        // Furniture on a square with no ground is still something the data can light.
        for (square, code) in &furniture_codes {
            if terrain_codes.contains_key(square) {
                continue;
            }
            let emitted = furniture.light(*code);
            if emitted > 0.0 {
                emitters.insert(*square, emitted);
            }
        }
    }
    // The table is cloned out first: a `query` borrows the world mutably, and the table is immutable, so holding both at once is the borrow checker's business rather
    // than something to work around with an index.
    let table = singletons::field_table(world).cloned().unwrap_or_default();
    let mut field_query = world.query::<(&Position, &Field)>();
    for (position, field) in field_query.iter(world) {
        let emitted = table.light(field.id.0, field.entry.intensity());
        if emitted > 0.0 {
            let entry = emitters.entry((position.x, position.y)).or_insert(0.0);
            *entry = entry.max(emitted);
        }
    }
    let buffered: Vec<Emitter> = emitters
        .into_iter()
        .map(|(square, luminance)| (square.0, square.1, luminance))
        .collect();
    (buffered, placed)
}

fn as_i32(value: usize) -> i32 {
    i32::try_from(value).unwrap_or(i32::MAX)
}

/// The one implementation every entry point above funnels into.
fn cast_lights_inner(
    world: &mut World,
    terrain_transparency: impl Fn(u32) -> f32,
    furniture_transparency: impl Fn(u32) -> f32,
    outside_of_square: impl Fn(i32, i32) -> bool,
    fields: &FieldOpacityGrid,
    sight_penalty: f32,
) {
    // Fields multiply a square's transparency, layer by layer (`map::build_transparency_cache`, `lightmap.cpp:158-165`), where the
    // The multiplier's direction is worth stating because it reads backwards: the cast attenuates by `e^( transparency * distance )`, so a **higher**
    // transparency is more absorbing. Open air is `2.3/72`; smoke at its second intensity is `10`, which is why smoke blinds the room it fills, and `fd_fire`
    // uses `0.7`, `0.4` and `0.1` - each *less* absorbing than open air, because a fire is meant to be seen.
    //
    // The grid is built once for the whole cast, by the same function the sight query asks, so a square the ray cannot see through is the same square that stops
    // a light: one answer, two readers.
    let mut dimming: BTreeMap<(i32, i32), Vec<cdda_core_types::transparency::FieldOpacity>> =
        BTreeMap::new();
    for ((x, y), translucency) in &fields.squares {
        dimming
            .entry((*x, *y))
            .or_default()
            .push(cdda_core_types::transparency::FieldOpacity {
                // A field that is `transparent` is skipped by the reference; the loaded levels carry their own flag and a multiplier of exactly one would be the
                // identity anyway, so the grid keeps only the number.
                transparent: false,
                translucency: *translucency,
            });
    }

    let (buffered, placed) = light_sources(world);

    // Both layers come out of the store in one sweep each, before any casting: the cast walks the
    // window per source, and reading a map is far cheaper than asking the store per ray.
    let terrains: BTreeMap<(i32, i32), u32> = crate::square::terrain_by_square(world);
    let furniture: BTreeMap<(i32, i32), u32> = crate::square::furniture_by_square(world);
    // A square the world has made is a square; one it has not is solid, which `transparency_grid`
    // enforces on its own.
    let exists = |x: i32, y: i32| terrains.contains_key(&(x, y));
    let facts = |field_x: i32, field_y: i32| cdda_core_types::transparency::SquareFacts {
        terrain_transparent: terrains
            .get(&(field_x, field_y))
            .is_none_or(|id| terrain_transparency(*id) > 0.0),
        // The conjunction of the two layers (`lightmap.cpp:146-149`): either one being opaque stops the ray.
        furniture_transparent: furniture
            .get(&(field_x, field_y))
            .is_none_or(|code| furniture_transparency(*code) > 0.0),
        outside: outside_of_square(field_x, field_y),
        fields: dimming
            .get(&(field_x, field_y))
            .cloned()
            .unwrap_or_default(),
    };
    let mut light: BTreeMap<(i32, i32), cdda_core_types::shadowcasting::FourQuadrants> =
        BTreeMap::new();
    // **Record first, cast second** (`lightmap.cpp:463-464`, `654-659`): every source the map itself emits is buffered, and only then applied, so each square can ask
    // whether a neighbour is already lit at least as brightly before deciding which directions are worth casting into.
    let mut buffer: BTreeMap<(i32, i32), f32> = BTreeMap::new();
    for (x, y, luminance) in buffered {
        let entry = buffer.entry((x, y)).or_insert(0.0);
        if luminance > *entry {
            *entry = luminance;
        }
    }
    cdda_core_types::level::apply_buffered_sources(
        &buffer,
        sight_penalty,
        &exists,
        facts,
        &mut light,
    );
    // Then the sources a caller placed, which the reference casts immediately and does not buffer.
    for (x, y, luminance) in placed {
        cdda_core_types::level::cast_light_source(
            &cdda_core_types::level::Source {
                at: (x, y),
                luminance,
                sight_penalty,
                facts: &facts,
            },
            &exists,
            &mut light,
            [true; cdda_core_types::shadowcasting::OCTANTS],
        );
    }
    // Sources merge rather than replace: the sun may already have written an answer this
    // turn, and a torch does not undo the daylight.
    {
        let mut answer = singletons::light_mut(world);
        answer.merge(light);
        answer.note_build();
    }
}

/// Works out the sunlight on the level and raises [`Light`] by it.
///
/// Reference: the sun half of `map::generate_lightmap` (`src/lightmap.cpp:443-470`) calling
/// `build_sunlight_cache`, and the ambient pass that lifts the quarters of a tile facing open
/// sky. Daylight and torches meet here, in one answer, rather than in two that have to be
/// reconciled by whoever reads them.
pub fn cast_sunlight(
    world: &mut World,
    turns: f64,
    indoors: f32,
    outside: impl Fn(i32, i32) -> bool,
) {
    #[allow(clippy::cast_possible_truncation)]
    let altitude = cdda_core_types::astronomy::sun_altitude_degrees(turns) as f32;
    let outside_light = cdda_core_types::light::sun_light_from_altitude(altitude);
    run_sunlight(world, outside_light, indoors, 1.0, &outside);
}

/// Works out the daylight from loaded prototypes and the transcribed cascade.
///
/// Reference: `build_sunlight_cache` (`lightmap.cpp:306-437`), which is not a cast - daylight is parallel - but a walk down the levels where each square takes its light from
/// the square above it or, failing that, from one of the four cardinals.
///
/// # The roof is what "indoors" means
///
/// The port has one level and no roof tiles above it, so "indoors" has to be given a meaning. The one that matches the reference is: a square the outside cache calls indoors
/// is a square **under a roof**, so the level above it at that square is opaque and has a floor. That is exactly what the reference's `prev_transparency_cache` and
/// `prev_floor_cache` would say about a hut - and it is why daylight still reaches a square just inside a doorway, from the cardinal that is in the open.
pub fn cast_sunlight_with(world: &mut World, turns: f64) {
    let sight_penalty = singletons::weather(world).unwrap_or_default().sight_penalty();
    let (terrain, furniture) = singletons::prototypes(world).map_or_else(
        || {
            (
                terrain_table::TerrainTable::new(),
                terrain_table::TerrainTable::new(),
            )
        },
        |prototypes| (prototypes.terrain.clone(), prototypes.furniture.clone()),
    );
    #[allow(clippy::cast_possible_truncation)]
    let altitude = cdda_core_types::astronomy::sun_altitude_degrees(turns) as f32;
    let outside_light = cdda_core_types::light::sun_light_from_altitude(altitude);
    let inside_light =
        cdda_core_types::sunlight_cache::inside_light_level(STREET_LEVEL, outside_light);

    let terrain_codes = crate::square::terrain_by_square(world);
    let furniture_codes = crate::square::furniture_by_square(world);
    if terrain_codes.is_empty() {
        return;
    }
    let side = crate::square::extent(world);
    let cells = side * side;

    // The sky: open where the outside cache says so, roofed where it does not.
    let mut indoors = vec![false; cells];
    for y in 0..side {
        for x in 0..side {
            let terrain_code = terrain_codes
                .get(&(as_i32(x), as_i32(y)))
                .copied()
                .unwrap_or(0);
            let furniture_code = furniture_codes
                .get(&(as_i32(x), as_i32(y)))
                .copied()
                .unwrap_or(0);
            if let Some(slot) = indoors.get_mut(x * side + y) {
                *slot = terrain.is_indoors(terrain_code) || furniture.is_indoors(furniture_code);
            }
        }
    }
    let outside_cache = cdda_core_types::outside_cache::outside_of(
        STREET_LEVEL,
        &cdda_core_types::outside_cache::OutsideFacts {
            side,
            indoors: indoors.clone(),
        },
    );

    let (layer, above) = sunlayers(
        &terrain_codes,
        &furniture_codes,
        &terrain,
        &furniture,
        side,
        &indoors,
        outside_cache,
        outside_light,
    );

    run_sunlight_layers(
        world,
        outside_light,
        inside_light,
        sight_penalty,
        &layer,
        &above,
    );
}

/// The cascade for an entry point that knows only which squares are under open sky.
///
/// Reference: `build_sunlight_cache` (`lightmap.cpp:306-437`), reached from the closure-shaped API. A square under open sky is lit by the sun itself; a covered one has a
/// solid, floored square above it, which is what a roof is to the level below.
fn run_sunlight(
    world: &mut World,
    outside_light: f32,
    inside_light: f32,
    sight_penalty: f32,
    outside: impl Fn(i32, i32) -> bool,
) {
    let side = crate::square::extent(world);
    if side == 0 {
        return;
    }
    let cells = side * side;
    let mut layer = cdda_core_types::sunlight_cache::SunLayer {
        side,
        transparency: vec![cdda_core_types::light::TRANSPARENCY_OPEN_AIR; cells],
        floor: vec![false; cells],
        outside: vec![true; cells],
    };
    let mut above = cdda_core_types::sunlight_cache::AboveLayer {
        transparency: vec![cdda_core_types::light::TRANSPARENCY_OPEN_AIR; cells],
        floor: vec![false; cells],
        light: vec![outside_light; cells],
    };
    for y in 0..side {
        for x in 0..side {
            if outside(as_i32(x), as_i32(y)) {
                continue;
            }
            let index = x * side + y;
            // Under a roof: the square above is solid and floored, so no light comes through it.
            if let Some(slot) = above.transparency.get_mut(index) {
                *slot = 0.0;
            }
            if let Some(slot) = above.floor.get_mut(index) {
                *slot = true;
            }
            if let Some(slot) = above.light.get_mut(index) {
                *slot = 0.0;
            }
            if let Some(slot) = layer.outside.get_mut(index) {
                *slot = false;
            }
        }
    }
    run_sunlight_layers(
        world,
        outside_light,
        inside_light,
        sight_penalty,
        &layer,
        &above,
    );
}

/// The three rungs of the reference's ladder, shared by both entry points.
///
/// The port lights one level, but a one-level world still has three levels to it: the empty sky, the roof over the ground, and the ground.
///
/// 1. the empty sky, which is the reference's first iteration (`lightmap.cpp:310-315`) and leaves the pass "fully outside";
/// 2. the roof layer, whose opaque and floored squares are what end the "fully outside" regime (`lightmap.cpp:344-364`) - this rung's fill is discarded, because it is the
///    answer for a level nobody stands on;
/// 3. the ground, which is the first level to do the per-square work.
fn run_sunlight_layers(
    world: &mut World,
    outside_light: f32,
    inside_light: f32,
    sight_penalty: f32,
    layer: &cdda_core_types::sunlight_cache::SunLayer,
    above: &cdda_core_types::sunlight_cache::AboveLayer,
) {
    let side = layer.side;
    let cells = side * side;
    let mut sun = cdda_core_types::sunlight_cache::Sunlight::new();
    let sky_layer = cdda_core_types::sunlight_cache::SunLayer {
        side,
        transparency: vec![cdda_core_types::light::TRANSPARENCY_OPEN_AIR; cells],
        floor: vec![false; cells],
        outside: vec![true; cells],
    };
    let _ = sun.step(&sky_layer, outside_light, inside_light, sight_penalty);
    let roof_layer = cdda_core_types::sunlight_cache::SunLayer {
        side,
        transparency: above.transparency.clone(),
        floor: above.floor.clone(),
        outside: vec![true; cells],
    };
    let _ = sun.step_with_above(
        &roof_layer,
        above,
        outside_light,
        inside_light,
        sight_penalty,
    );
    let lit = sun.step_with_above(layer, above, outside_light, inside_light, sight_penalty);

    let mut daylight = BTreeMap::new();
    for y in 0..side {
        for x in 0..side {
            let index = x * side + y;
            if let Some(cell) = lit.get(index)
                && cell.max() > 0.0
                && let (Ok(px), Ok(py)) = (i32::try_from(x), i32::try_from(y))
            {
                daylight.insert((px, py), *cell);
            }
        }
    }
    singletons::light_mut(world).merge(daylight);
}

/// Builds the ground layer and the sky-or-roof layer above it.
///
/// Two different questions, deliberately answered from two different places: the **roof** above a square is what its own `INDOORS` flag says, while whether the square is
/// *charged for the weather* is what the outside cache's three-square stamp says (`build_outside_cache`). Reading the stamp for the roof would give every wall one square
/// of phantom ceiling and seal every doorway - which is exactly what a test caught.
#[allow(clippy::too_many_arguments)]
fn sunlayers(
    terrain_codes: &BTreeMap<(i32, i32), u32>,
    furniture_codes: &BTreeMap<(i32, i32), u32>,
    terrain: &terrain_table::TerrainTable,
    furniture: &terrain_table::TerrainTable,
    side: usize,
    indoors: &[bool],
    outside_cache: Vec<bool>,
    outside_light: f32,
) -> (
    cdda_core_types::sunlight_cache::SunLayer,
    cdda_core_types::sunlight_cache::AboveLayer,
) {
    let cells = side * side;
    let mut layer = cdda_core_types::sunlight_cache::SunLayer {
        side,
        transparency: vec![0.0; cells],
        // The ground has no floor of its own to stop light reaching the level below it; the port lights one level.
        floor: vec![false; cells],
        outside: outside_cache,
    };
    let mut above = cdda_core_types::sunlight_cache::AboveLayer {
        transparency: vec![0.0; cells],
        floor: vec![true; cells],
        light: vec![0.0; cells],
    };
    for y in 0..side {
        for x in 0..side {
            let index = x * side + y;
            let terrain_code = terrain_codes
                .get(&(as_i32(x), as_i32(y)))
                .copied()
                .unwrap_or(0);
            let furniture_code = furniture_codes
                .get(&(as_i32(x), as_i32(y)))
                .copied()
                .unwrap_or(0);
            let transparency = terrain
                .transparency(terrain_code)
                .min(furniture.transparency(furniture_code));
            let open = !indoors.get(index).copied().unwrap_or(false);
            if let Some(slot) = layer.transparency.get_mut(index) {
                *slot = transparency;
            }
            // Above an open square is the sky itself; above a roofed one is a solid ceiling with a floor over it.
            if let Some(slot) = above.transparency.get_mut(index) {
                *slot = if open { transparency } else { 0.0 };
            }
            if let Some(slot) = above.floor.get_mut(index) {
                *slot = !open;
            }
            if let Some(slot) = above.light.get_mut(index) {
                *slot = if open { outside_light } else { 0.0 };
            }
        }
    }
    (layer, above)
}

/// Clears the answer before a turn recomputes it, so nothing survives from the last turn.
pub fn clear_light(world: &mut World) {
    *singletons::light_mut(world) = Light::default();
}

/// Moves part of a field onto a neighbouring square: the arithmetic of `map::gas_spread_to`
/// (`src/map_field.cpp:250-273`).
///
/// Gas thins as it spreads and gives up its share of the age: the fraction moved is the age
/// divided by the intensity. A square that already holds the same gas gets thicker by one, so a
/// fire's smoke accumulates downwind instead of sitting in a single tile.
///
/// The direction is the caller's decision. In the reference it comes from wind and a dice roll
/// (`map::spread_gas`), neither of which is ported yet, so this is the arithmetic that the
/// roll feeds into - tested on its own so that wiring the wind in later cannot quietly change
/// it.
pub fn gas_spread_to(world: &mut World, source: Entity, target: (i32, i32)) {
    let Some(source_field) = world.get::<Field>(source) else {
        return;
    };
    let intensity = source_field.entry.intensity();
    // The reference bails below two on the *caller's* side; keeping the guard here as well
    // means a single unit of gas cannot be squeezed into nothing by a second caller.
    if intensity <= 1 {
        return;
    }
    let age = source_field.entry.age();
    let age_fraction = age / i64::from(intensity);
    let id = source_field.id;
    let priority = source_field.priority;

    // Find the square's existing field of the same kind, if any.
    let mut existing = None;
    let mut query = world.query::<(Entity, &Position, &Field)>();
    for (entity, position, field) in query.iter(world) {
        if position.x == target.0 && position.y == target.1 && field.id == id {
            existing = Some(entity);
            break;
        }
    }

    if let Some(entity) = existing {
        if let Some(mut field) = world.get_mut::<Field>(entity) {
            let raised = field.entry.intensity() + 1;
            field.entry.set_intensity(raised);
            let share = field.entry.age() + age_fraction;
            field.entry.set_age(share);
        }
    } else {
        let mut entry = FieldEntry::new(3, 100, true, false);
        entry.set_intensity(1);
        entry.set_age(age_fraction);
        world.spawn((
            Position {
                x: target.0,
                y: target.1,
                z: 0,
            },
            Field {
                id,
                entry,
                priority,
            },
        ));
    }

    if let Some(mut field) = world.get_mut::<Field>(source) {
        field.entry.set_intensity(intensity - 1);
        field.entry.set_age(age - age_fraction);
    }
}

/// The squares a wind direction makes spread-resistant, as the reference's table has them.
///
/// Reference: `map::get_wind_blockers` (`src/map_field.cpp:2081-2105`). Matched by descending
/// threshold like every other wind table in the reference, which is why 330 and 0 agree.
#[must_use]
pub fn wind_blocker_offsets(angle: i32) -> [(i32, i32); 3] {
    const TABLE: [(i32, [(i32, i32); 3]); 9] = [
        (330, [(1, 0), (1, -1), (1, 1)]),
        (301, [(1, 1), (1, 0), (0, 1)]),
        (240, [(0, 1), (-1, 1), (1, 1)]),
        (211, [(-1, 1), (-1, 0), (0, 1)]),
        (150, [(-1, 0), (-1, -1), (-1, 1)]),
        (121, [(-1, -1), (0, -1), (-1, 0)]),
        (60, [(0, -1), (-1, -1), (1, -1)]),
        (31, [(1, -1), (1, 0), (0, -1)]),
        (0, [(1, 0), (1, -1), (1, 1)]),
    ];
    TABLE
        .iter()
        .find(|(threshold, _)| angle >= *threshold)
        .map_or([(0, 0); 3], |(_, offsets)| *offsets)
}

/// The eight neighbours in the reference's order: north-west first, then clockwise to
/// south-east (`eight_horizontal_neighbors`, `src/point.h:450-460`).
/// The eight neighbours, in the reference's order: north first, clockwise.
pub const NEIGHBOURS: [(i32, i32); 8] = [
    (-1, -1),
    (0, -1),
    (1, -1),
    (-1, 0),
    (1, 0),
    (-1, 1),
    (0, 1),
    (1, 1),
];

/// Chooses where a gas spreads to and moves it there.
///
/// Reference: `map::spread_gas` (`src/map_field.cpp:276-368`). The order of the dice is the
/// order the reference uses, because every roll advances one shared stream:
/// the chance to spread, then the starting neighbour, then the coin for a plain spread.
/// A square that cannot be entered is skipped; when the wind is strong, the squares facing it
/// are only taken on a `x_in_y(1, max(2, windpower))` chance.
///
/// Not ported, and declared: the reference first tries *down*, then a neighbour, then *up*.
/// The port has no z-levels here, so only the neighbour branch runs.
/// Everything a gas needs to know about the wind and the sky.
pub struct GasContext {
    /// The chance in a hundred that this gas spreads on a windless turn.
    pub percent_spread: i32,
    /// Wind strength where the gas is.
    pub windpower: i32,
    /// Wind direction, in degrees, for the blockers.
    pub wind_angle: i32,
    /// Whether the square is outdoors, which makes the gas dissipate faster.
    pub outdoors: bool,
    /// How much faster, in turns.
    pub outdoor_age_speedup: i64,
}

/// Chooses where a gas spreads to and moves it there, aging it first if it is outdoors.
///
/// **The same rule as the sweep's**, through [`process_one_gas`]: down first, then the neighbours in the wind's order, then up. The door exists because the older
/// per-field turn has its own idea of what a "cell" is; the *rule* behind it must not have two versions, and this one used to - it never looked up or down at all,
/// which is why a fall could pass every session gate and still not happen here.
pub fn spread_gas(
    world: &mut World,
    source: Entity,
    context: &GasContext,
    can_take: impl Fn((i32, i32, i32), (i32, i32, i32)) -> bool,
    rolls: &mut cdda_core_types::rng::Rolls,
) {
    let percent_spread = context.percent_spread;
    let outdoors = context.outdoors;
    let outdoor_age_speedup = context.outdoor_age_speedup;
    // Dissipate faster outdoors, before any of the spread dice - and before the intensity bail, which is what the reference does: a thin gas outdoors still ages
    // away faster.
    if outdoors && outdoor_age_speedup != 0 {
        let age = world.get::<Field>(source).map_or(0, |field| field.entry.age());
        if let Some(mut field) = world.get_mut::<Field>(source) {
            field.entry.set_age(age + outdoor_age_speedup);
        }
    }
    let wind = Wind {
        power: context.windpower,
        direction: context.wind_angle,
        sheltered: false,
    };
    process_one_gas(world, source, percent_spread, wind, 1_000_000, can_take, rolls);
}

/// Replaces one field on a square with another kind, carrying the age across.
///
/// The reference does this by setting the old field's intensity to zero and adding the new one
/// on the same square; the store hands out ids, so "replace" is a spawn and a despawn. Doing it
/// here keeps the two processors below readable and makes the age rule impossible to forget.
fn transform_field(
    world: &mut World,
    entity: Entity,
    new_id: FieldId,
    new_intensity: i32,
    age: i64,
    is_fire: bool,
) {
    let Some(position) = world.get::<Position>(entity).copied() else {
        return;
    };
    let priority = world.get::<Field>(entity).map_or(0, |field| field.priority);
    let mut entry = FieldEntry::new(3, 100, true, is_fire);
    entry.set_intensity(new_intensity);
    entry.set_age(age);
    world.despawn(entity);
    world.spawn((
        Position {
            x: position.x,
            y: position.y,
            z: 0,
        },
        Field {
            id: new_id,
            entry,
            priority,
        },
    ));
}

/// Field ids used by the two processors below.
///
/// These are the loader's ids, not placeholders: `fd_flame_burst` is the 28th field in the
/// reference's dump (plus one, so zero stays free for the null field), and `crates/sim/tests/
/// field_ids.rs` checks every constant here against the registry the data produces. The previous
/// values were 900 and up, and moving them exposed two tests that had quietly assumed the
/// placeholders' *ordering* - one filtering an inclusive range, one filtering a numeric span.
pub const FLAME_BURST: FieldId = FieldId(27);
/// See [`FLAME_BURST`].
pub const FIRE_VENT: FieldId = FieldId(25);

/// A flame burst burns down, and when it has burnt out it becomes a fire vent.
///
/// Reference: `field_processor_fd_flame_burst` (`src/map_field.cpp:640-650`). The reference also
/// makes hot air on every step that thins it; hot air is not ported, so that is missing rather
/// than forgotten.
pub fn process_flame_burst(
    world: &mut World,
    entity: Entity,
    rolls: &mut cdda_core_types::rng::Rolls,
) {
    let Some(field) = world.get::<Field>(entity) else {
        return;
    };
    let intensity = field.entry.intensity();
    let age = field.entry.age();
    if intensity > 1 {
        if let Some(mut field) = world.get_mut::<Field>(entity) {
            field.entry.set_intensity(intensity - 1);
        }
        // The reference makes hot air on the step that thins it, at the new intensity.
        if let Some(position) = world.get::<Position>(entity).copied() {
            create_hot_air(world, (position.x, position.y), position.z, intensity - 1, rolls);
        }
    } else {
        transform_field(world, entity, FIRE_VENT, 3, age, true);
    }
}

/// A fire vent thins one step in three, and when it is spent it becomes a flame burst.
///
/// Reference: `field_processor_fd_fire_vent` (`src/map_field.cpp:626-637`). The dice are drawn
/// only when the field is strong, exactly as the reference does - a spent vent draws nothing,
/// so a replay does not shift.
pub fn process_fire_vent(
    world: &mut World,
    entity: Entity,
    rolls: &mut cdda_core_types::rng::Rolls,
) {
    let Some(field) = world.get::<Field>(entity) else {
        return;
    };
    let intensity = field.entry.intensity();
    let age = field.entry.age();
    if intensity > 1 {
        if rolls.one_in(3)
            && let Some(mut field) = world.get_mut::<Field>(entity)
        {
            field.entry.set_intensity(intensity - 1);
        }
        if let Some(position) = world.get::<Position>(entity).copied() {
            create_hot_air(world, (position.x, position.y), position.z, intensity - 1, rolls);
        }
    } else {
        transform_field(world, entity, FLAME_BURST, 3, age, true);
    }
}

/// Places five thin wisps of hot air around a square.
///
/// Reference: `map::create_hot_air` (`src/map_field.cpp:375-400`). The intensity argument picks
/// *which* hot air this is - four different field types, not four strengths - and each of the
/// five attempts lands on the square itself or one of its eight neighbours, drawing the offset
/// from the shared dice.
///
/// The reference's switch has no case for anything outside one to four and falls through to a
/// debug message: that path draws no dice and creates nothing, which the tests hold onto because
/// a port that rolled there would shift every later roll for a case that never happens in play.
pub fn create_hot_air(
    world: &mut World,
    square: (i32, i32),
    z: i32,
    intensity: i32,
    rolls: &mut cdda_core_types::rng::Rolls,
) {
    let id = match intensity {
        1 => HOT_AIR_1,
        2 => HOT_AIR_2,
        3 => HOT_AIR_3,
        4 => HOT_AIR_4,
        _ => return,
    };
    for _ in 0..5 {
        let dx = rolls.rng(-1, 1);
        let dy = rolls.rng(-1, 1);
        add_or_thicken(world, (square.0 + dx, square.1 + dy), z, id, 1);
    }
}

/// Puts a field on a square, thickening the one already there rather than stacking a second.
///
/// Reference: `map::add_field` (`src/map_field.cpp:1260-1310`), where a square holds one entry per field type and adding to it raises the intensity. Two wisps of hot
/// air landing on the same square are one field at intensity two, and a gas spreading back into the square it came from thickens rather than duplicates.
pub fn add_or_thicken(world: &mut World, square: (i32, i32), z: i32, id: FieldId, intensity: i32) {
    let existing = {
        let mut query = world.query::<(Entity, &Position, &Field)>();
        query
            .iter(world)
            .find(|(_, position, field)| {
                (position.x, position.y, position.z) == (square.0, square.1, z) && field.id == id
            })
            .map(|(entity, _, field)| (entity, field.entry.intensity()))
    };
    if let Some((entity, thick)) = existing {
        if let Some(mut field) = world.get_mut::<Field>(entity) {
            field.entry.set_intensity(thick + intensity);
        }
        return;
    }
    let mut entry = FieldEntry::new(3, 100, true, false);
    entry.set_intensity(intensity);
    world.spawn((
        Position {
            x: square.0,
            y: square.1,
            z,
        },
        Field {
            id,
            entry,
            priority: 0,
        },
    ));
}

/// The numbers one turn of burning produced, grouped because they all come from the same processor run and travel together.
struct Discharging {
    intensity: i32,
    adjacent_fires: i32,
    smoke_made: i32,
    windpower: i32,
}

/// The smoke and the hot air a fire leaves behind, in the reference's order.
///
/// Split out of [`process_fires`] only because the system had grown past the point where a reader can hold it: the order is the rule, and the order is
/// smoke (`map_field.cpp:1407`) then hot air (`:1429`).
/// Burns whatever is lying on a square and writes back what is left of it.
///
/// The items are entities with their own state, so this has to read them out, let the rule decide what was destroyed, and then write the survivors back -
/// a fire eats a plank over many turns and the arithmetic only works if each turn starts from where the last one left it.
fn burn_items_at(
    world: &mut World,
    square: (i32, i32, i32),
    intensity: i32,
    contained: bool,
    rolls: &mut cdda_core_types::rng::Rolls,
) -> cdda_core_types::fire::ItemBurn {
    let mut entities: Vec<(i32, Entity)> = Vec::new();
    let mut query = world.query::<(Entity, &Position, &GroundItem)>();
    for (entity, position, item) in query.iter(world) {
        if (position.x, position.y, position.z) == square && item.burn.is_some() {
            entities.push((item.kind.0.cast_signed(), entity));
        }
    }
    if entities.is_empty() {
        return cdda_core_types::fire::ItemBurn::default();
    }
    // The reference burns a square's items in the order they were dropped, which is spawn order for the port.
    entities.sort_by_key(|&(kind, entity)| (kind, entity.to_bits()));
    let mut burning: Vec<cdda_core_types::fire::BurningItem> = Vec::new();
    for (_, entity) in &entities {
        if let Some(item) = world.get::<GroundItem>(*entity)
            && let Some(burn) = item.burn.clone()
        {
            burning.push(burn);
        }
    }
    let before = burning.len();
    let outcome = cdda_core_types::fire::burn_the_items(&mut burning, intensity, contained, rolls);
    // Whatever the rule removed is gone from the ground; the rest keep the burnt they reached, which is how a plank lasts several turns.
    let destroyed = before.saturating_sub(burning.len());
    let mut kept = burning.into_iter();
    for (_, entity) in &entities {
        let Some(state) = kept.next() else {
            break;
        };
        if let Some(mut item) = world.get_mut::<GroundItem>(*entity) {
            item.burn = Some(state);
        }
    }
    for (_, entity) in entities.iter().rev().take(destroyed) {
        world.despawn(*entity);
    }
    outcome
}

fn fire_discharge(
    world: &mut World,
    square: (i32, i32, i32),
    fire: &Discharging,
    map: &impl FireMap,
    rolls: &mut cdda_core_types::rng::Rolls,
) {
    // **Smoke first** (`map_field.cpp:1407`), then hot air (`:1429`). I had these the other way round and the recording caught it: the port asked the
    // hot-air gate where the reference asked the smoke gate, one draw after the spread loop.
    if let Some(smoke) = cdda_core_types::fire::smoke_from_fire(
        rolls,
        fire.intensity,
        fire.smoke_made,
        fire.windpower,
        map.suppress_smoke(square),
        // `p.z() < OVERMAP_HEIGHT` - there is always a level above in this store, and the top of the world is not modelled.
        true,
        map.smoke_ceiling_open(square),
    ) {
        let floor = if smoke.up { square.2 + 1 } else { square.2 };
        add_field_strength(world, (square.0, square.1, floor), SMOKE, smoke.intensity);
    }
    if cdda_core_types::fire::wants_hot_air(fire.adjacent_fires, rolls)
        && let Some(kind) = cdda_core_types::fire::hot_air_field(fire.intensity)
        && let Some(hot_air) = hot_air_id(kind)
    {
        for (dx, dy) in cdda_core_types::fire::hot_air_squares(rolls) {
            add_field_strength(world, (square.0 + dx, square.1 + dy, square.2), hot_air, 1);
        }
    }
}
/// The field sweep: fires, then incendiaries, then gases, then the ageing of what the sweep found.
///
/// Reference: `map::process_fields` (`src/map_field.cpp:164-184`), submap by submap in ascending
/// `(z, x, y)`. Split out of [`run_environment_turn`] so the turn's stages can be run as systems.
pub fn run_field_sweep(
    world: &mut World,
    map: &impl FireMap,
    windpower: i32,
    can_take: impl Fn((i32, i32, i32), (i32, i32, i32)) -> bool + Copy,
    rolls: &mut cdda_core_types::rng::Rolls,
) {
    // **The sweep, submap by submap in the reference's own order** (`map::process_fields`, `src/map_field.cpp:164-184`): ascending `(z, x, y)`, each submap's fires,
    // then its gases, then the ageing of every field the sweep found there. A field born in a submap the sweep has already passed waits for the next turn, which is why
    // a gas that falls keeps the age it was born with while one that spreads forward is aged in the same turn. A cursor rather than a list, because a field born in a
    // later submap is visited by the same sweep.
    let mut cursor: Option<(i32, i32, i32)> = None;
    while let Some(key) = submaps_with_fields(world)
        .into_iter()
        .find(|key| cursor.is_none_or(|cursor| *key > cursor))
    {
        cursor = Some(key);
        // **Square by square, as the reference does it.** `process_fields_in_submap` walks every square of the submap in `(x, y)` order and, for each
        // one, runs the fire processor, then the gases standing on it, then decays every field there - reading the square **live**, so a field that
        // spread into a later square is aged by the same sweep and one that spread into an earlier square waits for the next turn. A list of the
        // squares that held fields when the sweep began cannot express that: it misses everything born ahead of the cursor, which is exactly the
        // fields a spreading gas makes.
        for sx in 0..12 {
            for sy in 0..12 {
                let square = (key.1 * 12 + sx, key.2 * 12 + sy, key.0);
                let here = Where::Square(square);
            // Fires first: everything else this turn reacts to what they left behind.
            process_fires_in_submap(world, map, windpower, here, rolls);
        // Then the incendiaries, which can light the fire the next turn burns.
        process_incendiaries_in_submap(
            world,
            here,
            |x, y, z| {
                // The three flammable flags, read back from the ground the fire rules already ask about: a `FIRE_CONTAINER` answers `Inert`, which is right - a
                // brazier holds its fire rather than catching, and the reference's three flags agree with that by all being false on the container itself.
                match map.ground((x, y, z)) {
                    cdda_core_types::fire::Ground::Flammable => TerrainFacts { flammable: true, ..TerrainFacts::default() },
                    cdda_core_types::fire::Ground::FlammableHard => TerrainFacts { flammable_hard: true, ..TerrainFacts::default() },
                    cdda_core_types::fire::Ground::FlammableAsh => TerrainFacts { flammable_ash: true, ..TerrainFacts::default() },
                    cdda_core_types::fire::Ground::Inert => TerrainFacts::default(),
                }
            },
            rolls,
        );
        // Then the gases, including the haze the fires just made. `percent_spread` is `fd_smoke`'s and the hot air kinds' own data.
        for (id, percent_spread) in [
            (SMOKE, 10),
            (HOT_AIR_1, 10),
            (HOT_AIR_2, 10),
            (HOT_AIR_3, 10),
            (HOT_AIR_4, 10),
        ] {
            process_gas_in_submap(world, id, here, percent_spread, windpower, can_take, rolls);
        }
            // **This square's fields age now**, including whatever the lines above just made here. A field born on a square the sweep has already passed is not
            // in this list and waits for the next turn, which is the one-turn difference the roof fixture measured.
            let found: Vec<(Entity, i32, i32)> = {
                let mut query = world.query::<(Entity, &Position)>();
                query
                    .iter(world)
                    .filter(|(entity, position)| {
                        (position.x, position.y, position.z) == square
                            && world.get::<Field>(*entity).is_some()
                    })
                    .map(|(entity, position)| (entity, position.x, position.y))
                    .collect()
            };
                decay_the_fields_the_sweep_found(world, &found, 0, rolls, |_field| None, |_turn, _square, _rolls| {});
            }
        }
    }
}

/// Runs one environment turn against a map the caller supplies.
///
/// The rule-level entry point: a replay or a unit test hands over the answers it wants the rules to
/// see. A world loaded into entities uses [`run_environment_turn_from_store`] instead, which builds
/// those answers from the store.
pub fn run_environment_turn(
    world: &mut World,
    map: &impl FireMap,
    windpower: i32,
    scent_center: (i32, i32),
    scent_deposit: i32,
    can_take: impl Fn((i32, i32, i32), (i32, i32, i32)) -> bool + Copy,
    rolls: &mut cdda_core_types::rng::Rolls,
) {
    // **The world's wind, not the caller's, when the world has a weather system.** The parameter stays because a session replay drives the wind from its fixture and has no climate;
    // a world that does have one must not be able to disagree with itself about which way the smoke blows.
    let windpower = weather::wind_of(world, windpower);
    // Scent first, because the reference's turn starts there (`do_turn.cpp:642-648`): the character deposits and the window diffuses before the fields are processed.
    // The deposit is nobody's yet - there is no creature layer - so the caller passes it through `scent_deposit`, and zero means "nothing deposited this turn".
    scent::advance_scent(world, scent_center, scent_deposit);
    // **Then what fell.** `do_turn.cpp:648-655` runs `scent.update`, then `map::build_floor_caches`, then `m.process_falling()`, and only then `m.process_fields()` - so a
    // thing that falls is aged by the same turn's field sweep rather than the next one. The queue is taken whole and cleared, which is why a square is asked once.
    process_falling_turn(world);
    run_field_sweep(world, map, windpower, can_take, rolls);
    // The light map is rebuilt for the world this turn made, the way the draw path rebuilds it after the turn's work (`map::build_map_cache`). Clearing first is part of the
    // rule rather than tidiness: the cast merges by maximum, so a fire that went out this turn would otherwise stay lit by last turn's answer.
    clear_light(world);
    cast_lights_with(world);
}

/// Runs the turn's falling: everything left on a square whose support went away, against what the map says about floors.
///
/// Reference: `map::process_falling` (`src/map.cpp:3018-3035`) and `map::drop_everything` (`src/map.cpp:2693-2705`); the rules are [`falling`]'s. The queue is taken and
/// cleared before anything moves, because dropping one square can add another to it - the reference moves the set out and works on the copy so that a second drop is the
/// next turn's business.
///
/// The rules come from a [`falling::FieldRules`] resource when the world has one; without it nothing accelerates, so only items fall. A world that has loaded its field
/// prototypes should insert it.
pub fn process_falling_turn(world: &mut World) {
    let squares: Vec<(i32, i32, i32)> = singletons::support_lost(world).squares.iter().copied().collect();
    if squares.is_empty() {
        return;
    }
    let rules = singletons::field_rules(world).clone();
    // The floor question comes from the squares themselves: a square whose facts say it has no floor
    // is a hole. Read once, before anything moves, because the drop below moves things.
    let floors = StoreFloors::from_world(world);
    for square in squares {
        falling::drop_everything(world, &floors, &rules, square);
    }
    // The queue is empty for the next turn: `support_cache_dirty.clear()` after the copy was taken.
    singletons::support_lost(world).squares.clear();
}

/// The squares as [`falling::Floors`] asks them: which squares have no floor.
///
/// A snapshot rather than a borrow, because the drop below moves the world while it asks.
struct StoreFloors {
    holed: std::collections::BTreeSet<(i32, i32, i32)>,
}

impl StoreFloors {
    fn from_world(world: &World) -> Self {
        let mut holed = std::collections::BTreeSet::new();
        let mut query = world.try_query::<(&Position, &fire_facts::FireFacts)>();
        if let Some(query) = query.as_mut() {
            for (position, facts) in query.iter(world) {
                if facts.facts.no_floor {
                    holed.insert((position.x, position.y, position.z));
                }
            }
        }
        Self { holed }
    }
}

impl falling::Floors for StoreFloors {
    fn no_floor(&self, at: (i32, i32, i32)) -> bool {
        self.holed.contains(&at)
    }

    fn in_bounds(&self, at: (i32, i32, i32)) -> bool {
        // **Declared:** the port's world is one loaded level and this says every square of it is in bounds, because the reference's `inbounds` is about submap allocation
        // and the port does not allocate. What stops a fall at the bottom is the world's own depth (`falling::OVERMAP_DEPTH`), not this.
        let _ = at;
        true
    }
}

/// The hot air field for a fire of this intensity, as `create_hot_air` chooses it.
#[must_use]
pub fn hot_air_id(intensity: i32) -> Option<FieldId> {
    match intensity {
        1 => Some(HOT_AIR_1),
        2 => Some(HOT_AIR_2),
        3 => Some(HOT_AIR_3),
        4 => Some(HOT_AIR_4),
        _ => None,
    }
}

/// Hot air a fire of intensity 1 leaves behind (`fd_hot_air1`).
pub const HOT_AIR_1: FieldId = FieldId(57);
/// See [`HOT_AIR_1`].
/// Hot air a fire of intensity 2 leaves behind (`fd_hot_air2`).
pub const HOT_AIR_2: FieldId = FieldId(58);
/// See [`HOT_AIR_1`].
/// Hot air a fire of intensity 3 leaves behind (`fd_hot_air3`).
pub const HOT_AIR_3: FieldId = FieldId(59);
/// See [`HOT_AIR_1`].
/// Hot air a fire of intensity 4 leaves behind (`fd_hot_air4`).
pub const HOT_AIR_4: FieldId = FieldId(60);

/// Smoke, as the field registry numbers it.
pub const SMOKE: FieldId = FieldId(18);

/// Fire, as the field registry numbers it.
pub const FIRE: FieldId = FieldId(13);
/// An extinguisher's cloud, as the field registry numbers it.
pub const EXTINGUISHER: FieldId = FieldId(15);

/// An extinguisher cloud fights fire on its own square, one intensity for one intensity.
///
/// Reference: `field_processor_fd_extinguisher` (`src/map_field.cpp:559-569`). The reference takes
/// the smaller of the two strengths and removes that much from both, so a cloud that is weaker
/// than the fire is spent entirely and leaves the fire burning lower; a cloud that is stronger
/// puts the fire out and survives with what is left. Neither side draws dice.
pub fn process_extinguisher(world: &mut World, entity: Entity) {
    let Some(position) = world.get::<Position>(entity).copied() else {
        return;
    };
    let Some(cloud) = world
        .get::<Field>(entity)
        .map(|field| field.entry.intensity())
    else {
        return;
    };
    let fire = {
        let mut query = world.query::<(Entity, &Position, &Field)>();
        query
            .iter(world)
            .filter(|(_, at, field)| at.x == position.x && at.y == position.y && field.id == FIRE)
            .map(|(fire_entity, _, _)| fire_entity)
            .next()
    };
    let Some(fire_entity) = fire else {
        return;
    };
    let Some(burning) = world
        .get::<Field>(fire_entity)
        .map(|field| field.entry.intensity())
    else {
        return;
    };
    let fought = cloud.min(burning);
    for (target, intensity) in [(fire_entity, burning), (entity, cloud)] {
        let left = intensity - fought;
        if left <= 0 {
            world.despawn(target);
        } else if let Some(mut field) = world.get_mut::<Field>(target) {
            field.entry.set_intensity(left);
        }
    }
}

/// An acid vent, as the field registry numbers it.
pub const ACID_VENT: FieldId = FieldId(32);

/// A vent that is at full strength thins once its age passes a minute; a spent one refills.
///
/// Reference: `field_processor_fd_acid_vent` (`src/map_field.cpp:877-901`). A vent stronger than 1
/// thins by one and resets its age once it is at least a minute old, drawing nothing. A vent at 1
/// goes back to 3.
///
/// The reference's second half then walks a radius of 5 looking for acid fields **whose intensity
/// is zero** and gives them strength. That branch is dead in the reference as well: `set_intensity`
/// floors a field at 1, so a zero-intensity field cannot exist, and no acid is ever revived by it.
/// The port therefore does not reproduce it - a divergence recorded in the ledger, not an omission
/// - and the observable half, the refill, is what the tests pin.
pub fn process_acid_vent(
    world: &mut World,
    entity: Entity,
    _rolls: &mut cdda_core_types::rng::Rolls,
) {
    let Some(field) = world.get::<Field>(entity) else {
        return;
    };
    let intensity = field.entry.intensity();
    let age = field.entry.age();
    if intensity > 1 {
        if age >= 60
            && let Some(mut field) = world.get_mut::<Field>(entity)
        {
            field.entry.set_intensity(intensity - 1);
            field.entry.set_age(0);
        }
    } else if let Some(mut field) = world.get_mut::<Field>(entity) {
        field.entry.set_intensity(3);
    }
}

/// The incendiary field, as the reference numbers it.
///
/// The numbers are the reference's own - `field.id.id().to_i()` as `dump_fields` reports it, checked against
/// `corpus/field_types.json` for all fourteen constants. This one said thirty for two rounds: an off-by-one convention that
/// held for thirteen other constants, and here pointed `process_incendiary` at `fd_fatigue`, a field nothing in the game
/// puts down.
pub const INCENDIARY: FieldId = FieldId(47);

/// An incendiary field sets fire to one neighbour, and warms its own square either way.
///
/// Reference: `field_processor_fd_incendiary` (`src/map_field.cpp:928-946`). The two offsets are drawn **first**, so a field about to die still spends them; the
/// square they pick may be the field's own; it catches if the terrain can burn by any of the three flammable flags; and the hot air is made whether or not anything
/// caught. The reference also asks `flammable_items_at`, which belongs to the item layer the port does not have yet - the gap is in the ledger, not hidden in the code.
pub fn process_incendiary(
    world: &mut World,
    entity: Entity,
    facts: impl Fn(i32, i32, i32) -> TerrainFacts,
    rolls: &mut cdda_core_types::rng::Rolls,
) {
    let (dx, dy) = (rolls.rng(-1, 1), rolls.rng(-1, 1));
    let Some(position) = world.get::<Position>(entity).copied() else {
        return;
    };
    let Some(intensity) = world.get::<Field>(entity).map(|field| field.entry.intensity()) else {
        return;
    };
    let destination = (position.x + dx, position.y + dy);
    let terrain = facts(destination.0, destination.1, position.z);
    if terrain.flammable || terrain.flammable_ash || terrain.flammable_hard {
        add_or_thicken(world, destination, position.z, FIRE, 1);
    }
    // The reference's `create_hot_air`, which draws its own ten dice — five attempts of two each — whether or not anything caught.
    create_hot_air(world, (position.x, position.y), position.z, intensity, rolls);
}

/// Electricity, as the field registry numbers it.
pub const ELECTRICITY: FieldId = FieldId(28);
/// A shock vent, as the field registry numbers it.
pub const SHOCK_VENT: FieldId = FieldId(31);

/// A shock vent thins one time in five, and when spent erupts into lightning bolts.
///
/// Reference: `field_processor_fd_shock_vent` (`src/map_field.cpp:838-876`). A vent stronger than 1
/// draws `one_in(5)` and thins if it fires. A vent at 1 goes back to 3, draws 3 to 6 bolts, and for
/// each one picks a direction by drawing twice from -1..1 until the pair is not both zero, a length
/// from 4 to 12, then walks that many steps: every step lays electricity of strength 2 or 3, and
/// after each step two `one_in(4)` draws may turn the bolt - the x draw turns x on or off, the y
/// draw turns y on or off, and a direction that is already zero is given a fresh 0-or-1 sign.
///
/// The dice order is the contract here: a replay that drew in a different order would light a
/// different corridor with the same seed.
pub fn process_shock_vent(
    world: &mut World,
    entity: Entity,
    rolls: &mut cdda_core_types::rng::Rolls,
) {
    let Some(position) = world.get::<Position>(entity).copied() else {
        return;
    };
    let Some(field) = world.get::<Field>(entity) else {
        return;
    };
    let intensity = field.entry.intensity();
    if intensity > 1 {
        if rolls.one_in(5)
            && let Some(mut field) = world.get_mut::<Field>(entity)
        {
            field.entry.set_intensity(intensity - 1);
        }
        return;
    }
    if let Some(mut field) = world.get_mut::<Field>(entity) {
        field.entry.set_intensity(3);
    }
    let bolts = rolls.rng(3, 6);
    for _ in 0..bolts {
        let (mut dx, mut dy) = (0, 0);
        while dx == 0 && dy == 0 {
            dx = rolls.rng(-1, 1);
            dy = rolls.rng(-1, 1);
        }
        let length = rolls.rng(4, 12);
        let (mut x, mut y) = (position.x, position.y);
        for _ in 0..length {
            x += dx;
            y += dy;
            lay_electricity(world, (x, y), rolls.rng(2, 3));
            if rolls.one_in(4) {
                dx = if dx == 0 { rolls.rng(0, 1) * 2 - 1 } else { 0 };
            }
            if rolls.one_in(4) {
                dy = if dy == 0 { rolls.rng(0, 1) * 2 - 1 } else { 0 };
            }
        }
    }
}

/// Adds electricity to a square, keeping the stronger of what was there and what arrives.
fn lay_electricity(world: &mut World, square: (i32, i32), strength: i32) {
    let existing = {
        let mut query = world.query::<(Entity, &Position, &Field)>();
        query
            .iter(world)
            .find(|(_, at, field)| at.x == square.0 && at.y == square.1 && field.id == ELECTRICITY)
            .map(|(entity, _, field)| (entity, field.entry.intensity()))
    };
    if let Some((entity, already)) = existing {
        if strength > already
            && let Some(mut field) = world.get_mut::<Field>(entity)
        {
            field.entry.set_intensity(strength);
        }
        return;
    }
    // The square's cap comes from the field **type**, not from a constant: `field_type::get_max_intensity` is the number of the
    // type's intensity levels, and the data's fields run from one to ten of them.
    let cap = crate::field_registry::max_intensity_of(world, ELECTRICITY);
    let mut entry = FieldEntry::new(cap, 100, true, false);
    entry.set_intensity(strength);
    world.spawn((
        Position {
            x: square.0,
            y: square.1,
            z: 0,
        },
        Field {
            id: ELECTRICITY,
            entry,
            priority: 0,
        },
    ));
}

/// An item type, as the item registry numbers it.
///
/// Zero is reserved for "no item", as with fields, so a stack entry can be cleared without
/// carrying an `Option` in every tile.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ItemTypeId(pub u32);

/// An item lying on the ground.
///
/// Reference: `map_stack` / `map::i_at` (`src/map.cpp`), which keeps a per-tile stack in the order
/// things were added. The port keeps the same order by entity index, which is spawn order: a fire
/// that burns a pile must consume things in the same sequence the reference would, or the same seed
/// gives different results.
#[derive(Debug, Clone, Component)]
pub struct GroundItem {
    /// What it is.
    pub kind: ItemTypeId,
    /// How much of it: charges for a stack, 1 for a single object.
    pub charges: i32,
    /// How long it has been lying there, in turns.
    pub age: i64,
    /// How much of a solid item has burned away so far.
    pub burnt: i32,
    /// How damaged it is, in the reference's own units.
    pub damage: i32,
    /// What a fire would make of it, once its type is known: the material tables are resolved when the item is put down, so the burning rule never needs a
    /// content catalogue in the room. `None` until an item's type has been looked up.
    pub burn: Option<cdda_core_types::fire::BurningItem>,
}

/// Puts an item on a square.
pub fn drop_item(world: &mut World, square: (i32, i32), kind: ItemTypeId, charges: i32) -> Entity {
    world
        .spawn((
            Position {
                x: square.0,
                y: square.1,
                z: 0,
            },
            GroundItem {
                burnt: 0,
                damage: 0,
                kind,
                charges,
                age: 0,
                burn: None,
            },
        ))
        .id()
}

/// Puts an item on a square with an age, for things that have already been lying there a while.
pub fn drop_item_aged(
    world: &mut World,
    square: (i32, i32),
    kind: ItemTypeId,
    charges: i32,
    age: i64,
) -> Entity {
    let entity = drop_item(world, square, kind, charges);
    if let Some(mut item) = world.get_mut::<GroundItem>(entity) {
        item.age = age;
    }
    entity
}

/// Everything lying on one square, in the order it was put there.
#[must_use]
pub fn items_at(world: &mut World, square: (i32, i32)) -> Vec<Entity> {
    let mut found = Vec::new();
    {
        let mut query = world.query::<(Entity, &Position, &GroundItem)>();
        for (entity, at, _) in query.iter(world) {
            // Something inside a container is not lying on the square, whatever square its container is
            // standing on. Without this, a craft that holds its own parts looks like it left them there.
            if at.x == square.0 && at.y == square.1 && world.get::<Inside>(entity).is_none() {
                found.push(entity);
            }
        }
    }
    found.sort_by_key(|entity| entity.index());
    found
}

/// Takes an item off the ground, if it is still there.
pub fn take_item(world: &mut World, entity: Entity) {
    if world.get::<GroundItem>(entity).is_some() {
        world.despawn(entity);
    }
}

/// Fire with nothing to eat burns out faster.
///
/// Reference: `field_processor_fd_fire` (`src/map_field.cpp:1128-1132`). When a fire did not gain
/// time from consuming something, and its tile has nothing burnable on it, the fire's age advances
/// by ten seconds per point of intensity - so a strong fire over bare ground dies sooner than a
/// weak one, which is the reference's own comment: "Drain more power from big fires, so that they
/// stop raging over nothing".
///
/// This is the first slice of the fire processor, and it is the one that needs no wind, no
/// neighbours and no items: the caller says whether the tile has fuel, because what counts as fuel
/// is terrain and furniture data the caller holds.
pub fn process_fire_burnout(world: &mut World, entity: Entity, has_fuel: bool) {
    if has_fuel {
        return;
    }
    let Some(intensity) = world
        .get::<Field>(entity)
        .map(|field| field.entry.intensity())
    else {
        return;
    };
    if let Some(mut field) = world.get_mut::<Field>(entity) {
        let age = field.entry.age();
        field.entry.set_age(age + i64::from(intensity) * 10);
    }
}

/// What the fire needs to know about a neighbouring square.
///
/// Five independent facts, so it is deliberately a bag of bools rather than an enum: the fuel flags
/// are not exclusive (terrain can carry more than one), and they are checked in the reference's
/// order rather than by kind.
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NeighbourFacts {
    /// Something there is already burning.
    pub fire: bool,
    /// The terrain carries `FLAMMABLE`.
    pub flammable: bool,
    /// The terrain carries `FLAMMABLE_ASH`.
    pub flammable_ash: bool,
    /// The terrain carries `FLAMMABLE_HARD`.
    pub flammable_hard: bool,
    /// There are items lying there.
    pub items: bool,
}

/// Whether the fire sets one neighbour alight, drawing in the reference's order.
///
/// Reference: `field_processor_fd_fire` (`src/map_field.cpp:1290-1345`). The dice come in this
/// order and only when reached: `power = intensity + one_in(5)`, then `rng(1, 100)` against a
/// spread chance of 25 per point of intensity above one, then the fuel checks in flag order -
/// `FLAMMABLE` and `FLAMMABLE_ASH` each on `one_in(2)`, `FLAMMABLE_HARD` on `one_in(5)` and only
/// when power is at least 3 - then `one_in(5)` for a pile of items. A square already on fire is
/// skipped before any dice are drawn.
pub fn should_ignite(
    intensity: i32,
    facts: &NeighbourFacts,
    rolls: &mut cdda_core_types::rng::Rolls,
) -> bool {
    if facts.fire {
        return false;
    }
    let spread_chance = 25 * (intensity - 1);
    let power = intensity + i32::from(rolls.one_in(5));
    if rolls.rng(1, 100) >= spread_chance {
        return false;
    }
    if power >= 2 && facts.flammable && rolls.one_in(2) {
        return true;
    }
    if power >= 2 && facts.flammable_ash && rolls.one_in(2) {
        return true;
    }
    if power >= 3 && facts.flammable_hard && rolls.one_in(5) {
        return true;
    }
    rolls.one_in(5) && facts.items
}

/// Starts a fire of strength 1, two minutes old, on a neighbouring square if the dice allow.
///
/// Reference: `add_field( dst, fd_fire, 1, 2_minutes, false )` followed by the burning square
/// spending a minute of its own life (`src/map_field.cpp:1330-1336`). The neighbours are visited
/// from a random starting point and wrap around, as the reference does, so the same seed picks the
/// same squares in the same order.
///
/// The caller supplies the fuel facts, because they come from terrain and item data the sim does
/// not own.
pub fn process_fire_spread<F>(
    world: &mut World,
    entity: Entity,
    facts: F,
    rolls: &mut cdda_core_types::rng::Rolls,
) where
    F: Fn((i32, i32)) -> NeighbourFacts,
{
    let Some(position) = world.get::<Position>(entity).copied() else {
        return;
    };
    let Some(intensity) = world
        .get::<Field>(entity)
        .map(|field| field.entry.intensity())
    else {
        return;
    };
    let start = rolls.rng(0, 7);
    for step in 0..8 {
        let index = usize::try_from((start + 1 + step).rem_euclid(8)).unwrap_or(0);
        let Some((dx, dy)) = NEIGHBOURS.get(index).copied() else {
            continue;
        };
        let square = (position.x + dx, position.y + dy);
        if should_ignite(intensity, &facts(square), rolls) {
            let mut entry = FieldEntry::new(3, 100, true, true);
            entry.set_intensity(1);
            entry.set_age(-120);
            world.spawn((
                Position {
                    x: square.0,
                    y: square.1,
                    z: 0,
                },
                Field {
                    id: FIRE,
                    entry,
                    priority: 0,
                },
            ));
            if let Some(mut field) = world.get_mut::<Field>(entity) {
                let age = field.entry.age();
                field.entry.set_age(age + 60);
            }
        }
    }
}

/// How big a fire can get, given how long it has burned and what is next to it.
///
/// Reference: `field_processor_fd_fire` (`src/map_field.cpp:1240-1262`). A fire under half an hour
/// of fuel age (-500 minutes) can reach 3 outright. Otherwise its ceiling comes from neighbours -
/// three adjacent fires allow 2, seven allow 3 - and a fire with more than fifty minutes of fuel
/// age gets 2 even when it is alone.
#[must_use]
pub fn growth_ceiling(age: i64, adjacent_fires: usize) -> i32 {
    if age < -30_000 {
        return 3;
    }
    let by_neighbours = 1 + i32::from(adjacent_fires >= 3) + i32::from(adjacent_fires >= 7);
    if by_neighbours < 2 && age < -3_000 {
        return 2;
    }
    by_neighbours
}

/// Grows a fire one step when it has fuel to spare, and weakens what it gained.
///
/// Reference: `src/map_field.cpp:1255-1262`. A fire only grows while its age is negative - fuel in
/// the bank - and growing costs it: ten minutes of age per point of intensity it ends up with, so a
/// fire that jumps to 3 pays half an hour for the privilege.
#[must_use]
pub fn grow_fire(intensity: i32, age: i64, adjacent_fires: usize) -> Option<(i32, i64)> {
    if age >= 0 || intensity >= growth_ceiling(age, adjacent_fires) {
        return None;
    }
    let grown = intensity + 1;
    Some((grown, age + 600 * i64::from(grown)))
}

/// Whether a fire hands five minutes of its life to a neighbouring fire.
///
/// Reference: `field_processor_fd_fire` (`src/map_field.cpp:1178-1181`). The neighbour is boosted
/// when it is no stronger than this fire, or when it is older - "older" here means a larger age,
/// which is a shorter life left. Either way the fire that gives up the time is the one that could
/// spare it.
#[must_use]
pub fn should_boost(
    self_intensity: i32,
    self_age: i64,
    other_intensity: i32,
    other_age: i64,
) -> bool {
    other_intensity <= self_intensity || other_age > self_age
}

/// A strong fire spends its excess age making its neighbours bigger: the reference's flashpoint.
///
/// Reference: `field_processor_fd_fire` (`src/map_field.cpp:1164-1215`). Only a fire above intensity
/// 1 takes part, and only when `one_in(3)` fires. It then visits the eight neighbours from a random
/// start, wrapping around, and stops as soon as its own age is no longer negative - each neighbour
/// it feeds costs it five minutes. A fed neighbour under intensity 2 grows to 2 and always gains
/// five minutes of life (its age drops by 300).
///
/// Returns how many neighbours were on fire, which the caller uses for the growth ceiling. The
/// reference counts that in the same pass; so does this.
pub fn process_flashpoint(
    world: &mut World,
    entity: Entity,
    rolls: &mut cdda_core_types::rng::Rolls,
) -> usize {
    let Some(position) = world.get::<Position>(entity).copied() else {
        return 0;
    };
    let Some(field) = world.get::<Field>(entity) else {
        return 0;
    };
    let intensity = field.entry.intensity();
    if intensity <= 1 || !rolls.one_in(3) {
        return 0;
    }
    let start = rolls.rng(0, 7);
    let mut adjacent = 0;
    for step in 0..8 {
        let age = world
            .get::<Field>(entity)
            .map_or(0, |field| field.entry.age());
        if age >= 0 {
            break;
        }
        let index = usize::try_from((start + 1 + step).rem_euclid(8)).unwrap_or(0);
        let Some((dx, dy)) = NEIGHBOURS.get(index).copied() else {
            continue;
        };
        let square = (position.x + dx, position.y + dy);
        let neighbour = {
            let mut query = world.query::<(Entity, &Position, &Field)>();
            query
                .iter(world)
                .find(|(_, at, field)| at.x == square.0 && at.y == square.1 && field.id == FIRE)
                .map(|(other, _, field)| (other, field.entry.intensity(), field.entry.age()))
        };
        let Some((other, other_intensity, other_age)) = neighbour else {
            continue;
        };
        adjacent += 1;
        if !should_boost(intensity, age, other_intensity, other_age) {
            continue;
        }
        if other_intensity < 2
            && let Some(mut field) = world.get_mut::<Field>(other)
        {
            field.entry.set_intensity(2);
        }
        if let Some(mut field) = world.get_mut::<Field>(other) {
            field.entry.set_age(other_age - 300);
        }
        if let Some(mut field) = world.get_mut::<Field>(entity) {
            field.entry.set_age(age + 300);
        }
    }
    adjacent
}

/// What a fire leaves behind: smoke, and heat.
///
/// Reference: `field_processor_fd_fire` (`src/map_field.cpp:1400-1425`). Smoke is made only when the
/// terrain does not suppress it, when `rng(0, 100 - windpower) <= smoke` passes - so wind makes
/// smoke rarer - and when `rng(3, 35)` comes in under the fire's intensity times ten. Hot air is
/// made when fewer than five fires are adjacent, on a `rng(0, 4 - adjacent)` that must be non-zero,
/// which is the reference's way of thinning it out as the neighbouring fires multiply.
///
/// The reference prefers to put smoke on the square above and falls back to its own square. The port
/// has no levels yet, so it always takes the fallback: smoke at the fire's square, at full intensity
/// of the fire. When levels land, this is the function that grows a branch.
pub fn process_fire_residue(
    world: &mut World,
    entity: Entity,
    smoke: i32,
    windpower: i32,
    suppressed: bool,
    adjacent_fires: usize,
    rolls: &mut cdda_core_types::rng::Rolls,
) {
    let Some(position) = world.get::<Position>(entity).copied() else {
        return;
    };
    let Some(intensity) = world
        .get::<Field>(entity)
        .map(|field| field.entry.intensity())
    else {
        return;
    };
    if !suppressed && rolls.rng(0, 100 - windpower) <= smoke && rolls.rng(3, 35) < intensity * 10 {
        add_field_strength(
            world,
            (position.x, position.y, position.z),
            SMOKE,
            intensity,
        );
    }
    let ceiling = 4 - i32::try_from(adjacent_fires).unwrap_or(4);
    if adjacent_fires < 5 && ceiling > 0 && rolls.rng(0, ceiling) != 0 {
        create_hot_air(world, (position.x, position.y), position.z, intensity, rolls);
    }
}

/// Adds radiation to a square, the way `field_processor_extra_radiation` does.
///
/// Reference: `field_processor_extra_radiation` (`src/map_field.cpp:664-673`): the guard is on the **maximum**, so a level that radiates nothing returns before it
/// draws - which matters because the draw is a die, and a die that is not thrown is a world that does not move.
pub fn process_extra_radiation(
    world: &mut World,
    square: (i32, i32, i32),
    min: i32,
    max: i32,
    rolls: &mut cdda_core_types::rng::Rolls,
) {
    if max <= 0 {
        return;
    }
    let extra_radiation = rolls.rng(min, max);
    singletons::adjust_radiation(world, square, extra_radiation);
}



/// Fills the squares around a vent with the gas it wanders into.
///
/// Reference: `field_processor_wandering_field` (`src/map_field.cpp:675-690`). Two details are the whole point of the function:
///
/// * the reach is `intensity - 1` squares, so the weakest vent fills only its own square;
/// * a square whose wandering field is **weaker** gains one level, and any other square - including one already at least as strong - is handed to `add_field`, which
///   **sums** the strengths.
pub fn process_wandering_field(
    world: &mut World,
    square: (i32, i32, i32),
    intensity: i32,
    wandering_field: cdda_core_types::field::FieldId,
) {
    let reach = intensity - 1;
    if reach < 0 {
        return;
    }
    // **A square, not a circle.** `points_in_radius` is `tripoint_range( center - ( r, r, 0 ), center + ( r, r, 0 ) )` (`src/map_iterator.h:153-160`) - the circle is
    // the separate `points_in_radius_circ` - so a vent at intensity two fills all nine squares and not merely the five that a circle of radius one would hold.
    for dy in -reach..=reach {
        for dx in -reach..=reach {
            let at = (square.0 + dx, square.1 + dy, square.2);
            let existing = {
                let mut query = world.query::<(Entity, &Position, &Field)>();
                query
                    .iter(world)
                    .find(|(_, position, field)| {
                        (position.x, position.y, position.z) == at && field.id == wandering_field
                    })
                    .map(|(entity, _, field)| (entity, field.entry.intensity()))
            };
            match existing {
                Some((entity, already)) if already < intensity => {
                    if let Some(mut field) = world.get_mut::<Field>(entity) {
                        field.entry.set_intensity(already + 1);
                    }
                }
                _ => add_field_strength(world, at, wandering_field, intensity),
            }
        }
    }
}

/// Whether a square has a field of a given type on it.
#[must_use]
pub fn has_field_at(world: &World, square: (i32, i32, i32), id: FieldId) -> bool {
    world
        .try_query::<(Entity, &Position, &Field)>()
        .is_some_and(|mut query| {
            query
                .iter(world)
                .any(|(_, at, field)| (at.x, at.y, at.z) == square && field.id == id)
        })
}

/// How strongly a field sits on a square, or zero when it is not there.
///
/// The reader beside [`add_field_strength`], needed by anything that asks "how thick is it here" - the field spread does, once
/// per square, the way the reference's `get_field_intensity` does.
#[must_use]
pub fn field_strength_at(world: &World, square: (i32, i32, i32), id: FieldId) -> i32 {
    let Some(mut query) = world.try_query::<(&Position, &Field)>() else {
        return 0;
    };
    query
        .iter(world)
        .filter(|(at, field)| (at.x, at.y, at.z) == square && field.id == id)
        .map(|(_, field)| field.entry.intensity())
        .max()
        .unwrap_or(0)
}

/// Takes a field type off a square, answering whether there was one.
///
/// Reference: `map::remove_field( pos, field_type_id )` (`src/map.cpp`), which erases that field's entry and leaves every
/// other field on the square alone.
pub fn remove_field(world: &mut World, square: (i32, i32, i32), id: FieldId) -> bool {
    let found: Option<Entity> = {
        let mut query = world.query::<(Entity, &Position, &Field)>();
        query
            .iter(world)
            .find(|(_, at, field)| (at.x, at.y, at.z) == square && field.id == id)
            .map(|(entity, _, _)| entity)
    };
    match found {
        Some(entity) => {
            world.despawn(entity);
            true
        }
        None => false,
    }
}

/// Adds or strengthens a field on a square: a field already there gets thicker, and an empty square grows one.
///
/// Reference: `map::add_field` (`src/map.cpp`), which adds the arriving strength to what is there and lets the setter
/// clamp it.
pub fn add_field_strength(world: &mut World, square: (i32, i32, i32), id: FieldId, strength: i32) {
    let existing = {
        let mut query = world.query::<(Entity, &Position, &Field)>();
        query
            .iter(world)
            .find(|(_, at, field)| (at.x, at.y, at.z) == square && field.id == id)
            .map(|(entity, _, field)| (entity, field.entry.intensity()))
    };
    if let Some((entity, already)) = existing {
        // The reference **adds** the two strengths and lets the setter clamp them (`src/field.cpp:173`). Keeping the stronger of the two looks
        // reasonable and is wrong: a second patch of smoke on a smoky square makes it smokier, not merely as smoky as it already was.
        if let Some(mut field) = world.get_mut::<Field>(entity) {
            field.entry.set_intensity(already + strength);
        }
        return;
    }
    // The square's cap comes from the field **type**, not from a constant: `field_type::get_max_intensity` is the number of the
    // type's intensity levels, and the data's fields run from one to ten of them.
    let cap = crate::field_registry::max_intensity_of(world, id);
    let mut entry = FieldEntry::new(cap, 100, true, false);
    entry.set_intensity(strength);
    world.spawn((
        Position {
            x: square.0,
            y: square.1,
            z: square.2,
        },
        Field {
            id,
            entry,
            priority: 0,
        },
    ));
}

/// What a square's terrain has to offer a fire.
#[derive(Debug, Clone, Copy, Default)]
#[allow(clippy::struct_excessive_bools)]
pub struct TerrainFuel {
    /// `FLAMMABLE`: burns every turn.
    pub flammable: bool,
    /// `FLAMMABLE_HARD`: burns only when `one_in(3)` allows.
    pub flammable_hard: bool,
    /// `FLAMMABLE_ASH`: burns and leaves ash behind.
    pub flammable_ash: bool,
}

/// What one turn of burning terrain gives the fire.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerrainBurn {
    /// Turns of life added to the fire.
    pub time_added: i64,
    /// Smoke contribution.
    pub smoke: i32,
    /// Whether the terrain is wrecked this turn.
    pub bashes: bool,
    /// Ash left behind, as charges, when the terrain was of the ashy kind.
    pub ash_charges: Option<i32>,
}

/// Consuming the ground a fire is standing on.
///
/// Reference: `field_processor_fd_fire` (`src/map_field.cpp:1051-1090`). `FLAMMABLE` ground feeds the
/// fire `5 - intensity` turns and always burns. `FLAMMABLE_HARD` needs `one_in(3)` first and gives
/// `4 - intensity`. `FLAMMABLE_ASH` always burns, gives `5 - intensity`, and leaves ash when the
/// ground is wrecked. Every burning kind adds two smoke plus one per five points of wind power, and
/// a fire above intensity 1 wrecks the terrain on `one_in(200 - intensity * 50)`; the ashy kind then
/// leaves 10 to 1000 charges of ash.
///
/// The dice order is the reference's: the hard-ground check first, then the wrecking check, then the
/// ash. A square that is merely `FLAMMABLE_HARD` and fails its `one_in(3)` burns nothing at all.
#[must_use]
pub fn burn_terrain(
    fuel: TerrainFuel,
    intensity: i32,
    windpower: i32,
    rolls: &mut cdda_core_types::rng::Rolls,
) -> Option<TerrainBurn> {
    let (time_added, ashy) = if fuel.flammable {
        (i64::from(5 - intensity), false)
    } else if fuel.flammable_hard && rolls.one_in(3) {
        (i64::from(4 - intensity), false)
    } else if fuel.flammable_ash {
        (i64::from(5 - intensity), true)
    } else {
        return None;
    };
    let mut smoke = 2 + windpower / 5;
    let mut bashes = false;
    let mut ash_charges = None;
    if intensity > 1 && rolls.one_in(200 - intensity * 50) {
        bashes = true;
        if ashy {
            ash_charges = Some(rolls.rng(10, 1000));
        }
    }
    if smoke < 0 {
        smoke = 0;
    }
    Some(TerrainBurn {
        time_added,
        smoke,
        bashes,
        ash_charges,
    })
}

/// Which neighbours a fire may reach through the wind, and which the wind shields.
///
/// Reference: `field_processor_fd_fire` (`src/map_field.cpp:1136-1152`). The reference caches three
/// wind-blocking neighbours for the wind direction and then keeps a neighbour when
///
/// ```text
/// (x != b1.x && y != b1.y) || (x != b2.x && y != b2.y) || (x != b3.x && y != b3.y)
///     || x_in_y( 1, max( 2, windpower ) )
/// ```
///
/// The "differs" test is `x != b.x && y != b.y`, so a neighbour that shares **either** coordinate
/// with a blocker counts as not differing from it. Only a neighbour that shares coordinates with all
/// three blockers reaches the `x_in_y` and can be shielded; every other neighbour is kept without
/// drawing. For the usual three distinct blockers that means exactly those blockers are the ones the
/// wind can stop, and only sometimes. The port reproduces the expression as written - dice included,
/// because a replay has to draw the same numbers.
#[must_use]
pub fn wind_reach(
    neighbour: (i32, i32),
    blockers: [(i32, i32); 3],
    windpower: i32,
    rolls: &mut cdda_core_types::rng::Rolls,
) -> bool {
    let differs = |blocker: (i32, i32)| neighbour.0 != blocker.0 && neighbour.1 != blocker.1;
    differs(blockers[0])
        || differs(blockers[1])
        || differs(blockers[2])
        || rolls.x_in_y(1.0, f64::from(windpower.max(2)))
}

/// How many of the eight neighbours are on fire.
fn count_adjacent_fires(world: &mut World, entity: Entity) -> usize {
    let Some(position) = world.get::<Position>(entity).copied() else {
        return 0;
    };
    let mut query = world.query::<(&Position, &Field)>();
    query
        .iter(world)
        .filter(|(at, field)| {
            field.id == FIRE
                && (at.x != position.x || at.y != position.y)
                && (at.x - position.x).abs() <= 1
                && (at.y - position.y).abs() <= 1
        })
        .count()
}

/// Everything one turn of burning needs from the world around the fire.
pub struct FireContext<'a> {
    /// What the fire is standing on.
    pub fuel: TerrainFuel,
    /// What a neighbouring square offers, asked once per neighbour.
    pub neighbour: &'a dyn Fn((i32, i32)) -> NeighbourFacts,
    /// Wind strength at the fire.
    pub windpower: i32,
    /// Items consumed this turn, which gate smoke.
    pub consumed: i32,
    /// Whether the terrain suppresses smoke.
    pub smoke_suppressed: bool,
}

/// One turn of a fire, in the reference's order.
///
/// Reference: `field_processor_fd_fire` (`src/map_field.cpp:945-1425`), which runs its steps in this
/// sequence and draws dice in this sequence:
///
/// 1. eat the ground (terrain fuel decides the time and smoke gained);
/// 2. age: gained time is subtracted, otherwise a fire with fuel ages ten seconds per intensity;
/// 3. a strong fire's `one_in(3)` flashpoint, or the growth check for the others - never both;
/// 4. spreading to neighbours;
/// 5. smoke and hot air.
///
/// What is not here yet, and is named rather than implied: items burning, vehicles, zero-level
/// fall-through, webs, pits, and the effects a fire has on creatures.
pub fn process_fire(
    world: &mut World,
    entity: Entity,
    context: &FireContext<'_>,
    rolls: &mut cdda_core_types::rng::Rolls,
) {
    let Some(intensity) = world
        .get::<Field>(entity)
        .map(|field| field.entry.intensity())
    else {
        return;
    };
    let terrain_burn = burn_terrain(context.fuel, intensity, context.windpower, rolls);
    let mut smoke = context.consumed;
    if let Some(burn) = terrain_burn {
        smoke += burn.smoke;
    }
    // Age: gained time is spent first; a fire with nothing to eat starves.
    if let Some(burn) = terrain_burn
        && burn.time_added != 0
    {
        if let Some(mut field) = world.get_mut::<Field>(entity) {
            let age = field.entry.age();
            field.entry.set_age(age - burn.time_added);
        }
    } else {
        let has_fuel = terrain_burn.is_some();
        process_fire_burnout(world, entity, has_fuel);
    }
    // A strong fire spends its extra life on neighbours, or else grows itself.
    let adjacent = if intensity > 1 {
        process_flashpoint(world, entity, rolls)
    } else {
        let age = world
            .get::<Field>(entity)
            .map_or(0, |field| field.entry.age());
        let adjacent = count_adjacent_fires(world, entity);
        if let Some((grown, age)) = grow_fire(intensity, age, adjacent)
            && let Some(mut field) = world.get_mut::<Field>(entity)
        {
            field.entry.set_intensity(grown);
            field.entry.set_age(age);
        }
        adjacent
    };
    process_fire_spread(world, entity, |square| (context.neighbour)(square), rolls);
    process_fire_residue(
        world,
        entity,
        smoke,
        context.windpower,
        context.smoke_suppressed,
        adjacent,
        rolls,
    );
}

/// A field that throws rocks around.
pub const PUSH_ITEMS: FieldId = FieldId(30);

/// A push-items field hurls aged rocks at its neighbours that also hold one.
///
/// Reference: `field_processor_fd_push_items` (`src/map_field.cpp:790-836`). Only rocks count, and
/// only rocks that have lain there at least one turn; a rock is thrown to a random one of the
/// neighbouring squares that has a push-items field of its own, losing its age on the way. A square
/// with no such neighbour keeps its rocks. The reference also hurts whatever is standing where a
/// rock lands - the port has no creatures yet, so that part is named in the ledger rather than
/// guessed at.
///
/// The caller supplies the item id for a rock, because item ids come from the item registry.
pub fn process_push_items(
    world: &mut World,
    square: (i32, i32),
    rock: ItemTypeId,
    rolls: &mut cdda_core_types::rng::Rolls,
) -> usize {
    let rocks: Vec<Entity> = items_at(world, square)
        .into_iter()
        .filter(|entity| {
            world
                .get::<GroundItem>(*entity)
                .is_some_and(|item| item.kind == rock && item.age >= 1)
        })
        .collect();
    if rocks.is_empty() {
        return 0;
    }
    let destinations: Vec<(i32, i32)> = {
        let mut found = Vec::new();
        for dy in -1..=1 {
            for dx in -1..=1 {
                if dx == 0 && dy == 0 {
                    continue;
                }
                let candidate = (square.0 + dx, square.1 + dy);
                let mut query = world.query::<(&Position, &Field)>();
                if query.iter(world).any(|(at, field)| {
                    field.id == PUSH_ITEMS && at.x == candidate.0 && at.y == candidate.1
                }) {
                    found.push(candidate);
                }
            }
        }
        found
    };
    if destinations.is_empty() {
        return 0;
    }
    let mut moved = 0;
    for entity in rocks {
        let Some(item) = world.get::<GroundItem>(entity).cloned() else {
            continue;
        };
        let pick = rolls.rng(0, i32::try_from(destinations.len() - 1).unwrap_or(0));
        let Some(destination) = destinations
            .get(usize::try_from(pick).unwrap_or(0))
            .copied()
        else {
            continue;
        };
        take_item(world, entity);
        drop_item_aged(world, destination, item.kind, item.charges, 0);
        moved += 1;
    }
    moved
}

/// The gas processor: fields whose data says they spread do so every turn.
///
/// Reference: `field_processor_spread_gas` (`src/map_field.cpp:579-585`), which runs for any field
/// with a non-zero spread chance (`field_type::gas_can_spread`). Everything else about the turn -
/// the chance, the damping by wind, where the gas goes - is [`spread_gas`].
///
/// The caller supplies the spread chance, the wind and the outdoor speedup, because those come from
/// the field's prototype, which the sim does not own.
pub fn process_spread_gas(
    world: &mut World,
    source: Entity,
    context: &GasContext,
    can_enter: impl Fn(i32, i32) -> bool,
    rolls: &mut cdda_core_types::rng::Rolls,
) {
    if context.percent_spread <= 0 {
        return;
    }
    spread_gas(
        world,
        source,
        context,
        |from, to| to.2 == from.2 && can_enter(to.0, to.1),
        rolls,
    );
}

/// A field under water ages away faster.
///
/// Reference: `field_processor_underwater_dissipation` (`src/map_field.cpp:516-523`). If the terrain
/// carries the swimmable flag the field's age advances by the field type's
/// `underwater_age_speedup`; nothing else happens and no dice are drawn.
pub fn process_underwater_dissipation(
    world: &mut World,
    entity: Entity,
    swimmable: bool,
    speedup: i64,
) {
    if !swimmable || speedup == 0 {
        return;
    }
    let Some(age) = world.get::<Field>(entity).map(|field| field.entry.age()) else {
        return;
    };
    if let Some(mut field) = world.get_mut::<Field>(entity) {
        field.entry.set_age(age + speedup);
    }
}

/// A field that grows one intensity on its own, once in a while.
///
/// Reference: `field_processor_upgrade_intensity` (`src/map_field.cpp:503-514`). The level's
/// `upgrade_chance` and `upgrade_duration` must both be positive, the calendar must land on the
/// period (`calendar::once_every`), and only then is `one_in(chance)` drawn.
///
/// The data says exactly one field in the corpus can do this at all (`fd_fatigue`), which is worth
/// knowing before wondering why a processor never seems to fire.
pub fn process_upgrade_intensity(
    world: &mut World,
    entity: Entity,
    now: i64,
    chance: i32,
    duration: i64,
    rolls: &mut cdda_core_types::rng::Rolls,
) {
    if chance <= 0 || duration <= 0 || now.rem_euclid(duration) != 0 {
        return;
    }
    if !rolls.one_in(chance) {
        return;
    }
    let Some(intensity) = world
        .get::<Field>(entity)
        .map(|field| field.entry.intensity())
    else {
        return;
    };
    if let Some(mut field) = world.get_mut::<Field>(entity) {
        field.entry.set_intensity(intensity + 1);
    }
}

/// A monster type, as the monster registry numbers it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MonsterKindId(pub u32);

/// Puts a creature on a square, with a turn's worth of moves in hand.
///
/// Reference: `monster::monster` leaves `moves` at zero and the turn loop's `regen_moves` grants them before
/// anything acts. This helper does that granting up front, because its callers spawn a creature and run its
/// turn without a world in between.
pub fn spawn_monster(
    world: &mut World,
    square: (i32, i32),
    kind: MonsterKindId,
    hp: i32,
) -> Entity {
    let entity = crate::creature::spawn_creature(world, "", (square.0, square.1, 0));
    crate::creature::set_kind(world, entity, kind);
    crate::creature::set_health(world, entity, hp);
    // A creature made this way is one the world is about to give a turn to - the older callers spawn and act
    // in the same breath, and a creature with no moves would silently stand still. `creature::spawn_creature`
    // is the reference's own constructor and leaves moves at zero; this one is the "ready to act" door.
    crate::creature::set_moves(world, entity, 100);
    entity
}

/// Every creature standing on one square, in spawn order.
#[must_use]
/// Applies a gas effect to one creature.
///
/// Returns whether the creature died. Reference: the per-creature part of
/// `map_field::process_fields_in_submap` (`src/map_field.cpp:1900-1990`), which deals damage and
/// takes moves in the same pass.
pub fn breathe_gas(world: &mut World, entity: Entity, effect: &GasEffect) -> bool {
    crate::creature::spend_moves(world, entity, effect.moves_penalty);
    damage_monster(world, entity, effect.damage)
}

/// Hurts a creature, and takes it off the map when it has nothing left.
///
/// Returns whether the creature died. Reference: `Creature::deal_damage` and the death check that
/// follows it (`src/monster.cpp`); the port keeps the part a field needs - damage goes in, and a
/// creature at zero or below leaves the world.
pub fn damage_monster(world: &mut World, entity: Entity, amount: i32) -> bool {
    let Some(hp) = crate::creature::health_of(world, entity) else {
        return false;
    };
    let left = hp - amount.max(0);
    if left <= 0 {
        // The index goes with the entity, or the square keeps claiming an occupant that no longer exists.
        if let Some(place) = world.get::<Position>(entity).map(|place| (place.x, place.y, place.z)) {
            crate::singletons::index_mut(world).remove(entity, place);
        }
        world.despawn(entity);
        return true;
    }
    crate::creature::set_health_left(world, entity, left);
    false
}

/// What breathing a gas does to a creature this turn.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GasEffect {
    /// Hit points lost, before armour.
    pub damage: i32,
    /// Moves taken away, in raw move units.
    pub moves_penalty: f64,
}

/// Toxic gas: a little damage, a little slowness, per point of intensity.
///
/// Reference: `field_processor_fd_toxic_gas` (`src/map_field.cpp:1966-1971`). A creature that
/// breathes takes damage equal to the intensity and loses that many moves. Nothing is drawn.
#[must_use]
pub fn toxic_gas_effect(intensity: i32, no_breathe: bool) -> GasEffect {
    if no_breathe {
        return GasEffect {
            damage: 0,
            moves_penalty: 0.0,
        };
    }
    GasEffect {
        damage: intensity,
        moves_penalty: f64::from(intensity),
    }
}

/// Nuke gas: worse with intensity, and much worse for plants.
///
/// Reference: `field_processor_fd_nuke_gas` (`src/map_field.cpp:1972-1990`). Each intensity draws its
/// own move penalty and damage; a plant takes a second move penalty on top, drawn from the intensity,
/// and then has its damage multiplied by the intensity.
///
/// The draw order matters: the move penalty first, the damage second, and the plant's penalty last.
#[must_use]
pub fn nuke_gas_effect(
    intensity: i32,
    no_breathe: bool,
    veggy: bool,
    rolls: &mut cdda_core_types::rng::Rolls,
) -> GasEffect {
    if no_breathe {
        return GasEffect {
            damage: 0,
            moves_penalty: 0.0,
        };
    }
    let (mut moves_penalty, mut damage) = match intensity {
        3 => (rolls.rng_float(0.6, 1.2), rolls.rng(30, 50)),
        2 => (rolls.rng_float(0.2, 0.5), rolls.rng(10, 25)),
        _ => (rolls.rng_float(0.0, 0.15), rolls.rng(0, 12)),
    };
    if veggy {
        moves_penalty += rolls.rng_float(f64::from(intensity) * 0.05, f64::from(intensity) * 0.12);
        damage *= intensity;
    }
    GasEffect {
        damage,
        moves_penalty,
    }
}

/// How many spawn attempts a field asks for this turn, and whether it is allowed any at all.
///
/// Reference: `field_processor_monster_spawn` (`src/map_field.cpp:763-787`). A level needs a positive
/// count and a positive chance, and then a single `one_in(chance)` decides; the count is how many
/// monsters the group will be asked for, not how many separate rolls happen.
///
/// The port stops here, at the gate, and says so: turning a monster group into actual creatures needs
/// the group loader and a deferred spawn list, and neither exists yet. What is ported is the part the
/// data decides - 15 for the Tindalos rift, 600 for the fatigue tear.
#[must_use]
pub fn monster_spawn_attempts(
    spawn_chance: i32,
    spawn_count: i32,
    rolls: &mut cdda_core_types::rng::Rolls,
) -> i32 {
    if spawn_chance <= 0 || spawn_count <= 0 {
        return 0;
    }
    if rolls.one_in(spawn_chance) {
        spawn_count
    } else {
        0
    }
}

/// One material in an item: its portions, and its behaviour at each intensity.
#[derive(Debug, Clone, Copy)]
pub struct MaterialPortion {
    /// How many portions of it the item has.
    pub portions: i32,
    /// Its behaviour at intensity 1, 2 and 3, in that order.
    pub levels: [BurnDataView; 3],
}

impl MaterialPortion {
    /// The material's behaviour at an intensity, clamped to the three the data describes.
    #[must_use]
    pub fn at(&self, intensity: i32) -> &BurnDataView {
        // The data describes exactly three intensities, so the clamp above always lands inside it.
        self.levels
            .get(usize::try_from(intensity.clamp(1, 3) - 1).unwrap_or(0))
            .unwrap_or(&self.levels[0])
    }
}

/// What a material contributes at one intensity, as the item sees it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BurnDataView {
    /// Nothing burns.
    pub immune: bool,
    /// Volume the material can burn per turn, in millilitres. Zero means unconstrained.
    pub volume_per_turn: f64,
    /// Fuel contributed.
    pub fuel: f64,
    /// Smoke produced.
    pub smoke: f64,
    /// Fraction destroyed per turn.
    pub burn: f64,
}

/// An item as the fire sees it.
#[derive(Debug, Clone)]
pub struct Burnable {
    /// Its materials and their portions.
    pub materials: Vec<MaterialPortion>,
    /// Its base volume, in millilitres.
    pub volume_ml: f64,
    /// Whether it is a liquid, which smothers weak fires.
    pub liquid: bool,
    /// The sum of material portions the item type declares, one when it declares none.
    pub material_total: i32,
    /// Whether it is counted by charges (a stack rather than one object).
    pub by_charges: bool,
    /// How many charges it has.
    pub charges: i32,
    /// How many charges make one stack.
    pub stack_size: i32,
}

/// What one turn of burning does to an item.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BurnResult {
    /// Fuel added to the fire.
    pub fuel: f64,
    /// Smoke produced.
    pub smoke: f64,
    /// Fraction of the item destroyed, which the caller turns into charges.
    pub burn: f64,
}

/// How much of an item a fire consumes in one turn.
///
/// Reference: `item::simulate_burn` (`src/item.cpp:12543-12592`). A contained fire burns at intensity
/// 3 whatever its real intensity. A material that is immune stops the whole item burning. Each
/// material contributes fuel, smoke and burn, scaled by the volume it can burn per turn against the
/// item's volume unless the fire is contained or the material has no volume limit. A liquid that
/// would give the fire less than 200 draws `rng(400 * litres, 1200 * litres)` to *subtract*, because
/// a wet thing smothers a small fire. Several materials are averaged by the item's declared portions,
/// and an item with no materials at all burns at a flat rate of one. A stack multiplies all three by
/// its number of stacks.
#[must_use]
pub fn simulate_burn(
    item: &Burnable,
    fire_intensity: i32,
    contained: bool,
    rolls: &mut cdda_core_types::rng::Rolls,
) -> BurnResult {
    let intensity = if contained { 3 } else { fire_intensity };
    let mut fuel = 0.0;
    let mut smoke = 0.0;
    let mut burn = 0.0;
    for material in &item.materials {
        let burn_data = material.at(intensity);
        if burn_data.immune {
            return BurnResult {
                fuel: 0.0,
                smoke: 0.0,
                burn: 0.0,
            };
        }
        let scale = f64::from(material.portions);
        if contained || burn_data.volume_per_turn <= 0.0 {
            fuel += burn_data.fuel * scale;
            smoke += burn_data.smoke * scale;
            burn += burn_data.burn * scale;
        } else {
            let litres = item.volume_ml / 1000.0;
            let rate = if litres > 0.0 {
                (burn_data.volume_per_turn / 1000.0) / litres
            } else {
                0.0
            };
            fuel += burn_data.fuel * rate * scale;
            smoke += burn_data.smoke * rate * scale;
            burn += burn_data.burn * rate * scale;
        }
    }
    let materials = i32::try_from(item.materials.len()).unwrap_or(0);
    if item.liquid && fuel < 200.0 {
        let litres = item.volume_ml / 1000.0;
        fuel -= rolls.rng_float(400.0 * litres, 1200.0 * litres);
    } else if materials > 1 {
        let total = f64::from(item.material_total.max(1));
        fuel /= total;
        smoke /= total;
        burn /= total;
    } else if materials == 0 {
        burn = 1.0;
    }
    if item.by_charges {
        let stacks = (item.charges / item.stack_size.max(1)).max(1);
        let scale = f64::from(stacks);
        fuel *= scale;
        smoke *= scale;
        burn *= scale;
    }
    BurnResult { fuel, smoke, burn }
}

/// Rounds a value down, and up with the probability of its fraction.
///
/// Reference: `roll_remainder` (`src/rng.cpp`). A whole value draws nothing; a fractional one draws
/// once. The order of these draws is part of the contract for anything that burns.
#[must_use]
pub fn roll_remainder(value: f64, rolls: &mut cdda_core_types::rng::Rolls) -> i32 {
    let whole = value.trunc();
    let fraction = value - whole;
    let bumped = if fraction > 0.0 {
        rolls.rng_float(0.0, 1.0) < fraction
    } else {
        false
    };
    #[allow(clippy::cast_possible_truncation)]
    let result = whole as i32;
    if bumped { result + 1 } else { result }
}

/// What a burning item remembers between turns.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BurnState {
    /// Whether it is a stack counted in charges.
    pub by_charges: bool,
    /// Charges left, for a stack.
    pub charges: i32,
    /// Charges in one stack.
    pub stack_size: i32,
    /// Its base volume, in millilitres.
    pub volume_ml: f64,
    /// How much of a solid item has burned away so far.
    pub burnt: i32,
}

/// Applies a turn of burning to one item, and says whether it is gone.
///
/// Reference: the bookkeeping half of `item::burn` (`src/item.cpp:12609-12643`). A stack loses
/// `roll_remainder(burn * 250ml * stack_size / (3 * volume))` charges, and a stack with no volume
/// loses them all at once. A solid item accumulates `roll_remainder(burn)` and is gone once it has
/// burned `volume / 250ml * 3`.
pub fn burn_item(
    state: &mut BurnState,
    burn_added: f64,
    rolls: &mut cdda_core_types::rng::Rolls,
) -> bool {
    if state.by_charges {
        if state.volume_ml <= 0.0 {
            state.charges = 0;
        } else {
            let lost = roll_remainder(
                burn_added * 250.0 * f64::from(state.stack_size) / (3.0 * state.volume_ml),
                rolls,
            );
            state.charges -= lost;
        }
        return state.charges <= 0;
    }
    state.burnt += roll_remainder(burn_added, rolls);
    let threshold = (state.volume_ml / 250.0 * 3.0).trunc();
    #[allow(clippy::cast_possible_truncation)]
    let limit = threshold as i32;
    state.burnt >= limit
}

/// How to build the fire's view of one kind of item.
#[derive(Debug, Clone)]
pub struct ItemBurnPlan {
    /// Its materials and their portions.
    pub materials: Vec<MaterialPortion>,
    /// Its base volume, in millilitres.
    pub volume_ml: f64,
    /// Whether it is a liquid.
    pub liquid: bool,
    /// The item type's declared material portions, one when it declares none.
    pub material_total: i32,
    /// Whether it is counted in charges.
    pub by_charges: bool,
    /// How many charges make one stack.
    pub stack_size: i32,
}

/// What one square's worth of burning produced.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ItemBurnTotals {
    /// Smoke produced, before rounding.
    pub smoke: f64,
    /// Fuel produced, before rounding.
    pub fuel: f64,
    /// How many items the fire finished off.
    pub consumed: i32,
    /// How many items went off instead of burning.
    pub detonated: i32,
}

/// What the burning loop needs to ask about an item, bundled so the signature stays readable.
pub struct ItemFireRules<F, I, E> {
    /// How it burns, or `None` when it does not.
    pub plan_of: F,
    /// Whether it cannot be taken out of its container.
    pub irremovable: I,
    /// Whether a fire sets it off rather than burning it.
    pub explodes: E,
}

/// Burns what is lying on one square.
///
/// Reference: the item loop inside `field_processor_fd_fire` (`src/map_field.cpp:980-1031`). A fire
/// consumes at most `intensity * 2` items a turn and stops as soon as it has. Each item's contribution
/// comes from [`simulate_burn`] and its bookkeeping from [`burn_item`]; something the fire finishes
/// leaves the map. Smoke and fuel are summed for the caller, which rounds them the way the reference
/// does.
///
/// Not ported, and declared: items that explode in fire (`will_explode_in_fire`), the contents of a
/// destroyed container, and burn products spawned by lost weight. All three need item contents, which
/// the port does not carry yet.
pub fn burn_items_in_fire(
    world: &mut World,
    square: (i32, i32),
    intensity: i32,
    contained: bool,
    rules: &ItemFireRules<
        impl Fn(ItemTypeId) -> Option<ItemBurnPlan>,
        impl Fn(Entity) -> bool,
        impl Fn(Entity) -> bool,
    >,
    rolls: &mut cdda_core_types::rng::Rolls,
) -> ItemBurnTotals {
    let max_consume = intensity * 2;
    let mut totals = ItemBurnTotals {
        smoke: 0.0,
        fuel: 0.0,
        consumed: 0,
        detonated: 0,
    };
    // Items that go off in a fire are removed before anything burns (`src/map_field.cpp:982-993`):
    // the reference detonates them and swallows whatever they throw out. The port removes them and
    // spills what they held; the explosion itself needs blast damage, which is declared not ported.
    for entity in items_at(world, square) {
        if !(rules.explodes)(entity) {
            continue;
        }
        for held in contents(world, entity) {
            take_out(world, held);
            if (rules.irremovable)(held) {
                world.despawn(held);
            }
        }
        world.despawn(entity);
        totals.detonated += 1;
    }
    let here = items_at(world, square);
    for entity in here {
        if totals.consumed >= max_consume {
            break;
        }
        let (kind, charges, burnt) = match world.get::<GroundItem>(entity) {
            Some(item) => (item.kind, item.charges, item.burnt),
            None => continue,
        };
        let Some(plan) = (rules.plan_of)(kind) else {
            continue;
        };
        let burnable = Burnable {
            materials: plan.materials,
            volume_ml: plan.volume_ml,
            liquid: plan.liquid,
            material_total: plan.material_total,
            by_charges: plan.by_charges,
            charges,
            stack_size: plan.stack_size,
        };
        let burn = simulate_burn(&burnable, intensity, contained, rolls);
        totals.smoke += burn.smoke;
        totals.fuel += burn.fuel;
        let mut state = BurnState {
            by_charges: plan.by_charges,
            charges,
            stack_size: plan.stack_size,
            volume_ml: plan.volume_ml,
            burnt,
        };
        let destroyed = burn_item(&mut state, burn.burn, rolls);
        if destroyed {
            // A container that burns away spills what was inside it onto the square, which is the
            // reference's `new_content` list (`src/map_field.cpp:1015-1028`). An irremovable item
            // stays where it was, so it does not spill.
            for held in contents(world, entity) {
                take_out(world, held);
                // The reference collects only contents without `NO_UNLOAD` into `new_content`;
                // anything irremovable is erased along with the container rather than spilling.
                if (rules.irremovable)(held) {
                    world.despawn(held);
                }
            }
            world.despawn(entity);
            totals.consumed += 1;
        } else if let Some(mut item) = world.get_mut::<GroundItem>(entity) {
            item.charges = state.charges;
            item.burnt = state.burnt;
        }
    }
    totals
}

/// What a turn needs to know about one kind of field.
#[derive(Debug, Clone, Copy, Default)]
pub struct FieldTurnData {
    /// The chance in a hundred that this gas spreads to a neighbour.
    pub percent_spread: i32,
    /// How much faster it ages away outdoors, in turns.
    pub outdoor_age_speedup: i64,
    /// How much faster it ages away under water, in turns.
    pub underwater_age_speedup: i64,
    /// The self-upgrade chance and period for the intensity it is at, zero when it cannot grow.
    pub upgrade_chance: i32,
    /// How often the upgrade chance is rolled, in turns.
    pub upgrade_duration: i64,
    /// Whether the square it stands on is outdoors.
    pub outdoors: bool,
    /// Whether the square it stands on is swimmable.
    pub swimmable: bool,
    /// The least and most radiation the intensity it is at adds each processing turn, zero for a field that radiates nothing.
    ///
    /// Reference: `field_intensity_level::extra_radiation_min` and `_max` (`src/field.h:106-107`). Only `fd_nuke_gas` has a non-zero maximum in the data.
    pub extra_radiation: (i32, i32),
    /// The field this type wanders into around itself, or `None` when it wanders into nothing.
    ///
    /// Reference: `field_type::wandering_field` (`src/field_type.h:225`). Three vents in the data wander.
    pub wandering_field: Option<cdda_core_types::field::FieldId>,
}

/// The part of the world a turn touches: the reference's reality bubble.
///
/// The reference keeps 11 by 11 submaps loaded around the player (`MAPSIZE` in
/// `src/map_scale.h`), each 12 by 12, which is 132 squares on a side and a radius of 66 from the
/// player's square. Anything further away is not simulated at all, which is why a fire two
/// overmap tiles away does not burn on its own.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Bubble {
    /// The square the bubble is centred on, usually the player's.
    pub center: (i32, i32),
    /// How far from the centre the bubble reaches, in squares.
    pub radius: i32,
}

impl Bubble {
    /// The bubble the reference uses, centred on a square.
    #[must_use]
    pub fn around(center: (i32, i32)) -> Self {
        Self { center, radius: 66 }
    }

    /// Whether a square is inside it. The edge counts.
    #[must_use]
    pub fn contains(&self, x: i32, y: i32) -> bool {
        (x - self.center.0).abs() <= self.radius && (y - self.center.1).abs() <= self.radius
    }
}

/// What a turn needs to know about the world rather than about a field.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TurnContext {
    /// The turn number, in turns since the start.
    pub now: i64,
    /// Wind strength where the fields are.
    pub windpower: i32,
    /// Wind direction, in degrees.
    pub wind_angle: i32,
    /// The part of the world that is simulated.
    pub bubble: Bubble,
}

/// What the map says about one square.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CellFacts {
    /// Whether a gas may spread into it.
    pub can_enter: bool,
    /// Whether it is under water, which makes fields age faster.
    pub swimmable: bool,
    /// Whether it is outdoors, which makes gases dissipate faster.
    pub outdoors: bool,
}

impl CellFacts {
    /// A square of open ground with nothing special about it.
    #[must_use]
    pub fn open() -> Self {
        Self {
            can_enter: true,
            swimmable: false,
            outdoors: false,
        }
    }
}

/// What one turn of field processing did.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FieldTurnSummary {
    /// Fields that were looked at.
    pub processed: u32,
    /// Fields that aged away and were removed.
    pub removed: u32,
    /// Gases that got the chance to spread.
    pub spread_attempts: u32,
}

/// Runs one turn over every field in the world.
///
/// This is the assembly `docs/05-roadmap.md` calls for in P2: a field's own aging first, then the
/// processors the port has, in the order the reference runs them inside
/// `map::process_fields_in_submap` (`src/map_field.cpp:160-460`).
///
/// The order matters and is deliberate: decay, then outdoor dissipation (which is part of spreading),
/// then underwater dissipation, then the self-upgrade, then the spread attempt itself. A field that
/// aged away is gone before any of the later steps run, which is what the reference's `!is_alive()`
/// check does.
pub fn process_field_turn(
    world: &mut World,
    context: &TurnContext,
    data_of: impl Fn(FieldId) -> FieldTurnData,
    cell_of: impl Fn(i32, i32) -> CellFacts,
    rolls: &mut cdda_core_types::rng::Rolls,
) -> FieldTurnSummary {
    let now = context.now;
    let mut summary = FieldTurnSummary::default();
    let mut order: Vec<(i32, i32, Entity)> = Vec::new();
    {
        let mut query = world.query::<(Entity, &Position)>();
        for (entity, position) in query.iter(world) {
            order.push((position.x, position.y, entity));
        }
    }
    order.sort_by_key(|&(x, y, entity)| (x, y, entity.to_bits()));
    for (x, y, entity) in order {
        // Outside the bubble nothing happens at all: not even aging. A field out there keeps the
        // age it had, which is what the reference means by leaving it unloaded.
        if !context.bubble.contains(x, y) {
            continue;
        }
        let Some(field) = world.get::<Field>(entity) else {
            continue;
        };
        let id = field.id;
        // The map has the last word on water and sky: the prototype says how much faster a field
        // ages in each, the terrain says whether it is there at all.
        let cell = cell_of(x, y);
        let mut data = data_of(id);
        data.swimmable = cell.swimmable;
        data.outdoors = cell.outdoors;
        summary.processed += 1;
        // 1. The field's own clock.
        if let Some(mut field) = world.get_mut::<Field>(entity) {
            field.entry.do_decay(now, rolls);
        }
        // 2. Dissipation, both kinds, before anything else.
        process_underwater_dissipation(world, entity, data.swimmable, data.underwater_age_speedup);
        if !field_is_alive(world, entity) {
            world.despawn(entity);
            summary.removed += 1;
            continue;
        }
        // 3. A field that grows on its own.
        process_upgrade_intensity(
            world,
            entity,
            now,
            data.upgrade_chance,
            data.upgrade_duration,
            rolls,
        );
        // 4. The spread attempt, which also ages an outdoor gas.
        if data.percent_spread > 0 {
            summary.spread_attempts += 1;
            let gas = GasContext {
                percent_spread: data.percent_spread,
                windpower: context.windpower,
                wind_angle: context.wind_angle,
                outdoors: data.outdoors,
                outdoor_age_speedup: data.outdoor_age_speedup,
            };
            // The older door has no level facts to give, so it answers the lateral half and refuses a step between levels rather than guessing at one.
            spread_gas(
                world,
                entity,
                &gas,
                |from, to| to.2 == from.2 && cell_of(to.0, to.1).can_enter,
                rolls,
            );
        }
        // 5. Radiation, then 6. the wandering field: the order `processors_for_type` pushes them in (`src/map_field.cpp:2326-2330`).
        let current = world
            .get::<Field>(entity)
            .map_or(0, |field| field.entry.intensity());
        let (least, most) = data.extra_radiation;
        let z = world.get::<Position>(entity).map_or(0, |position| position.z);
        if most > 0 {
            process_extra_radiation(world, (x, y, z), least, most, rolls);
        }
        if let Some(wandering) = data.wandering_field {
            process_wandering_field(world, (x, y, z), current, wandering);
        }
        if !field_is_alive(world, entity) {
            world.despawn(entity);
            summary.removed += 1;
        }
    }
    summary
}

/// Whether a field entity still exists and still holds a living field.
fn field_is_alive(world: &World, entity: Entity) -> bool {
    world
        .get::<Field>(entity)
        .is_some_and(|field| field.entry.is_alive())
}

/// What decides whether two items on the ground become one stack.
///
/// Reference: `item::stacks_with` (`src/item.cpp:1693-1760`). The full rule also compares relics,
/// variants, contents, degradation, temperature, filth and activity; the port compares what its
/// ground items actually carry and the ledger names the rest.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StackFacts {
    /// What kind of item it is.
    pub kind: ItemTypeId,
    /// How many charges it holds.
    pub charges: i32,
    /// How much of it has burned away.
    pub burnt: i32,
    /// Whether the type is counted in charges rather than as separate objects.
    pub by_charges: bool,
    /// How damaged it is, as the reference's level.
    pub damage_level: i32,
}

/// Whether two of these would merge into one stack.
///
/// Two things of the same kind stack. Charges are ignored for a type counted in charges - that is
/// what makes a pile of rocks a pile - and must match exactly otherwise, so two partly-used
/// batteries never merge. Burnt amounts must agree.
#[must_use]
pub fn stacks_with(left: &StackFacts, right: &StackFacts) -> bool {
    left.kind == right.kind
        && left.burnt == right.burnt
        && left.damage_level == right.damage_level
        && (left.by_charges || left.charges == right.charges)
}

/// Drops items on a square, merging into an existing stack when the rule allows it.
///
/// Returns the entity that now holds them: the existing stack when the new charges joined it, or a
/// new one. Reference: `map::add_item` and the merge it performs through `item::stacks_with`.
pub fn drop_and_merge(world: &mut World, square: (i32, i32), facts: StackFacts) -> Entity {
    let here = items_at(world, square);
    for entity in here {
        let Some(existing) = world.get::<GroundItem>(entity) else {
            continue;
        };
        let existing_facts = StackFacts {
            kind: existing.kind,
            charges: existing.charges,
            burnt: existing.burnt,
            damage_level: existing.damage,
            by_charges: facts.by_charges,
        };
        if stacks_with(&existing_facts, &facts) {
            let charges = existing.charges + facts.charges;
            if let Some(mut item) = world.get_mut::<GroundItem>(entity) {
                item.charges = charges;
            }
            return entity;
        }
    }
    drop_item(world, square, facts.kind, facts.charges)
}

/// How many charges of an item fit in a given volume.
///
/// Reference: `item::charges_per_volume` (`src/item.cpp:1391-1420`). For a type counted in charges,
/// one charge takes `volume / stack_size`, so the answer is `wanted * stack_size / volume`. For
/// anything else the answer is how many whole items fit. A type with no volume at all reports
/// `INT_MAX`, which is the reference's way of saying "no limit" without a sentinel of its own.
#[must_use]
pub fn charges_per_volume(
    wanted_ml: i64,
    item_volume_ml: i64,
    stack_size: i32,
    by_charges: bool,
) -> i32 {
    // The reference only reports "no limit" when the item has no volume. A wanted volume of zero
    // simply holds nothing, which is what its arithmetic says and what this returns.
    if item_volume_ml <= 0 {
        return i32::MAX;
    }
    if wanted_ml <= 0 {
        return 0;
    }
    let fits = if by_charges {
        wanted_ml.saturating_mul(i64::from(stack_size.max(1))) / item_volume_ml
    } else {
        wanted_ml / item_volume_ml
    };
    i32::try_from(fits).unwrap_or(i32::MAX)
}

/// How damaged an item is, as one of six levels.
///
/// Reference: `itype::damage_level` (`src/itype.cpp:117-126`) and `item::damage_level` above it. An
/// undamaged item is level 0. A type counted in charges has no partial damage at all - any damage
/// puts it at level 5, which is how a leaking bottle reads as ruined rather than scratched. Anything
/// else is scaled against the type's own maximum: a quarter of the way through its damage is level 2,
/// and the scale saturates at 5.
#[must_use]
pub fn damage_level(damage: i32, damage_max: i32, by_charges: bool) -> i32 {
    if damage == 0 {
        return 0;
    }
    if by_charges {
        return 5;
    }
    if damage_max <= 0 {
        return 5;
    }
    (1 + 4 * damage / damage_max).clamp(0, 5)
}

/// A thing that holds other things: a box, a bottle, a backpack.
///
/// Reference: `item::is_container` and the pocket capacity check in `item::can_contain`
/// (`src/item.cpp`). The reference's pockets each have their own volume, weight and length limits
/// and an allow-list of what they accept; the port keeps one capacity per container and grows
/// pockets when something needs them.
#[derive(Debug, Clone, Component)]
pub struct Container {
    /// The pockets it holds things in, in the order the reference offers them.
    ///
    /// **A pocket is its own limit.** The reference's `item_contents::insert_item` offers a payload to each pocket in turn and takes the first that accepts it, so a
    /// backpack whose pockets hold 25,000 + 510 + 5,000 + 5,000 ml **refuses** a 30,000 ml branch (`src/item_pocket.cpp:1435-1560`). One pooled capacity would take it.
    pub pockets: Vec<Pocket>,
}

/// One pocket: how much it holds, and what it will accept.
///
/// Reference: `pocket_data` (`src/item_pocket.h:500-570`). The port keeps the three facts the rules read: the volume, [`Pocket::watertight`] for liquids and
/// [`Pocket::airtight`] for gases.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pocket {
    /// How much volume it holds, in millilitres.
    pub capacity_ml: i64,
    /// How heavy an item it will take, in grams.
    ///
    /// Reference: `pocket_data::max_contains_weight`, asked as `it.weight() > weight_capacity()` (`src/item_pocket.cpp:1578-1582`). It is **per item** and not a running
    /// total: the reference compares each payload against the pocket's whole capacity, so ten items of half the limit each are fine.
    pub weight_capacity_grams: i64,
    /// Whether a liquid may go in (`pocket_data::watertight`, `src/item_pocket.h:512`).
    pub watertight: bool,
    /// Whether a gas may go in (`pocket_data::airtight`, `src/item_pocket.h:514`).
    pub airtight: bool,
    /// The longest item it will take, in millimetres; zero for no limit.
    ///
    /// Reference: `pocket_data::max_contains_length`, asked as `it.length() > data->max_item_length` and reported as `"item is too long"`
    /// (`src/item_pocket.cpp:1418-1421`). It is asked **before** the weight and the room, because it sits in `is_compatible`, which every `can_contain` runs first.
    pub max_item_length_mm: i64,
    /// The shortest item it will take, in millimetres; zero for no limit. `"item is too short"` (`src/item_pocket.cpp:1423-1426`).
    pub min_item_length_mm: i64,
    /// The smallest item it will take, in millilitres; zero for no limit. `"item is too small"` (`src/item_pocket.cpp:1428-1431`).
    pub min_item_volume_ml: i64,
    /// The item ids it takes, in the data's order; empty with the other two lists for "anything" (`pocket_data::item_id_restriction`).
    pub items: Vec<String>,
    /// The item flags it takes; one is enough (`pocket_data::flag_restrictions`).
    pub flags: Vec<String>,
    /// The materials it takes, one being enough (`pocket_data::material_restriction`).
    pub materials: Vec<String>,
    /// Whether it holds exactly one item, so that a second must be the same kind to go in (`pocket_data::holster`).
    pub holster: bool,
    /// Whether it takes an item of up to a given volume and no bigger, in millilitres (`pocket_data::max_item_volume`).
    pub max_item_volume_ml: Option<i64>,
    /// The ammunition types it takes, from `ammo_restriction`; empty when it takes anything. A pocket that states them takes **only** ammunition
    /// (`src/item_pocket.cpp:1375-1395`).
    pub ammo: Vec<String>,
}

impl Pocket {
    /// A pocket of a given capacity that takes anything of the right phase.
    #[must_use]
    pub const fn of(capacity_ml: i64) -> Self {
        Self {
            capacity_ml,
            weight_capacity_grams: i64::MAX,
            watertight: false,
            airtight: false,
            max_item_length_mm: 0,
            min_item_length_mm: 0,
            min_item_volume_ml: 0,
            items: Vec::new(),
            flags: Vec::new(),
            materials: Vec::new(),
            holster: false,
            max_item_volume_ml: None,
            ammo: Vec::new(),
        }
    }

    /// The same pocket as the data states it.
    #[must_use]
    pub fn from_spec(spec: &cdda_content::pockets::PocketSpec) -> Self {
        Self {
            capacity_ml: spec.max_volume_ml,
            weight_capacity_grams: spec.max_weight_grams,
            watertight: spec.watertight,
            airtight: spec.airtight,
            max_item_length_mm: spec.max_length_mm,
            min_item_length_mm: spec.min_length_mm,
            min_item_volume_ml: spec.min_volume_ml,
            items: spec.items.clone(),
            flags: spec.flags.clone(),
            materials: spec.materials.clone(),
            holster: spec.holster,
            max_item_volume_ml: spec.max_item_volume_ml,
            ammo: spec.ammo.clone(),
        }
    }
}

/// Whether a pocket's accept-lists let a thing in.
///
/// Reference: `item_pocket::is_compatible` (`src/item_pocket.cpp:1362-1372`): with **any** of the three lists stated, the thing has to be named by one of them, or
/// carry one of the flags, or be made of one of the materials - any one is enough, so a named id and a named flag are alternatives rather than requirements. With
/// none of the lists stated the pocket takes anything, and the reference does not even look.
///
/// The refusal is the reference's own: `"holster does not accept this item type or form factor"`, which is the message every restricted pocket gives, holster or
/// not.
#[must_use]
pub fn accepts(
    pocket: &Pocket,
    id: &str,
    flags: &std::collections::BTreeSet<String>,
    base_material: &str,
) -> bool {
    if pocket.items.is_empty() && pocket.flags.is_empty() && pocket.materials.is_empty() {
        return true;
    }
    pocket.items.iter().any(|allowed| allowed == id)
        || pocket.flags.iter().any(|allowed| flags.contains(allowed))
        || pocket.materials.iter().any(|allowed| allowed == base_material)
}

impl Container {
    /// A container with one pocket of a given capacity that takes solids only, which is what most containers are.
    #[must_use]
    pub fn new(capacity_ml: i64) -> Self {
        Self::with_pockets(vec![Pocket::of(capacity_ml)])
    }

    /// A container of the pockets the data gives it.
    #[must_use]
    pub fn with_pockets(pockets: Vec<Pocket>) -> Self {
        Self { pockets }
    }

    /// The same container with every pocket able to hold a liquid.
    #[must_use]
    pub fn watertight(mut self) -> Self {
        for pocket in &mut self.pockets {
            pocket.watertight = true;
        }
        self
    }

    /// The same container with every pocket able to hold a gas.
    #[must_use]
    pub fn airtight(mut self) -> Self {
        for pocket in &mut self.pockets {
            pocket.airtight = true;
        }
        self
    }

    /// What all its pockets hold together, in millilitres.
    #[must_use]
    pub fn capacity_ml(&self) -> i64 {
        self.pockets.iter().map(|pocket| pocket.capacity_ml).sum()
    }
}

/// What phase an item is in: a solid, a liquid or a gas.
///
/// Reference: `phase_id` (`src/units.h`), which the reference asks through `item::made_of( phase_id )`. Liquids and gases are stored as charges and take room a charge at a
/// time; the phase also decides which pockets will take them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    /// A solid.
    Solid,
    /// A liquid, which needs a watertight pocket.
    Liquid,
    /// A gas, which needs an airtight one.
    Gas,
}

/// What [`insert_into`] needs to know about an item: how much room it takes and what phase it is in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemFacts {
    /// Its volume in millilitres.
    pub volume_ml: i64,
    /// Its weight in grams.
    pub weight_grams: i64,
    /// Its phase.
    pub phase: Phase,
    /// Its type id, which the accept-lists name things by (`item_pocket.cpp:1362-1372`).
    pub id: String,
    /// The flags it carries, which the accept-lists and the unwield rule read.
    pub flags: std::collections::BTreeSet<String>,
    /// The material it is mostly made of, which the accept-lists read (`item::get_base_material`).
    pub base_material: String,
    /// The ammunition types it belongs to; empty for anything that is not ammunition (`itype::ammo->type`).
    pub ammo_types: Vec<String>,
    /// Whether it **is** ammunition, which is not the same as naming a caliber: a magazine names the caliber it holds (`item::is_ammo`, `src/item.cpp:10040-10043`).
    pub is_ammo: bool,
    /// How many charges it holds, which is what a biggest-item volume is measured against one of (`charges_per_volume_recursive`).
    pub charges: i32,
    /// Whether it is soft, which its length and its biggest-item volume both read (`item::is_soft`, `src/item.cpp:15199`).
    pub soft: bool,
    /// Its longest side, in millimetres; **zero means "not measured"**, and then the length limits are not asked.
    ///
    /// Reference: `item::length` (`src/item.cpp:7343-7398`), which reads the prototype's `longest_side`. The port does not thread that yet, and it must not invent one: a
    /// real 50 litre item is not a 370 mm cube, so a length derived from volume refuses payloads the reference accepts. Zero therefore means "unknown", and a caller that
    /// knows the real side sets it and gets the real rule.
    pub length_mm: i64,
}

impl ItemFacts {
    /// A solid that takes up `volume_ml`, with the length a cube of that volume has.
    #[must_use]
    pub const fn solid(volume_ml: i64) -> Self {
        Self {
            volume_ml,
            weight_grams: 0,
            phase: Phase::Solid,
            id: String::new(),
            flags: std::collections::BTreeSet::new(),
            base_material: String::new(),
            ammo_types: Vec::new(),
            is_ammo: false,
            charges: 1,
            soft: false,
            length_mm: 0,
        }
    }

    /// A solid of a given volume and weight.
    #[must_use]
    pub const fn solid_weighing(volume_ml: i64, weight_grams: i64) -> Self {
        Self {
            volume_ml,
            weight_grams,
            phase: Phase::Solid,
            id: String::new(),
            flags: std::collections::BTreeSet::new(),
            base_material: String::new(),
            ammo_types: Vec::new(),
            is_ammo: false,
            charges: 1,
            soft: false,
            length_mm: 0,
        }
    }

    /// A liquid that takes up `volume_ml`.
    ///
    /// Its length is zero because a liquid has none: `item::length` answers `0_mm` for a liquid before asking anything else (`src/item.cpp:7345-7347`).
    #[must_use]
    pub const fn liquid(volume_ml: i64) -> Self {
        Self {
            volume_ml,
            weight_grams: 0,
            phase: Phase::Liquid,
            id: String::new(),
            flags: std::collections::BTreeSet::new(),
            base_material: String::new(),
            ammo_types: Vec::new(),
            is_ammo: false,
            charges: 1,
            soft: false,
            length_mm: 0,
        }
    }

    /// A gas that takes up `volume_ml`.
    #[must_use]
    pub const fn gas(volume_ml: i64) -> Self {
        Self {
            volume_ml,
            weight_grams: 0,
            phase: Phase::Gas,
            id: String::new(),
            flags: std::collections::BTreeSet::new(),
            base_material: String::new(),
            ammo_types: Vec::new(),
            is_ammo: false,
            charges: 1,
            soft: false,
            length_mm: 0,
        }
    }
}

/// Marks an item as being inside a container.
#[derive(Debug, Clone, Copy, Component)]
pub struct Inside(pub Entity);

/// Which of the container's pockets took the item.
///
/// The reference keeps the payload inside the pocket object itself; the port names the pocket by its index, so the room used in one pocket does not pay for another.
#[derive(Debug, Clone, Copy, Component)]
pub struct InPocket(pub usize);

/// Why something could not be put in a container.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InsertError {
    /// The container has no room for it.
    TooBig {
        /// Room it had left, in millilitres.
        remaining_ml: i64,
        /// What it was asked to take, in millilitres.
        wanted_ml: i64,
    },
    /// The item has already been put somewhere else.
    AlreadyInside,
    /// The pocket's accept-lists do not name it.
    ///
    /// Reference: `"holster does not accept this item type or form factor"` (`src/item_pocket.cpp:1362-1372`), which is the message every restricted pocket gives.
    NotAccepted,
    /// The pocket holds one thing already, and it is a holster.
    ///
    /// Reference: `"holster already contains an item"` (`src/item_pocket.cpp:1548-1560`), which only ammunition may overrule since ammunition stacks.
    HolsterOccupied,
    /// The pocket has the room in total but not left.
    ///
    /// Reference: `"not enough space"` (`src/item_pocket.cpp:1595-1598`), distinct from `"item is too big"`, which is a thing bigger than the pocket **entirely**
    /// (`src/item_pocket.cpp:1583-1586`).
    NotEnoughSpace {
        /// Room the pocket had left, in millilitres.
        remaining_ml: i64,
        /// What it was asked to take, in millilitres.
        wanted_ml: i64,
    },
    /// Every pocket refused, and the reference names them all: `pockets unacceptable because a, b, or c`.
    ///
    /// Reference: `item_contents_helper::find_pocket_for` (`src/item_contents.cpp:802-822`), which sorts the messages, removes the duplicates and joins them with
    /// `or`. The order is the reference's `localized_compare`, which for these messages is alphabetical.
    AnyOf {
        /// The distinct messages, sorted.
        messages: Vec<String>,
    },
    /// The pocket takes only ammunition and the thing is not ammunition.
    ///
    /// Reference: `"item is not ammunition"` (`src/item_pocket.cpp:1377-1381`).
    NotAmmo,
    /// The pocket takes only certain ammunition and the thing is another kind.
    ///
    /// Reference: `"item is not the correct ammo type"` (`src/item_pocket.cpp:1385-1388`).
    WrongAmmo,
    /// The thing can never be put anywhere, because it cannot be unwielded.
    ///
    /// Reference: `"cannot unwield item"` (`src/item_pocket.cpp:1359-1362`).
    CannotUnwield,
    /// The thing is bigger than the largest item the pocket takes.
    ///
    /// Reference: `"item is too big"` (`src/item_pocket.cpp:1411-1418`), asked of the **per-charge** volume and skipped for liquids, gases and frozen liquids.
    TooBigItem {
        /// The largest item volume the pocket takes, in millilitres.
        limit_ml: i64,
        /// The thing's own volume per charge, in millilitres.
        wanted_ml: i64,
    },
    /// The item is heavier than the pocket will take.
    TooHeavy {
        /// The heaviest item the pocket takes, in grams.
        capacity_grams: i64,
        /// What it was asked to take, in grams.
        wanted_grams: i64,
    },
    /// The container cannot hold this phase at all - a liquid in a pocket that is not watertight, or a gas in one that is not airtight.
    WrongPhase {
        /// The phase that was refused.
        phase: Phase,
    },
    /// The item is longer than the pocket takes.
    ///
    /// Reference: `"item is too long"` (`src/item_pocket.cpp:1418-1421`).
    TooLong {
        /// The longest item the pocket takes, in millimetres.
        limit_mm: i64,
        /// The item's own longest side.
        wanted_mm: i64,
    },
    /// The item is shorter than the pocket takes. The reference refuses these too: a pocket can be sized for a magazine and nothing smaller.
    ///
    /// Reference: `"item is too short"` (`src/item_pocket.cpp:1423-1426`).
    TooShort {
        /// The shortest item the pocket takes, in millimetres.
        limit_mm: i64,
        /// The item's own longest side.
        wanted_mm: i64,
    },
    /// The item is smaller than the pocket takes.
    ///
    /// Reference: `"item is too small"` (`src/item_pocket.cpp:1428-1431`).
    TooSmall {
        /// The smallest item the pocket takes, in millilitres.
        limit_ml: i64,
        /// The item's own volume.
        wanted_ml: i64,
    },
}

impl InsertError {
    /// The message the reference gives this refusal, character for character, so that a gate can compare the two as text.
    ///
    /// Every message here is transcribed from `src/item_pocket.cpp`; a refusal the reference does not name (the port's own bookkeeping) answers an empty string.
    #[must_use]
    pub fn message(&self) -> String {
        match self {
            Self::TooBig { .. } | Self::TooBigItem { .. } => "item is too big".to_owned(),
            Self::NotEnoughSpace { .. } => "not enough space".to_owned(),
            Self::HolsterOccupied => "holster already contains an item".to_owned(),
            Self::TooHeavy { .. } => "item is too heavy".to_owned(),
            Self::WrongPhase {
                phase: Phase::Liquid,
            } => "can't contain liquid".to_owned(),
            Self::WrongPhase { .. } => "can't contain gas".to_owned(),
            Self::TooLong { .. } => "item is too long".to_owned(),
            Self::TooShort { .. } => "item is too short".to_owned(),
            Self::TooSmall { .. } => "item is too small".to_owned(),
            Self::NotAccepted => "holster does not accept this item type or form factor".to_owned(),
            Self::NotAmmo => "item is not ammunition".to_owned(),
            Self::WrongAmmo => "item is not the correct ammo type".to_owned(),
            Self::CannotUnwield => "cannot unwield item".to_owned(),
            Self::AlreadyInside => String::new(),
            // The reference's own wording for a container whose pockets all refused: `pockets unacceptable because a, b, or c`
            // (`src/item_contents.cpp:818-822`). A single pocket's refusal is returned as its own variant instead, so this wording appears only where several rules
            // were named.
            Self::AnyOf { messages } => format!("pockets unacceptable because {}", join_with_or(messages)),
        }
    }
}

/// Joins messages the way the reference enumerates them: `a`, `a or b`, `a, b, or c` (`enumeration_conjunction::or_`).
fn join_with_or(messages: &[String]) -> String {
    match messages {
        [] => String::new(),
        [only] => only.clone(),
        [first, second] => format!("{first} or {second}"),
        _ => {
            let Some((last, rest)) = messages.split_last() else {
                return String::new();
            };
            format!("{}, or {last}", rest.join(", "))
        }
    }
}

/// Whether a pocket refuses an item, and which rule refused it.
///
/// Reference: `item_pocket::is_compatible` (`src/item_pocket.cpp:1318-1433`) followed by the weight and the room of `_can_contain` (`src/item_pocket.cpp:1570-1590`),
/// **in the reference's order**, because the order decides which message a payload hears:
///
/// 1. a thing that cannot be unwielded can never go anywhere (`"cannot unwield item"`);
/// 2. the accept-lists (`"holster does not accept this item type or form factor"`);
/// 3. an ammunition pocket takes ammunition and nothing else, and only its own calibres - and then it **stops**, which is why an ammo pocket never complains about a
///    length or a weight;
/// 4. a liquid needs a watertight pocket and a gas an airtight one;
/// 5. the largest item the pocket takes, measured **per charge** and skipped for liquids, gases and frozen liquids;
/// 6. the longest side, the shortest side and the smallest volume;
/// 7. the weight, then the room.
/// # Errors
///
/// The first rule of [`check_in`] that refuses the thing.
pub fn check(pocket: &Pocket, facts: &ItemFacts, remaining_ml: i64) -> Result<(), InsertError> {
    check_in(pocket, facts, remaining_ml, false)
}

/// [`check`], for a pocket that may already hold something.
///
/// # Errors
///
/// The first rule that refuses the thing, in the reference's order.
pub fn check_in(
    pocket: &Pocket,
    facts: &ItemFacts,
    remaining_ml: i64,
    occupied: bool,
) -> Result<(), InsertError> {
    if facts.flags.contains("NO_UNWIELD") {
        return Err(InsertError::CannotUnwield);
    }
    if !accepts(pocket, &facts.id, &facts.flags, &facts.base_material) {
        return Err(InsertError::NotAccepted);
    }
    if !pocket.ammo.is_empty() {
        if !facts.is_ammo {
            return Err(InsertError::NotAmmo);
        }
        if !pocket
            .ammo
            .iter()
            .any(|allowed| facts.ammo_types.iter().any(|mine| mine == allowed))
        {
            return Err(InsertError::WrongAmmo);
        }
        // An ammunition pocket answers here, and the weight, the room and the limits are never asked of it.
        return Ok(());
    }
    let frozen = facts.flags.contains("FROM_FROZEN_LIQUID");
    if facts.phase == Phase::Liquid && !pocket.watertight && !frozen {
        return Err(InsertError::WrongPhase {
            phase: Phase::Liquid,
        });
    }
    if facts.phase == Phase::Gas && !pocket.airtight {
        return Err(InsertError::WrongPhase { phase: Phase::Gas });
    }
    // **A soft thing is never too big**, because the reference's `charges_per_volume_recursive` answers `INFINITE_CHARGES` for a soft item with nothing inside it:
    // its loop never runs, so nothing is smaller than the number it returns (`src/item_pocket.cpp:1304-1318`). That is why a 500 ml pair of stockings goes into a
    // 17 ml bottle's pocket and a 500 ml rock does not.
    if let Some(limit) = pocket.max_item_volume_ml
        && facts.phase != Phase::Liquid
        && facts.phase != Phase::Gas
        && !frozen
        && !facts.soft
    {
        let per_charge = facts.volume_ml / i64::from(facts.charges.max(1));
        if per_charge > limit {
            return Err(InsertError::TooBigItem {
                limit_ml: limit,
                wanted_ml: per_charge,
            });
        }
    }
    if let Some(refusal) = compatibility_refusal(pocket, facts) {
        return Err(refusal);
    }
    if pocket.holster && occupied {
        return Err(InsertError::HolsterOccupied);
    }
    if facts.weight_grams > pocket.weight_capacity_grams {
        return Err(InsertError::TooHeavy {
            capacity_grams: pocket.weight_capacity_grams,
            wanted_grams: facts.weight_grams,
        });
    }
    if facts.volume_ml > pocket.capacity_ml {
        return Err(InsertError::TooBig {
            remaining_ml: pocket.capacity_ml,
            wanted_ml: facts.volume_ml,
        });
    }
    if facts.volume_ml > remaining_ml {
        return Err(InsertError::NotEnoughSpace {
            remaining_ml,
            wanted_ml: facts.volume_ml,
        });
    }
    Ok(())
}

/// Whether a pocket's own limits refuse an item, and which one did.
///
/// Reference: `item_pocket::is_compatible` (`src/item_pocket.cpp:1408-1432`). The three questions are a maximum length, a minimum length and a minimum volume, in that
/// order - which is the order the reference asks them in, and the reason `"item is too long"` wins over `"item is too small"` for an item that is both.
///
/// **The length is always asked, and zero is a real answer**: a soft thing with nothing in it has no length, and the reference refuses it into an exterior pocket as
/// `"item is too short"` exactly as it refuses a short blade. That is why the limits are compared unconditionally.
fn compatibility_refusal(pocket: &Pocket, facts: &ItemFacts) -> Option<InsertError> {
    if pocket.max_item_length_mm > 0 && facts.length_mm > pocket.max_item_length_mm {
        return Some(InsertError::TooLong {
            limit_mm: pocket.max_item_length_mm,
            wanted_mm: facts.length_mm,
        });
    }
    if pocket.min_item_length_mm > 0 && facts.length_mm < pocket.min_item_length_mm {
        return Some(InsertError::TooShort {
            limit_mm: pocket.min_item_length_mm,
            wanted_mm: facts.length_mm,
        });
    }
    if pocket.min_item_volume_ml > 0 && facts.volume_ml < pocket.min_item_volume_ml {
        return Some(InsertError::TooSmall {
            limit_ml: pocket.min_item_volume_ml,
            wanted_ml: facts.volume_ml,
        });
    }
    None
}

/// Puts an item into a container, if there is room.
///
/// The volume of what is already inside is asked of `volume_of`, because an item's volume belongs to
/// its prototype and the sim does not own prototypes.
///
/// # Errors
///
/// Returns [`InsertError::TooBig`] when the contents would exceed the capacity, and
/// [`InsertError::AlreadyInside`] when the item is already in a container.
pub fn insert_into(
    world: &mut World,
    container: Entity,
    item: Entity,
    facts_of: impl Fn(Entity) -> ItemFacts,
) -> Result<(), InsertError> {
    if world.get::<Inside>(item).is_some() {
        return Err(InsertError::AlreadyInside);
    }
    let Some(capacity) = world.get::<Container>(container).cloned() else {
        return Err(InsertError::TooBig {
            remaining_ml: 0,
            wanted_ml: facts_of(item).volume_ml,
        });
    };
    let wanted_facts = facts_of(item);
    // **What each pocket has left**, and whether it will take this phase at all. The reference's `insert_item` walks the pockets in order and stops at the first that
    // accepts, which is why a big payload cannot be paid for out of two small pockets.
    let mut room: Vec<i64> = capacity
        .pockets
        .iter()
        .map(|pocket| pocket.capacity_ml)
        .collect();
    let occupants: Vec<(Entity, usize)> = world
        .query::<(Entity, &Inside, &InPocket)>()
        .iter(world)
        .filter(|(_, inside, _)| inside.0 == container)
        .map(|(entity, _, InPocket(index))| (entity, *index))
        .collect();
    let mut occupants_by_pocket: BTreeMap<usize, usize> = BTreeMap::new();
    for (entity, index) in occupants {
        if let Some(slot) = room.get_mut(index) {
            *slot -= facts_of(entity).volume_ml;
        }
        *occupants_by_pocket.entry(index).or_default() += 1;
    }
    let mut best_room: Option<i64> = None;
    let mut chosen: Option<usize> = None;
    let mut refusals: Vec<InsertError> = Vec::new();
    for (index, pocket) in capacity.pockets.iter().enumerate() {
        let left = room.get(index).copied().unwrap_or(0);
        best_room = Some(best_room.map_or(left, |best| best.max(left)));
        let occupied = occupants_by_pocket.get(&index).copied().unwrap_or(0) > 0;
        match check_in(pocket, &wanted_facts, left, occupied) {
            Ok(()) => {
                if chosen.is_none() {
                    chosen = Some(index);
                }
            }
            Err(refusal) => refusals.push(refusal),
        }
    }
    let Some(best_room) = best_room else {
        // No pocket at all, which is not a refusal but an empty container.
        return Err(InsertError::TooBig {
            remaining_ml: 0,
            wanted_ml: wanted_facts.volume_ml,
        });
    };
    let Some(index) = chosen else {
        // **Every pocket refused, and the reference names them all.** `find_pocket_for` collects the messages of the pockets it tried, sorts them, removes the
        // duplicates and joins them with `or` (`src/item_contents.cpp:802-822`), so a backpack answers "holster already contains an item or item is too long"
        // rather than whichever pocket happened to come first.
        let mut messages: Vec<String> = refusals.iter().map(InsertError::message).collect();
        messages.sort_unstable();
        messages.dedup();
        if messages.len() == 1 {
            return Err(refusals.first().cloned().unwrap_or(InsertError::TooBig {
                remaining_ml: best_room,
                wanted_ml: wanted_facts.volume_ml,
            }));
        }
        return Err(InsertError::AnyOf { messages });
    };
    world.entity_mut(item).insert((Inside(container), InPocket(index)));
    Ok(())
}

/// Everything inside a container, in insertion order.
#[must_use]
pub fn contents(world: &mut World, container: Entity) -> Vec<Entity> {
    let mut found = Vec::new();
    for (entity, inside) in world.query::<(Entity, &Inside)>().iter(world) {
        if inside.0 == container {
            found.push(entity);
        }
    }
    found.sort_by_key(|entity| entity.index());
    found
}

/// Takes an item out of whatever holds it.
pub fn take_out(world: &mut World, item: Entity) {
    world.entity_mut(item).remove::<Inside>();
}

/// What an item weighs, contents included.
///
/// Reference: `item::weight` (`src/item.cpp:7229-7300`). The base weight comes from the prototype, an
/// item counted in charges weighs that much per charge, and **what is inside a container counts** -
/// that is why the reference takes `include_contents` and why a full box is heavier than an empty one.
/// The recursion is bounded by the tree of contents, which cannot be cyclic because an item can only be
/// in one place at once.
#[must_use]
pub fn item_weight(
    world: &mut World,
    entity: Entity,
    facts: impl Fn(ItemTypeId) -> TypeWeight,
) -> i64 {
    weight_with(world, entity, &facts)
}

/// What a type says about its own weight.
///
/// Two facts, because the reference asks two questions of a type before it weighs anything: what a unit weighs, and whether the type is **not really there** -
/// `item::weight` returns zero for a `NO_DROP` item before it looks at charges or contents (`src/item.cpp:7235-7239`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TypeWeight {
    /// What one charge of the type weighs, in **milligrams**.
    ///
    /// Milligrams, not grams, because grams are not enough: twenty-four charge-counted types weigh a fraction of a gram a charge (`aluminum_foil` is 135 mg), and a
    /// pile of two hundred of them weighs 27 grams. A port that rounded the unit weight to grams first got zero.
    pub milligrams: i64,
    /// What one charge weighs when the caller asks for the **integral** weight, in grams.
    ///
    /// Reference: `itype::integral_weight` (`src/itype.h:1327`), which `item::weight( integral: true )` reads instead of `weight` (`src/item.cpp:7256-7261`). The loader
    /// copies `weight` into it when the data does not say (`src/item_factory.cpp:398-400`), and the data may say **zero** on purpose - a gunmod's weight does not count
    /// toward the gun's integral weight, and three gunmods say `"integral_weight": "0 g"`.
    pub integral_milligrams: i64,
    /// Whether the type is counted in charges, which is what makes the charge count multiply the weight.
    ///
    /// **Not every item with charges is counted in charges.** A torch carries twenty charges and weighs what it weighs; a battery carries a hundred and weighs a hundred
    /// times one. The reference multiplies inside `if( count_by_charges() )` (`src/item.cpp:7273-7275`) and nowhere else, so the flag is part of the rule.
    pub count_by_charges: bool,
    /// Whether the type carries `NO_DROP`, which makes it weigh nothing at all.
    pub no_drop: bool,
}

impl TypeWeight {
    /// A type that weighs what it says and is really there, which is almost all of them.
    ///
    /// The integral weight is the same number, because that is what the loader does when the data does not give one.
    #[must_use]
    pub const fn plain(grams: i64) -> Self {
        Self {
            milligrams: grams.saturating_mul(1_000),
            integral_milligrams: grams.saturating_mul(1_000),
            count_by_charges: false,
            no_drop: false,
        }
    }

    /// A type counted in charges, so that a pile of `charges` weighs `charges` times one.
    #[must_use]
    pub const fn counted(grams: i64) -> Self {
        Self {
            milligrams: grams.saturating_mul(1_000),
            integral_milligrams: grams.saturating_mul(1_000),
            count_by_charges: true,
            no_drop: false,
        }
    }

    /// A type whose weight is only exact in milligrams, which twenty-four of them are.
    #[must_use]
    pub const fn from_milligrams(milligrams: i64) -> Self {
        Self {
            milligrams,
            integral_milligrams: milligrams,
            count_by_charges: false,
            no_drop: false,
        }
    }

    /// A type whose two weights differ, which two types in the data do.
    #[must_use]
    pub const fn with_integral(grams: i64, integral_grams: i64) -> Self {
        Self {
            milligrams: grams.saturating_mul(1_000),
            integral_milligrams: integral_grams.saturating_mul(1_000),
            count_by_charges: false,
            no_drop: false,
        }
    }

    /// A type the data marks `NO_DROP`.
    #[must_use]
    pub const fn hidden() -> Self {
        Self {
            milligrams: 0,
            integral_milligrams: 0,
            count_by_charges: false,
            no_drop: true,
        }
    }
}

/// What a type says about its own volume.
///
/// Reference: `itype::volume`, `itype::stack_size` and `itype::count_by_charges()` (`src/itype.h`), which `item::volume` (`src/item.cpp:7511-7590`) reads. Two facts
/// rather than one because the charge rule needs both: a type counted in charges takes `volume * charges / stack_size` millilitres, and `stack_size` is where that
/// division's rounding comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TypeVolume {
    /// What one unit takes up, in millilitres.
    pub millilitres: i64,
    /// What one unit takes up when its caller asks for the **integral** volume, in millilitres.
    ///
    /// Reference: `itype::integral_volume`, read by `item::volume( integral: true )` (`src/item.cpp:7540-7544`) and loaded with the same "copy the ordinary one when the
    /// data is silent" rule as the weight (`src/item_factory.cpp:394-396`). **120 types in the data differ**, 97 of them saying zero on purpose.
    pub integral_millilitres: i64,
    /// Whether the type is counted in charges rather than as separate objects.
    pub count_by_charges: bool,
    /// How many charges share one stack's volume; zero when the type is not counted in charges.
    pub stack_size: i64,
}

impl TypeVolume {
    /// A type that is one object of a fixed size, which is almost all of them.
    #[must_use]
    pub const fn plain(millilitres: i64) -> Self {
        Self {
            millilitres,
            integral_millilitres: millilitres,
            count_by_charges: false,
            stack_size: 0,
        }
    }

    /// A type whose two volumes differ, which 120 of them do.
    #[must_use]
    pub const fn with_integral_volume(millilitres: i64, integral_millilitres: i64) -> Self {
        Self {
            millilitres,
            integral_millilitres,
            count_by_charges: false,
            stack_size: 0,
        }
    }

    /// A charge-counted type whose two volumes differ.
    #[must_use]
    pub const fn charges_with_integral_volume(
        millilitres: i64,
        integral_millilitres: i64,
        stack_size: i64,
    ) -> Self {
        Self {
            millilitres,
            integral_millilitres,
            count_by_charges: true,
            stack_size,
        }
    }

    /// A type counted in charges, whose volume is shared between `stack_size` of them.
    #[must_use]
    pub const fn charges(millilitres: i64, stack_size: i64) -> Self {
        Self {
            millilitres,
            integral_millilitres: millilitres,
            count_by_charges: true,
            stack_size,
        }
    }
}

/// What an item takes up, contents included.
///
/// Reference: `item::volume` (`src/item.cpp:7511-7590`). The parts the port has the data for:
///
/// 1. a charge-counted item's volume is `volume * charges / stack_size`, and the division **rounds up a millilitre**;
/// 2. anything else is its own volume, whatever its charge count;
/// 3. a container is its own volume plus what is inside it - the reference's `contents.item_size_modifier()`.
///
/// Not ported and declared in the ledger: corpses, crafted items, the per-item `volume` override, gun mods and `collapsed_volume_delta`.
pub fn item_volume(
    world: &mut World,
    entity: Entity,
    facts: impl Fn(ItemTypeId) -> TypeVolume,
) -> i64 {
    volume_with(world, entity, &facts, false)
}

/// A set of item entities a person or a place can be asked about, which is the port's stand-in for the reference's `inventory`.
///
/// Reference: `class inventory` (`src/inventory.h:240-300`). The reference keeps an `item_location` per item so that a use can reach the item in a container, in a
/// vehicle part, or on the floor; the port keeps the entity, because an entity *is* reachable — that is the whole reason the world is built this way.
#[derive(Debug, Clone, Default, Component)]
pub struct Inventory {
    /// The items it holds, in the order they were added.
    pub items: Vec<Entity>,
}

impl Inventory {
    /// An empty inventory.
    #[must_use]
    pub const fn new() -> Self {
        Self { items: Vec::new() }
    }

    /// Takes an item in.
    pub fn add(&mut self, item: Entity) {
        self.items.push(item);
    }
}

impl Inventory {
    /// Whether it holds at least `quantity` items of a kind.
    ///
    /// Reference: `visitable::has_amount` (`src/visitable.h`), which is `amount_of( what, true, quantity ) == quantity` — the `limit` is the quantity asked for, so the
    /// walk stops as soon as the answer is yes.
    pub fn has_amount(&self, world: &mut World, kind: ItemTypeId, quantity: i32) -> bool {
        amount_of(world, self, Some(kind), true, quantity) >= quantity
    }

    /// Whether it holds at least `quantity` charges of a kind.
    ///
    /// Reference: `visitable::has_charges` (`src/visitable.h`).
    pub fn has_charges(&self, world: &mut World, kind: ItemTypeId, quantity: i32) -> bool {
        charges_of(world, self, kind, quantity) >= quantity
    }

    /// Takes charges of a kind, in visit order.
    ///
    /// Reference: `inventory::use_charges` over `use_charges_internal` (`src/visitable.cpp:819-875`), whose walk is **pre-order**: an item before the things inside it.
    pub fn use_charges(
        &self,
        world: &mut World,
        kind: ItemTypeId,
        quantity: i32,
        accept: impl FnMut(&GroundItem) -> bool,
    ) -> Consumed {
        charges_from_items(world, self.items.clone(), kind, quantity, accept)
    }

    /// Takes whole items, contents first and bottom up.
    ///
    /// Reference: `inventory::use_amount` over `use_amount_internal`, which is the same rule as [`consume_items_by_amount`] and is **not** the same rule as
    /// [`Inventory::use_charges`]: a match consumes the item, not a charge from it.
    pub fn use_amount(
        &self,
        world: &mut World,
        kind: ItemTypeId,
        quantity: i32,
        accept: impl FnMut(&GroundItem) -> bool,
    ) -> Consumed {
        take_amount_from(world, self.items.clone(), kind, quantity, accept)
    }
}

/// Takes charges from an explicit list of items, in pre-order.
pub fn charges_from_items(
    world: &mut World,
    items: Vec<Entity>,
    kind: ItemTypeId,
    quantity: i32,
    mut accept: impl FnMut(&GroundItem) -> bool,
) -> Consumed {
    let mut remaining = quantity.max(0);
    let mut taken = 0;
    for entity in items {
        if remaining == 0 {
            break;
        }
        offer_pile(world, entity, kind, &mut remaining, &mut taken, &mut accept);
    }
    Consumed {
        taken,
        satisfied: remaining == 0,
    }
}

/// How many **items** of a type an inventory holds, contents counted.
///
/// Reference: `visitable::amount_of` (`src/visitable.cpp:964-978`) — the count is of items, never of charges, so a pile of two hundred nails answers one; a broken
/// item is not counted at all; `any` counts everything; and `limit` stops the walk, which is what makes it safe to ask "are there two?" of a full inventory.
///
/// `any` is [`None`], which the reference spells `itype_id( "any" )`.
#[must_use]
pub fn amount_of(
    world: &mut World,
    inventory: &Inventory,
    kind: Option<ItemTypeId>,
    pseudo: bool,
    limit: i32,
) -> i32 {
    let mut total: i32 = 0;
    for item in inventory.items.clone() {
        total = total.saturating_add(amount_of_items(world, item, kind, pseudo, limit - total));
        if total >= limit {
            break;
        }
    }
    total.min(limit)
}

/// The same count over one item and everything inside it, because the reference's `visit_items` descends into contents.
fn amount_of_items(
    world: &mut World,
    item: Entity,
    kind: Option<ItemTypeId>,
    pseudo: bool,
    limit: i32,
) -> i32 {
    if limit <= 0 {
        return 0;
    }
    let mut total = 0;
    if let Some(ground) = world.get::<GroundItem>(item).cloned()
        && !item_is_broken(world, item)
        && kind.is_none_or(|wanted| wanted == ground.kind)
        && (pseudo || !item_has_flag(world, item, "PSEUDO"))
    {
        total += 1;
    }
    for held in contents(world, item) {
        total += amount_of_items(world, held, kind, pseudo, limit - total);
    }
    total
}

/// How many charges of a type an inventory holds, contents counted.
///
/// Reference: `visitable::charges_of` (`src/visitable.cpp:819-875`) with the UPS branches left to the player layer: a charge-counted item answers its **charges**, and
/// anything else answers what it holds — a magazine's rounds — which is `ammo_remaining_linked` in the reference. A broken item answers nothing. The walk only
/// descends into containers, which the port reads as "descend always", because only a container can have contents in the first place.
#[must_use]
pub fn charges_of(
    world: &mut World,
    inventory: &Inventory,
    kind: ItemTypeId,
    limit: i32,
) -> i32 {
    let mut total: i32 = 0;
    for item in inventory.items.clone() {
        total = total.saturating_add(charges_of_item(
            world,
            item,
            kind,
            limit - total,
        ));
        if total >= limit {
            break;
        }
    }
    total.min(limit)
}

/// The same count over one item and everything inside it.
fn charges_of_item(world: &mut World, item: Entity, kind: ItemTypeId, limit: i32) -> i32 {
    if limit <= 0 {
        return 0;
    }
    let mut total = 0;
    let ground = world.get::<GroundItem>(item).cloned();
    if let Some(ground) = ground
        && ground.kind == kind
        && !item_is_broken(world, item)
    {
        total += ground.charges.max(0);
    }
    // Only a container is walked into, the same gate the spending walk keeps (`src/visitable.cpp:853`).
    if world.get::<Container>(item).is_none() {
        return total;
    }
    for held in contents(world, item) {
        total += charges_of_item(world, held, kind, limit - total);
    }
    total
}

/// Whether an item is broken, which the reference records as the `ITEM_BROKEN` flag (`src/visitable.cpp:969`).
fn item_is_broken(world: &World, item: Entity) -> bool {
    item_has_flag(world, item, "ITEM_BROKEN")
}

/// Whether an item carries one of its own flags.
fn item_has_flag(world: &World, item: Entity, flag: &str) -> bool {
    world
        .get::<ItemFlags>(item)
        .is_some_and(|flags| flags.0.contains(flag))
}

/// What an item takes up when its caller asks for the **integral** volume.
///
/// Reference: `item::volume( integral: true )` (`src/item.cpp:7540-7544`). The number comes from the type's other volume; everything else - the charge rule, the stack
/// size, the round-up, the contents - is the same rule.
pub fn item_volume_integral(
    world: &mut World,
    entity: Entity,
    facts: impl Fn(ItemTypeId) -> TypeVolume,
) -> i64 {
    volume_with(world, entity, &facts, true)
}

fn volume_with(
    world: &mut World,
    entity: Entity,
    facts: &dyn Fn(ItemTypeId) -> TypeVolume,
    integral: bool,
) -> i64 {
    let Some(item) = world.get::<GroundItem>(entity).cloned() else {
        return 0;
    };
    // **An unfinished craft is exactly as big as the parts it holds.** Reference: `item::volume`'s craft branch (`src/item.cpp:7522-7534`) sums the components and
    // floors the answer at one millilitre, because a recipe of nothing but practices would otherwise be a zero-volume item in the world.
    if world.get::<CraftInProgress>(entity).is_some() {
        let mut total: i64 = 0;
        for held in contents(world, entity) {
            total = total.saturating_add(volume_with(world, held, facts, integral));
        }
        return total.max(1);
    }
    // **A corpse is not measured like the item it is carried as**: its volume is the creature's, quartered and dressed and gibbed and skinned
    // (`src/item.cpp:7518-7520`). Everything the corpse holds is still counted, because a corpse is a container like any other.
    if let Some(corpse) = world.get::<CorpseOf>(entity).copied() {
        let mut total = corpse_volume(corpse.monster_volume_ml, &item_flags(world, entity));
        for held in contents(world, entity) {
            total = total.saturating_add(volume_with(world, held, facts, integral));
        }
        return total;
    }
    let facts_of_type = facts(item.kind);
    let unit = if integral {
        facts_of_type.integral_millilitres
    } else {
        facts_of_type.millilitres
    };
    let mut total = if facts_of_type.count_by_charges && facts_of_type.stack_size > 0 {
        let charges = i64::from(item.charges);
        let product = unit.saturating_mul(charges);
        let whole = product / facts_of_type.stack_size;
        // **Rounded up, in millilitres.** The reference adds one millilitre when the division has a remainder, so a pile one charge past a stack takes one more
        // millilitre and not a fraction of one.
        if product % facts_of_type.stack_size != 0 {
            whole.saturating_add(1)
        } else {
            whole
        }
    } else {
        unit
    };
    for held in contents(world, entity) {
        total = total.saturating_add(volume_with(world, held, facts, integral));
    }
    total
}

/// What an item weighs when its caller asks for the **integral** weight.
///
/// Reference: `item::weight( include_contents, integral )` with the second argument true (`src/item.cpp:7256-7261`): the same rule as [`item_weight`] over the type's other
/// number. Everything that is not the base weight is unchanged - charges still multiply, contents still count, and `NO_DROP` still weighs nothing.
pub fn item_weight_integral(
    world: &mut World,
    entity: Entity,
    facts: impl Fn(ItemTypeId) -> TypeWeight,
) -> i64 {
    weight_with_mode(world, entity, &facts, true)
}

/// The recursion itself, taking the lookup as a trait object so it does not monomorphise per depth.
fn weight_with(world: &mut World, entity: Entity, facts: &dyn Fn(ItemTypeId) -> TypeWeight) -> i64 {
    weight_with_mode(world, entity, facts, false)
}
fn weight_milligrams(
    world: &mut World,
    entity: Entity,
    facts: &dyn Fn(ItemTypeId) -> TypeWeight,
    integral: bool,
) -> i64 {
    let Some(item) = world.get::<GroundItem>(entity).cloned() else {
        return 0;
    };
    let facts_of_type = facts(item.kind);
    if facts_of_type.no_drop {
        return 0;
    }
    // **An unfinished craft weighs what it holds and nothing more.** Reference: `item::weight`'s craft branch (`src/item.cpp:7240-7251`) sums the components with **no**
    // floor - unlike its volume, which is floored at a millilitre - and it runs before the `NO_DROP` check's siblings because a craft of nothing weighs nothing.
    if world.get::<CraftInProgress>(entity).is_some() {
        let mut total: i64 = 0;
        for held in contents(world, entity) {
            total = total.saturating_add(weight_milligrams(world, held, facts, integral));
        }
        return total;
    }
    // **Charges multiply only for a charge-counted type.** A torch with twenty charges weighs one torch.
    let charges = if facts_of_type.count_by_charges {
        i64::from(item.charges.max(1))
    } else {
        1
    };
    let unit = if integral {
        facts_of_type.integral_milligrams
    } else {
        facts_of_type.milligrams
    };
    let mut total = unit.saturating_mul(charges);
    for held in contents(world, entity) {
        total = total.saturating_add(weight_milligrams(world, held, facts, integral));
    }
    total
}

fn weight_with_mode(
    world: &mut World,
    entity: Entity,
    facts: &dyn Fn(ItemTypeId) -> TypeWeight,
    integral: bool,
) -> i64 {
    // **Reported in grams, counted in milligrams.** `to_gram` truncates, so a pile of 27,000 mg is 27 g here as it is in the reference - and the division happens
    // once, at the top, because a container of a container must not lose a gram at every level.
    weight_milligrams(world, entity, facts, integral) / 1_000
}

/// What burning a portion of an item leaves behind.
///
/// Reference: `map::create_burnproducts` (`src/map_field.cpp:116-143`). For each material the item is
/// made of, each of that material's burn products is spawned at
/// `floor(efficiency * burned / product_weight * portion / total_portions)`, skipping a product that is
/// the item itself - the reference notes that would recurse forever. A count of zero or less leaves
/// nothing behind.
#[must_use]
pub fn burn_products(
    portions: &[(String, f64)],
    burned_grams: i64,
    material_total: i32,
    product_weight_grams: impl Fn(&str) -> i64,
    fuel_kind: &str,
) -> Vec<(String, i32)> {
    let mut out = Vec::new();
    for (product, efficiency) in portions {
        if product == fuel_kind {
            continue;
        }
        let weight = product_weight_grams(product);
        if weight <= 0 {
            continue;
        }
        #[allow(clippy::cast_precision_loss)]
        let share = f64::from(material_total.max(1));
        #[allow(clippy::cast_precision_loss)]
        let burned = burned_grams as f64;
        #[allow(clippy::cast_precision_loss)]
        let per_item = weight as f64;
        #[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)]
        let count = (efficiency * (burned / per_item) * (1.0 / share)).floor() as i32;
        if count > 0 {
            out.push((product.clone(), count));
        }
    }
    out
}

/// What came of asking a square for some items.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Consumed {
    /// How much was taken: charges for a stack, objects otherwise.
    pub taken: i32,
    /// Whether the request was met in full.
    pub satisfied: bool,
}

/// Takes up to `quantity` of one kind of item from a square.
///
/// Reference: `map::use_amount_square` with `use_amount_stack` and `item::use_charges`
/// (`src/map.cpp:5863-5897`, `src/item.cpp:13135-13180`). Items are taken in the order they lie there -
/// which is spawn order, and therefore part of the deterministic contract - and a stack that runs out is
/// removed. The request can come up short: the caller learns both how much it got and whether that was
/// everything it asked for, because crafting needs to put back what it took when it fails.
pub fn consume_items(
    world: &mut World,
    square: (i32, i32),
    kind: ItemTypeId,
    quantity: i32,
) -> Consumed {
    consume_items_with(world, square, kind, quantity, |_item| true)
}

/// The same, but only the piles a filter accepts.
///
/// Reference: `item::use_charges` (`src/item.cpp:13135-13180`). The filter is asked **first**, before the type is compared: an item it rejects is skipped and the walk
/// carries on (`if( !filter( *e ) ) return VisitResponse::NEXT;`). The order between the piles is still spawn order, so a caller that forbids the first pile gets the
/// second one rather than nothing.
///
/// Not modelled: the reference visits an item's **contents** as well as the item (`visit_items`), so a filter here is only ever asked about the pile itself.
pub fn consume_items_with(
    world: &mut World,
    square: (i32, i32),
    kind: ItemTypeId,
    quantity: i32,
    accept: impl FnMut(&GroundItem) -> bool,
) -> Consumed {
    consume_items_held(world, square, kind, quantity, |_kind| false, true, accept)
}

/// The same, with the square's own facts: whether it holds liquid, and which kinds are liquids.
///
/// Reference: `item_stack::use_charges` (`src/item_stack.cpp:146-160`). The walk only offers an item to the per-item rule when it is **not a liquid**, or when the square
/// carries `TFLAG_LIQUIDCONT` - a puddle on a plain floor is scenery rather than a source of charges. The gate is asked **before** the filter, so a predicate never sees a
/// puddle it could not have been given.
///
/// `holds_liquids` is the square's `TFLAG_LIQUIDCONT`, which is a terrain **or** furniture flag; `is_liquid` is the item type's phase, which is content the caller has
/// and this crate does not.
///
/// # Panics
///
/// Panics if a filter asks the world for something it is not allowed to touch; the filters here are pure.
pub fn consume_items_held(
    world: &mut World,
    square: (i32, i32),
    kind: ItemTypeId,
    quantity: i32,
    is_liquid: impl Fn(ItemTypeId) -> bool,
    holds_liquids: bool,
    mut accept: impl FnMut(&GroundItem) -> bool,
) -> Consumed {
    let mut remaining = quantity.max(0);
    let mut taken = 0;
    let items = items_at(world, square);
    for entity in items {
        if remaining == 0 {
            break;
        }
        // The liquid gate is asked about the **top-level** pile only: a puddle on a plain floor is not offered at all, and neither is anything supposedly inside it
        // (`item_stack::use_charges`, `src/item_stack.cpp:150-155`), while a canteen's water is offered because a canteen is not a liquid.
        let Some(pile) = world.get::<GroundItem>(entity).cloned() else {
            continue;
        };
        if is_liquid(pile.kind) && !holds_liquids {
            continue;
        }
        offer_pile(world, entity, kind, &mut remaining, &mut taken, &mut accept);
    }
    Consumed {
        taken,
        satisfied: remaining == 0,
    }
}

/// Offers one pile, and then its contents, the way `visit_items` walks them.
///
/// Reference: `item::use_charges` (`src/item.cpp:13140-13165`). The visit is **pre-order**: an item is offered before the things inside it, and an item the filter rejects
/// is skipped while its contents are still walked (`VisitResponse::NEXT`). A container is not itself counted by charges, so it can only ever supply what it holds.
///
/// An emptied pile is removed: at the top level that means the pile leaves the square, and inside a container it leaves the container, which is the reference's `del`
/// list and its `on_contents_changed` in one step.
fn offer_pile(
    world: &mut World,
    entity: Entity,
    kind: ItemTypeId,
    remaining: &mut i32,
    taken: &mut i32,
    accept: &mut impl FnMut(&GroundItem) -> bool,
) {
    // **Only a container is walked into.** The reference ends its charge visit with `return e->is_container() ? VisitResponse::NEXT : VisitResponse::SKIP`
    // (`src/visitable.cpp:853`), so a thing that merely happens to hold something - a corpse with a knife forced into it - supplies nothing to `charges_of`, while
    // `amount_of` still counts what is inside (`src/visitable.cpp:964-978` has no such gate).
    if *remaining == 0 {
        return;
    }
    let Some(item) = world.get::<GroundItem>(entity).cloned() else {
        return;
    };
    // The filter, then the type: the reference's own order (`src/item.cpp:13145-13149`).
    if accept(&item) && item.kind == kind {
        let available = item.charges.max(0);
        let take = available.min(*remaining);
        if take > 0 {
            *remaining -= take;
            *taken += take;
            if take == available {
                // A removed pile leaves with what it holds; the reference's item is one object.
                remove_item_with_contents(world, entity);
                return;
            }
            if let Some(mut item) = world.get_mut::<GroundItem>(entity) {
                item.charges = available - take;
            }
        }
    }
    if *remaining == 0 {
        return;
    }
    // Then the contents, in the order they went in — but only into a container, which is the reference's `SKIP` for anything else.
    if world.get::<Container>(entity).is_none() {
        return;
    }
    for child in contents(world, entity) {
        offer_pile(world, child, kind, remaining, taken, accept);
        if *remaining == 0 {
            return;
        }
    }
}

/// What a spawning group does with an item its container will not take.
///
/// Reference: `Item_spawn_data::overflow_behaviour` (`src/item_group.h`), named by `on_overflow` in the data.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Overflow {
    /// Put it on the ground beside the container.
    Spill,
    /// Throw it away on purpose.
    Discard,
    /// The data was wrong: report it and throw the item away all the same.
    ///
    /// This is not the same as "leave it out", which is what the name suggests. `put_into_container` erases **every** drawn item from the group's list and puts back only
    /// what reached its `excess` pile, and a `none` item never gets there (`src/item_group.cpp:157-171`) - so the player finds nothing, exactly as with a discard.
    None,
}


/// What a line of fire needs to know about the map.
///
/// Reference: `map::clear_path` asks three things of the map - whether a square is in bounds, what it costs to move through, and whether anything supports a body at a
/// level (`src/map.cpp:7723-7800`). They are a trait rather than a map borrow so that the rule can be tested against a fixture the size of a recording.
pub trait LineOfFire {
    /// The reference's `inbounds`.
    fn in_bounds(&self, square: [i32; 3]) -> bool;
    /// The reference's `move_cost`: zero for a wall, and the terrain's own cost otherwise.
    fn move_cost(&self, square: [i32; 3]) -> i32;
    /// The reference's `has_floor_or_support`: whether something can stand on this square at this level.
    fn has_floor_or_support(&self, square: [i32; 3]) -> bool;
}

/// The reference's `fov_3d_z_range`: how many levels apart two squares may be and still be in sight of each other.
const FOV_3D_Z_RANGE: i32 = 10;

/// Whether a line of fire from `from` to `to` is clear.
///
/// Reference: `map::clear_path` (`src/map.cpp:7723-7800`). The walk skips the **last** square - a target standing on a wall is still a legal shot - and it does two
/// different things depending on whether a step climbs:
///
/// * a step on one level is allowed when the square's move cost falls inside `cost_min..=cost_max`;
/// * a step that changes level is allowed when one of the two columns has support at the higher level **and** the square it would stand on costs **strictly** between
///   the bounds. The reference really does compare strictly here and inclusively there, so a window of one value can carry a flat shot and never a climb.
///
/// `range` is the reference's `range`: a negative value means "no limit", otherwise the target must be within it.
#[must_use]
pub fn clear_path(
    from: [i32; 3],
    to: [i32; 3],
    range: i32,
    cost_min: i32,
    cost_max: i32,
    facts: &impl LineOfFire,
) -> bool {
    if (from[2] - to[2]).abs() > FOV_3D_Z_RANGE {
        return false;
    }
    if (range >= 0 && range < rl_dist(from, to)) || !facts.in_bounds(to) {
        return false;
    }
    let mut last = from;
    for cell in line_to_3d(from, to, 0, 0) {
        // "Exit before checking the last square, it's still reachable even if it is an obstacle."
        if cell == to {
            return true;
        }
        if cell[2] == last[2] {
            let cost = facts.move_cost(cell);
            if cost < cost_min || cost > cost_max {
                return false;
            }
        } else {
            let max_z = cell[2].max(last[2]);
            let mut this_clear = false;
            if !facts.has_floor_or_support([cell[0], cell[1], max_z]) {
                let cost = facts.move_cost([cell[0], cell[1], last[2]]);
                if cost > cost_min && cost < cost_max {
                    this_clear = true;
                }
            }
            if !this_clear && facts.has_floor_or_support([last[0], last[1], max_z]) {
                let cost = facts.move_cost([last[0], last[1], cell[2]]);
                if cost > cost_min && cost < cost_max {
                    this_clear = true;
                }
            }
            if !this_clear {
                return false;
            }
        }
        last = cell;
    }
    true
}

/// The reference's `rl_dist` with the default options: the Chebyshev distance, the longest axis.
fn rl_dist(from: [i32; 3], to: [i32; 3]) -> i32 {
    (to[0] - from[0])
        .abs()
        .max((to[1] - from[1]).abs())
        .max((to[2] - from[2]).abs())
}

/// How a group wants its items put into the container it names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Fill {
    /// What to do with an item the container will not take.
    pub overflow: Overflow,
    /// Whether the container is a **corpse**.
    ///
    /// Reference: `put_into_container` (`src/item_group.cpp:137-145`). When an item does not fit, the reference asks whether the *container* is a corpse and, if so,
    /// forces the item in whatever the pockets say (`force_insert_item`). Corpses are the one container the data cannot size, so the reference stops asking rather than
    /// spilling a dead man's pockets onto the pavement.
    pub container_is_corpse: bool,
}

impl Default for Fill {
    fn default() -> Self {
        Self {
            overflow: Overflow::Spill,
            container_is_corpse: false,
        }
    }
}

/// What became of a group's items.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Filled {
    /// How many went into the container.
    pub inside: usize,
    /// How many did not fit.
    pub left_over: usize,
}

/// Puts a run of freshly spawned items into a container, spilling or losing what does not fit.
///
/// Reference: `put_into_container` (`src/item_group.cpp:115-171`). Each item is offered to the container in turn; one that does not fit is **spilled** (left on the
/// ground), **discarded**, or lost with a diagnostic, according to the group's `on_overflow`.
///
/// The run is **shuffled first** (`std::shuffle( items.end() - num_items, items.end(), rng_get_engine() )`, `src/item_group.cpp:127-129`), so that it is not always the
/// last drawn items that are rejected. `rolls` is the game's own engine, which is what the reference shuffles with - not `rng()`, so no die is recorded for it, and a
/// replay that reaches a spawning group's container has to drive the same engine.
///
/// A corpse is not special-cased: the reference forces one into a container whatever its volume says (`ctr.is_corpse()`), and the port has no corpse container rule yet.
pub fn fill_container(
    world: &mut World,
    container: Entity,
    kinds: &[ItemTypeId],
    fill: Fill,
    facts_of: impl Fn(Entity) -> ItemFacts + Copy,
    rolls: &mut Rolls,
) -> Filled {
    let Some(position) = world.get::<Position>(container).copied() else {
        return Filled { inside: 0, left_over: kinds.len() };
    };
    let mut kinds = kinds.to_vec();
    // The shuffle comes before anything is offered, and over the whole run rather than a prefix.
    shuffle(&mut kinds, rolls);
    let mut filled = Filled { inside: 0, left_over: 0 };
    for kind in &kinds {
        let item = world
            .spawn((
                Position {
                    x: position.x,
                    y: position.y,
                    z: position.z,
                },
                GroundItem {
                    burnt: 0,
                    damage: 0,
                    kind: *kind,
                    charges: 1,
                    age: 0,
                    burn: None,
                },
            ))
            .id();
        let fits = insert_into(world, container, item, facts_of).is_ok();
        if !fits && fill.container_is_corpse {
            // A corpse takes it anyway (`ctr.force_insert_item`, `src/item_group.cpp:141-144`).
            // The item is freshly spawned, so it cannot already be inside anything.
            if force_into(world, container, item).is_ok() {
                filled.inside += 1;
            }
            continue;
        }
        if fits {
            filled.inside += 1;
            continue;
        }
        filled.left_over += 1;
        if fill.overflow != Overflow::Spill {
            // `discard` and `none` both end with the item out of the group's list and out of the world.
            remove_item_with_contents(world, item);
        }
    }
    filled
}

/// Puts an item inside a container without asking whether it fits.
///
/// Reference: `item::force_insert_item` (`src/item.cpp`), which the corpse branch of `put_into_container` calls. The one thing it still refuses is an item that is
/// already inside something, because there is nowhere else to record the second parent.
///
/// # Errors
///
/// [`InsertError::AlreadyInside`] if the item already has a parent.
pub fn force_into(world: &mut World, container: Entity, item: Entity) -> Result<(), InsertError> {
    if world.get::<Inside>(item).is_some() {
        return Err(InsertError::AlreadyInside);
    }
    world.entity_mut(item).insert((Inside(container), InPocket(0)));
    Ok(())
}

/// Removes an item **and everything inside it**.
///
/// An item in the reference is one object: taking the container takes what it holds, and the contents are not left behind as things on the square. The port's contents
/// are entities of their own, so removing a parent has to walk its children first - otherwise they stay in the world with an `Inside` pointing at something that no
/// longer exists, and every query that ignores `Inside` starts seeing them as loose items on the floor.
pub fn remove_item_with_contents(world: &mut World, entity: Entity) {
    for child in contents(world, entity) {
        remove_item_with_contents(world, child);
    }
    world.despawn(entity);
}

/// Takes whole **items** off a square, which is not the same question as taking charges.
///
/// Reference: `map::use_amount_square` over `use_amount_stack` and `item::use_amount` (`src/map.cpp:5889-5902, 5903-5920`, `src/item.cpp:12868-12921`). The differences
/// from [`consume_items`] are the whole point of having both:
///
/// * a match consumes the **item**, not a charge from it - `quantity--` whatever the item holds - so asking for one of a pile of twenty takes all twenty;
/// * a container's contents are visited **first**, from the bottom up, and the container itself last (`contained_list.emplace_front`), the reverse of the charges walk;
/// * the filter is asked about the item being consumed, and a rejected item is passed over.
///
/// `taken` counts items here, not charges.
pub fn consume_items_by_amount(
    world: &mut World,
    square: (i32, i32),
    kind: ItemTypeId,
    quantity: i32,
    accept: impl FnMut(&GroundItem) -> bool,
) -> Consumed {
    let items = items_at(world, square);
    take_amount_from(world, items, kind, quantity, accept)
}

/// Takes whole items from an explicit list, which is what an inventory is: [`consume_items_by_amount`] asks a square, this asks whatever a caller hands it.
pub fn take_amount_from(
    world: &mut World,
    items: Vec<Entity>,
    kind: ItemTypeId,
    quantity: i32,
    mut accept: impl FnMut(&GroundItem) -> bool,
) -> Consumed {
    let mut remaining = quantity.max(0);
    let mut taken = 0;
    for entity in items {
        if remaining == 0 {
            break;
        }
        take_amount(world, entity, kind, &mut remaining, &mut taken, &mut accept);
    }
    Consumed {
        taken,
        satisfied: remaining == 0,
    }
}

/// One pile, contents first, then the pile itself.
///
/// Reference: `item::use_amount` (`src/item.cpp:12868-12905`): the contained items are walked **before** the item, and from the bottom of the stack up, so removing one
/// does not invalidate the walk. Thinking an item matched means the whole item goes, contents and all.
fn take_amount(
    world: &mut World,
    entity: Entity,
    kind: ItemTypeId,
    remaining: &mut i32,
    taken: &mut i32,
    accept: &mut impl FnMut(&GroundItem) -> bool,
) {
    if *remaining == 0 {
        return;
    }
    // Bottom up: the reference reverses the list it got from the top down.
    let mut children = contents(world, entity);
    children.reverse();
    for child in children {
        take_amount(world, child, kind, remaining, taken, accept);
        if *remaining == 0 {
            return;
        }
    }
    let Some(item) = world.get::<GroundItem>(entity) else {
        return;
    };
    if item.kind == kind && accept(item) {
        // If the world only has this to give and the caller still wants more, the count stands: one item is one item.
        *remaining -= 1;
        *taken += 1;
        remove_item_with_contents(world, entity);
    }
}

/// Why a craft did not happen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CraftError {
    /// A requirement group could not be met; nothing was taken.
    Missing {
        /// Which group failed, counting from zero.
        group: usize,
    },
}

/// What a craft produced.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Crafted {
    /// The item that appeared on the square.
    pub result: Entity,
    /// How many of it.
    pub count: i32,
}

/// How many of one kind lie on a square.
#[must_use]
pub fn count_on_square(world: &mut World, square: (i32, i32), kind: ItemTypeId) -> i32 {
    items_at(world, square)
        .into_iter()
        .filter_map(|entity| world.get::<GroundItem>(entity).cloned())
        .filter(|item| item.kind == kind)
        .map(|item| item.charges.max(0))
        .sum()
}
/// How many moves a batch takes to craft, once the crafter's speed is known.
///
/// Reference: `recipe::batch_time` (`src/recipe.cpp:108-149`). Everything about this is fiddly and every
/// part of it is load-bearing:
///
/// * a speed multiplier of zero means "cannot craft here", and the reference reports the time as if the
///   crafter were at full speed rather than dividing by zero;
/// * a recipe that does not benefit from batching takes the whole time per unit;
/// * one that does follows a scaled logistic curve, which is why a batch of twenty is far less than twenty
///   times one - the per-unit saving deepens with the batch and flattens out at `batch_rscale`;
/// * assistants take a fixed cut, one giving three quarters and two or more giving three fifths;
/// * and no arrangement ever makes a batch faster than a single unit.
#[must_use]
pub fn batch_time_moves(
    time_moves: i64,
    batch: i32,
    batch_rscale: f64,
    batch_rsize: f64,
    multiplier: f64,
    assistants: usize,
) -> i64 {
    let multiplier = if multiplier == 0.0 { 1.0 } else { multiplier };
    #[allow(clippy::cast_precision_loss)]
    let local_time = time_moves as f64 / multiplier;
    if batch_rscale == 0.0 && assistants == 0 {
        #[allow(clippy::cast_possible_truncation)]
        return (local_time as i64) * i64::from(batch.max(1));
    }
    let mut total = 0.0;
    if batch_rscale == 0.0 {
        total = local_time * f64::from(batch.max(1));
    } else {
        let scale = batch_rsize / 6.0;
        for unit in 0..batch.max(1) {
            #[allow(clippy::cast_precision_loss)]
            let step = f64::from(unit) / scale;
            let logf = (2.0 / (1.0 + (-step).exp())) - 1.0;
            total += local_time * (1.0 - (batch_rscale * logf));
        }
    }
    match assistants {
        1 => total *= 0.75,
        count if count >= 2 => total *= 0.60,
        _ => {}
    }
    if total < local_time {
        total = local_time;
    }
    #[allow(clippy::cast_possible_truncation)]
    let total = total as i64;
    total
}

/// A craft in progress: how much work it needs and how much has been done.
///
/// Reference: `craft_activity_actor::do_turn` (`src/activity_actor.cpp:4163-4240`). The reference stores
/// progress as a percentage of the *base* batch time in a fixed-point field and scales each turn's moves by
/// `base_total / current_total`, so a slow crafter accrues less progress per turn rather than needing a
/// different denominator. The port keeps the same relationship in plain moves, which is easier to read and
/// identical in behaviour.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CraftingWork {
    /// Moves the craft needs with no speed modifier, as `recipe::batch_time` reports for full speed.
    pub base_total_moves: f64,
    /// Moves it needs at the current speed and with the current assistants.
    pub current_total_moves: f64,
    /// Moves of base work done so far.
    pub progress: f64,
}

/// How a craft turn went.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkOutcome {
    /// Still going.
    Working,
    /// It is done this turn.
    Finished,
    /// The crafter cannot work here at all, so the craft is abandoned.
    Cancelled,
}

impl CraftingWork {
    /// A craft of `base_total_moves`, starting at the same speed.
    #[must_use]
    pub fn new(base_total_moves: f64) -> Self {
        Self {
            base_total_moves: base_total_moves.max(1.0),
            current_total_moves: base_total_moves.max(1.0),
            progress: 0.0,
        }
    }

    /// How far along it is, from zero to one.
    #[must_use]
    pub fn fraction(&self) -> f64 {
        (self.progress / self.base_total_moves).clamp(0.0, 1.0)
    }
}

/// Spends one turn of work on a craft.
///
/// A speed of zero or less cancels it, which is the reference's own answer when nothing about the place
/// allows crafting. Progress never passes the total, so a slow turn at the end does not overshoot.
pub fn craft_turn(work: &mut CraftingWork, moves: i64, speed: f64) -> WorkOutcome {
    if speed <= 0.0 {
        return WorkOutcome::Cancelled;
    }
    #[allow(clippy::cast_precision_loss)]
    let spent = moves.max(0) as f64;
    let delta = spent * work.base_total_moves / work.current_total_moves.max(1.0);
    work.progress = (work.progress + delta).min(work.base_total_moves);
    if work.progress >= work.base_total_moves {
        WorkOutcome::Finished
    } else {
        WorkOutcome::Working
    }
}

/// The kind an unfinished craft carries until it becomes its result.
pub const UNFINISHED_CRAFT: ItemTypeId = ItemTypeId(u32::MAX);

/// Marks the item that holds a craft in progress, together with what it will become.
///
/// Reference: `craft_activity_actor` (`src/activity_actor.cpp:4163-4335`). The reference's in-progress craft
/// is an item: it **holds the components from the moment they are taken**, which is why cancelling a craft
/// loses nothing - the unfinished item stays where it is with its contents inside, and only an empty
/// componentless recipe item is removed outright.
#[derive(Debug, Clone, Copy, Component)]
pub struct CraftInProgress {
    /// What it will become.
    pub result_kind: ItemTypeId,
    /// How many at once.
    pub batch: i32,
    /// Where the work stands.
    pub work: CraftingWork,
}

/// How a craft ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CraftEnd {
    /// Still working.
    Working,
    /// Finished: the result is on the square and this is it.
    Made(Entity),
    /// Cancelled: the parts are still inside the unfinished craft.
    Cancelled,
}

/// Starts a craft, taking the parts into an unfinished item on the square.
///
/// # Errors
///
/// Returns [`CraftError::Missing`] when a requirement group cannot be met; nothing is taken then.
pub fn start_craft(
    world: &mut World,
    square: (i32, i32),
    components: &[Vec<(ItemTypeId, i32)>],
    batch: i32,
    result_kind: ItemTypeId,
    base_total_moves: f64,
) -> Result<Entity, CraftError> {
    // Take the parts first, exactly as `craft_on_square` does, and keep them where they came from.
    let mut parts: Vec<(ItemTypeId, i32)> = Vec::new();
    for (group, alternatives) in components.iter().enumerate() {
        let mut chosen = None;
        for (kind, count) in alternatives {
            let wanted = count.saturating_mul(batch.max(1));
            if count_on_square(world, square, *kind) >= wanted {
                chosen = Some((*kind, wanted));
                break;
            }
        }
        if let Some(part) = chosen {
            parts.push(part);
        } else {
            for (kind, count) in parts.iter().rev() {
                drop_item(world, square, *kind, *count);
            }
            return Err(CraftError::Missing { group });
        }
    }
    for (kind, wanted) in &parts {
        consume_items(world, square, *kind, *wanted);
    }
    // An unfinished craft is an item that holds its parts, so nothing can be lost by cancelling it. It
    // carries a sentinel kind until it is finished, so nothing can mistake it for the result.
    let craft = drop_item(world, square, UNFINISHED_CRAFT, 1);
    world
        .entity_mut(craft)
        .insert(Container::new(i64::MAX))
        .insert(CraftInProgress {
            result_kind,
            batch: batch.max(1),
            work: CraftingWork::new(base_total_moves),
        });
    for (kind, count) in parts {
        let part = drop_item(world, square, kind, count);
        // Capacity is unbounded, so this cannot fail.
        let _ = insert_into(world, craft, part, |_| ItemFacts::solid(0));
    }
    Ok(craft)
}

/// Spends one turn on a craft.
pub fn craft_turn_on(world: &mut World, craft: Entity, moves: i64, speed: f64) -> CraftEnd {
    let Some(mut in_progress) = world.get::<CraftInProgress>(craft).copied() else {
        return CraftEnd::Cancelled;
    };
    let outcome = craft_turn(&mut in_progress.work, moves, speed);
    if let Some(mut component) = world.get_mut::<CraftInProgress>(craft) {
        component.work = in_progress.work;
    }
    match outcome {
        WorkOutcome::Working => CraftEnd::Working,
        WorkOutcome::Cancelled => CraftEnd::Cancelled,
        WorkOutcome::Finished => {
            let square = world
                .get::<Position>(craft)
                .map_or((0, 0), |position| (position.x, position.y));
            // The parts are consumed and the unfinished item becomes the result.
            for held in contents(world, craft) {
                world.despawn(held);
            }
            world.despawn(craft);
            CraftEnd::Made(drop_item(
                world,
                square,
                in_progress.result_kind,
                in_progress.batch,
            ))
        }
    }
}

/// Cancels a craft. The unfinished item stays where it is with its parts inside it.
pub fn cancel_craft(world: &mut World, craft: Entity) {
    world.entity_mut(craft).remove::<CraftInProgress>();
}

/// Why something could not be eaten.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EatError {
    /// Nothing of that kind is on the square.
    NotThere,
    /// It is not food: the reference's comestibles are the only things with a nutrition value.
    NotFood,
    /// There is some, but not a full serving.
    NotEnoughCharges {
        /// What was there.
        available: i32,
        /// What one serving costs.
        needed: i32,
    },
}

/// What is left of a meal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Meal {
    /// Nutrition gained, in the reference's own units.
    pub nutr: i32,
    /// Thirst quenched.
    pub quench: i32,
    /// Charges spent.
    pub charges: i32,
}

/// Eats one serving of something on a square.
///
/// Reference: the comestible path of `Character::consume` and `itype::charges_to_use`
/// (`src/itype.cpp:151-159`): **one serving spends `charges_to_use` charges** - which is one for food, so
/// the rule only shows when it is not - and the nutrition gained is the type's per serving. The port keeps
/// the spending and the accounting and leaves hunger, thirst and the eating time to the systems that own
/// them.
///
/// `nutrition_of` answers with `(nutr, quench, charges_to_use)` for a kind.
///
/// # Errors
///
/// Returns [`EatError`] when there is nothing there, it is not food, or there are not enough charges for a
/// full serving. Nothing is consumed when it fails.
pub fn eat(
    world: &mut World,
    square: (i32, i32),
    kind: ItemTypeId,
    nutrition_of: impl Fn(ItemTypeId) -> (i32, i32, i32),
) -> Result<Meal, EatError> {
    let (nutr, quench, charges_to_use) = nutrition_of(kind);
    if nutr == 0 && quench == 0 {
        return Err(EatError::NotFood);
    }
    let charges_to_use = charges_to_use.max(1);
    let available = count_on_square(world, square, kind);
    if available == 0 {
        return Err(EatError::NotThere);
    }
    if available < charges_to_use {
        return Err(EatError::NotEnoughCharges {
            available,
            needed: charges_to_use,
        });
    }
    consume_items(world, square, kind, charges_to_use);
    Ok(Meal {
        nutr,
        quench,
        charges: charges_to_use,
    })
}

/// What a character has eaten and drunk, in the reference's units.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Component)]
pub struct MealTally {
    /// Nutrition taken in.
    pub nutr: i32,
    /// Thirst quenched.
    pub quench: i32,
}

impl MealTally {
    /// Adds a meal to the tally.
    pub fn eat(&mut self, meal: Meal) {
        self.nutr = self.nutr.saturating_add(meal.nutr);
        self.quench = self.quench.saturating_add(meal.quench);
    }
}

/// One square of what a player would see.
///
/// The shape mirrors `--dump-frame` in the oracle (`oracle/harness/harness.cpp`, `dump_frame`), so the two
/// can be compared square by square: the glyph the interface would draw, the terrain under it, how many
/// items lie there, and the light.
#[derive(Debug, Clone, PartialEq)]
pub struct FrameSquare {
    /// Where it is.
    pub x: i32,
    /// Where it is.
    pub y: i32,
    /// What the interface draws: an item over furniture over terrain, which is the reference's order.
    pub glyph: char,
    /// The terrain's id.
    pub terrain: u32,
    /// How many items lie there.
    pub items: i32,
    /// Ambient light at the square.
    pub light: f32,
}

/// What a square looks like before anything is drawn on it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SquareAppearance {
    /// The terrain's own glyph.
    pub glyph: char,
    /// The furniture's glyph, when something is standing there.
    pub furniture: Option<char>,
}

/// Draws the view around a centre.
///
/// This is the port's half of the frame gate: nothing here is a game rule, and everything here is a choice
/// What stepping onto a square costs, in moves.
///
/// Reference: `monmove.cpp` uses `50 * here.move_cost( p )` for the cost of leaving a square. Default
/// terrain has a move cost of 2, so an ordinary step costs 100 moves - one turn at normal speed.
#[must_use]
pub fn step_cost(terrain_move_cost: i32) -> i32 {
    50 * terrain_move_cost
}

/// A creature's moves at the start of its turn: what it had, plus its speed.
///
/// Reference: `Creature::get_speed` is what a turn grants (`monster::process_turn`, `mod_moves( get_speed() )`).
/// Saturating because a pathological speed should slow a creature down, not wrap it into negative moves.
#[must_use]
pub fn regen_moves(moves: i32, speed: i32) -> i32 {
    moves.saturating_add(speed)
}

/// Whether a creature has moves left to act.
///
/// Reference: an action is taken while moves remain (`monster::move`), so zero moves means waiting.
#[must_use]
pub fn can_act(moves: i32) -> bool {
    moves > 0
}

/// Whether a creature is next to a square, which is where it can reach to attack.
///
/// Reference: `Creature::is_adjacent` (`creature.cpp`), which asks `rl_dist( pos, target ) != 1` - so the four
/// sides and the four diagonals count, and a creature standing on the square does not.
#[must_use]
pub fn is_adjacent(from: (i32, i32), to: (i32, i32)) -> bool {
    let dx = (from.0 - to.0).abs();
    let dy = (from.1 - to.1).abs();
    dx.max(dy) == 1
}

/// One step toward a square, which is where an idle creature walks.
///
/// Reference: `monster::move` falls back to stepping at the player when there is nothing better to do; the step
/// is one square in each axis that is not already aligned.
#[must_use]
pub fn step_toward(from: (i32, i32), to: (i32, i32)) -> (i32, i32) {
    let step = |delta: i32| match delta.cmp(&0) {
        std::cmp::Ordering::Less => -1,
        std::cmp::Ordering::Equal => 0,
        std::cmp::Ordering::Greater => 1,
    };
    (from.0 + step(to.0 - from.0), from.1 + step(to.1 - from.1))
}

/// What a creature does with its turn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    /// Hit what is next to it.
    Attack,
    /// Walk one square toward the target.
    Step((i32, i32)),
    /// Nothing to do.
    Wait,
}

/// Chooses an action: attack what is next to it, otherwise close the distance, otherwise wait.
///
/// Reference: `monster::move` (`monmove.cpp`), whose first branch is a melee attack on an adjacent target and
/// whose fallback is a step toward the player. This is the ladder without the refinements - no pathfinding, no
/// fleeing, no special attacks - because those are separate rules with separate tests.
#[must_use]
pub fn choose_action(from: (i32, i32), target: (i32, i32)) -> Action {
    if from == target {
        return Action::Wait;
    }
    if is_adjacent(from, target) {
        return Action::Attack;
    }
    Action::Step(step_toward(from, target))
}

/// What one creature did with its turn.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TurnOutcome {
    /// Every action taken, in order.
    pub actions: Vec<Action>,
    /// Where the creature ended up.
    pub position: (i32, i32),
    /// Moves left over, which a faster creature can keep for later.
    pub moves: i32,
    /// How many times it hit the target.
    pub hits: i32,
}

/// Gives a creature its turn: it acts while it can pay for acting, and stops when it cannot.
///
/// Reference: `monster::move` (`monmove.cpp`) charges **the creature's own speed** for an action
/// (`mod_moves( -get_speed() )`), not a flat 100 - so a slow creature pays less per action and acts more often
/// within the same number of moves, and a fast one pays more. That is easy to get backwards, so it is written
/// down here and pinned by a test.
#[must_use]
pub fn take_turn(
    position: (i32, i32),
    target: (i32, i32),
    mut moves: i32,
    speed: i32,
) -> TurnOutcome {
    let mut actions = Vec::new();
    let mut at = position;
    let mut hits = 0;
    // A creature that cannot pay for an action does not get a free one, and speed <= 0 would loop forever.
    while speed > 0 && moves >= speed {
        let action = choose_action(at, target);
        if action == Action::Wait {
            break;
        }
        match action {
            Action::Attack => hits += 1,
            Action::Step(to) => at = to,
            Action::Wait => break,
        }
        actions.push(action);
        moves -= speed;
    }
    TurnOutcome {
        actions,
        position: at,
        moves,
        hits,
    }
}

/// A turn taken in a world that can refuse a step.
///
/// Reference: `monster::move_to` (`monmove.cpp`) returns **false and charges nothing** when the destination is
/// blocked by terrain or by another creature - the moves are only taken after the checks
/// (`mod_moves( -ceil( cost ) )` sits below them). So a monster that walks into a wall wastes nothing; it simply
/// does not move, which is why a blocked monster still has its moves next turn.
#[must_use]
pub fn take_turn_blocked(
    position: (i32, i32),
    target: (i32, i32),
    mut moves: i32,
    speed: i32,
    passable: impl Fn((i32, i32)) -> bool,
) -> TurnOutcome {
    let mut actions = Vec::new();
    let mut at = position;
    let mut hits = 0;
    while speed > 0 && moves >= speed {
        match choose_action(at, target) {
            Action::Wait => break,
            Action::Attack => {
                hits += 1;
                actions.push(Action::Attack);
                moves -= speed;
            }
            Action::Step(to) => {
                if !passable(to) {
                    // Blocked: the step never happens and costs nothing, so the turn ends here.
                    break;
                }
                at = to;
                actions.push(Action::Step(to));
                moves -= speed;
            }
        }
    }
    TurnOutcome {
        actions,
        position: at,
        moves,
        hits,
    }
}

/// Whether anything alive stands on a square.
///
/// This is the occupancy question a moving creature asks, and it is answered from the World rather than from a
/// list kept alongside it: a second copy of where things are is a copy that can disagree.
#[must_use]
pub fn occupied(world: &mut World, square: (i32, i32)) -> bool {
    let mut query = world.query::<(Entity, &Position, &crate::creature::Creature)>();
    query.iter(world).any(|(entity, position, _)| {
        (position.x, position.y) == square && crate::creature::health_of(world, entity).is_none_or(|hp| hp > 0)
    })
}

/// Rounds a creature's moves to the whole number the turn loop works in.
fn whole_moves(moves: f64) -> i32 {
    let rounded = moves.round();
    if rounded >= f64::from(i32::MAX) {
        i32::MAX
    } else if rounded <= f64::from(i32::MIN) {
        i32::MIN
    } else {
        // Inside the range above, so the cast cannot lose anything the caller would notice.
        #[allow(clippy::cast_possible_truncation)]
        let whole = rounded as i32;
        whole
    }
}

/// One creature's state when its turn starts, gathered before anything moves.
struct CreatureTurn {
    /// Which thing it is.
    entity: Entity,
    /// Where it stands.
    start: (i32, i32),
    /// Moves it has this turn.
    moves: i32,
    /// What kind it is, which decides its speed.
    kind: MonsterKindId,
}

/// Gives every creature in the world a turn, in the order the world lists them.
///
/// Reference: the reference asks each creature to act in turn and a creature may not step onto a square another
/// creature occupies (`monster::move_to`, which refuses when `creature_at( destination )` is not null). Order
/// matters and is therefore the World's own order rather than a hash order, so a replay is a replay.
///
/// `speed_of` supplies a creature's speed because speed belongs to its kind, which the content owns.
pub fn monster_turns(
    world: &mut World,
    target: (i32, i32),
    speed_of: impl Fn(MonsterKindId) -> i32,
    passable: impl Fn(u32) -> bool,
    level: &cdda_core_types::level::Level,
) {
    let mut occupied_squares: std::collections::BTreeSet<(i32, i32)> = {
        let mut query = world.query::<(Entity, &Position, &crate::creature::Creature)>();
        query
            .iter(world)
            .filter(|(entity, _, _)| {
                crate::creature::health_of(world, *entity).is_none_or(|hp| hp > 0)
            })
            .map(|(_, position, _)| (position.x, position.y))
            .collect()
    };
    let creatures: Vec<CreatureTurn> = {
        let mut query = world.query::<(Entity, &Position, &crate::creature::Creature)>();
        query
            .iter(world)
            .filter(|(entity, _, _)| {
                crate::creature::health_of(world, *entity).is_none_or(|hp| hp > 0)
            })
            .map(|(entity, position, _)| CreatureTurn {
                entity,
                start: (position.x, position.y),
                // Moves are whole in practice. The conversion is explicit about what happens at the ends
                // rather than letting a cast decide.
                moves: whole_moves(f64::from(crate::creature::moves_left(world, entity))),
                kind: world
                    .get::<crate::creature::CreatureKind>(entity)
                    .map_or(MonsterKindId(0), |kind| kind.0),
            })
            .collect()
    };
    for CreatureTurn {
        entity,
        start,
        moves,
        kind,
    } in creatures
    {
        let speed = speed_of(kind);
        // The creature's own square is passable to itself, and another creature's is not.
        let can_enter = |square: (i32, i32)| {
            if square == start {
                return true;
            }
            let terrain_ok = level.terrain_at(square.0, square.1).is_some_and(&passable);
            terrain_ok && !occupied_squares.contains(&square)
        };
        let turn = take_turn_blocked(start, target, moves, speed, can_enter);
        crate::creature::set_moves(world, entity, turn.moves);
        if turn.position != start {
            occupied_squares.remove(&start);
            occupied_squares.insert(turn.position);
            if let Some(mut position) = world.get_mut::<Position>(entity) {
                position.x = turn.position.0;
                position.y = turn.position.1;
            }
        }
    }
}

/// What a creature's blow does before armour: its dice, rolled.
///
/// Reference: `monster::melee_attack` (`monster.cpp`), which adds bash damage of
/// `dice( type->melee_dice, type->melee_sides )` when the kind has any dice at all. The roll comes from the
/// stream, so its order matters: a port that rolls at a different moment produces a different fight from the
/// same seed.
pub fn melee_blow(
    rolls: &mut cdda_core_types::rng::Rolls,
    dice: i32,
    sides: i32,
    bonus: i32,
) -> i32 {
    if dice <= 0 || sides <= 0 {
        return bonus.max(0);
    }
    let mut total = bonus;
    for _ in 0..dice {
        total += rolls.rng(1, sides);
    }
    total.max(0)
}

/// What gets through: the blow, less armour, never below nothing.
///
/// Reference: armour reduces each damage type before it lands (`Creature::deal_damage`/`absorb_hit`); a blow
/// smaller than the armour does nothing rather than healing the target.
#[must_use]
pub fn through_armour(blow: i32, armour: i32) -> i32 {
    (blow - armour).max(0)
}

/// Applies a blow to a creature, and says whether that killed it.
///
/// Reference: hit points reach zero and the creature dies (`monster::die`). The port keeps the death as a
/// property of the state rather than removing the entity here, because removal is a system's decision and a
/// caller may want the corpse's square.
pub fn strike(world: &mut World, target: Entity, blow: i32, armour: i32) -> bool {
    let dealt = through_armour(blow, armour);
    let Some(hp) = crate::creature::health_of(world, target) else {
        return false;
    };
    let left = hp.saturating_sub(dealt);
    crate::creature::set_health_left(world, target, left);
    left <= 0
}

/// The rules a creature's turn needs, gathered so the turn functions do not grow a parameter per fact.
pub struct TurnRules<'a> {
    /// How fast a kind is, which decides what an action costs it.
    pub speed_of: &'a dyn Fn(MonsterKindId) -> i32,
    /// A kind's blow: its dice, its sides and its bonus.
    pub blow_of: &'a dyn Fn(MonsterKindId) -> (i32, i32, i32),
    /// A kind's armour, which reduces what lands on it.
    pub armour_of: &'a dyn Fn(MonsterKindId) -> i32,
    /// Whether the terrain lets a creature through.
    pub passable: &'a dyn Fn(u32) -> bool,
    /// The player's accuracy in tenths, which sets the mean of its attack roll.
    ///
    /// The reference takes this from the character's melee skill and its weapon (`Character::melee_attack`), and
    /// without it a blow cannot be dodged, only absorbed - which is why it is a rule and not a constant.
    pub player_accuracy_tenths: i32,
}

/// Gives every creature a turn, and lets its attacks land on whoever is standing where it strikes.
///
/// This is where two rules meet: the turn loop decides that a creature attacks, and the damage rule decides what
/// that costs the target. Keeping the dice in the caller's hands matters - the reference rolls from one stream, so
/// a port that gave each system its own dice would produce a different fight from the same seed.
///
/// Returns how many blows landed.
pub fn monster_turns_striking(
    world: &mut World,
    target: (i32, i32),
    rules: &TurnRules<'_>,
    rolls: &mut cdda_core_types::rng::Rolls,
    level: &cdda_core_types::level::Level,
) -> i32 {
    let mut occupied_squares: std::collections::BTreeSet<(i32, i32)> = {
        let mut query = world.query::<(Entity, &Position, &crate::creature::Creature)>();
        query
            .iter(world)
            .filter(|(entity, _, _)| {
                crate::creature::health_of(world, *entity).is_none_or(|hp| hp > 0)
            })
            .map(|(_, position, _)| (position.x, position.y))
            .collect()
    };
    let creatures: Vec<CreatureTurn> = {
        let mut query = world.query::<(Entity, &Position, &crate::creature::Creature)>();
        query
            .iter(world)
            .filter(|(entity, _, _)| {
                crate::creature::health_of(world, *entity).is_none_or(|hp| hp > 0)
            })
            .map(|(entity, position, _)| CreatureTurn {
                entity,
                start: (position.x, position.y),
                moves: whole_moves(f64::from(crate::creature::moves_left(world, entity))),
                kind: world
                    .get::<crate::creature::CreatureKind>(entity)
                    .map_or(MonsterKindId(0), |kind| kind.0),
            })
            .collect()
    };
    let mut landed = 0;
    for CreatureTurn {
        entity,
        start,
        moves,
        kind,
    } in creatures
    {
        let speed = (rules.speed_of)(kind);
        let can_enter = |square: (i32, i32)| {
            if square == start {
                return true;
            }
            let terrain_ok = level
                .terrain_at(square.0, square.1)
                .is_some_and(rules.passable);
            terrain_ok && !occupied_squares.contains(&square)
        };
        let turn = take_turn_blocked(start, target, moves, speed, can_enter);
        // Every hit the turn produced lands on whoever stands on the target square.
        for _ in 0..turn.hits {
            let victim = {
                let mut monsters =
                    world.query::<(Entity, &Position, &crate::creature::Creature)>();
                let found = monsters
                    .iter(world)
                    .find(|(entity, position, _)| {
                        (position.x, position.y) == target
                            && crate::creature::health_of(world, *entity).is_none_or(|hp| hp > 0)
                    })
                    .map(|(victim, _, _)| victim);
                found.map_or_else(
                    || {
                        let mut players = world.query::<(Entity, &Position, &Player)>();
                        players
                            .iter(world)
                            .find(|(entity, position, _)| {
                                (position.x, position.y) == target
                                    && crate::creature::health_of(world, *entity)
                                        .is_none_or(|hp| hp > 0)
                            })
                            .map(|(victim, _, _)| (victim, true))
                    },
                    |entity| Some((entity, false)),
                )
            };
            let Some((victim, is_player)) = victim else {
                break;
            };
            let (dice, sides, bonus) = (rules.blow_of)(kind);
            let blow = melee_blow(rolls, dice, sides, bonus);
            if is_player {
                strike_player(world, victim, blow);
            } else {
                let armour = world
                    .get::<crate::creature::CreatureKind>(victim)
                    .map_or(0, |kind| (rules.armour_of)(kind.0));
                strike(world, victim, blow, armour);
            }
            landed += 1;
        }
        crate::creature::set_moves(world, entity, turn.moves);
        if turn.position != start {
            occupied_squares.remove(&start);
            occupied_squares.insert(turn.position);
            if let Some(mut position) = world.get_mut::<Position>(entity) {
                position.x = turn.position.0;
                position.y = turn.position.1;
            }
        }
    }
    landed
}

/// The player, as a thing in the world rather than a pair of coordinates.
///
/// Reference: `avatar` (`src/avatar.h`) is a `Character`, which is a `Creature`: it has hit points and armour and
/// can be struck like anything else. The port gives it the two facts the striking rules need now, and adds the
/// rest as rules arrive that ask for them.
#[derive(Debug, Clone, Copy, Component)]
pub struct Player;

/// Puts the player in the world.
///
/// The avatar is a [`crate::creature::Creature`] like any other - same components, same systems - and [`Player`]
/// is only the marker that says which one the player is looking through. That is the reference's own shape too:
/// `avatar` is a `Character`, and a `Character` is a `Creature`.
pub fn spawn_player(world: &mut World, square: (i32, i32), hp_max: i32, armour: i32) -> Entity {
    let entity = world
        .spawn((
            Position {
                x: square.0,
                y: square.1,
                z: 0,
            },
            crate::creature::Creature {
                name: "you".to_owned(),
                size: crate::CreatureSize::Medium,
            },
            crate::creature::Moves::default(),
            crate::creature::Speed::default(),
            crate::creature::Health {
                hp: hp_max,
                max_hp: hp_max,
            },
            crate::creature::Armour {
                bash: armour,
                cut: armour,
            },
            Player,
        ))
        .id();
    crate::singletons::index_mut(world).insert(entity, (square.0, square.1, 0));
    entity
}

/// Whether the player is dead, which is the moment the reference ends the game.
#[must_use]
pub fn player_dead(world: &mut World) -> bool {
    let mut query = world.query::<(Entity, &Player)>();
    query
        .iter(world)
        .any(|(entity, _)| crate::creature::health_of(world, entity).is_none_or(|hp| hp <= 0))
}

/// Applies a blow to the player.
pub fn strike_player(world: &mut World, target: Entity, blow: i32) -> bool {
    let Some(hp) = crate::creature::health_of(world, target) else {
        return false;
    };
    let armour = world.get::<crate::creature::Armour>(target).map_or(0, |worn| worn.bash);
    let dealt = through_armour(blow, armour);
    let left = hp.saturating_sub(dealt);
    crate::creature::set_health_left(world, target, left);
    left <= 0
}

/// Gives the player its turn, by the same ladder a monster uses: strike what is next to it, otherwise step
/// toward the target.
///
/// The player's blow is a parameter rather than a lookup, because the player has no kind to look one up from -
/// what it is holding decides that, and equipment is a later rule.
///
/// Returns how many blows landed.
pub fn player_turn(
    world: &mut World,
    target: (i32, i32),
    blow: (i32, i32, i32),
    speed: i32,
    rules: &TurnRules<'_>,
    rolls: &mut cdda_core_types::rng::Rolls,
    level: &cdda_core_types::level::Level,
) -> i32 {
    let Some((entity, start, moves)) = ({
        let mut query = world.query::<(Entity, &Position, &Player)>();
        query.iter(world).find_map(|(entity, position, _)| {
            crate::creature::health_of(world, entity)
                .is_none_or(|hp| hp > 0)
                .then_some((
                    entity,
                    (position.x, position.y),
                    crate::creature::moves_left(world, entity),
                ))
        })
    }) else {
        return 0;
    };
    // Occupancy is read once, because the turn loop asks its question many times and may not borrow the World.
    let mut taken: std::collections::BTreeSet<(i32, i32)> = {
        let mut query = world.query::<(Entity, &Position, &crate::creature::Creature)>();
        query
            .iter(world)
            .filter(|(entity, _, _)| {
                crate::creature::health_of(world, *entity).is_none_or(|hp| hp > 0)
            })
            .map(|(_, position, _)| (position.x, position.y))
            .collect()
    };
    let mut players: std::collections::BTreeSet<(i32, i32)> = {
        let mut query = world.query::<(Entity, &Position, &Player)>();
        query
            .iter(world)
            .filter(|(entity, _, _)| {
                crate::creature::health_of(world, *entity).is_none_or(|hp| hp > 0)
            })
            .map(|(_, position, _)| (position.x, position.y))
            .collect()
    };
    taken.append(&mut players);
    let can_enter = |square: (i32, i32)| {
        if square == start {
            return true;
        }
        level
            .terrain_at(square.0, square.1)
            .is_some_and(rules.passable)
            && !taken.contains(&square)
    };
    let turn = take_turn_blocked(start, target, moves, speed, can_enter);
    let mut landed = 0;
    for _ in 0..turn.hits {
        let victim = {
            let mut query = world.query::<(Entity, &Position, &crate::creature::Creature)>();
            query
                .iter(world)
                .find(|(entity, position, _)| {
                    (position.x, position.y) == target
                        && crate::creature::health_of(world, *entity).is_none_or(|hp| hp > 0)
                })
                .map(|(victim, _, _)| victim)
        };
        let Some(victim) = victim else {
            break;
        };
        // The player's blow goes through exactly the rules a monster's does: a roll against the target's dodge
        // and size, and then the armour. A second path here would have been a second set of numbers.
        let (dice, sides, bonus) = blow;
        let attack = crate::melee::Attack {
            accuracy_tenths: rules.player_accuracy_tenths,
            fixed: crate::melee::Damage {
                bash: bonus,
                cut: 0,
                penetration: 0,
            },
            dice,
            sides,
        };
        let swung = crate::melee::strike(
            rolls,
            attack,
            crate::creature::defence_of(world, victim),
        );
        if swung.hit {
            crate::creature::hurt(world, victim, swung.damage);
        }
        landed += 1;
    }
    crate::creature::set_moves(world, entity, turn.moves);
    if turn.position != start
        && let Some(mut position) = world.get_mut::<Position>(entity)
    {
        position.x = turn.position.0;
        position.y = turn.position.1;
    }
    landed
}

/// Marks an item as the body of a particular kind of creature.
///
/// Reference: `item::make_corpse` (`src/item.cpp`) does not make a new kind of item - it makes an item of the
/// `corpse` type and sets its `corpse` field to the monster's id, which is how butchering later knows what it is
/// looking at. The port attaches the same fact as a component on the corpse item, so the item keeps being an item
/// and gains the monster it used to be.
#[derive(Debug, Clone, Copy, Component, PartialEq, Eq)]
pub struct CorpseOf {
    /// What kind of creature this was.
    pub kind: MonsterKindId,
    /// What that creature took up while it was alive, in millilitres, which is what its corpse is measured from (`item::corpse_volume`).
    pub monster_volume_ml: i64,
}

/// The flags an item carries of its own, which is what a corpse's butchering state is: the prototype's flags are the type's, and these are this one's.
#[derive(Debug, Clone, Default, Component)]
pub struct ItemFlags(pub std::collections::BTreeSet<String>);

/// The flags an item carries, if it carries any.
fn item_flags(world: &World, entity: Entity) -> std::collections::BTreeSet<String> {
    world
        .get::<ItemFlags>(entity)
        .map(|flags| flags.0.clone())
        .unwrap_or_default()
}

/// How damaged one item is, and how much of that damage cannot be repaired.
///
/// Reference: `item::damage_` and `item::degradation_` (`src/item.h`). Two numbers, and the invariant between them is the whole point: **damage is never below
/// degradation**, because degradation is the damage that repairs cannot undo (`item::set_damage`, `src/item.cpp:967-970`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Component)]
pub struct ItemDamage {
    /// How damaged it is, from zero to the type's maximum.
    pub damage: i32,
    /// How much of that damage can never be repaired.
    pub degradation: i32,
}

/// What a type says about damage, passed in because it is content and this crate is not.
#[derive(Debug, Clone, Copy)]
pub struct DamageLimits {
    /// `ItemType::damage_max`: 4,000 for anything not counted in charges, 0 for anything that is.
    pub max_damage: i32,
    /// `ItemType::degrade_increments`: zero for every type that does not degrade, which is all but the vehicle parts.
    pub degrade_increments: i32,
    /// The type's stack size, which is how a charge-counted type turns damage into charges lost.
    pub stack_size: i32,
    /// Whether it is counted in charges, which is what makes damage eat charges instead of health.
    pub count_by_charges: bool,
    /// Whether it carries `UNBREAKABLE`, which refuses all damage.
    pub unbreakable: bool,
}

impl ItemDamage {
    /// Sets how damaged it is, but never below what can no longer be repaired.
    ///
    /// Reference: `item::set_damage` (`src/item.cpp:967-970`) is `damage_ = clamp( qty, degradation_, max_damage() )` — the clamp's **lower** bound is the
    /// degradation, not zero.
    pub fn set_damage(&mut self, quantity: i32, limits: &DamageLimits) {
        self.damage = quantity.clamp(self.degradation, limits.max_damage);
    }

    /// Sets how much of the damage cannot be repaired, and pulls the damage up with it.
    ///
    /// Reference: `item::set_degradation` (`src/item.cpp:972-979`). A type with no degradation increments keeps zero however hard it is pushed, and the trailing
    /// `set_damage( damage_ )` is what enforces the invariant when the floor rises past the damage.
    pub fn set_degradation(&mut self, quantity: i32, limits: &DamageLimits) {
        self.degradation = if limits.degrade_increments > 0 {
            quantity.clamp(0, limits.max_damage)
        } else {
            0
        };
        self.set_damage(self.damage, limits);
    }

    /// Damages it, and answers whether that destroyed it.
    ///
    /// Reference: `item::mod_damage` (`src/item.cpp:9440-9468`). Three branches that behave nothing alike:
    ///
    /// * `UNBREAKABLE` refuses outright;
    /// * a **charge-counted** item loses charges, `stack_size * qty / damage_scale` of them, and is destroyed when it runs out of charges rather than of health;
    /// * anything else takes the damage, and if it survived, **automatic degradation** comes with it — the fraction of a degradation increment the new damage level
    ///   crossed, which is what [`degrade_amount`] computes.
    ///
    /// The destruction test is made **before** the damage is applied (`( damage_ + qty ) > max_damage()`), so the hit that takes an item exactly to its maximum is not
    /// the hit that destroys it.
    #[must_use]
    pub fn mod_damage(self, quantity: i32, limits: &DamageLimits) -> (Self, bool) {
        if limits.unbreakable {
            return (self, false);
        }
        let mut damage = self;
        if limits.count_by_charges {
            // A charge-counted item has no health to damage: the reference eats its **charges** and returns whether that emptied it, so the caller owns the count
            // ([`mod_damage_charges`]) and this state stays at zero.
            return (damage, false);
        }
        let before = damage.damage;
        let destroyed = damage.damage.saturating_add(quantity) > limits.max_damage;
        damage.set_damage(damage.damage.saturating_add(quantity), limits);
        if quantity > 0 && !destroyed {
            let step = degrade_amount(&damage, before, limits);
            damage.set_degradation(damage.degradation.saturating_add(step), limits);
        }
        (damage, destroyed)
    }
}

/// The faults one item carries.
///
/// Reference: `item::faults` (`src/item.h`), a set of fault ids. Distinct from the **type's** weighted list, which is what the item *could* catch: this is what it
/// has.
#[derive(Debug, Clone, Default, PartialEq, Eq, Component)]
pub struct ItemFaults(pub BTreeSet<String>);

/// Gives an item a fault, and clears the ones it prevents.
///
/// Reference: `item::set_fault` (`src/item.cpp:7828-7845`). Three steps in a fixed order, and the middle one is the one that is easy to miss: the fault's own
/// `block_faults` are **removed first**, because a thing cannot have a chipped blade when the blade is gone — so giving an item "broken" takes away its "chipped".
/// A forced fault skips the check; otherwise [`can_have_fault`] decides and nothing changes when it says no.
///
/// Returns whether the fault was applied.
pub fn set_fault(
    faults: &mut ItemFaults,
    fault: &str,
    force: bool,
    weight: i32,
    blocks: impl Fn(&str) -> BTreeSet<String>,
) -> bool {
    if !force && !can_have_fault(weight, fault, &faults.0, &blocks) {
        return false;
    }

    for blocked in blocks(fault) {
        faults.0.remove(&blocked);
    }
    faults.0.insert(fault.to_owned());
    true
}

/// Takes a fault away.
///
/// Reference: `item::remove_fault` (`src/item.cpp:7869-7872`) is one `erase`, and `remove_single_fault_of_type` (`7874-7882`) removes **the first** fault of a type
/// and stops — not every one of them, which is what a caller who has both a chipped and a cracked blade would notice.
pub fn remove_fault(faults: &mut ItemFaults, fault: &str) {
    faults.0.remove(fault);
}

/// Which fault an item catches when it takes damage, or `None` when it can catch none.
///
/// Reference: `item::set_random_fault_of_type` (`src/item.cpp:7848-7865`). The list is rebuilt from the **type's** weighted faults, keeping only those of the
/// requested kind that the item can still have ([`can_have_fault`]); the pick then goes through `weighted_int_list::pick` (`src/weighted_list.h:120-129`,
/// `247-267`), which is `rng_bits()` and not `rng( lo, hi )` — a raw 32-bit draw, reduced modulo the total weight and then walked from the front. A list with one
/// entry in it returns that entry **without touching a die at all** (`if( objects.size() == 1 ) return 0;`), which is the sort of thing a recording notices.
///
/// `kinds` answers a fault's `fault_type`, and `blocks` answers which faults one of the item's existing faults prevents — both are content, so both are passed in.
#[must_use]
pub fn pick_fault(
    faults: &[(String, i32)],
    existing: &BTreeSet<String>,
    fault_type: &str,
    kinds: impl Fn(&str) -> Option<String>,
    blocks: impl Fn(&str) -> BTreeSet<String>,
    rolls: &mut Rolls,
) -> Option<String> {
    let blocked: BTreeSet<String> = existing
        .iter()
        .flat_map(|fault| blocks(fault))
        .collect();
    let candidates: Vec<&(String, i32)> = faults
        .iter()
        .filter(|(id, weight)| {
            *weight > 0
                && kinds(id).is_some_and(|kind| kind == fault_type)
                && !blocked.contains(id)
                && !existing.contains(id)
        })
        .collect();
    let total: i32 = candidates.iter().map(|(_, weight)| *weight).sum();
    if candidates.is_empty() || total <= 0 {
        return None;
    }
    if candidates.len() == 1 {
        return candidates.first().map(|(id, _)| id.clone());
    }
    let raw = rolls.rng_bits();
    let picked = raw % u32::try_from(total).unwrap_or(u32::MAX) + 1;
    let mut accumulated: u32 = 0;
    for (id, weight) in candidates {
        accumulated = accumulated.saturating_add(u32::try_from(*weight).unwrap_or(0));
        if accumulated >= picked {
            return Some(id.clone());
        }
    }
    None
}

/// Whether an item can still take a fault: the type must rate it above zero and no fault the item already has may block it.
///
/// Reference: `item::can_have_fault` (`src/item.cpp:7809-7826`). The block rule is **one-directional** — a broken handle prevents chipping, but having chipping does
/// not prevent being broken — and a fault already present is not filtered here; [`pick_fault`] excludes those, because the reference's weighted list does not hold
/// duplicates.
#[must_use]
pub fn can_have_fault(
    weight: i32,
    fault: &str,
    existing: &BTreeSet<String>,
    blocks: impl Fn(&str) -> BTreeSet<String>,
) -> bool {
    weight > 0
        && !existing.contains(fault)
        && !existing
            .iter()
            .any(|present| blocks(present).contains(fault))
}

/// Degradation a badly-made item starts with, rolled rather than stated.
///
/// Reference: `item::rand_degradation` (`src/item.cpp:903-909`). An item spawned damaged gets a degradation somewhere between undamaged and its own damage, scaled
/// so that a `max_damage / degrade_increments` slice of health is one increment: `rng( 0, damage() ) * 50.0f / degrade_increments()`, converted to a whole number
/// when `set_degradation` takes it — which **truncates**, so the result is never rounded up.
///
/// A type that does not degrade rolls nothing at all: the reference returns before it touches a die, which matters because a roll that does not happen is a die the
/// recording does not contain.
#[must_use]
pub fn rand_degradation(rolls: &mut Rolls, damage: i32, degrade_increments: i32) -> i32 {
    if degrade_increments == 0 {
        return 0;
    }
    let drawn = rolls.rng(0, damage);
    #[allow(clippy::cast_possible_truncation)]
    let degradation = (f64::from(drawn) * 50.0 / f64::from(degrade_increments)) as i32;
    degradation
}

/// The bar the interface draws beside an item to show what repairs cannot fix.
///
/// Reference: `item::degradation_symbol` (`src/item.cpp:6918-6941`). Five bars, from a full block at no degradation down to a single row at four increments or more,
/// and **nothing at all for a type that does not degrade** — the type check is the last thing the reference does, not the first.
#[must_use]
pub fn degradation_symbol(degradation: i32, max_damage: i32, degrade_increments: i32) -> &'static str {
    if degrade_increments == 0 {
        return "";
    }
    let slice = max_damage / 5;
    let level = degradation / if slice > 0 { slice } else { 1 };
    match level {
        0 => "█",
        1 => "▇",
        2 => "▅",
        3 => "▃",
        _ => "▁",
    }
}

/// Damages a **charge-counted** pile by taking charges, and answers whether that emptied it.
///
/// Reference: the charge branch of `item::mod_damage` (`src/item.cpp:9444-9447`): `charges -= std::min( stack_size * qty / damage_scale, charges ); return charges == 0;`.
/// A pile of two hundred nails does not get "hurt"; it loses nails.
///
/// Returns the charges left and whether the pile is now gone.
#[must_use]
pub fn mod_damage_charges(charges: i32, quantity: i32, stack_size: i32) -> (i32, bool) {
    let lost = stack_size.max(0).saturating_mul(quantity) / DAMAGE_SCALE;
    let left = charges - lost.min(charges.max(0));
    (left, left <= 0)
}

/// The damage scale: a thousand units to a point of health, which is why the numbers above are in the thousands.
pub const DAMAGE_SCALE: i32 = 1_000;

/// How much degradation a hit adds, as a fraction of an increment.
///
/// Reference: `get_degrade_amount` (`src/item.cpp:9428-9439`). The two "factors" are the damage's position in its five bands, computed as `( damage - 1 ) * 5 / max`
/// — so **zero damage is factor minus one**, not zero, which is what makes the first hit add two factors instead of one. The answer is
/// `max( facNow - facPrev, 0 ) * max_damage / degrade_increments`: the number of bands crossed, scaled to an increment. A type that does not degrade, or one counted
/// in charges, answers zero.
#[must_use]
pub fn degrade_amount(after: &ItemDamage, before_damage: i32, limits: &DamageLimits) -> i32 {
    if limits.max_damage == 0 || limits.degrade_increments == 0 {
        return 0;
    }
    let now = factor(after.damage, limits.max_damage);
    let previous = factor(before_damage, limits.max_damage);
    (now - previous).max(0) * limits.max_damage / limits.degrade_increments
}

/// A damage's position in the type's five bands, with the reference's odd `-1` for undamaged.
fn factor(damage: i32, max_damage: i32) -> i32 {
    if damage == 0 { -1 } else { (damage - 1) * 5 / max_damage }
}

/// The largest volume any item may take up: `MAX_ITEM_VOLUME` (`src/game_constants.h:35-36`), one tile, 1,000 litres.
pub const MAX_ITEM_VOLUME_ML: i64 = 1_000_000;

/// What a corpse takes up.
///
/// Reference: `item::corpse_volume` (`src/item.cpp:7455-7479`), reached from `item::volume` for anything that is a corpse (`src/item.cpp:7518-7520`). The volume is the
/// **creature's** own, multiplied by four fractions in a fixed order — quartered by 4, field-dressed by 3/4, gibbed by 0.85, skinned by 0.85 — and every multiplication
/// truncates, because the arithmetic is on whole millilitres. The result is capped at [`MAX_ITEM_VOLUME_ML`], so a creature larger than a tile still leaves a corpse that
/// fits in one.
///
/// Field-dressed and field-dressed-**badly** are the same fraction, which is why the reference treats them as one flag pair here: a botched butchery still removes the
/// parts it removed.
#[must_use]
pub fn corpse_volume(monster_volume_ml: i64, flags: &std::collections::BTreeSet<String>) -> i64 {
    let mut volume = monster_volume_ml;
    if flags.contains("QUARTERED") {
        volume = fraction(volume, 0.25);
    }
    if flags.contains("FIELD_DRESS") || flags.contains("FIELD_DRESS_FAILED") {
        volume = fraction(volume, 0.75);
    }
    if flags.contains("GIBBED") {
        volume = fraction(volume, 0.85);
    }
    if flags.contains("SKINNED") {
        volume = fraction(volume, 0.85);
    }
    volume.min(MAX_ITEM_VOLUME_ML)
}

/// A volume multiplied by a fraction and truncated, the way `units::volume *= double` does it.
fn fraction(volume_ml: i64, factor: f64) -> i64 {
    #[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)]
    let scaled = (volume_ml as f64 * factor) as i64;
    scaled
}

/// Takes the dead off the field and leaves what they were standing on, as the reference leaves it: an item.
///
/// Reference: `monster::die` calls `m.add_item_or_charges( pos, item::make_corpse( ... ) )`, so a corpse is a
/// **corpse item** rather than a creature-shaped thing the world has to know about. `corpse_kind` is the item type
/// the content calls `corpse`, supplied by the caller because the item layer numbers types rather than naming them.
///
/// Returns how many bodies were cleared.
pub fn reap_dead_monsters(
    world: &mut World,
    corpse_kind: ItemTypeId,
    volume_of: impl Fn(MonsterKindId) -> i64,
) -> usize {
    // One reaper, not two: this name is kept because the item and loot tests ask for it, and it now runs the
    // creature module's own sweep, which also clears the index entry the store no longer justifies.
    crate::creature::reap_dead_creatures_as_corpses(world, corpse_kind, volume_of)
}

/// How many corpses lie on a square.
#[must_use]
pub fn corpses_at(world: &mut World, square: (i32, i32)) -> usize {
    let mut query = world.query::<(&Position, &CorpseOf)>();
    query
        .iter(world)
        .filter(|(position, _)| (position.x, position.y) == square)
        .count()
}

/// What kind of creature a corpse was, if the square holds one.
#[must_use]
pub fn corpse_kind_at(world: &mut World, square: (i32, i32)) -> Option<MonsterKindId> {
    let mut query = world.query::<(&Position, &CorpseOf)>();
    query
        .iter(world)
        .find(|(position, _)| (position.x, position.y) == square)
        .map(|(_, corpse)| corpse.kind)
}

/// Whether an item is the body of a creature.
///
/// Reference: `item::is_corpse` (`item.cpp`), which asks whether the item names a monster **and** carries the
/// `CORPSE` flag. The port's corpse item carries the first fact as `CorpseOf`; the flag comes with the item type
/// the caller supplies, so both halves are asked here rather than one of them being assumed.
#[must_use]
pub fn is_corpse(
    world: &mut World,
    item: Entity,
    type_has_corpse_flag: impl Fn(ItemTypeId) -> bool,
) -> bool {
    let Some(corpse) = world.get::<CorpseOf>(item) else {
        return false;
    };
    let kind = world.get::<GroundItem>(item).map(|item| item.kind);
    let _ = corpse;
    kind.is_some_and(type_has_corpse_flag)
}

/// A day, in turns, which is the unit the reference measures rot in.
///
/// It used to say `24 * 60 * 60 / 6`, from an older Cataclysm whose turn was six seconds; the reference's `from_days` multiplies by
/// sixty at every step (`calendar.h:214-234`), so a day is 86,400 turns and every duration measured in days was six times too short.
pub use crate::calendar::TURNS_PER_DAY;

/// How long a corpse has been lying there, in turns.
#[must_use]
pub fn corpse_age(world: &mut World, item: Entity) -> Option<i64> {
    world.get::<CorpseOf>(item)?;
    world.get::<GroundItem>(item).map(|item| item.age)
}

/// Whether a corpse has rotted away entirely and can no longer be butchered.
///
/// Reference: `item::has_rotten_away` (`item.cpp`) is special-cased for corpses - `get_rot() > 10_days` when the
/// corpse cannot revive - rather than using the relative rot that other food uses. Ten days is long, and getting it
/// wrong would either delete butchering entirely or leave a permanent wall of rotten bodies.
#[must_use]
pub fn corpse_rotten_away(world: &mut World, item: Entity, can_revive: bool) -> bool {
    let Some(age) = corpse_age(world, item) else {
        return false;
    };
    if can_revive {
        return false;
    }
    age > 10 * TURNS_PER_DAY
}

/// How big a creature was, which decides the base cost of cutting it up.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CreatureSize {
    /// A cat.
    Tiny,
    /// A dog.
    Small,
    /// A person.
    Medium,
    /// A cow.
    Large,
    /// Something you need a ladder for.
    Huge,
}

/// What kind of butchery is being done.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ButcherAction {
    /// Cut it up roughly, where it lies.
    Quick,
    /// Drain it first.
    Bleed,
    /// Remove the organs.
    FieldDress,
    /// Take the hide.
    Skin,
    /// Quarter it into pieces.
    Quarter,
    /// Take it apart for parts.
    Dismember,
    /// Learn something from its anatomy.
    Dissect,
    /// The whole job, in a workshop.
    Full,
}

/// What is known about the corpse and the person cutting it.
///
/// The bools are the corpse's own flags - dressed, failed dressing, quartered - plus one for a butcher who bleeds
/// instantly. Four separate facts, mirroring the reference's item flags rather than a bitfield of our own.
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, Copy)]
pub struct ButcherState {
    /// The creature's size.
    pub size: CreatureSize,
    /// What is being done.
    pub action: ButcherAction,
    /// The best cutting tool's quality, which shortens the work.
    pub tool_factor: i32,
    /// Whether the corpse has already been field dressed.
    pub field_dressed: bool,
    /// Whether field dressing went wrong.
    pub field_dress_failed: bool,
    /// Whether the corpse has already been quartered.
    pub quartered: bool,
    /// Whether the butcher bleeds things instantly.
    pub instant_bleed: bool,
    /// Practice with butchering, from nothing to certain.
    pub butchering_basic: f64,
    /// Practice with butchery beyond the basics.
    pub butchering_advanced: f64,
    /// Practice with skinning.
    pub skinning_basic: f64,
    /// How many helpers are lending a hand, at most three counting.
    pub helpers: i32,
}

/// How many moves cutting up a corpse takes.
///
/// Reference: `butcher_time_to_cut` (`src/butchery.cpp`), transcribed in its own order of operations, because the
/// order is what the numbers depend on: the size gives a base, the tool shortens it with a **floor of a quarter**,
/// the action multiplies it, and the proficiency and helper adjustments come last as floating point. Rounding
/// differently at any step produces a different number of turns for the same corpse.
///
/// The conversions below are the reference's own: it holds these values in `int` and truncates the same way.
#[must_use]
#[allow(clippy::cast_possible_truncation)]
pub fn butcher_time_to_cut(state: ButcherState) -> i32 {
    let mut time: i32 = match state.size {
        CreatureSize::Tiny => 150,
        CreatureSize::Small => 300,
        CreatureSize::Medium => 450,
        CreatureSize::Large => 600,
        CreatureSize::Huge => 1800,
    };
    // A good tool cannot save more than three quarters of the work, however fine it is.
    time *= (100 - state.tool_factor).max(25);
    time = match state.action {
        ButcherAction::Quick => time,
        ButcherAction::Bleed => {
            if state.instant_bleed {
                1
            } else {
                time
            }
        }
        ButcherAction::Full => {
            if state.field_dressed && !state.field_dress_failed {
                time * 4
            } else {
                time * 6
            }
        }
        ButcherAction::FieldDress | ButcherAction::Skin => time * 2,
        ButcherAction::Quarter => (time / 4).max(1200),
        ButcherAction::Dismember => (time / 10).max(600),
        ButcherAction::Dissect => time * 6,
    };
    if state.quartered {
        time /= 4;
    }
    // Proficiency penalties are whole numbers in the reference, truncated from the sum.
    let butcher_penalty =
        (1.5 * (1.0 - state.butchering_basic) + 0.5 * (1.0 - state.butchering_advanced)) as i32;
    let skin_penalty = (0.5 * (1.0 - state.skinning_basic)) as i32;
    if state.action == ButcherAction::Full {
        time = (f64::from(time)
            * (0.4 * (1.0 + f64::from(butcher_penalty))
                + 0.4 * (1.0 + f64::from(skin_penalty))
                + 0.2)) as i32;
    }
    if state.action == ButcherAction::Quick {
        time = (f64::from(time)
            * (0.7 * (1.0 + f64::from(butcher_penalty))
                + 0.15 * (1.0 + f64::from(skin_penalty))
                + 0.15)) as i32;
    }
    if state.action == ButcherAction::FieldDress {
        time = (f64::from(time) * (1.0 + f64::from(butcher_penalty))) as i32;
    }
    if state.action == ButcherAction::Skin {
        time = (f64::from(time) * (1.0 + f64::from(skin_penalty))) as i32;
    }
    // Helpers: the reference multiplies the time by a fraction, not by a whole number of them.
    // Truncating the fraction to zero first would delete the work instead of shortening it.
    (f64::from(time) * (1.0 - f64::from(state.helpers.clamp(0, 3)) / 10.0)) as i32
}

/// What a butcher can take from a corpse.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HarvestEntry {
    /// Meat.
    Flesh,
    /// Organs and guts.
    Offal,
    /// Hide.
    Skin,
    /// Bones.
    Bone,
    /// Blood, which damage leaves alone.
    Blood,
    /// Anything the reference knows that this port does not, which damage also leaves alone.
    Other,
}

/// How badly the corpse was damaged, in the reference's own levels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CorpseDamage {
    /// Hardly marked, or not at all.
    Light,
    /// "damaged".
    Damaged,
    /// "mangled".
    Mangled,
    /// "pulped".
    Pulped,
}

/// What a damaged corpse still gives up, as a weight.
///
/// Reference: `corpse_damage_effect` (`src/butchery.cpp`). The interesting part is how differently it treats the
/// parts: a damaged corpse loses flesh before it loses hide, a mangled one loses **all** of its offal, and a pulped
/// one keeps only bone - so pulping a body is not a small penalty, it is a different set of products. Rounding is
/// `std::round`, not truncation, which matters at the halves.
#[must_use]
pub fn harvest_after_damage(weight: i32, entry: HarvestEntry, damage: CorpseDamage) -> i32 {
    let round = |value: f64| -> i32 {
        // Round half away from zero, as std::round does.
        let rounded = if value < 0.0 {
            -(-value + 0.5).floor()
        } else {
            (value + 0.5).floor()
        };
        #[allow(clippy::cast_possible_truncation)]
        let whole = rounded as i32;
        whole
    };
    let exact = f64::from(weight);
    match damage {
        CorpseDamage::Light => weight,
        // Blood and unknown parts are not reduced by damage in the reference's switch.
        _ if matches!(entry, HarvestEntry::Blood | HarvestEntry::Other) => weight,
        CorpseDamage::Damaged => match entry {
            HarvestEntry::Offal | HarvestEntry::Skin => round(exact * 0.75),
            HarvestEntry::Flesh => round(exact * 0.9),
            HarvestEntry::Bone | HarvestEntry::Blood | HarvestEntry::Other => weight,
        },
        CorpseDamage::Mangled => match entry {
            HarvestEntry::Offal => 0,
            HarvestEntry::Skin => round(exact * 0.5),
            HarvestEntry::Bone => round(exact * 0.9),
            HarvestEntry::Flesh => round(exact * 0.75),
            HarvestEntry::Blood | HarvestEntry::Other => weight,
        },
        CorpseDamage::Pulped => match entry {
            HarvestEntry::Offal | HarvestEntry::Skin => 0,
            HarvestEntry::Bone => round(exact * 0.75),
            HarvestEntry::Flesh => round(exact * 0.5),
            HarvestEntry::Blood | HarvestEntry::Other => weight,
        },
    }
}

/// One harvest entry, as the rule needs it: what it is and how much.
///
/// The sim does not depend on the content crate, so the numbers arrive here rather than being looked up: the
/// caller reads them from a harvest definition and hands them over.
#[derive(Debug, Clone, Copy)]
pub struct HarvestSpec {
    /// What part of the body it is, when the reference says.
    pub part: Option<HarvestEntry>,
    /// A count of items, low and high, before scaling.
    pub base_num: (i32, i32),
    /// How the count grows with the butcher's skill, low and high.
    pub scale_num: (f64, f64),
    /// A share of the corpse's mass, which overrides the count when set.
    pub mass_ratio: Option<f64>,
    /// A ceiling on the count.
    pub max: Option<i32>,
}

/// How much of one harvest entry a butchered corpse gives, before the proficiency adjustments.
///
/// Reference: `butchery.cpp`, in the loop that walks a monster's harvest entries. Two paths, and they are not
/// symmetrical:
///
/// - a **bulk** entry (`mass_ratio`) is a share of the corpse's weight, rounded, and then damaged;
/// - a **count** entry rolls between `base_num + butchery * scale_num` at each end, is capped by `max`, is damaged,
///   and then **cannot fall below `base_num.first`** - the floor is applied after the damage, so a mangled corpse
///   still gives the minimum the JSON promises.
///
/// On top of that, a **gibbed** corpse halves its flesh and bone, and a **skinned** one yields no skin at all.
/// `draw` is the caller's uniform roll in `[0, 1]`, for the same reason every other rule takes its dice from the
/// caller: one stream, one replay.
#[must_use]
#[allow(clippy::cast_possible_truncation)]
pub fn harvest_yield(
    entry: HarvestSpec,
    butchery: i32,
    corpse_weight: f64,
    damage: CorpseDamage,
    gibbed: bool,
    skinned: bool,
    draw: f64,
) -> i32 {
    let part = entry.part;
    let damages_anything = matches!(
        part,
        Some(HarvestEntry::Flesh | HarvestEntry::Offal | HarvestEntry::Skin | HarvestEntry::Bone)
    );
    let apply_damage = |count: i32| -> i32 {
        if damages_anything {
            harvest_after_damage(count, part.unwrap_or(HarvestEntry::Other), damage)
        } else {
            count
        }
    };
    let mut roll = match entry.mass_ratio {
        Some(ratio) if ratio != 0.0 => {
            let exact = ratio * corpse_weight;
            let rounded = if exact < 0.0 {
                -(-exact + 0.5).floor() as i32
            } else {
                (exact + 0.5).floor() as i32
            };
            apply_damage(rounded)
        }
        _ => {
            let (low, high) = entry.base_num;
            let (scale_low, scale_high) = entry.scale_num;
            let min_num = f64::from(low) + f64::from(butchery) * scale_low;
            let max_num = f64::from(high) + f64::from(butchery) * scale_high;
            let between = min_num + (max_num - min_num) * draw.clamp(0.0, 1.0);
            let rolled = (between + 0.5).floor() as i32;
            let capped = match entry.max {
                Some(max) => rolled.min(max),
                None => rolled,
            };
            apply_damage(capped).max(low)
        }
    };
    // A body that was smashed apart has less of the parts that scatter.
    if gibbed && matches!(part, Some(HarvestEntry::Flesh | HarvestEntry::Bone)) {
        roll /= 2;
    }
    if skinned && part == Some(HarvestEntry::Skin) {
        roll = 0;
    }
    roll
}

/// Cutting up a corpse, over several turns.
///
/// Reference: `butchery_activity_actor` (`activity_actor_definitions.h`) holds the corpse, the kind of butchery and
/// the time to cut, and each turn of the activity spends moves against that time. The port keeps the same three
/// facts on the corpse itself: only one activity can be working on a body at a time, and putting them on the body
/// says so without a second index to keep in step.
#[derive(Debug, Clone, Copy, Component)]
pub struct Butchery {
    /// The corpse being cut up.
    pub corpse: Entity,
    /// What kind of butchery this is.
    pub action: ButcherAction,
    /// Moves still to spend.
    pub moves_left: i32,
}

/// How far a turn of butchery got.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ButcheryStep {
    /// Work was done and there is more to do.
    Working,
    /// The work is done; the caller should now take the yields.
    Finished,
    /// Nothing to do, or nothing moved.
    Idle,
}

/// Starts cutting up a corpse, and says how many moves it will take.
///
/// Returns `None` when the square holds nothing that names a creature, which is the reference's refusal to butcher
/// something that is not a corpse.
pub fn start_butchery(world: &mut World, corpse: Entity, butchery: ButcherState) -> Option<i32> {
    world.get::<CorpseOf>(corpse)?;
    let moves = butcher_time_to_cut(butchery);
    world.entity_mut(corpse).insert(Butchery {
        corpse,
        action: butchery.action,
        moves_left: moves,
    });
    Some(moves)
}

/// Spends one turn's moves on a butchery, and reports whether that finished it.
pub fn butchery_turn(world: &mut World, corpse: Entity, moves: i32) -> ButcheryStep {
    let left = {
        let Some(mut work) = world.get_mut::<Butchery>(corpse) else {
            return ButcheryStep::Idle;
        };
        if moves <= 0 || work.moves_left <= 0 {
            return ButcheryStep::Idle;
        }
        work.moves_left -= moves;
        work.moves_left
    };
    if left > 0 {
        return ButcheryStep::Working;
    }
    // Done: the activity comes off the body, and the caller takes the yields.
    world.entity_mut(corpse).remove::<Butchery>();
    ButcheryStep::Finished
}

/// How much work is left on a butchery, in moves.
#[must_use]
pub fn butchery_moves_left(world: &World, corpse: Entity) -> Option<i32> {
    world.get::<Butchery>(corpse).map(|work| work.moves_left)
}

/// What a harvest is drawn from: the entries, the ids their drops name, and the corpse's weight.
#[derive(Debug, Clone, Copy)]
pub struct HarvestTable<'a> {
    /// What the corpse yields, in the definition's order.
    pub entries: &'a [HarvestSpec],
    /// The item ids the entries name, in the same order.
    pub ids: &'a [&'a str],
    /// How much the creature weighed, in grams.
    pub weight: f64,
}

/// The facts a finished butchery needs that are not in the entries themselves.
#[derive(Debug, Clone, Copy)]
pub struct ButcherFinish {
    /// The butcher's effective skill, which scales counts.
    pub butchery: i32,
    /// How badly the corpse was damaged.
    pub damage: CorpseDamage,
    /// Whether the body was smashed apart before it was cut up.
    pub gibbed: bool,
    /// Whether the hide has already been taken.
    pub skinned: bool,
}

/// Takes a finished butchery's yields and puts them on the ground.
///
/// Reference: the loop in `butchery.cpp` after the cuts are done - each entry is rolled, damaged, and turned into
/// items on the corpse's square, and the corpse itself is consumed. The port does the same in that order and
/// returns what it dropped, so a test can ask what a player would find rather than what the rule computed.
///
/// The uniform draw comes from `rolls`, one per entry, in entry order: a different order is a different harvest
/// from the same seed, which is why the order is the definition's own.
pub fn butcher_finish(
    world: &mut World,
    corpse: Entity,
    table: HarvestTable<'_>,
    state: ButcherFinish,
    drop_of: impl Fn(&str) -> ItemTypeId,
    rolls: &mut cdda_core_types::rng::Rolls,
) -> Vec<(ItemTypeId, i32)> {
    let Some(position) = world.get::<Position>(corpse).copied() else {
        return Vec::new();
    };
    let square = (position.x, position.y);
    let mut dropped = Vec::new();
    for (entry, id) in table.entries.iter().zip(table.ids.iter()) {
        let draw = f64::from(rolls.rng(0, 10_000)) / 10_000.0;
        let count = harvest_yield(
            *entry,
            state.butchery,
            table.weight,
            state.damage,
            state.gibbed,
            state.skinned,
            draw,
        );
        if count <= 0 {
            continue;
        }
        let kind = drop_of(id);
        drop_item(world, square, kind, count);
        dropped.push((kind, count));
    }
    // The body has been used up.
    world.despawn(corpse);
    dropped
}
