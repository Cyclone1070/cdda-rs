//! Applying a mapgen update to the world.
//!
//! Reference: `mapgen::update` (`mapgen.cpp`): an update mapgen is drawn like any other and its terrain written over the squares it covers, which
//! is how a lab's corridors are opened or a wall is torn out by a script. The port keeps the mapgens as data in one entity and writes their terrain
//! into the squares the store already holds.

use std::collections::BTreeMap;

use bevy_ecs::prelude::*;

/// Every mapgen the world can apply, keyed by the id the data gives it.
#[derive(Debug, Clone, Default, PartialEq, Component)]
/// **Every** mapgen filed under a name, because an overmap terrain may have several and the game picks among them by weight (`mapgen.cpp:1078-1088`). An update mapgen
/// is filed once, so the list a caller reading an update wants is its first entry - which is why this is a list and not a map of maps.
pub struct Mapgens(pub BTreeMap<String, Vec<cdda_content::mapgen::MapgenObject>>);

/// The marker that says a single entity holds the mapgens.
#[derive(Debug, Clone, Copy, Default, Component)]
pub struct TheMapgens;

/// What number the world calls each terrain, which is how a drawn id reaches a square.
#[derive(Debug, Clone, Default, PartialEq, Component)]
pub struct TerrainCodes(pub BTreeMap<String, u32>);

/// The marker that says a single entity holds the terrain codes.
#[derive(Debug, Clone, Copy, Default, Component)]
pub struct TheTerrainCodes;

/// Installs the mapgens, replacing whatever the world held.
pub fn set_mapgens(
    world: &mut World,
    mapgens: impl IntoIterator<Item = (String, cdda_content::mapgen::MapgenObject)>,
) {
    let holder = crate::singletons::singleton::<Mapgens, TheMapgens>(world);
    let mut filed: BTreeMap<String, Vec<cdda_content::mapgen::MapgenObject>> = BTreeMap::new();
    for (name, mapgen) in mapgens {
        filed.entry(name).or_default().push(mapgen);
    }
    world.entity_mut(holder).insert(Mapgens(filed));
}

/// Installs the terrain codes, so a drawn terrain id can be written to a square.
pub fn set_terrain_codes(world: &mut World, codes: BTreeMap<String, u32>) {
    let holder = crate::singletons::singleton::<TerrainCodes, TheTerrainCodes>(world);
    world.entity_mut(holder).insert(TerrainCodes(codes));
}

/// What number the world calls each furniture, which is how a drawn furniture id reaches a square.
#[derive(Debug, Clone, Default, Component)]
pub struct FurnitureCodes(pub BTreeMap<String, u32>);

/// The marker that says a single entity holds the furniture codes.
#[derive(Debug, Clone, Copy, Default, Component)]
pub struct TheFurnitureCodes;

/// Files what number the world calls each furniture.
pub fn set_furniture_codes(world: &mut World, codes: BTreeMap<String, u32>) {
    let holder = crate::singletons::singleton::<FurnitureCodes, TheFurnitureCodes>(world);
    world.entity_mut(holder).insert(FurnitureCodes(codes));
}

/// Applies an update mapgen at a square, answering how many squares it wrote.
///
/// Reference: `mapgen::update` (`mapgen.cpp`) draws the mapgen and writes the terrain it names, leaving squares it says nothing about alone - which
/// is why an update mapgen may cover only part of a tile.
#[must_use]
pub fn apply(world: &mut World, update: &str, origin: (i32, i32, i32)) -> Option<usize> {
    let mapgen = crate::singletons::holder::<TheMapgens>(world)
        .and_then(|holder| world.get::<Mapgens>(holder))
        // An update mapgen is filed once; a terrain's several mapgens are not this path's business, so the first is the one an update writes.
        .and_then(|mapgens| {
            mapgens
                .0
                .get(update)
                .and_then(|filed| filed.first())
                .cloned()
        })?;
    Some(write(world, &mapgen, origin))
}

/// Writes one mapgen's terrain at an origin, answering how many squares it filled.
///
/// This is the single writer of a drawn mapgen: [`apply`] writes an update's, and [`crate::mapgen_tile::generate`] writes whichever one the weight picked for a tile. Two
/// writers would eventually disagree about what a symbol means, which is exactly the second source of truth the architecture forbids.
pub fn write(
    world: &mut World,
    mapgen: &cdda_content::mapgen::MapgenObject,
    origin: (i32, i32, i32),
) -> usize {
    // **The dice are taken once for the whole write**, because a layer that stopped mid-chunk would leave the generator half-spent - and a caller that held them would have to
    // hand them back for every square.
    let mut rolls = crate::singletons::take_rolls(world);
    // **The rotation is drawn first**, because the reference draws it before it draws anything else (`mapgen.cpp:6072`) - and it is a dice roll, so where it comes in the
    // stream is part of the mapgen's result.
    let turns = match rolls.as_mut() {
        Some(dice) if mapgen.rotation.1 > mapgen.rotation.0 => {
            dice.rng(mapgen.rotation.0, mapgen.rotation.1).rem_euclid(4)
        }
        _ => mapgen.rotation.0.rem_euclid(4),
    };
    let codes = crate::singletons::holder::<TheTerrainCodes>(world)
        .and_then(|holder| world.get::<TerrainCodes>(holder))
        .map(|codes| codes.0.clone())
        .unwrap_or_default();
    // **The removal phase is the first phase there is** (`mapgen.h:88-97` lists `removal` before `terrain`), so a `remove_all` strips its squares before the fill and the
    // rows put anything back - which is what makes it able to clear a room the mapgen itself does not draw.
    remove_everything(world, mapgen, origin);
    // **The fill comes first**, the whole chunk at the mapgen's own terrain (`mapgen.cpp:5100-5110`), so a map that draws six squares still fills its twenty-four. The count is
    // of squares **touched**, so a row drawn over a filled square is not counted twice: a chunk is a chunk however many layers land on it.
    let side = crate::overmap::SQUARES_PER_OMT;
    let filled = mapgen
        .fill_ter
        .as_deref()
        .and_then(|fill| codes.get(fill).copied());
    if let Some(code) = filled {
        for column in 0..side {
            for row in 0..side {
                crate::square::set_terrain(
                    world,
                    (origin.0 + column, origin.1 + row, origin.2),
                    code,
                );
            }
        }
    }
    let furniture_codes = crate::singletons::holder::<TheFurnitureCodes>(world)
        .and_then(|holder| world.get::<FurnitureCodes>(holder))
        .map(|codes| codes.0.clone())
        .unwrap_or_default();
    let mut written = if filled.is_some() {
        usize::try_from(side * side).unwrap_or(0)
    } else {
        0
    };
    let drew_squares = draw_rows(world, mapgen, origin, &codes, &furniture_codes, &mut rolls);
    if filled.is_none() {
        written += drew_squares;
    }
    // **The nested phase comes after the terrain and the objects** (`mapgen.h:88-97`: `removal`, `terrain`, `furniture`, `default_`, `nested_mapgen`, ...), so a nested chunk
    // drawn at a marker is not overwritten by the outer rows drawn after that marker - which is exactly what putting it inside the per-square pass did.
    draw_the_nested_mapgens(world, mapgen, origin, &codes, &furniture_codes, &mut rolls);
    place_the_nested_areas(world, mapgen, origin, &codes, &furniture_codes, &mut rolls);
    place_monsters(world, mapgen, origin, &mut rolls);
    place_area_items(world, mapgen, origin, &mut rolls);
    place_area_sets(world, mapgen, origin, &mut rolls);
    place_item_areas(world, mapgen, origin, &mut rolls);
    place_sign_areas(world, mapgen, origin, &mut rolls);
    place_liquid_areas(world, mapgen, origin, &mut rolls);
    place_set_points(world, mapgen, origin, &mut rolls);
    place_set_radiation(world, mapgen, origin, &mut rolls);
    place_rubble_areas(world, mapgen, origin, &mut rolls);
    place_set_variables(world, mapgen, origin, &mut rolls);
    place_loot(world, mapgen, origin, &mut rolls);
    // **The turn comes last**, after every phase (`mapgen.cpp:6096`): the chunk is rotated as a whole, so a rotated mapgen is not one whose layers disagree about where it stands.
    crate::mapgen_rotation::rotate_chunk(world, origin, turns);
    if let Some(rolls) = rolls {
        crate::singletons::put_rolls(world, rolls);
    }
    written
}

