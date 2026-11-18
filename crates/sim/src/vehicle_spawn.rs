//! **What a placed vehicle finds in its cargo.**
//!
//! Reference: `vehicle::place_spawn_items` (`vehicle.cpp:6379-6450`). A prototype lists spawns per square - a chance of a batch, the items a batch draws, and the item groups it
//! draws from - and placing the vehicle fills the cargo part at that square.
//!
//! **The dice are the caller's.** Every count and every item is a roll, so the rule is written to take a [`Rolls`] and nothing here decides a value on its own; a test can hand it a
//! recording and get the reference's own session back.

use bevy_ecs::prelude::*;

use cdda_core_types::rng::Rolls;

use crate::vehicles::Vehicle;

/// **A count rounded by dice**: the whole part, plus one if a die falls inside the fraction.
///
/// Reference: `roll_remainder` (`rng.cpp:136-147`). The reference asks the dice **only when there is a fraction** - `value > integ` - so a whole number consumes no roll, which
/// matters for a replay that must stay in step with the reference's own stream.
#[must_use]
pub fn roll_remainder(value: f64, rolls: &mut Rolls) -> i32 {
    let whole = value.trunc();
    let fraction = value - whole;
    let rolled = if value > 0.0 && value > whole && rolls.x_in_y(fraction, 1.0) {
        whole + 1.0
    } else if value < 0.0 && value < whole && rolls.x_in_y(-fraction, 1.0) {
        whole - 1.0
    } else {
        whole
    };
    #[expect(clippy::cast_possible_truncation, reason = "a count of items is small")]
    let count = rolled as i32;
    count
}

/// **How many batches a spawn produces**: its chance, scaled by the `ITEM_SPAWNRATE` option, and rounded by the dice.
///
/// Reference: `vehicle.cpp:6405` - `roll_remainder( spawn.chance * std::max( spawn_rate, 1.0f ) / 100.0f )`. The option is floored at one, so a spawn rate below one cannot
/// silence a spawn that has a chance.
#[must_use]
pub fn batch_count(chance: i32, spawn_rate: f64, rolls: &mut Rolls) -> i32 {
    let listed = f64::from(chance);
    roll_remainder(listed * spawn_rate.max(1.0) / 100.0, rolls)
}

/// The part index at a square that carries the `CARGO` feature, if any - the reference's `part_with_feature( pos, "CARGO", false )` (`vehicle.cpp:6397`).
#[must_use]
pub fn cargo_part_at(world: &World, vehicle: Entity, x: i32, y: i32) -> Option<usize> {
    let count = world
        .get::<Vehicle>(vehicle)
        .map_or(0, |built| built.parts.len());
    (0..count).find(|index| {
        let Some(part) = world
            .get::<Vehicle>(vehicle)
            .and_then(|built| built.parts.get(*index))
        else {
            return false;
        };
        part.x == x
            && part.y == y
            && crate::vehicles::part_spec(world, vehicle, *index)
                .is_some_and(|spec| spec.flags.iter().any(|flag| flag == "CARGO"))
    })
}

/// **Fills a placed vehicle's cargo**, and answers how many items went in.
///
/// Reference: `vehicle::place_spawn_items` (`vehicle.cpp:6379-6450`), in its own order:
///
/// 1. each spawn entry finds the cargo part at its square (`part_with_feature( pos, "CARGO", false )`);
/// 2. the batch count is [`batch_count`] of its chance;
/// 3. a batch on a **broken** part is halved (`broken && one_in( 2 )` skips it whole);
/// 4. each listed item is made when `rng_float( 0, 1 ) < spawn_rate`, each variant likewise, and each listed group is **drawn** - the reference's group draw takes
///    `spawn_flags::use_spawn_rate` and applies the rate itself, so the port draws without a second test;
/// 5. everything made is stowed into the part.
///
/// **Named, not guessed:** the reference also damages what a broken part lets through (`e.mod_damage( rng( 1, e.max_damage() ) )`), fills magazines and ammunition for tools and
/// guns, copies the vehicle's owner onto each item, and wraps items in their container. The port's `GroundItem` carries damage but has no ammunition, owner or container model.
pub fn place_spawn_items(
    world: &mut World,
    vehicle: Entity,
    spawns: &[cdda_content::vehicles::Spawn],
    spawn_rate: f64,
) -> usize {
    let mut placed = 0;
    for spawn in spawns {
        let Some(part) = cargo_part_at(world, vehicle, spawn.x, spawn.y) else {
            continue;
        };
        let broken = world
            .get::<crate::vehicles::Vehicle>(vehicle)
            .and_then(|built| built.parts.get(part))
            .is_none_or(|held| held.broken);
        let batches = {
            let mut dice = crate::singletons::take_rolls(world);
            let count = dice
                .as_mut()
                .map(|rolls| batch_count(spawn.chance, spawn_rate, rolls));
            if let Some(dice) = dice.take() {
                crate::singletons::put_rolls(world, dice);
            }
            count.unwrap_or(0)
        };
        let mut drawn: Vec<(String, i32)> = Vec::new();
        for _ in 0..batches {
            if broken && roll_one_in_two(world) {
                continue;
            }
            for item in &spawn.items {
                if rolls_below(world, spawn_rate) {
                    drawn.push((item.clone(), 1));
                }
            }
            for group in &spawn.groups {
                if let Some(items) = crate::item_groups::draw_at_rate(world, group, spawn_rate) {
                    drawn.extend(items);
                }
            }
        }
        for (id, count) in drawn {
            let Some(kind) = crate::item_registry::id_in(world, &id) else {
                continue;
            };
            if crate::vehicle_cargo::stow_item(world, vehicle, part, kind, count).is_some() {
                placed += 1;
            }
        }
    }
    placed
}

/// One die in two, spent from the world's own stream.
fn roll_one_in_two(world: &mut World) -> bool {
    let mut dice = crate::singletons::take_rolls(world);
    let answer = dice.as_mut().is_some_and(|rolls| rolls.one_in(2));
    if let Some(dice) = dice.take() {
        crate::singletons::put_rolls(world, dice);
    }
    answer
}

/// One die against a probability, spent from the world's own stream - the reference's `rng_float( 0, 1 ) < spawn_rate`.
fn rolls_below(world: &mut World, rate: f64) -> bool {
    let mut dice = crate::singletons::take_rolls(world);
    let answer = dice
        .as_mut()
        .is_some_and(|rolls| rolls.rng_float(0.0, 1.0) < rate);
    if let Some(dice) = dice.take() {
        crate::singletons::put_rolls(world, dice);
    }
    answer
}
