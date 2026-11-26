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
    // **The parameters are drawn before the rotation**, because the reference draws every parameter once when it resolves the palette (`mapgen.cpp:440-470`, `:5176-5180`)
    // and every symbol that names one reads that single value for the whole chunk - which is what makes a variant mapgen **coherent**: one wall type, not one per square.
    let resolved = resolve_parameters(mapgen, rolls.as_mut());
    let mapgen = &resolved;
    // **The rotation is drawn next**, because the reference draws it before it draws anything else (`mapgen.cpp:6072`) - and it is a dice roll, so where it comes in the
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
    // A chunk fills **its own size**, which is the overmap tile's 24 unless it declares a smaller one (`mapgen.cpp:5105-5110`, `:5130-5140`).
    let (side, side_y) = mapgen.size();
    let filled = mapgen
        .fill_ter
        .as_deref()
        .and_then(|fill| codes.get(fill).copied());
    if let Some(code) = filled {
        for column in 0..side {
            for row in 0..side_y {
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
        usize::try_from(side * side_y).unwrap_or(0)
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
    place_the_zones(world, mapgen, origin, &mut rolls);
    spawn_by_density(world, mapgen, origin, &mut rolls);
    park_the_vehicles(world, mapgen, origin, &mut rolls);
    apply_the_transforms(world, mapgen, origin, &mut rolls);
    write_the_graffiti(world, mapgen, origin, &mut rolls);
    place_the_corpses(world, mapgen, origin, &mut rolls);
    own_what_the_mapgen_names(world, mapgen, origin);
    place_the_npcs(world, mapgen, origin, &mut rolls);
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
        .and_then(cdda_content::mapgen::Piece::id)
        .and_then(|terrain| codes.get(terrain).copied())
    {
        crate::square::set_terrain(world, at, code);
        drew = true;
    }
    if let Some(code) = mapgen
        .furniture
        .get(&symbol)
        .and_then(cdda_content::mapgen::Piece::id)
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
/// rectangle. The reference also uncaches vehicles here, which the port has no cache for - named, and empty on purpose.
fn remove_everything(
    world: &mut World,
    mapgen: &cdda_content::mapgen::MapgenObject,
    origin: (i32, i32, i32),
) {
    for area in &mapgen.remove_all {
        for x in area.x.0..=area.x.1 {
            for y in area.y.0..=area.y.1 {
                if !in_bounds(mapgen, x, y) {
                    continue;
                }
                let at = (origin.0 + x, origin.1 + y, origin.2);
                crate::square::set_furniture(world, at, 0);
                for item in crate::items_at(world, (at.0, at.1)) {
                    world.despawn(item);
                }
                crate::square::remove_trap(world, at);
                clear_fields(world, at);
                crate::square::delete_graffiti(world, at);
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
        for spot in repeat_spots(mapgen, rolls, entry) {
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
        for spot in repeat_spots(mapgen, rolls, entry) {
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
        for spot in repeat_spots(mapgen, rolls, entry) {
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
    mapgen: &cdda_content::mapgen::MapgenObject,
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
        if in_bounds(mapgen, x, y) {
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

/// Whether a placement's square is inside the chunk the mapgen declares.
///
/// Reference: `check_inbounds` (`mapgen.cpp:5130-5140`) refuses a placement whose coordinates fall outside `mapgensize`, and the loader **defers** the whole entry rather than
/// clipping it - so a 1x1 nested chunk cannot place furniture at x: 10, which is a real thing the data writes when a palette is reused at a smaller size.
fn in_bounds(mapgen: &cdda_content::mapgen::MapgenObject, x: i32, y: i32) -> bool {
    let (side, side_y) = mapgen.size();
    (0..side).contains(&x) && (0..side_y).contains(&y)
}

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
                if !in_bounds(mapgen, x, y) {
                    continue;
                }
                write_the_layer(world, entry, (origin.0 + x, origin.1 + y, origin.2), rolls);
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
        if !in_bounds(mapgen, x, y) {
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
        if !in_bounds(mapgen, x, y) {
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

/// Burns one square the way a mapgen's `burn` set point does.
///
/// Reference: `jmapgen_setmap::apply`'s `JMAPGEN_SETMAP_BURN` arm (`mapgen.cpp:5702-5735`). The two halves are told apart by the square's **flags**, not by what is on it:
/// `FLAMMABLE`/`FLAMMABLE_HARD` bash the square apart and, on `one_in( 5 )` and when the ground is not liquid, leave one to ten ash; `FLAMMABLE_ASH` bashes it apart too and
/// then, unless the square is open air, puts `f_ash` there and ten to a thousand ash.
///
/// **Named gaps**, both of them about items rather than the square: the reference goes on to burn every **flammable item** on the square and to turn it into its materials'
/// burn products (`create_burnproducts`), and the port has neither item flammability nor burn products. What the port does do is the square itself, which is what the member's
/// two data uses turn on.
fn burn_the_square(
    world: &mut World,
    at: (i32, i32, i32),
    rolls: &mut Option<cdda_core_types::rng::Rolls>,
) {
    let Some(dice) = rolls.as_mut() else {
        return;
    };
    let Some(facts) = crate::mapsquare::at(world, at) else {
        return;
    };
    let flagged = |flag: &str| facts.has_flag(flag);
    let liquid = flagged("LIQUID");
    let ash_bearing = flagged("FLAMMABLE_ASH");
    if !flagged("FLAMMABLE") && !flagged("FLAMMABLE_HARD") && !ash_bearing {
        return;
    }
    // **One hit is not enough**: the reference loops while the square can still be bashed, because a wall's rubble may itself be bashable. The cap is the port's guard
    // against data that would bash forever - the reference has none and would hang.
    for _ in 0..100 {
        let still_bashable = crate::mapsquare::at(world, at).is_some_and(|facts| facts.bashable());
        if !still_bashable {
            break;
        }
        crate::bash::bash_square_with_rolls(world, at, 9999, true, dice);
    }
    if ash_bearing {
        if crate::square::terrain_at(world, at)
            .and_then(|code| crate::terrain_types::name_of(world, code))
            .as_deref()
            != Some("t_open_air")
        {
            if let Some(code) = crate::furniture_types::code_of(world, "f_ash") {
                crate::square::set_furniture(world, at, code);
            }
            if !liquid {
                spawn_ash(world, at, dice.rng(10, 1000));
            }
        }
    } else if dice.one_in(5) && !liquid {
        spawn_ash(world, at, dice.rng(1, 10));
    }
}

/// Drops a heap of ash on a square, when the world knows what ash is.
fn spawn_ash(world: &mut World, at: (i32, i32, i32), amount: i32) {
    if let Some(ash) = crate::item_registry::id_of(world, "ash") {
        crate::drop_item(world, (at.0, at.1), ash, amount);
    }
}

/// Puts the people a mapgen names where it draws them.
///
/// Reference: `jmapgen_npc::apply` (`mapgen.cpp:2612-2630`): the entry names an `npc` **template** in its `class` member, the template names the `npc_class` that shapes the
/// person, and `map::place_npc` makes them on a rolled square - with `add_trait` traits on top, an optional `unique_id`, and a `target` flag a mission reads.
///
/// **Named gaps, all of them what a class means**: the port makes a person and records the class's **name**; the class's stats, skills, clothing and weapons are template
/// numbers the port has not read, and the reference's `unique_npc_exists` guard needs a registry of the living that the port keeps as the store's own entities.
fn place_the_npcs(
    world: &mut World,
    mapgen: &cdda_content::mapgen::MapgenObject,
    origin: (i32, i32, i32),
    rolls: &mut Option<cdda_core_types::rng::Rolls>,
) {
    for entry in &mapgen.place_npcs {
        let (x, y) = roll_a_square(rolls, entry.x, entry.y);
        if !in_bounds(mapgen, x, y) {
            continue;
        }
        let square = (origin.0 + x, origin.1 + y, origin.2);
        let made = crate::creature::spawn_creature(world, &entry.template, square);
        let id = if entry.unique_id.is_empty() {
            entry.template.clone()
        } else {
            entry.unique_id.clone()
        };
        crate::npcs::make_npc(world, made, &id);
        world
            .entity_mut(made)
            .insert(crate::npcs::NpcClass(entry.template.clone()));
        if entry.target {
            world.entity_mut(made).insert(crate::npcs::ScenarioTarget);
        }
        for trait_name in &entry.add_trait {
            crate::traits::add_trait(world, made, trait_name);
        }
    }
}

/// Rolls the lights and the doors of a vehicle that is not mint.
///
/// Reference: `vehicle::init_state`'s "provide some variety" pass (`vehicle.cpp:441-478`): every light class gets its own coin - headlights and wide headlights one in twenty,
/// a dome one in sixteen, an aisle light one in eight, a half-circle or circle one in four, an atomic one in two - a door on a part that opens is left open one time in four, and
/// a fridge, freezer or water purifier is switched **on** whatever the dice say, because those three lines have no coin in them.
fn set_the_lights_and_doors(
    world: &mut World,
    vehicle: bevy_ecs::prelude::Entity,
    parts: &mut [crate::vehicles::VehiclePart],
    dice: &mut cdda_core_types::rng::Rolls,
) {
    for index in 0..parts.len() {
        let Some(flags) = crate::vehicles::part_spec(world, vehicle, index).map(|spec| spec.flags)
        else {
            continue;
        };
        let light = if flags.contains("CONE_LIGHT") || flags.contains("WIDE_CONE_LIGHT") {
            Some(dice.one_in(20))
        } else if flags.contains("DOME_LIGHT") {
            Some(dice.one_in(16))
        } else if flags.contains("AISLE_LIGHT") {
            Some(dice.one_in(8))
        } else if flags.contains("HALF_CIRCLE_LIGHT") || flags.contains("CIRCLE_LIGHT") {
            Some(dice.one_in(4))
        } else if flags.contains("ATOMIC_LIGHT") {
            Some(dice.one_in(2))
        } else {
            None
        };
        let fridge = flags.contains("FRIDGE")
            || flags.contains("FREEZER")
            || flags.contains("WATER_PURIFIER");
        let door = flags.contains("OPENABLE") && dice.one_in(4);
        if let Some(part) = parts.get_mut(index) {
            if let Some(on) = light {
                part.enabled = on;
            }
            if fridge {
                part.enabled = true;
            }
            if door {
                part.open = true;
            }
        }
    }
}

/// Gives a placed vehicle the damage and the fuel its placement asks for.
///
/// Reference: `vehicle::init_state`'s damage loop (`vehicle.cpp:513-537`): for anything but a mint vehicle (`status == 0`), each part rolls `dice( 4, 8 )` - four to thirty-two -
/// and answers:
///
/// - eight or less: the part is **broken** (hit points zero);
/// - eight to twenty: hit points are `( roll - 8 ) / 12` of its durability, so an abandoned car has parts somewhere between nothing and whole;
/// - twenty or more: the part is whole.
///
/// **Named gaps, both of them whole branches of `init_state`**: the fuel fractions (`init_veh_fuel`) need a tank's capacity in charges, which needs an item's charge volume - the
/// port's item kinds do not carry one yet - and the `status == 1` extras (a destroyed seat, controls, tank, engine or tyre, and the engine faults) need the part categories the
/// reference reads from a part's flags. The `fuel` member is read and passed here so the gap is visible rather than implied.
fn damage_the_vehicle(
    world: &mut World,
    vehicle: bevy_ecs::prelude::Entity,
    status: i32,
    fuel: i32,
    dice: &mut cdda_core_types::rng::Rolls,
) {
    let _ = fuel;
    if status == 0 {
        return;
    }
    // **The destroy flags first**, in the reference's own draw order: the disabled vehicle loses one class of part (`rng( 1, 5 )`), then the key, the alarm and the engine
    // are each rolled for - and a "mint with no faults" vehicle (`status == 2`) clears all three after the rolls, which is why the rolls still happen.
    let (mut destroy_seats, mut destroy_controls, mut destroy_tank, mut destroy_engine) =
        (false, false, false, false);
    let mut destroy_tires = false;
    if status == 1 {
        match dice.rng(1, 5) {
            1 => destroy_seats = true,
            2 => destroy_controls = true,
            3 => destroy_tank = true,
            4 => destroy_engine = true,
            _ => destroy_tires = true,
        }
    }
    let _has_no_key = dice.one_in(3);
    let destroy_alarm = !dice.one_in(3);
    let destroy_engine = destroy_engine || dice.one_in(3);
    if status == 2 {
        // `veh_status` is 0 for the mint-with-no-faults state: the rolls above are drawn and thrown away (the reference clears all three flags), and the damage loop never runs.
        return;
    }
    let Some(record) = world.get::<crate::vehicles::Vehicle>(vehicle).cloned() else {
        return;
    };
    let mut parts = record.parts;
    set_the_lights_and_doors(world, vehicle, &mut parts, dice);
    for (index, part) in parts.iter_mut().enumerate() {
        let flags = crate::vehicles::part_spec(world, vehicle, index)
            .map_or_else(std::collections::BTreeSet::new, |spec| spec.flags);
        let roll = dice.dice(4, 8);
        if roll <= 8 {
            // Eight or less is `set_hp( pt, 0 )`: the part is broken, and a broken part is empty.
            part.hp = 0;
            part.broken = true;
            part.charges = 0;
            part.fuel.clear();
        } else if roll < 20 {
            let durability = part.hp.max(1);
            part.hp = (roll - 8) * durability / 12;
        }
        // `set_hp( pt, 0 )`: broken, and - for a part that holds something - empty too.
        let mut destroy = false;
        if flags.contains("ENGINE") {
            // **Named gap**: the reference sets an engine *fault* here when the part type has any; the port has no faults, so a doomed engine is simply broken.
            destroy = destroy_engine;
        } else if (destroy_seats && (flags.contains("SEAT") || flags.contains("SEATBELT")))
            || (destroy_controls && (flags.contains("CONTROLS") || flags.contains("SECURITY")))
            || (destroy_alarm && flags.contains("SECURITY"))
        {
            destroy = true;
        }
        if destroy_tank && (flags.contains("FUEL_TANK") || flags.contains("BATTERY")) {
            destroy = true;
        }
        // Solar panels have a one-in-four chance of being destroyed - but never on a `status: 2` vehicle, which the early return above already guarantees.
        if flags.contains("SOLAR_PANEL") && dice.one_in(4) {
            destroy = true;
        }
        if destroy {
            part.hp = 0;
            part.broken = true;
            part.charges = 0;
            part.fuel.clear();
        }
    }
    if let Some(mut record) = world.get_mut::<crate::vehicles::Vehicle>(vehicle) {
        record.parts = parts;
    }
    // **The tyres last**, because the reference destroys them *after* the part loop and asks the vehicle itself whether it can still drive (`vehicle.cpp:602-610`): while
    // `valid_wheel_config` holds and fewer than a hundred tyres have gone, one wheel is picked from the wheel cache at random and broken. A vehicle with four wheels therefore
    // loses three and keeps one - a single wheel is not a valid configuration - and the loop stops on the question rather than on a count.
    if destroy_tires {
        let mut tries = 0;
        while tries < 100 && crate::vehicles::valid_wheel_config(world, vehicle) {
            let Some(wheels) = world
                .get::<crate::vehicles::Vehicle>(vehicle)
                .map(|record| {
                    record
                        .parts
                        .iter()
                        .enumerate()
                        // **A broken wheel is not in the cache.** The reference's `wheelcache` is rebuilt from the vehicle's live parts, so the same wheel cannot be picked
                        // twice - and a filter that forgot the `broken` half picks the same one a hundred times, which is exactly what this loop did before the test saw it.
                        .filter(|(index, part)| {
                            !part.broken
                                && crate::vehicles::part_spec(world, vehicle, *index)
                                    .is_some_and(|spec| spec.flags.contains("WHEEL"))
                        })
                        .map(|(index, _)| index)
                        .collect::<Vec<_>>()
                })
            else {
                break;
            };
            if wheels.is_empty() {
                break;
            }
            // The reference picks with `random_entry`, which is a draw over the cache's indices.
            let Some(last) = wheels
                .len()
                .checked_sub(1)
                .and_then(|last| i32::try_from(last).ok())
            else {
                break;
            };
            let pick = dice.rng(0, last);
            let Some(index) = usize::try_from(pick)
                .ok()
                .and_then(|pick| wheels.get(pick).copied())
            else {
                break;
            };
            if let Some(mut record) = world.get_mut::<crate::vehicles::Vehicle>(vehicle)
                && let Some(part) = record.parts.get_mut(index)
            {
                part.hp = 0;
                part.broken = true;
                part.charges = 0;
                part.fuel.clear();
            }
            tries += 1;
        }
    }
    // **Half the parts are smashed last**, after the tyres, for a disabled vehicle and only on a coin - the reference's own order (`vehicle.cpp:613-617`).
    if status == 1 && dice.one_in(2) {
        crate::vehicle_smash::smash(world, vehicle, &crate::vehicle_smash::Smash::half(), dice);
    }
}

/// Stamps the faction ownership a mapgen names on the squares it covers.
///
/// Reference: `jmapgen_faction::apply` (`mapgen.cpp:2662-2670`): the entry's **ranges** are the rectangle (`x.val` to `x.valmax`), and the ownership lands on the items and the
/// unowned vehicle of every square in it. The reference skips a null faction id; the port has no faction registry to ask, so every id the data writes is stamped as written -
/// **named gap**, and the one place this rule can differ.
fn own_what_the_mapgen_names(
    world: &mut World,
    mapgen: &cdda_content::mapgen::MapgenObject,
    origin: (i32, i32, i32),
) {
    for entry in &mapgen.faction_owner {
        crate::ownership::own_the_rectangle(
            world,
            origin.0 + entry.x.0,
            origin.1 + entry.y.0,
            origin.0 + entry.x.1,
            origin.1 + entry.y.1,
            origin.2,
            &entry.id,
        );
    }
}

/// Lays out the corpses a mapgen names.
///
/// Reference: `jmapgen_corpse::apply` (`mapgen.cpp:2965-2974`): a monster is drawn **uniformly** from the group's flattened list, made into a corpse item dated `age` days
/// before now (but never before the cataclysm), and dropped on the square.
///
/// **Named gap**: the reference adds the entry's `z` to the square's height; the port's area members carry no `z`, as with every other area in this writer.
fn place_the_corpses(
    world: &mut World,
    mapgen: &cdda_content::mapgen::MapgenObject,
    origin: (i32, i32, i32),
    rolls: &mut Option<cdda_core_types::rng::Rolls>,
) {
    if mapgen.place_corpses.is_empty() {
        return;
    }
    for entry in &mapgen.place_corpses {
        let (x, y) = roll_a_square(rolls, entry.x, entry.y);
        let square = (origin.0 + x, origin.1 + y);
        let Some(dice) = rolls.as_mut() else {
            continue;
        };
        let Some(name) = crate::monster_groups::pick_flat(world, &entry.group, dice) else {
            continue;
        };
        let Some(kind) = crate::monster_registry::id_of(world, &name) else {
            continue;
        };
        let Some((corpse_item, volumes_ml)) = crate::creature::corpse_kind(world) else {
            continue;
        };
        let now = crate::calendar::of(world).map_or(0, |calendar| calendar.turn);
        let died_on = (now - i64::from(entry.age_days) * crate::calendar::TURNS_PER_DAY)
            .max(crate::calendar::of(world).map_or(0, |calendar| calendar.start_of_cataclysm));
        // The flag is asked **before** the corpse entity is borrowed, because asking it needs the world mutably.
        let revives = crate::monster_registry::kind_has_flag(world, kind, "REVIVES");
        let corpse = crate::drop_item(world, square, corpse_item, 1);
        world.entity_mut(corpse).insert(crate::CorpseOf {
            kind,
            died_on,
            revives,
            monster_volume_ml: volumes_ml.get(&kind).copied().unwrap_or(0),
        });
    }
}

/// Writes the graffiti a mapgen names.
///
/// Reference: `jmapgen_graffiti::apply` (`mapgen.cpp:2750-2776`): one rolled square, the **text** or a line drawn from the **snippet** category, then
/// `apply_all_tags` expands the placeholders (`SNIPPET.expand`, the city tag) and `map::set_graffiti` writes it.
///
/// **Named gaps**: the tag expansion is partial - the city tag becomes the reference's own fallback, "illegible city name", because the port has no city to name, and the
/// snippet placeholders (`<given_name>`, ...) are not expanded at all. Both are named here rather than silently written into the map.
fn write_the_graffiti(
    world: &mut World,
    mapgen: &cdda_content::mapgen::MapgenObject,
    origin: (i32, i32, i32),
    rolls: &mut Option<cdda_core_types::rng::Rolls>,
) {
    for entry in &mapgen.place_graffiti {
        let at = roll_a_square(rolls, entry.x, entry.y);
        let square = (origin.0 + at.0, origin.1 + at.1, origin.2);
        let drawn = if entry.snippet.is_empty() {
            entry.text.clone()
        } else {
            crate::snippets::pick(world, &entry.snippet, rolls.as_mut())
                .map(|line| line.text)
                .unwrap_or_default()
        };
        if drawn.is_empty() {
            continue;
        }
        crate::square::set_graffiti(world, square, &expand_graffiti_tags(&drawn));
    }
}

/// Expands the placeholders a graffiti text may carry, to the extent the port can answer them.
///
/// Reference: `jmapgen_graffiti::apply_all_tags` (`mapgen.cpp:2777-2790`), which expands the snippet tags and then replaces the city tag with the nearest city's name - or, when
/// the overmap has no city there, the literal `illegible city name`. The port has no city yet, so it writes that literal; the other tags are left as they are, which is why this
/// is a partial rule and is named as one.
fn expand_graffiti_tags(text: &str) -> String {
    text.replace("<city_name>", "illegible city name")
}

/// Applies the terrain/furniture transforms a mapgen names.
///
/// Reference: `jmapgen_ter_furn_transform::apply` (`mapgen.cpp:3773-3784`): the entry's two ranges are the **rectangle** it walks - `for ix in x.val..=x.valmax, iy in
/// y.val..=y.valmax` - and each square gets `ter_furn_transform::transform` once. There is no repetition and no chance: a transform is a fact about the map, not a spawn.
fn apply_the_transforms(
    world: &mut World,
    mapgen: &cdda_content::mapgen::MapgenObject,
    origin: (i32, i32, i32),
    rolls: &mut Option<cdda_core_types::rng::Rolls>,
) {
    // **The ranges are the rectangle, not a rolled square**: `jmapgen_int::val` and `valmax` are the two ends of the member itself, so a transform covers every square between
    // them and draws no dice. That is the difference between this rule and a symbol placement, and the test pins it.
    let _ = rolls;
    for entry in &mapgen.place_ter_furn_transforms {
        let (x1, x2) = if entry.x.0 <= entry.x.1 {
            (entry.x.0, entry.x.1)
        } else {
            (entry.x.1, entry.x.0)
        };
        let (y1, y2) = if entry.y.0 <= entry.y.1 {
            (entry.y.0, entry.y.1)
        } else {
            (entry.y.1, entry.y.0)
        };
        for x in x1..=x2 {
            for y in y1..=y2 {
                crate::terrain_transform::apply_at(
                    world,
                    &entry.transform,
                    (origin.0 + x, origin.1 + y, origin.2),
                );
            }
        }
    }
}

/// Parks the vehicles a mapgen places.
///
/// Reference: `jmapgen_vehicle::apply` (`mapgen.cpp:3180-3200`): `one_in( chance )`, then `map::add_vehicle( vgroup, pos, rotation, fuel, status, merge )`. The group member is a
/// `vgroup_id`, so the draw goes through `VehicleGroup::pick` first and only then to a prototype; the port asks the group store whether the id is a group and falls back to the
/// prototype table when it is not, which is the same distinction the reference's type system makes.
///
/// **Named gaps**: `fuel` (the reference places `fuel` units into a tank, `map::add_vehicle` with `veh_fuel`) and `status` (the light-damage pass) are not read yet; a vehicle
/// placed here arrives undamaged and empty, and this note is where that is recorded.
fn park_the_vehicles(
    world: &mut World,
    mapgen: &cdda_content::mapgen::MapgenObject,
    origin: (i32, i32, i32),
    rolls: &mut Option<cdda_core_types::rng::Rolls>,
) {
    for entry in &mapgen.place_vehicles {
        let chance = match rolls.as_mut() {
            Some(dice) if entry.chance.1 > entry.chance.0 => {
                dice.rng(entry.chance.0, entry.chance.1)
            }
            _ => entry.chance.0,
        };
        let Some(dice) = rolls.as_mut() else {
            continue;
        };
        if !dice.one_in(chance.max(1)) {
            continue;
        }
        let x = dice.rng(entry.x.0, entry.x.1);
        let y = dice.rng(entry.y.0, entry.y.1);
        let rotation = if entry.rotation.1 > entry.rotation.0 {
            dice.rng(entry.rotation.0, entry.rotation.1)
        } else {
            entry.rotation.0
        };
        let prototype = if crate::vehicle_groups::is_group(world, &entry.vehicle) {
            let Some(picked) = crate::vehicle_groups::pick(world, &entry.vehicle, dice) else {
                continue;
            };
            picked
        } else {
            entry.vehicle.clone()
        };
        if let Some(placed) = crate::vehicles::spawn_facing(
            world,
            &prototype,
            (origin.0 + x, origin.1 + y, origin.2),
            f64::from(rotation),
        ) {
            damage_the_vehicle(world, placed, entry.status, entry.fuel, dice);
        }
    }
}

/// Draws every parameter a mapgen declares **once**, and rewrites every symbol that names one to the id it drew.
///
/// Reference: `mapgen_palette::load` (the `parameters` member) and `mapgen_palette::resolve_parameters` (`mapgen.cpp:440-520`), which is called once per generation: each
/// parameter takes one weighted value from its `default.distribution`, and every `{ "param": name }` in the symbol tables becomes that value. A symbol with no parameter and no
/// fallback is dropped, which is the reference's behaviour too - a symbol that resolves to nothing draws nothing.
fn resolve_parameters(
    mapgen: &cdda_content::mapgen::MapgenObject,
    mut dice: Option<&mut cdda_core_types::rng::Rolls>,
) -> cdda_content::mapgen::MapgenObject {
    use cdda_content::mapgen::{MapgenObject, Piece};
    // **The rewrite runs even when there is nothing to draw**, because a palette that declares no parameters may still name one - and then its fallback is the answer.
    // An early return here looked like an optimisation and was a bug: the unresolved piece then drew nothing at all.
    let mut args: std::collections::BTreeMap<String, String> = std::collections::BTreeMap::new();
    for (name, parameter) in &mapgen.parameters {
        if parameter.values.is_empty() {
            continue;
        }
        let drawn = match dice.as_deref_mut() {
            Some(dice) => {
                let list = cdda_core_types::weighted::WeightedList::from_entries(
                    parameter.values.iter().cloned(),
                );
                list.pick(dice).cloned()
            }
            None => parameter.values.first().map(|(id, _)| id.clone()),
        };
        if let Some(drawn) = drawn {
            args.insert(name.clone(), drawn);
        }
    }
    let resolve = |piece: &Piece| resolve_piece(piece, &args);
    let mut rewritten = MapgenObject {
        terrain: mapgen
            .terrain
            .iter()
            .filter_map(|(symbol, piece)| Some((*symbol, resolve(piece)?)))
            .collect(),
        furniture: mapgen
            .furniture
            .iter()
            .filter_map(|(symbol, piece)| Some((*symbol, resolve(piece)?)))
            .collect(),
        ..mapgen.clone()
    };
    rewritten.parameters.clear();
    rewritten
}

/// Resolves one piece against the values the generation drew.
///
/// **A switch is resolved after its own value is**: the inner piece resolves first - a parameter to the value it drew - and then that value picks the case. A value the switch
/// does not name draws nothing, which is the reference's `make_null_helper` and not a fallback. The recursion is a named function rather than a closure because a closure
/// cannot call itself, and a switch inside a switch is legal data.
fn resolve_piece(
    piece: &cdda_content::mapgen::Piece,
    args: &std::collections::BTreeMap<String, String>,
) -> Option<cdda_content::mapgen::Piece> {
    use cdda_content::mapgen::Piece;
    match piece {
        Piece::Id(id) => Some(Piece::Id(id.clone())),
        Piece::Param { name, fallback } => args
            .get(name)
            .cloned()
            .or_else(|| fallback.clone())
            .map(Piece::Id),
        Piece::Switch { on, cases } => match resolve_piece(on, args)? {
            Piece::Id(value) => cases.get(&value).cloned().map(Piece::Id),
            _ => None,
        },
    }
}

/// Fills a rectangle with what a group holds, at the entry's density.
///
/// Reference: `jmapgen_monster_group::apply` (`mapgen.cpp:3093-3100`) hands the entry to `map::place_spawns` (`:7231-7281`), whose rules are, in order:
///
/// - `one_in( chance )` - the whole entry may not fire at all;
/// - `multiplier = density * SPAWN_DENSITY`, and `num = roll_remainder( multiplier * rng_float( 10.0, 50.0 ) )`, which is why a density of a tenth still places a handful of
///   monsters in a room: ten to fifty times a tenth, rounded;
/// - while the budget is positive: pick a square in the rectangle, ask the group for a result, place it `pack` times, and **decrement the budget by
///   `max( 1, cost * pack )`** - the entry's cost is what stops the expensive monster from filling the room.
///
/// The reference retries a square up to ten times when it is impassable; the port's squares have no passability flag yet, so every drawn square is used, which is
/// logged here rather than silently changed.
fn spawn_by_density(
    world: &mut World,
    mapgen: &cdda_content::mapgen::MapgenObject,
    origin: (i32, i32, i32),
    rolls: &mut Option<cdda_core_types::rng::Rolls>,
) {
    for entry in &mapgen.place_monsters {
        let chance = match rolls.as_mut() {
            Some(dice) if entry.chance.1 > entry.chance.0 => {
                dice.rng(entry.chance.0, entry.chance.1)
            }
            _ => entry.chance.0,
        };
        let Some(dice) = rolls.as_mut() else {
            continue;
        };
        if !dice.one_in(chance.max(1)) {
            continue;
        }
        let density = if entry.density_thousandths < 0 {
            cdda_content::mapgen::SPAWN_DENSITY_DEFAULT
        } else {
            entry.density_thousandths
        };
        let multiplier = f64::from(density) / 1000.0;
        let thenum = multiplier * dice.rng_float(10.0, 50.0);
        let mut budget = dice.roll_remainder(thenum);
        // A budget that cannot be spent is not spent: the loop is bounded by the budget decreasing every pass.
        while budget > 0 {
            let (x, y) = (
                dice.rng(entry.x.0, entry.x.1),
                dice.rng(entry.y.0, entry.y.1),
            );
            let Some((monster, cost, pack)) =
                crate::monster_groups::pick_entry(world, &entry.group, dice)
            else {
                break;
            };
            let pack_size = if pack.1 > pack.0 {
                dice.rng(pack.0, pack.1)
            } else {
                pack.0
            }
            .max(1);
            let Some(kind) = crate::monster_registry::id_of(world, &monster) else {
                break;
            };
            let hp = crate::monster_registry::hit_points(world, kind);
            for _ in 0..pack_size {
                crate::spawn_monster(world, (origin.0 + x, origin.1 + y), kind, hp);
            }
            budget -= (cost * pack_size).max(1);
        }
    }
}

/// Records every zone a mapgen names.
///
/// Reference: `jmapgen_zone::apply` (`mapgen.cpp:4064-4072`) turns the entry's two corner rectanges into absolute squares and hands them to `mapgen_place_zone`. The reference's
/// **zones phase** runs after everything else the map draws, because a zone labels squares rather than filling them; the port places them last for the same reason.
fn place_the_zones(
    world: &mut World,
    mapgen: &cdda_content::mapgen::MapgenObject,
    origin: (i32, i32, i32),
    rolls: &mut Option<cdda_core_types::rng::Rolls>,
) {
    for entry in &mapgen.place_zones {
        let near = roll_a_square(rolls, entry.x, entry.y);
        let far = roll_a_square(rolls, entry.x2, entry.y2);
        crate::zones::place(
            world,
            (origin.0 + near.0, origin.1 + near.1, origin.2),
            (origin.0 + far.0, origin.1 + far.1),
            &entry.kind,
            &entry.faction,
            &entry.name,
            &entry.filter,
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
            if !in_bounds(mapgen, x, y) {
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
fn write_the_layer(
    world: &mut World,
    entry: &cdda_content::mapgen::SetPoint,
    at: (i32, i32, i32),
    rolls: &mut Option<cdda_core_types::rng::Rolls>,
) {
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
        // **A mapgen's bash is one hit at strength 9999, silent** (`mapgen.cpp:5696-5699`: `m.bash( target_pos, 9999, true )`), which is how the data draws a room that has
        // been broken open. The hit draws from the write's own stream, so where it lands in the sequence is part of the mapgen's result.
        // **A mapgen's fire** (`mapgen.cpp:5702-5735`): a `FLAMMABLE` or `FLAMMABLE_HARD` square is bashed apart, one hit after another, and one time in five leaves a little
        // ash; a `FLAMMABLE_ASH` square is bashed apart and becomes a furniture heap of ash with a great deal more. A square that is neither is left alone.
        cdda_content::mapgen::SetKind::Burn => burn_the_square(world, at, rolls),
        cdda_content::mapgen::SetKind::Radiation => {
            let amount = match rolls.as_mut() {
                Some(dice) if entry.amount.1 > entry.amount.0 => {
                    dice.rng(entry.amount.0, entry.amount.1)
                }
                _ => entry.amount.0,
            };
            crate::radiation::set(world, at, amount);
        }
        cdda_content::mapgen::SetKind::Bash => {
            if let Some(dice) = rolls.as_mut() {
                crate::bash::bash_square_with_rolls(world, at, 9999, true, dice);
            }
        }
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
    let Some(placements) = mapgen.items.get(&symbol) else {
        return false;
    };
    let mut placed = false;
    for placement in placements {
        placed |= place_one_item_placement(world, placement, at, rolls);
    }
    placed
}

/// Places one placement, in either of the two shapes the data writes it.
fn place_one_item_placement(
    world: &mut World,
    placement: &cdda_content::mapgen::ItemPlacement,
    at: (i32, i32, i32),
    rolls: &mut Option<cdda_core_types::rng::Rolls>,
) -> bool {
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