/// Draws a mapgen's rows, layer by layer, and answers how many squares it drew on.
///
/// Reference: `mapgen_function_json::generate` (`mapgen.cpp:5470-5530`). The order is the reference's own and it matters: terrain, then furniture, then traps, then items -
/// a trap on top of the terrain that was drawn for it, an item on the floor the row laid down.
fn draw_rows(
    world: &mut World,
    mapgen: &cdda_content::mapgen::MapgenObject,
    origin: (i32, i32, i32),
    codes: &BTreeMap<String, u32>,
    furniture_codes: &BTreeMap<String, u32>,
    rolls: &mut Option<cdda_core_types::rng::Rolls>,
) -> usize {
    let mut drawn = 0;
    for (row, line) in mapgen.rows.iter().enumerate() {
        for (column, symbol) in line.chars().enumerate() {
            let (Some(column), Some(row)) = (offset(column), offset(row)) else {
                continue;
            };
            let at = (origin.0 + column, origin.1 + row, origin.2);
            if draw_one_square(world, mapgen, at, symbol, codes, furniture_codes, rolls) {
                drawn += 1;
            }
        }
    }
    drawn
}

/// Places every layer one symbol carries, answering whether it drew anything at all.
fn draw_one_square(
    world: &mut World,
    mapgen: &cdda_content::mapgen::MapgenObject,
    at: (i32, i32, i32),
    symbol: char,
    codes: &BTreeMap<String, u32>,
    furniture_codes: &BTreeMap<String, u32>,
    rolls: &mut Option<cdda_core_types::rng::Rolls>,
) -> bool {
    // **A symbol may carry terrain, furniture, or both** (`mapgen.cpp:5480-5490`): each layer is placed if the mapgen names it, and a symbol that names neither is not drawn at
    // all - which keeps the count honest about what the chunk actually says.
    let mut drew = false;
    if let Some(code) = mapgen
        .terrain
        .get(&symbol)
        .and_then(|terrain| codes.get(terrain).copied())
    {
        crate::square::set_terrain(world, at, code);
        drew = true;
    }
    if let Some(code) = mapgen
        .furniture
        .get(&symbol)
        .and_then(|furniture| furniture_codes.get(furniture).copied())
    {
        crate::square::set_furniture(world, at, code);
        drew = true;
    }
    drew |= place_the_vending_machine(world, mapgen, at, symbol, rolls);
    drew |= place_the_sign(world, mapgen, at, symbol);
    drew |= place_the_gas_pump(world, mapgen, at, symbol, rolls);
    // **Furniture with something inside it** (`jmapgen_sealed_item`, `mapgen.cpp:4049-4060`): the contents are placed first and the furniture is set **over** them, so a
    // locker's contents are inside the locker rather than on the floor beside it.
    drew |= place_the_sealed_item(world, mapgen, at, symbol, rolls);
    // **A toilet is furniture with water in it** (`mapgen.cpp:2836-2844`, `place_toilet` at `:7296-7301`): the furniture is `f_toilet`, the item is `water` with the entry's
    // charges, and an amount of zero means the reference's default of 24 (`map.h:1679`).
    if let Some(amount) = mapgen.toilets.get(&symbol) {
        let rolled = match rolls.as_mut() {
            Some(rolls) if amount.1 > amount.0 => rolls.rng(amount.0, amount.1),
            _ => amount.0,
        };
        let charges = if rolled > 0 {
            rolled
        } else {
            DEFAULT_TOILET_CHARGES
        };
        if let Some(code) = crate::furniture_types::code_of(world, "f_toilet") {
            crate::square::set_furniture(world, at, code);
            drew = true;
        }
        if let Some(kind) = crate::item_registry::id_of(world, "water") {
            crate::drop_item(world, (at.0, at.1), kind, charges);
        }
    }
    // **Traps come after furniture** (`mapgen.cpp:5470-5480`), and a trap square is a drawn square even when the map names no terrain or furniture for it.
    if let Some(trap) = mapgen.traps.get(&symbol) {
        crate::square::set_trap(world, at, trap);
        drew = true;
    }
    drew |= place_the_items(world, mapgen, at, symbol, rolls);
    drew |= place_the_single_item(world, mapgen, at, symbol, rolls);
    drew |= place_the_field_symbol(world, mapgen, at, symbol, rolls);
    drew |= place_the_computer(world, mapgen, at, symbol);
    drew |= place_the_monster_symbol(world, mapgen, at, symbol, rolls);
    drew |= place_the_rubble(world, mapgen, at, symbol, rolls);
    drew |= place_the_liquid(world, mapgen, at, symbol, rolls);
    drew
}

/// What a toilet holds when the data names no amount: `place_toilet`'s own default (`map.h:1679`).
const DEFAULT_TOILET_CHARGES: i32 = 24;

/// Strips the squares of every `remove_all` rectangle: furniture, items, traps and fields.
///
/// Reference: `jmapgen_remove_all::apply` (`mapgen.cpp:4177-4205`): `furn_clear`, `i_clear`, `remove_trap`, `clear_fields` and `delete_graffiti` on **every** square of the
/// rectangle. **Named gap**: the port has no graffiti to delete and no vehicle to uncache; both are named with the stores they belong to.
fn remove_everything(
    world: &mut World,
    mapgen: &cdda_content::mapgen::MapgenObject,
    origin: (i32, i32, i32),
) {
    for area in &mapgen.remove_all {
        for x in area.x.0..=area.x.1 {
            for y in area.y.0..=area.y.1 {
                if !(0..SIDE).contains(&x) || !(0..SIDE).contains(&y) {
                    continue;
                }
                let at = (origin.0 + x, origin.1 + y, origin.2);
                crate::square::set_furniture(world, at, 0);
                for item in crate::items_at(world, (at.0, at.1)) {
                    world.despawn(item);
                }
                crate::square::remove_trap(world, at);
                clear_fields(world, at);
            }
        }
    }
}

/// Takes every field off a square.
fn clear_fields(world: &mut World, at: (i32, i32, i32)) {
    let standing: Vec<bevy_ecs::prelude::Entity> = world
        .query::<(bevy_ecs::prelude::Entity, &crate::Position, &crate::Field)>()
        .iter(world)
        .filter(|(_entity, position, _field)| (position.x, position.y, position.z) == at)
        .map(|(entity, _position, _field)| entity)
        .collect();
    for entity in standing {
        world.despawn(entity);
    }
}

/// Scatters terrain, furniture and traps over their rectangles.
///
/// Reference: `jmapgen_objects::apply` (`mapgen.cpp:6161-6164`): each entry is applied `repeat` times, and each application draws **one** square from the rectangle
/// (`jmapgen_int::get`), so a rectangle is a field of candidates and never a box that is filled.
fn place_area_sets(
    world: &mut World,
    mapgen: &cdda_content::mapgen::MapgenObject,
    origin: (i32, i32, i32),
    rolls: &mut Option<cdda_core_types::rng::Rolls>,
) {
    for entry in &mapgen.place_terrain {
        for spot in repeat_spots(rolls, entry) {
            if let Some(code) = crate::terrain_types::code_of(world, &entry.id) {
                crate::square::set_terrain(
                    world,
                    (origin.0 + spot.0, origin.1 + spot.1, origin.2),
                    code,
                );
            }
        }
    }
    for entry in &mapgen.place_furniture {
        for spot in repeat_spots(rolls, entry) {
            if let Some(code) = crate::furniture_types::code_of(world, &entry.id) {
                crate::square::set_furniture(
                    world,
                    (origin.0 + spot.0, origin.1 + spot.1, origin.2),
                    code,
                );
            }
        }
    }
    for entry in &mapgen.place_traps {
        for spot in repeat_spots(rolls, entry) {
            crate::square::set_trap(
                world,
                (origin.0 + spot.0, origin.1 + spot.1, origin.2),
                &entry.id,
            );
        }
    }
}

/// The squares one entry lands on: `repeat` draws from its rectangle, kept **inside the chunk**.
fn repeat_spots(
    rolls: &mut Option<cdda_core_types::rng::Rolls>,
    entry: &cdda_content::mapgen::AreaSet,
) -> Vec<(i32, i32)> {
    let count = match rolls.as_mut() {
        Some(dice) if entry.repeat.1 > entry.repeat.0 => dice.rng(entry.repeat.0, entry.repeat.1),
        _ => entry.repeat.0,
    }
    .max(0);
    let mut spots = Vec::new();
    for _ in 0..count {
        let (x, y) = match rolls.as_mut() {
            Some(dice) => (
                dice.rng(entry.x.0, entry.x.1),
                dice.rng(entry.y.0, entry.y.1),
            ),
            None => (entry.x.0, entry.y.0),
        };
        if (0..SIDE).contains(&x) && (0..SIDE).contains(&y) {
            spots.push((x, y));
        }
    }
    spots
}

