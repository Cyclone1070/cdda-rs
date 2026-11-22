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
    let codes = crate::singletons::holder::<TheTerrainCodes>(world)
        .and_then(|holder| world.get::<TerrainCodes>(holder))
        .map(|codes| codes.0.clone())
        .unwrap_or_default();
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
    place_monsters(world, mapgen, origin, &mut rolls);
    place_area_items(world, mapgen, origin, &mut rolls);
    place_loot(world, mapgen, origin, &mut rolls);
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
    drew |= place_the_liquid(world, mapgen, at, symbol, rolls);
    drew
}

/// What a toilet holds when the data names no amount: `place_toilet`'s own default (`map.h:1679`).
const DEFAULT_TOILET_CHARGES: i32 = 24;

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
    if let Some(code) = crate::furniture_types::code_of(world, &sign.furniture) {
        crate::square::set_furniture(world, at, code);
    }
    if !sign.text.is_empty() {
        crate::signage::set_text(world, at, &sign.text);
    }
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
    // `one_in( chance.get() )`: a chance of one is always, and a bigger number is rarer - the opposite of the item group's percentage.
    if placement.chance > 1 {
        let Some(dice) = rolls.as_mut() else {
            return false;
        };
        if !dice.one_in(placement.chance) {
            return false;
        }
    }
    let Some(kind) = crate::item_registry::id_of(world, &placement.liquid) else {
        return false;
    };
    let mut charges = if placement.amount.0 > -1 {
        match rolls.as_mut() {
            Some(dice) if placement.amount.1 > placement.amount.0 => {
                dice.rng(placement.amount.0, placement.amount.1)
            }
            _ => placement.amount.0,
        }
    } else {
        // The item type's own count, which the port's registry does not carry: one charge is the count every liquid in the data that omits an amount has.
        ONE_CHARGE
    };
    if matches!(
        placement.liquid.as_str(),
        "gasoline" | "avgas" | "diesel" | "jp8"
    ) {
        charges *= FUEL_CHARGES_PER_UNIT;
    }
    if charges > 0 {
        crate::drop_item(world, (at.0, at.1), kind, charges);
    }
    true
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
        if placement.chance < 100
            && !rolls
                .as_mut()
                .is_none_or(|rolls| rolls.x_in_y(f64::from(placement.chance), 100.0))
        {
            continue;
        }
        let count = match rolls.as_mut() {
            Some(rolls) => rolls.rng(placement.pack_size.0, placement.pack_size.1),
            None => placement.pack_size.0,
        }
        .max(0);
        // **An entry may name a group instead of a monster** (`mapgen.cpp:3110-3120`, `3266-3275`): `jmapgen_monster` reads `group` and calls the group's own draw, which
        // answers one member per `apply` - so the pack below is the group's answer repeated, not a draw per member.
        let monster = if let Some(monster) = placement.monster.as_deref() {
            monster.to_owned()
        } else {
            let Some(group) = placement.group.as_deref() else {
                continue;
            };
            let Some(rolls) = rolls.as_mut() else {
                continue;
            };
            let Some(picked) = crate::monster_groups::pick(world, group, rolls) else {
                continue;
            };
            picked
        };
        let Some(kind) = crate::monster_registry::id_of(world, &monster) else {
            continue;
        };
        let hp = crate::monster_registry::hit_points(world, kind);
        for _ in 0..count {
            let x = match rolls.as_mut() {
                Some(rolls) => rolls.rng(placement.x.0, placement.x.1),
                None => placement.x.0,
            };
            let y = match rolls.as_mut() {
                Some(rolls) => rolls.rng(placement.y.0, placement.y.1),
                None => placement.y.0,
            };
            let spawn = crate::spawn_monster(world, (origin.0 + x, origin.1 + y), kind, hp);
            // Reference: `add_spawn( ..., friendly, ... )` (`mapgen.cpp:3268-3270`) - a monster a mapgen marks friendly is on the player's side for good.
            if placement.friendly {
                crate::friendly::make_friendly(world, spawn, crate::friendly::FOR_EVER);
            }
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