/// Scatters the mapgen's item groups over their rectangles, once per chunk.
///
/// Reference: `jmapgen_item_group::apply` (`mapgen.cpp:3009-3015`) calls `map::place_items` (`:7376-7420`), which:
/// - draws `roll_remainder( chance / 100 )` spawns, so `chance` is a **percentage of one spawn**, not a per-square chance;
/// - picks a **random square in the rectangle** for each spawn;
/// - creates the group there, which may make several items that belong together.
///
/// The `repeat` range is applied the way the reference's `jmapgen_objects` applies it: the entry is placed that many times, each placement rolling its own count.
///
/// **Named gap**: the reference refuses squares whose terrain is impassable unless it is flagged `PLACE_ITEM` (`:7401-7415`), retrying up to twenty times. The port's terrain
/// table carries no move cost or flags here, so any square in the rectangle can take the spawn.
fn place_area_items(
    world: &mut World,
    mapgen: &cdda_content::mapgen::MapgenObject,
    origin: (i32, i32, i32),
    rolls: &mut Option<cdda_core_types::rng::Rolls>,
) {
    if mapgen.place_items.is_empty() {
        return;
    }
    let side = crate::overmap::SQUARES_PER_OMT;
    for entry in &mapgen.place_items {
        let times = match rolls.as_mut() {
            Some(dice) if entry.repeat.1 > entry.repeat.0 => {
                dice.rng(entry.repeat.0, entry.repeat.1)
            }
            _ => entry.repeat.0,
        };
        for _ in 0..times.max(0) {
            let spawns = match rolls.as_mut() {
                Some(dice) => roll_remainder(dice, f64::from(entry.chance) / 100.0),
                None => 0,
            };
            for _ in 0..spawns {
                let (x, y) = match rolls.as_mut() {
                    Some(dice) => (
                        dice.rng(entry.x.0, entry.x.1),
                        dice.rng(entry.y.0, entry.y.1),
                    ),
                    None => (entry.x.0, entry.y.0),
                };
                // The data's rectangles are inside the chunk; one that reaches past it is clamped away rather than written into the next room.
                if !(0..side).contains(&x) || !(0..side).contains(&y) {
                    continue;
                }
                if let Some(held) = rolls.take() {
                    crate::singletons::put_rolls(world, held);
                }
                let produced = crate::item_groups::draw(world, &entry.group);
                *rolls = crate::singletons::take_rolls(world);
                if let Some(produced) = produced {
                    for (id, amount) in produced {
                        if let Some(kind) = crate::item_registry::id_of(world, &id) {
                            for _ in 0..amount.max(0) {
                                crate::drop_item(world, (origin.0 + x, origin.1 + y), kind, 1);
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Drops the mapgen's loot: one item or group per entry, on one random square of its rectangle.
///
/// Reference: `jmapgen_loot::apply` (`mapgen.cpp:3056-3065`): `rng( 0, 99 ) < chance` is a **percentage** of placing anything at all, the square is a random point in the
/// rectangle, and there is no repeat - one entry is one roll. The item is spawned directly; the group is drawn as any other group.
fn place_loot(
    world: &mut World,
    mapgen: &cdda_content::mapgen::MapgenObject,
    origin: (i32, i32, i32),
    rolls: &mut Option<cdda_core_types::rng::Rolls>,
) {
    if mapgen.place_loot.is_empty() {
        return;
    }
    let side = crate::overmap::SQUARES_PER_OMT;
    for entry in &mapgen.place_loot {
        let placed = match rolls.as_mut() {
            Some(dice) => dice.x_in_y(f64::from(entry.chance), 100.0),
            None => false,
        };
        if !placed {
            continue;
        }
        let (x, y) = match rolls.as_mut() {
            Some(dice) => (
                dice.rng(entry.x.0, entry.x.1),
                dice.rng(entry.y.0, entry.y.1),
            ),
            None => (entry.x.0, entry.y.0),
        };
        if !(0..side).contains(&x) || !(0..side).contains(&y) {
            continue;
        }
        let at = (origin.0 + x, origin.1 + y);
        if let Some(item) = entry.item.as_deref() {
            if let Some(kind) = crate::item_registry::id_of(world, item) {
                crate::drop_item(world, at, kind, 1);
            }
            continue;
        }
        let Some(group) = entry.group.as_deref() else {
            continue;
        };
        if let Some(held) = rolls.take() {
            crate::singletons::put_rolls(world, held);
        }
        let produced = crate::item_groups::draw(world, group);
        *rolls = crate::singletons::take_rolls(world);
        if let Some(produced) = produced {
            for (id, amount) in produced {
                if let Some(kind) = crate::item_registry::id_of(world, &id) {
                    for _ in 0..amount.max(0) {
                        crate::drop_item(world, at, kind, 1);
                    }
                }
            }
        }
    }
}

/// The reference's `roll_remainder` (`mapgen.cpp:7397`): a fractional count's whole part, and the fraction as a chance of one more.
fn roll_remainder(dice: &mut cdda_core_types::rng::Rolls, value: f64) -> i32 {
    if value <= 0.0 {
        return 0;
    }
    #[expect(
        clippy::cast_possible_truncation,
        reason = "the value is positive and small: a percentage over a hundred"
    )]
    let whole = value.floor() as i32;
    #[expect(
        clippy::cast_possible_truncation,
        reason = "the fraction is between zero and one, so it fits a percentage"
    )]
    let fraction = ((value - value.floor()) * 100.0).round() as i32;
    if fraction > 0 && dice.x_in_y(f64::from(fraction), 100.0) {
        whole + 1
    } else {
        whole
    }
}

/// Draws every nested mapgen the chunk's rows name, in row order, once the outer rows are down.
fn draw_the_nested_mapgens(
    world: &mut World,
    mapgen: &cdda_content::mapgen::MapgenObject,
    origin: (i32, i32, i32),
    codes: &BTreeMap<String, u32>,
    furniture_codes: &BTreeMap<String, u32>,
    rolls: &mut Option<cdda_core_types::rng::Rolls>,
) {
    if mapgen.nested.is_empty() {
        return;
    }
    for (row, line) in mapgen.rows.iter().enumerate() {
        for (column, symbol) in line.chars().enumerate() {
            if !mapgen.nested.contains_key(&symbol) {
                continue;
            }
            let (Ok(column), Ok(row)) = (i32::try_from(column), i32::try_from(row)) else {
                continue;
            };
            let at = (origin.0 + column, origin.1 + row, origin.2);
            draw_the_nested_mapgen(
                world,
                mapgen,
                at,
                symbol,
                origin,
                codes,
                furniture_codes,
                rolls,
            );
        }
    }
}

/// Draws a nested mapgen at a symbol, and answers whether it drew one.
///
/// Reference: `jmapgen_nested::apply` (`mapgen.cpp:4488-4514`):
/// - a nested mapgen id is picked from the weighted `chunks` list;
/// - the function registered under that id is picked (by weight, in the reference - the port takes the first and names that below);
/// - `nest` applies the nested mapgen's own pieces **with the marker's offset subtracted** (`:6145`, `where.offset( -offset.raw() )`), so the nested chunk is drawn displaced:
///   its square `at - origin` lands on the marker's square and the rest extends up and left of it.
///
/// **Named gaps**: `else_chunks` and every neighbour condition (`neighbors`, `joins`, `flags`, `flags_any`, `predecessors`, `check_z`) are not evaluated; a nested mapgen id
/// registered with several weighted functions takes its first; and nesting is followed **four deep** so a data error cannot recurse for ever.
#[expect(
    clippy::too_many_arguments,
    reason = "the chunk's origin came on top of the layers a square is drawn from; the drawing context is worth its own type and is not this round's work"
)]
fn draw_the_nested_mapgen(
    world: &mut World,
    mapgen: &cdda_content::mapgen::MapgenObject,
    at: (i32, i32, i32),
    symbol: char,
    origin: (i32, i32, i32),
    codes: &BTreeMap<String, u32>,
    furniture_codes: &BTreeMap<String, u32>,
    rolls: &mut Option<cdda_core_types::rng::Rolls>,
) -> bool {
    let Some(nested) = mapgen.nested.get(&symbol) else {
        return false;
    };
    draw_a_nested_chunk(
        world,
        &nested.chunks,
        at,
        (origin.0, origin.1),
        codes,
        furniture_codes,
        rolls,
        0,
    )
}

/// Draws every `place_nested` entry: the same piece as the symbol map, with a **rectangle** for its square.
///
/// Reference: `jmapgen_nested::nest` is called with `( x.get(), y.get(), z.get() )` (`mapgen.cpp:4502`), so the piece draws one square from its own range - the symbol form
/// fixes that square to the symbol's own, the rectangle form leaves it to the dice. **1,382 uses** in the data.
fn place_the_nested_areas(
    world: &mut World,
    mapgen: &cdda_content::mapgen::MapgenObject,
    origin: (i32, i32, i32),
    codes: &BTreeMap<String, u32>,
    furniture_codes: &BTreeMap<String, u32>,
    rolls: &mut Option<cdda_core_types::rng::Rolls>,
) {
    for entry in &mapgen.place_nested {
        let (x, y) = match rolls.as_mut() {
            Some(dice) => (
                dice.rng(entry.x.0, entry.x.1),
                dice.rng(entry.y.0, entry.y.1),
            ),
            None => (entry.x.0, entry.y.0),
        };
        draw_a_nested_chunk(
            world,
            &entry.chunks,
            (origin.0 + x, origin.1 + y, origin.2),
            (origin.0, origin.1),
            codes,
            furniture_codes,
            rolls,
            0,
        );
    }
}

/// The recorder: how deep the nesting has gone, so the fourth level is the last.
const NESTING_DEPTH_LIMIT: u8 = 4;

/// Draws one nested chunk at a marker, then follows any nesting the chunk itself carries.
///
/// The offset is measured from the **outer chunk's origin** to the marker: the nested square at that distance lands on the outer square zero. That is why a nested mapgen whose
/// marker sits at the chunk's corner draws into the whole chunk, and one whose marker sits inside it draws mostly outside - which the reference's own anchoring does too.
#[expect(
    clippy::too_many_arguments,
    reason = "the layers a square is drawn from, as every other drawing helper here takes them"
)]
fn draw_a_nested_chunk(
    world: &mut World,
    chunks: &[(String, i32)],
    at: (i32, i32, i32),
    bounds: (i32, i32),
    codes: &BTreeMap<String, u32>,
    furniture_codes: &BTreeMap<String, u32>,
    rolls: &mut Option<cdda_core_types::rng::Rolls>,
    depth: u8,
) -> bool {
    if depth >= NESTING_DEPTH_LIMIT {
        return false;
    }
    let chosen = match rolls.as_mut() {
        Some(dice) => pick_by_weight(dice, chunks),
        None => chunks.first().map(|(id, _)| id.clone()),
    };
    let Some(chosen) = chosen else {
        return false;
    };
    let found = crate::singletons::holder::<TheMapgens>(world)
        .and_then(|holder| world.get::<Mapgens>(holder))
        .and_then(|mapgens| {
            mapgens
                .0
                .get(&chosen)
                .and_then(|filed| filed.first())
                .cloned()
        });
    let Some(inner) = found else {
        return false;
    };
    let mut drew = false;
    for (row, line) in inner.rows.iter().enumerate() {
        for (column, glyph) in line.chars().enumerate() {
            let (Ok(column), Ok(row)) = (i32::try_from(column), i32::try_from(row)) else {
                continue;
            };
            // **A nested chunk's rows stand at the marker plus their own square** (`jmapgen_setmap::apply` adds the offset, `mapgen.cpp:5627-5631`), and the reference clips
            // at the submap's edge - which is what makes a chunk placed near the edge lose the part that hangs over it.
            let spot = (at.0 + column, at.1 + row, at.2);
            if !(bounds.0..bounds.0 + SIDE).contains(&spot.0)
                || !(bounds.1..bounds.1 + SIDE).contains(&spot.1)
            {
                continue;
            }
            if draw_one_square(world, &inner, spot, glyph, codes, furniture_codes, rolls) {
                drew = true;
            }
            if let Some(deeper) = inner.nested.get(&glyph) {
                drew |= draw_a_nested_chunk(
                    world,
                    &deeper.chunks,
                    spot,
                    bounds,
                    codes,
                    furniture_codes,
                    rolls,
                    depth + 1,
                );
            }
        }
    }
    drew
}

/// The chunk's side, in squares.
const SIDE: i32 = 24;

/// Picks an id from a weighted list, which is the reference's `weighted_dbl_or_var_list::pick`.
fn pick_by_weight(
    dice: &mut cdda_core_types::rng::Rolls,
    entries: &[(String, i32)],
) -> Option<String> {
    let total: i32 = entries.iter().map(|(_, weight)| (*weight).max(0)).sum();
    if total <= 0 {
        return None;
    }
    let mut roll = dice.rng(0, total - 1);
    for (id, weight) in entries {
        roll -= (*weight).max(0);
        if roll < 0 {
            return Some(id.clone());
        }
    }
    entries.last().map(|(id, _)| id.clone())
}

/// Puts a gas pump on a square: the terrain it stands on and the fuel in it, and answers whether it placed one.
///
/// Reference: `jmapgen_gaspump::apply` (`mapgen.cpp:2872-2892`) and `place_gas_pump` (`:7278-7295`):
/// - the charges are the `amount` **times a hundred**, and no amount is a randomly filled pump of ten to fifty thousand charges;
/// - no `fuel` member picks **diesel one time in four** and gasoline otherwise;
/// - the square becomes **terrain**, not furniture: the pump is `t_gas_pump` or `t_diesel_pump` according to the fuel's own `pump_terrain` (`materials.json`).
fn place_the_gas_pump(
    world: &mut World,
    mapgen: &cdda_content::mapgen::MapgenObject,
    at: (i32, i32, i32),
    symbol: char,
    rolls: &mut Option<cdda_core_types::rng::Rolls>,
) -> bool {
    let Some(pump) = mapgen.gas_pumps.get(&symbol) else {
        return false;
    };
    let mut charges = match rolls.as_mut() {
        Some(dice) if pump.amount.1 > pump.amount.0 => dice.rng(pump.amount.0, pump.amount.1),
        _ => pump.amount.0,
    }
    .max(0)
        * FUEL_CHARGES_PER_UNIT;
    if charges == 0 {
        charges = match rolls.as_mut() {
            Some(dice) => dice.rng(EMPTY_PUMP_LOW, EMPTY_PUMP_HIGH),
            None => EMPTY_PUMP_LOW,
        };
    }
    let fuel = if let Some(named) = pump.fuel.as_deref() {
        named.to_owned()
    } else {
        // `one_in( 4 ) ? itype_diesel : itype_gasoline` (`mapgen.cpp:7294`): drawn once, because the draw is the decision.
        let diesel = rolls.as_mut().is_some_and(|dice| dice.one_in(4));
        if diesel {
            "diesel".to_owned()
        } else {
            "gasoline".to_owned()
        }
    };
    let terrain = match fuel.as_str() {
        "diesel" => "t_diesel_pump",
        "jp8" => "t_jp8_pump",
        "avgas" => "t_avgas_pump",
        _ => "t_gas_pump",
    };
    if let Some(code) = crate::terrain_types::code_of(world, terrain) {
        crate::square::set_terrain(world, at, code);
    }
    if let Some(kind) = crate::item_registry::id_of(world, &fuel) {
        crate::drop_item(world, (at.0, at.1), kind, charges);
    }
    true
}

/// The reference's `charges = rng( 10000, 50000 )` for a pump the data gives no amount (`mapgen.cpp:2879-2881`).
const EMPTY_PUMP_LOW: i32 = 10_000;
/// The top of the same range.
const EMPTY_PUMP_HIGH: i32 = 50_000;

/// Writes a sign: the furniture and the words on it, and answers whether it wrote any.
///
/// Reference: `jmapgen_sign::apply` (`mapgen.cpp:2701-2721`): the furniture (default `f_sign`) goes on the square and `set_signage` stores the text. The reference also
/// replaces tags such as a city name in that text (`apply_all_tags`, `:2719`), which the port has no overmap city for - the words are stored as written.
fn place_the_sign(
    world: &mut World,
    mapgen: &cdda_content::mapgen::MapgenObject,
    at: (i32, i32, i32),
    symbol: char,
) -> bool {
    let Some(sign) = mapgen.signs.get(&symbol) else {
        return false;
    };
    place_one_sign(world, &sign.furniture, &sign.text, at);
    true
}

/// Places a vending machine: the furniture its flags choose, and the stock it holds, and answers whether it placed any.
///
/// Reference: `jmapgen_vending_machine::apply` (`mapgen.cpp:2803-2812`) calls `place_vending` (`:7306-7336`), which sets one of six furniture ids and then stocks the square
/// with the machine's item group (`place_items( type, 100, p, p, ... )` - one hundred per cent is **one draw**, as in every other `place_items` call).
///
/// **Named gap**: a `lootable` machine may be found ransacked - the reference's `!one_in( days_since_cataclysm + 4 )` branch (`:7327-7335`) bashes the machine and scatters
/// glass instead of stocking it. The port stocks every machine; that branch needs the day count and a bash, and the day count is a calendar read the writer does not do.
fn place_the_vending_machine(
    world: &mut World,
    mapgen: &cdda_content::mapgen::MapgenObject,
    at: (i32, i32, i32),
    symbol: char,
    rolls: &mut Option<cdda_core_types::rng::Rolls>,
) -> bool {
    let Some(machine) = mapgen.vending_machines.get(&symbol) else {
        return false;
    };
    let furniture = match (machine.powered, machine.reinforced, machine.networked) {
        (false, false, _) => "f_vending_c_off",
        (false, true, _) => "f_vending_reinforced_off",
        (true, false, true) => "f_vending_c_networked",
        (true, false, false) => "f_vending_c",
        (true, true, true) => "f_vending_reinforced_networked",
        (true, true, false) => "f_vending_reinforced",
    };
    if let Some(code) = crate::furniture_types::code_of(world, furniture) {
        crate::square::set_furniture(world, at, code);
    }
    if let Some(held) = rolls.take() {
        crate::singletons::put_rolls(world, held);
    }
    let produced = crate::item_groups::draw(world, &machine.group);
    *rolls = crate::singletons::take_rolls(world);
    if let Some(produced) = produced {
        for (id, amount) in produced {
            if let Some(kind) = crate::item_registry::id_of(world, &id) {
                for _ in 0..amount.max(0) {
                    crate::drop_item(world, (at.0, at.1), kind, 1);
                }
            }
        }
    }
    true
}

/// Places what a symbol's sealed furniture holds, and answers whether it placed any.
///
/// Reference: `jmapgen_sealed_item::apply` (`mapgen.cpp:4049-4060`). The **chance** is a percentage of the whole entry, checked **before** anything is placed; the contents
/// follow; the furniture goes on top last. An entry with neither an item nor a group is one the reference complains about and the reader refuses, so a sealed square always
/// holds something.
fn place_the_sealed_item(
    world: &mut World,
    mapgen: &cdda_content::mapgen::MapgenObject,
    at: (i32, i32, i32),
    symbol: char,
    rolls: &mut Option<cdda_core_types::rng::Rolls>,
) -> bool {
    let Some(sealed) = mapgen.sealed_items.get(&symbol) else {
        return false;
    };
    if sealed.chance < 100
        && !rolls
            .as_mut()
            .is_none_or(|rolls| rolls.x_in_y(f64::from(sealed.chance), 100.0))
    {
        return false;
    }
    if let Some(item) = sealed.item.as_ref()
        && rolls
            .as_mut()
            .is_none_or(|rolls| item.chance >= 100 || rolls.x_in_y(f64::from(item.chance), 100.0))
        && let Some(kind) = crate::item_registry::id_of(world, &item.id)
    {
        let count = match rolls.as_mut() {
            Some(rolls) => rolls.rng(item.amount.0, item.amount.1),
            None => item.amount.0,
        };
        for _ in 0..count.max(0) {
            crate::drop_item(world, (at.0, at.1), kind, 1);
        }
    }
    if let Some(group) = sealed.group.as_ref()
        && rolls
            .as_mut()
            .is_none_or(|rolls| group.chance >= 100 || rolls.x_in_y(f64::from(group.chance), 100.0))
    {
        if let Some(held) = rolls.take() {
            crate::singletons::put_rolls(world, held);
        }
        let produced = crate::item_groups::draw(world, &group.group);
        *rolls = crate::singletons::take_rolls(world);
        if let Some(produced) = produced {
            for (id, amount) in produced {
                if let Some(kind) = crate::item_registry::id_of(world, &id) {
                    for _ in 0..amount.max(0) {
                        crate::drop_item(world, (at.0, at.1), kind, 1);
                    }
                }
            }
        }
    }
    // The furniture goes on **last**, over what it holds.
    if let Some(code) = crate::furniture_types::code_of(world, &sealed.furniture) {
        crate::square::set_furniture(world, at, code);
    }
    true
}

/// Places a computer terminal: its console furniture and the record it holds, and answers whether it placed one.
///
/// Reference: `jmapgen_computer::apply` (`mapgen.cpp:3878-3905`): the furniture is `f_console`, and `add_computer` stores the record with its name, security and options -
/// which is what a later hacking pass reads. The port stores the record as a component on the square, so the furniture and the terminal are two things a square carries.
fn place_the_computer(
    world: &mut World,
    mapgen: &cdda_content::mapgen::MapgenObject,
    at: (i32, i32, i32),
    symbol: char,
) -> bool {
    let Some(computer) = mapgen.computers.get(&symbol) else {
        return false;
    };
    if let Some(code) = crate::furniture_types::code_of(world, "f_console") {
        crate::square::set_furniture(world, at, code);
    }
    crate::computer_terminal::set(
        world,
        at,
        crate::computer_terminal::Computer {
            name: computer.name.clone(),
            access_denied: computer.access_denied.clone(),
            security: computer.security,
            options: computer
                .options
                .iter()
                .map(|(name, action)| crate::computer_terminal::TerminalOption {
                    name: name.clone(),
                    action: action.clone(),
                })
                .collect(),
            eocs: computer.eocs.clone(),
            chat_topics: computer.chat_topics.clone(),
        },
    );
    true
}

/// Places every `set` point entry: `repeat` draws from the rectangle, each writing one layer.
///
/// Reference: `jmapgen_setmap::apply` (`mapgen.cpp:5622-5640`): a **chance of one in n** (`one_in( chance )`), a `repeat`, and a square drawn from the entry's range - the
/// same shape as the area setters, which is why this reuses their spot rule instead of inventing a second one.
fn place_set_points(
    world: &mut World,
    mapgen: &cdda_content::mapgen::MapgenObject,
    origin: (i32, i32, i32),
    rolls: &mut Option<cdda_core_types::rng::Rolls>,
) {
    for entry in &mapgen.set_points {
        let count = match rolls.as_mut() {
            Some(dice) if entry.repeat.1 > entry.repeat.0 => {
                dice.rng(entry.repeat.0, entry.repeat.1)
            }
            _ => entry.repeat.0,
        }
        .max(0);
        for _ in 0..count {
            let corner = roll_a_square(rolls, entry.x, entry.y);
            let opposite = roll_a_square(rolls, entry.x2, entry.y2);
            let drawn = match entry.shape {
                cdda_content::mapgen::SetShape::Point => crate::mapgen_shapes::point(corner),
                cdda_content::mapgen::SetShape::Line => {
                    crate::mapgen_shapes::line(corner, opposite)
                }
                cdda_content::mapgen::SetShape::Square => {
                    crate::mapgen_shapes::square(corner, opposite)
                }
            };
            for (x, y) in drawn {
                if !(0..SIDE).contains(&x) || !(0..SIDE).contains(&y) {
                    continue;
                }
                write_the_layer(world, entry, (origin.0 + x, origin.1 + y, origin.2));
            }
        }
    }
}

/// Places the rubble a symbol carries, and answers whether it placed any.
fn place_the_rubble(
    world: &mut World,
    mapgen: &cdda_content::mapgen::MapgenObject,
    at: (i32, i32, i32),
    symbol: char,
    rolls: &mut Option<cdda_core_types::rng::Rolls>,
) -> bool {
    let Some(placement) = mapgen.rubble.get(&symbol) else {
        return false;
    };
    make_rubble(world, at, placement, rolls);
    true
}

/// Places every `place_rubble` entry.
fn place_rubble_areas(
    world: &mut World,
    mapgen: &cdda_content::mapgen::MapgenObject,
    origin: (i32, i32, i32),
    rolls: &mut Option<cdda_core_types::rng::Rolls>,
) {
    for entry in &mapgen.place_rubble {
        let (x, y) = roll_a_square(rolls, entry.x, entry.y);
        if !(0..SIDE).contains(&x) || !(0..SIDE).contains(&y) {
            continue;
        }
        make_rubble(
            world,
            (origin.0 + x, origin.1 + y, origin.2),
            &entry.rubble,
            rolls,
        );
    }
}

/// Makes one square of rubble, the way `map::make_rubble` does (`map.cpp:3239-3291`).
///
/// Two branches: `overwrite` forces the floor and the furniture; otherwise the furniture is set and the floor is left alone. The debris depends on the furniture - **the three
/// hardcoded lists** the reference keeps in this one function (`:3268-3290`), and the port copies the lists rather than inventing a table for them.
fn make_rubble(
    world: &mut World,
    at: (i32, i32, i32),
    placement: &cdda_content::mapgen::RubblePlacement,
    rolls: &mut Option<cdda_core_types::rng::Rolls>,
) {
    if placement.overwrite
        && let Some(code) = crate::terrain_types::code_of(world, &placement.floor_type)
    {
        crate::square::set_terrain(world, at, code);
    }
    if let Some(code) = crate::furniture_types::code_of(world, &placement.rubble_type) {
        crate::square::set_furniture(world, at, code);
    }
    if !placement.items {
        return;
    }
    // The reference's three lists, by furniture id (`map.cpp:3269-3290`).
    match placement.rubble_type.as_str() {
        "f_wreckage" => {
            drop_if_known(world, at, "steel_chunk", 1);
            drop_if_known(world, at, "scrap", 1);
            if rolls.as_mut().is_some_and(|dice| dice.one_in(5)) {
                drop_if_known(world, at, "pipe", 1);
                drop_if_known(world, at, "wire", 1);
            }
        }
        "f_rubble_rock" => {
            let count = roll_between(rolls, (1, 3));
            for _ in 0..count {
                drop_if_known(world, at, "rock", 1);
            }
        }
        _ => {
            let splinters = roll_between(rolls, (2, 8));
            for _ in 0..splinters {
                drop_if_known(world, at, "splinter", 1);
            }
            let nails = roll_between(rolls, (20, 50));
            if nails > 0 {
                drop_if_known(world, at, "nail", nails);
            }
        }
    }
}

/// Draws a number from a range, or its first end when the dice are spent.
fn roll_between(rolls: &mut Option<cdda_core_types::rng::Rolls>, range: (i32, i32)) -> i32 {
    match rolls.as_mut() {
        Some(dice) if range.1 > range.0 => dice.rng(range.0, range.1),
        _ => range.0,
    }
}

/// Drops an item when the registry knows it, which is what a hardcoded id needs: the reference's ids are the data's, and a port without one is not a crash.
fn drop_if_known(world: &mut World, at: (i32, i32, i32), id: &str, charges: i32) {
    if let Some(kind) = crate::item_registry::id_of(world, id) {
        crate::drop_item(world, (at.0, at.1), kind, charges);
    }
}

/// Names the squares the data asks to be named, in the **global** value store.
///
/// Reference: `queued_points` are collected while the map is drawn and written into `global_variables` at the very end (`mapgen.cpp:5737`, `6098`, `8762-8769`), so a
/// variable names a square of the world and every later reader - a mission target, a teleporter, a dialogue - reads it from one place. The port already has that one place:
/// the global values singleton, which is why this writes there rather than starting a store of its own.
fn place_set_variables(
    world: &mut World,
    mapgen: &cdda_content::mapgen::MapgenObject,
    origin: (i32, i32, i32),
    rolls: &mut Option<cdda_core_types::rng::Rolls>,
) {
    if mapgen.set_variables.is_empty() {
        return;
    }
    let holder = crate::dialogue_values::ensure_global(world);
    for entry in &mapgen.set_variables {
        let (x, y) = roll_a_square(rolls, entry.x, entry.y);
        if !(0..SIDE).contains(&x) || !(0..SIDE).contains(&y) {
            continue;
        }
        crate::dialogue_values::set(
            world,
            holder,
            &entry.name,
            crate::dialogue_values::Value::Position {
                x: origin.0 + x,
                y: origin.1 + y,
                z: origin.2,
            },
        );
    }
}

/// Places every `set` radiation entry: `repeat` squares from the range, each set to a dose the dice answer.
///
/// Reference: `jmapgen_setmap::apply`'s `JMAPGEN_SETMAP_RADIATION` arm (`mapgen.cpp:5676-5679`): `m.set_radiation( target_pos, val.get() )` - an **absolute** write, and its
/// range is the dose's, not the square's.
fn place_set_radiation(
    world: &mut World,
    mapgen: &cdda_content::mapgen::MapgenObject,
    origin: (i32, i32, i32),
    rolls: &mut Option<cdda_core_types::rng::Rolls>,
) {
    for entry in &mapgen.set_radiation {
        let count = match rolls.as_mut() {
            Some(dice) if entry.repeat.1 > entry.repeat.0 => {
                dice.rng(entry.repeat.0, entry.repeat.1)
            }
            _ => entry.repeat.0,
        }
        .max(0);
        for _ in 0..count {
            let (x, y) = roll_a_square(rolls, entry.x, entry.y);
            let dose = match rolls.as_mut() {
                Some(dice) if entry.amount.1 > entry.amount.0 => {
                    dice.rng(entry.amount.0, entry.amount.1)
                }
                _ => entry.amount.0,
            };
            if !(0..SIDE).contains(&x) || !(0..SIDE).contains(&y) {
                continue;
            }
            crate::radiation::set(world, (origin.0 + x, origin.1 + y, origin.2), dose);
        }
    }
}

/// Takes every item off a square (`map::i_clear`, `mapgen.cpp:5688-5690`).
fn clear_the_items(world: &mut World, at: (i32, i32, i32)) {
    for item in crate::items_at(world, (at.0, at.1)) {
        world.despawn(item);
    }
}

/// Takes every field off a square (`mapgen.cpp:5691-5693`).
///
/// The reference calls a helper that walks the square's field list; the port's fields are separate components on entities at the square, so the walk is the same query by a
/// different name. A field's strength is the thing carried, and a square with fields of several kinds loses all of them - which is what "clear the square" means.
fn clear_the_fields(world: &mut World, at: (i32, i32, i32)) {
    let fields: Vec<Entity> = {
        let mut query = world.query::<(Entity, &crate::Position, &crate::Field)>();
        query
            .iter(world)
            .filter(|(_, spot, _)| spot.x == at.0 && spot.y == at.1 && spot.z == at.2)
            .map(|(entity, _, _)| entity)
            .collect()
    };
    for field in fields {
        world.despawn(field);
    }
}

/// Kills whatever creature stands on a square, unless it is the player (`mapgen.cpp:5678-5685`).
fn kill_the_creature(world: &mut World, at: (i32, i32, i32)) {
    for creature in crate::creature::creature_at(world, at) {
        if world.get::<crate::missions::TheAvatar>(creature).is_some()
            || world.get::<crate::Player>(creature).is_some()
        {
            continue;
        }
        world.despawn(creature);
    }
}

/// Draws one square from a range, or its first end when the dice are spent.
fn roll_a_square(
    rolls: &mut Option<cdda_core_types::rng::Rolls>,
    x: (i32, i32),
    y: (i32, i32),
) -> (i32, i32) {
    match rolls.as_mut() {
        Some(dice) => (dice.rng(x.0, x.1), dice.rng(y.0, y.1)),
        None => (x.0, y.0),
    }
}

/// Writes one layer of a `set` entry on one square.
fn write_the_layer(world: &mut World, entry: &cdda_content::mapgen::SetPoint, at: (i32, i32, i32)) {
    match entry.kind {
        cdda_content::mapgen::SetKind::Terrain => {
            if let Some(code) = crate::terrain_types::code_of(world, &entry.id) {
                crate::square::set_terrain(world, at, code);
            }
        }
        cdda_content::mapgen::SetKind::Furniture => {
            if let Some(code) = crate::furniture_types::code_of(world, &entry.id) {
                crate::square::set_furniture(world, at, code);
            }
        }
        cdda_content::mapgen::SetKind::Trap => {
            crate::square::set_trap(world, at, &entry.id);
        }
        // Reference: `jmapgen_setmap::apply`'s removal arms (`mapgen.cpp:5627-5655`). A removal names no id, and each clears one thing on one square:
        // the trap, the items, the fields, or the creature standing there.
        cdda_content::mapgen::SetKind::TrapRemove => crate::square::remove_trap(world, at),
        cdda_content::mapgen::SetKind::ItemRemove => clear_the_items(world, at),
        cdda_content::mapgen::SetKind::FieldRemove => clear_the_fields(world, at),
        cdda_content::mapgen::SetKind::CreatureRemove => kill_the_creature(world, at),
    }
}

/// Places every `place_item` entry: the `item` symbol map with the square written out.
fn place_item_areas(
    world: &mut World,
    mapgen: &cdda_content::mapgen::MapgenObject,
    origin: (i32, i32, i32),
    rolls: &mut Option<cdda_core_types::rng::Rolls>,
) {
    for entry in &mapgen.place_item {
        let (x, y) = match rolls.as_mut() {
            Some(dice) => (
                dice.rng(entry.x.0, entry.x.1),
                dice.rng(entry.y.0, entry.y.1),
            ),
            None => (entry.x.0, entry.y.0),
        };
        place_one_item(
            world,
            &entry.item,
            entry.amount,
            entry.chance,
            (origin.0 + x, origin.1 + y, origin.2),
            rolls,
        );
    }
}

/// Places every `place_signs` entry: the `signs` symbol map with the square written out.
fn place_sign_areas(
    world: &mut World,
    mapgen: &cdda_content::mapgen::MapgenObject,
    origin: (i32, i32, i32),
    rolls: &mut Option<cdda_core_types::rng::Rolls>,
) {
    for entry in &mapgen.place_signs {
        let (x, y) = match rolls.as_mut() {
            Some(dice) => (
                dice.rng(entry.x.0, entry.x.1),
                dice.rng(entry.y.0, entry.y.1),
            ),
            None => (entry.x.0, entry.y.0),
        };
        place_one_sign(
            world,
            &entry.furniture,
            &entry.text,
            (origin.0 + x, origin.1 + y, origin.2),
        );
    }
}

/// The one shape of the single-item rule, whichever form the data wrote.
fn place_one_item(
    world: &mut World,
    item: &str,
    amount: (i32, i32),
    chance: i32,
    at: (i32, i32, i32),
    rolls: &mut Option<cdda_core_types::rng::Rolls>,
) {
    let spawns = if chance == 100 {
        1
    } else {
        match rolls.as_mut() {
            Some(dice) => roll_remainder(dice, f64::from(chance) / 100.0),
            None => 0,
        }
    };
    let Some(kind) = crate::item_registry::id_of(world, item) else {
        return;
    };
    for _ in 0..spawns {
        let count = match rolls.as_mut() {
            Some(dice) => dice.rng(amount.0, amount.1),
            None => amount.0,
        };
        for _ in 0..count.max(0) {
            crate::drop_item(world, (at.0, at.1), kind, 1);
        }
    }
}

/// The one shape of the sign rule, whichever form the data wrote.
fn place_one_sign(world: &mut World, furniture: &str, text: &str, at: (i32, i32, i32)) {
    if let Some(code) = crate::furniture_types::code_of(world, furniture) {
        crate::square::set_furniture(world, at, code);
    }
    if !text.is_empty() {
        crate::signage::set_text(world, at, text);
    }
}

/// Places the single item a symbol carries, and answers whether it placed any.
///
/// Reference: `jmapgen_spawn_item::apply` (`mapgen.cpp:3500-3520`): a chance of a hundred is **one** spawn; anything less is `roll_remainder( chance / 100 )` spawns, and each
/// spawn drops `amount` items. The `repeat` member multiplies the whole thing, as every piece's does.
fn place_the_single_item(
    world: &mut World,
    mapgen: &cdda_content::mapgen::MapgenObject,
    at: (i32, i32, i32),
    symbol: char,
    rolls: &mut Option<cdda_core_types::rng::Rolls>,
) -> bool {
    let Some(placement) = mapgen.items_single.get(&symbol) else {
        return false;
    };
    place_one_item(
        world,
        &placement.item,
        placement.amount,
        placement.chance,
        at,
        rolls,
    );
    true
}

/// Places the field patch a symbol carries, and answers whether it placed one.
///
/// Reference: `jmapgen_field::apply` (`mapgen.cpp:2564-2581`): a **percentage** chance, and a strength picked from the list the data writes. **Named gap**: the `remove`
/// member, which takes a field away rather than adding one, and `age` are not read.
fn place_the_field_symbol(
    world: &mut World,
    mapgen: &cdda_content::mapgen::MapgenObject,
    at: (i32, i32, i32),
    symbol: char,
    rolls: &mut Option<cdda_core_types::rng::Rolls>,
) -> bool {
    let Some(patch) = mapgen.field_symbols.get(&symbol) else {
        return false;
    };
    let Some(field) = crate::field_registry::id_of(world, &patch.field) else {
        return false;
    };
    if patch.chance < 100
        && !rolls
            .as_mut()
            .is_none_or(|dice| dice.x_in_y(f64::from(patch.chance), 100.0))
    {
        return false;
    }
    let strength = match rolls.as_mut() {
        Some(dice) if patch.intensity.1 > patch.intensity.0 => {
            dice.rng(patch.intensity.0, patch.intensity.1)
        }
        _ => patch.intensity.0,
    };
    crate::add_field_strength(world, at, field, strength);
    true
}

/// Places the liquid a symbol carries, and answers whether it placed any.
///
/// Reference: `jmapgen_liquid_item::apply` (`mapgen.cpp:2911-2943`): **one chance in `chance`** of the liquid being there at all, charges rolled from the amount unless the
/// amount is `-1` (which leaves the item type's own count alone), and a fuel's charges multiplied by **one hundred** - the reference measures fuel in hundreds of charges and
/// says so in the same block.
fn place_the_liquid(
    world: &mut World,
    mapgen: &cdda_content::mapgen::MapgenObject,
    at: (i32, i32, i32),
    symbol: char,
    rolls: &mut Option<cdda_core_types::rng::Rolls>,
) -> bool {
    let Some(placement) = mapgen.liquids.get(&symbol) else {
        return false;
    };
    place_one_liquid(
        world,
        &placement.liquid,
        placement.amount,
        placement.chance,
        at,
        rolls,
    )
}

/// The one shape of the liquid rule, whichever form the data wrote: the one-in-n chance, the amount, and the hundred that fuels get.
fn place_one_liquid(
    world: &mut World,
    liquid: &str,
    amount: (i32, i32),
    chance: i32,
    at: (i32, i32, i32),
    rolls: &mut Option<cdda_core_types::rng::Rolls>,
) -> bool {
    // `one_in( chance.get() )`: a chance of one is always, and a bigger number is rarer - the opposite of the item group's percentage.
    if chance > 1 {
        let Some(dice) = rolls.as_mut() else {
            return false;
        };
        if !dice.one_in(chance) {
            return false;
        }
    }
    let Some(kind) = crate::item_registry::id_of(world, liquid) else {
        return false;
    };
    let mut charges = if amount.0 > -1 {
        match rolls.as_mut() {
            Some(dice) if amount.1 > amount.0 => dice.rng(amount.0, amount.1),
            _ => amount.0,
        }
    } else {
        // The item type's own count, which the port's registry does not carry: one charge is the count every liquid in the data that omits an amount has.
        ONE_CHARGE
    };
    if matches!(liquid, "gasoline" | "avgas" | "diesel" | "jp8") {
        charges *= FUEL_CHARGES_PER_UNIT;
    }
    if charges > 0 {
        crate::drop_item(world, (at.0, at.1), kind, charges);
    }
    true
}

/// Places every `place_liquids` entry: the `liquids` symbol map with the square written out.
fn place_liquid_areas(
    world: &mut World,
    mapgen: &cdda_content::mapgen::MapgenObject,
    origin: (i32, i32, i32),
    rolls: &mut Option<cdda_core_types::rng::Rolls>,
) {
    for entry in &mapgen.place_liquids {
        let (x, y) = match rolls.as_mut() {
            Some(dice) => (
                dice.rng(entry.x.0, entry.x.1),
                dice.rng(entry.y.0, entry.y.1),
            ),
            None => (entry.x.0, entry.y.0),
        };
        place_one_liquid(
            world,
            &entry.liquid,
            entry.amount,
            entry.chance,
            (origin.0 + x, origin.1 + y, origin.2),
            rolls,
        );
    }
}

/// The reference's `migrated == itype_gasoline || avgas || diesel || jp8` branch multiplies a fuel's charges by this (`mapgen.cpp:2929-2934`).
const FUEL_CHARGES_PER_UNIT: i32 = 100;

/// What a liquid's charges are when the data gives no amount and the item type's own count is unavailable (`mapgen.cpp:2923`): the reference leaves the count alone.
const ONE_CHARGE: i32 = 1;

/// Places the items a symbol carries, and answers whether it placed any. Reference: `jmapgen_item_group::apply` (`mapgen.cpp:5360-5400`).
fn place_the_items(
    world: &mut World,
    mapgen: &cdda_content::mapgen::MapgenObject,
    at: (i32, i32, i32),
    symbol: char,
    rolls: &mut Option<cdda_core_types::rng::Rolls>,
) -> bool {
    let Some(placement) = mapgen.items.get(&symbol) else {
        return false;
    };
    if placement.chance < 100
        && !rolls
            .as_mut()
            .is_none_or(|rolls| rolls.x_in_y(f64::from(placement.chance), 100.0))
    {
        return false;
    }
    let count = match rolls.as_mut() {
        Some(rolls) => rolls.rng(placement.amount.0, placement.amount.1),
        None => placement.amount.0,
    }
    .max(0);
    if let Some(item) = placement.item.as_deref() {
        let Some(kind) = crate::item_registry::id_of(world, item) else {
            return false;
        };
        for _ in 0..count {
            crate::drop_item(world, (at.0, at.1), kind, 1);
        }
        return true;
    }
    let Some(group) = placement.group.as_deref() else {
        return false;
    };
    // **A group is drawn once per amount.** The group's own draw takes the dice, so the writer hands them back first and picks them up again after: one generator, one holder at
    // a time, and no second copy of the dice anywhere.
    for _ in 0..count {
        if let Some(held) = rolls.take() {
            crate::singletons::put_rolls(world, held);
        }
        let produced = crate::item_groups::draw(world, group);
        *rolls = crate::singletons::take_rolls(world);
        if let Some(produced) = produced {
            for (id, amount) in produced {
                if let Some(kind) = crate::item_registry::id_of(world, &id) {
                    for _ in 0..amount.max(0) {
                        crate::drop_item(world, (at.0, at.1), kind, 1);
                    }
                }
            }
        }
    }
    true
}

/// Spawns the monsters a mapgen places, answering nothing because a monster is not a drawn square.
///
/// Reference: `jmapgen_monster::apply` (`mapgen.cpp:5560-5610`): an entry names a square or a range of them, a chance out of a hundred, and a `pack_size` the dice answer
/// between - and each member of the pack rolls its own square inside the range, which is why a pack spreads rather than piling on one tile.
fn place_monsters(
    world: &mut World,
    mapgen: &cdda_content::mapgen::MapgenObject,
    origin: (i32, i32, i32),
    rolls: &mut Option<cdda_core_types::rng::Rolls>,
) {
    for placement in &mapgen.monsters {
        let x = match rolls.as_mut() {
            Some(rolls) => rolls.rng(placement.x.0, placement.x.1),
            None => placement.x.0,
        };
        let y = match rolls.as_mut() {
            Some(rolls) => rolls.rng(placement.y.0, placement.y.1),
            None => placement.y.0,
        };
        spawn_one(
            world,
            &Spawn {
                monster: placement.monster.clone(),
                group: placement.group.clone(),
                chance: placement.chance,
                pack_size: placement.pack_size,
                friendly: placement.friendly,
            },
            (origin.0 + x, origin.1 + y),
            rolls,
        );
    }
}

/// Places the monster a symbol carries, and answers whether it placed one.
///
/// The square is the symbol's, so this is [`place_monsters`] with one rectangle replaced by a point - the same `jmapgen_monster`, the same chance, the same pack.
fn place_the_monster_symbol(
    world: &mut World,
    mapgen: &cdda_content::mapgen::MapgenObject,
    at: (i32, i32, i32),
    symbol: char,
    rolls: &mut Option<cdda_core_types::rng::Rolls>,
) -> bool {
    let Some(entry) = mapgen.monster_symbols.get(&symbol) else {
        return false;
    };
    spawn_one(
        world,
        &Spawn {
            monster: entry.monster.clone(),
            group: entry.group.clone(),
            chance: entry.chance,
            pack_size: entry.pack_size,
            friendly: entry.friendly,
        },
        (at.0, at.1),
        rolls,
    );
    true
}

/// The one shape the spawn rule takes, whichever way the data wrote it.
struct Spawn {
    /// The monster id, when the entry names one.
    monster: Option<String>,
    /// The monster group, when the entry names one instead.
    group: Option<String>,
    /// How likely the spawn is at all, out of a hundred.
    chance: i32,
    /// How many, as a range.
    pack_size: (i32, i32),
    /// Whether the spawn is on the player's side.
    friendly: bool,
}

/// Applies the spawn rule once: the chance, the pack, the group draw and the friendly flag.
fn spawn_one(
    world: &mut World,
    spawn: &Spawn,
    at: (i32, i32),
    rolls: &mut Option<cdda_core_types::rng::Rolls>,
) {
    if spawn.chance < 100
        && !rolls
            .as_mut()
            .is_none_or(|rolls| rolls.x_in_y(f64::from(spawn.chance), 100.0))
    {
        return;
    }
    let count = match rolls.as_mut() {
        Some(rolls) => rolls.rng(spawn.pack_size.0, spawn.pack_size.1),
        None => spawn.pack_size.0,
    }
    .max(0);
    // **An entry may name a group instead of a monster** (`mapgen.cpp:3110-3120`, `3266-3275`): `jmapgen_monster` reads `group` and calls the group's own draw, which
    // answers one member per `apply` - so the pack below is the group's answer repeated, not a draw per member.
    let monster = if let Some(monster) = spawn.monster.as_deref() {
        monster.to_owned()
    } else {
        let Some(group) = spawn.group.as_deref() else {
            return;
        };
        let Some(rolls) = rolls.as_mut() else {
            return;
        };
        let Some(picked) = crate::monster_groups::pick(world, group, rolls) else {
            return;
        };
        picked
    };
    let Some(kind) = crate::monster_registry::id_of(world, &monster) else {
        return;
    };
    let hp = crate::monster_registry::hit_points(world, kind);
    for _ in 0..count {
        let made = crate::spawn_monster(world, at, kind, hp);
        // Reference: `add_spawn( ..., friendly, ... )` (`mapgen.cpp:3268-3270`) - a monster a mapgen marks friendly is on the player's side for good.
        if spawn.friendly {
            crate::friendly::make_friendly(world, made, crate::friendly::FOR_EVER);
        }
    }
}

/// Places the fields a mapgen scatters, answering how many patches landed.
///
/// Reference: `mapgen::place_fields` (`mapgen.cpp`): `repeat` patches on squares chosen inside the rectangle, each at a strength rolled between
/// the two ends - and a patch on a square that already holds the field makes it stronger rather than replacing it.
pub fn place_fields(world: &mut World, update: &str, origin: (i32, i32, i32)) -> Option<usize> {
    let mapgen = crate::singletons::holder::<TheMapgens>(world)
        .and_then(|holder| world.get::<Mapgens>(holder))
        // An update mapgen is filed once; a terrain's several mapgens are not this path's business, so the first is the one an update writes.
        .and_then(|mapgens| {
            mapgens
                .0
                .get(update)
                .and_then(|filed| filed.first())
                .cloned()
        })?;
    if mapgen.fields.is_empty() {
        return Some(0);
    }
    let mut placed = 0;
    for placement in &mapgen.fields {
        let Some(field) = crate::field_registry::id_of(world, &placement.field) else {
            continue;
        };
        for _ in 0..placement.repeat.max(0) {
            let mut dice = crate::singletons::take_rolls(world);
            let drawn = dice.as_mut().map(|dice| {
                (
                    dice.rng(placement.x.0, placement.x.1),
                    dice.rng(placement.y.0, placement.y.1),
                    dice.rng(placement.intensity.0, placement.intensity.1),
                )
            });
            if let Some(dice) = dice.take() {
                crate::singletons::put_rolls(world, dice);
            }
            let Some((x, y, strength)) = drawn else {
                return Some(placed);
            };
            crate::add_field_strength(
                world,
                (origin.0 + x, origin.1 + y, origin.2),
                field,
                strength,
            );
            placed += 1;
        }
    }
    Some(placed)
}

/// A row or column as a world offset, or nothing when it is too large to be one.
fn offset(index: usize) -> Option<i32> {
    i32::try_from(index).ok()
}

/// How many mapgens the world holds.
#[must_use]
pub fn census(world: &World) -> usize {
    crate::singletons::holder::<TheMapgens>(world)
        .and_then(|holder| world.get::<Mapgens>(holder))
        .map_or(0, |mapgens| mapgens.0.len())
}
