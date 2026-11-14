//! How likely a survivor is to get a craft right, and when it will go wrong.
//!
//! Reference: `Character::recipe_success_roll_data` (`crafting.cpp:1172-1263`), `crafting_success_roll`/`crafting_failure_roll` (`crafting.cpp:1275-1295`), and
//! `item::set_next_failure_point` (`crafting.cpp:1328-1344`). The recipe's difficulty and the crafter's skill give a **normal distribution**; a craft fails when the
//! work reaches the point that roll chose, and what happens then is `handle_craft_failure`.
//!
//! The port keeps the numbers and the arithmetic: the centre is the crafter's weighted skill, the spread widens when the recipe is harder than the crafter and
//! narrows when it is easier, and the difficulty the roll must beat is one **more** than the recipe's own - the reference adds one so a roll of exactly the
//! difficulty still fails.

use bevy_ecs::prelude::*;

/// The three numbers a craft's roll is drawn from.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RollData {
    /// The average of the roll: the crafter's own level, brought towards the recipe's difficulty by the weighting.
    pub center: f64,
    /// How wide the roll is. Wider when the crafter is under-skilled, so luck matters more.
    pub stddev: f64,
    /// What the roll has to beat, which is the recipe's difficulty **plus one**.
    pub final_difficulty: f64,
}

/// The roll a recipe is judged by, for one crafter with no helpers.
///
/// Reference: `recipe_success_roll_data` (`crafting.cpp:1172-1263`), with the helper and secondary-skill terms left out because the port has neither yet:
/// the centre is `( 2 * difficulty * skill + 0 ) / max( 1, 2 * difficulty )`, the spread is two widened by a third of every point the crafter is short and
/// narrowed by a quarter of every point it is over, to a floor of one, and the difficulty is the recipe's own plus one.
#[must_use]
pub fn success_data(difficulty: i32, skill_level: i32) -> RollData {
    // Reference: `get_recipe_weighted_skill_average` clamps the difficulty to at least one **before** weighting, so the centre is the crafter's own level even for a
    // recipe with no difficulty at all - while the difficulty the roll must beat stays the recipe's own, plus one.
    #[allow(clippy::cast_precision_loss)]
    let difficulty = f64::from(difficulty.max(0));
    #[allow(clippy::cast_precision_loss)]
    let weight = f64::from(i32::from(difficulty > 0.0) + 1);
    let _ = weight;
    #[allow(clippy::cast_precision_loss)]
    let skill = f64::from(skill_level.max(0));
    let clamped = difficulty.max(1.0);
    let weighted = (2.0 * clamped * skill) / (2.0 * clamped).max(1.0);
    let final_difficulty = (2.0 * difficulty * difficulty) / (2.0 * difficulty).max(1.0) + 1.0;
    let mut stddev = 2.0;
    if final_difficulty > weighted {
        stddev += (final_difficulty - weighted) / 3.0;
    }
    if final_difficulty < weighted {
        stddev -= ((weighted - final_difficulty) / 4.0).min(1.0);
    }
    RollData {
        center: weighted,
        stddev: stddev.max(1.0),
        final_difficulty,
    }
}

/// The same numbers, tightened for a **failure** check.
///
/// Reference: `recipe_failure_roll_data` (`crafting.cpp:1265-1273`): the difficulty drops by one and is quartered, and the spread halves - so a failure check
/// succeeds far more often than a success check, which is what makes a catastrophic failure rare rather than routine.
#[must_use]
pub fn failure_data(difficulty: i32, skill_level: i32) -> RollData {
    let mut data = success_data(difficulty, skill_level);
    data.final_difficulty = (data.final_difficulty - 1.0) * 0.25;
    data.stddev *= 0.5;
    data
}

/// How far past the difficulty a craft's success roll lands, never below zero.
///
/// Reference: `crafting_success_roll` (`crafting.cpp:1275-1284`): `max( normal_roll( center, stddev ), 0 ) - final_difficulty`, floored at zero.
#[must_use]
pub fn success_roll(world: &mut World, data: RollData) -> f64 {
    (draw(world, data.center, data.stddev) - data.final_difficulty).max(0.0)
}

/// The roll that decides **how bad** a failure is, never below zero.
///
/// Reference: `crafting_failure_roll` (`crafting.cpp:1286-1295`): the same draw with the failure data, and this time the difficulty is **not** subtracted - the
/// number itself is the chance of skipping each destroyed component.
#[must_use]
pub fn failure_roll(world: &mut World, data: RollData) -> f64 {
    draw(world, data.center, data.stddev).max(0.0)
}

/// Draws a normal roll from the world's dice, giving them back.
fn draw(world: &mut World, center: f64, stddev: f64) -> f64 {
    let mut held = crate::singletons::take_rolls(world);
    let drawn = held
        .as_mut()
        .map_or(0.0, |dice| dice.normal_roll(center, stddev));
    if let Some(dice) = held.take() {
        crate::singletons::put_rolls(world, dice);
    }
    drawn
}

/// When the work will next be checked for failure, as a fraction of the whole craft in ten-millionths.
///
/// Reference: `item::set_next_failure_point` (`crafting.cpp:1328-1344`): the point is the progress now plus `roll * 10000000`, so a crafter with a good roll never
/// comes near another check and one with a poor roll is checked again almost immediately.
#[must_use]
pub fn next_failure_point(progress: i64, roll: f64) -> i64 {
    let delta = roll.max(0.0) * 10_000_000.0;
    #[allow(clippy::cast_possible_truncation)]
    let delta = delta as i64;
    progress.saturating_add(delta)
}

/// What a failed check cost the craft.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Failure {
    /// How many components were destroyed.
    pub destroyed: i32,
    /// How much progress was lost, in ten-millionths of the craft.
    pub progress_lost: i64,
    /// Whether every component is gone, which is a **catastrophic** failure and ends the craft.
    pub catastrophic: bool,
}

/// What the reference does when a craft fails its check.
///
/// Reference: `item::handle_craft_failure` (`crafting.cpp:1360-1400`):
///
/// * the failure roll is drawn, and it is the chance of **skipping** each component;
/// * at most `max( 1, components * 3 / 4 )` components are tried, each destroyed on a failed skip;
/// * a craft whose components are all gone is catastrophic;
/// * otherwise the progress lost is `rng_exponential( 0.25, 0.35 ) * ( 1 - roll )` of the progress made so far.
///
/// The port destroys components by removing the carried stacks the craft was going to use, which is the same thing seen through one store.
pub fn handle_failure(
    world: &mut World,
    crafter: Entity,
    recipe: &cdda_content::recipes::Recipe,
    progress: i64,
    names: &crate::item_registry::ItemNames,
) -> Failure {
    let roll = failure_roll(
        world,
        failure_data(
            recipe.difficulty,
            crate::skills_of::level_of(world, crafter, &recipe.skill),
        ),
    );
    let mut components: Vec<Entity> = Vec::new();
    {
        let mut query = world.query::<(Entity, &crate::creature::Carried, &crate::GroundItem)>();
        for (entity, carried, stack) in query.iter(world) {
            if carried.0 != crafter {
                continue;
            }
            let Some(name) = names.name_of(stack.kind) else {
                continue;
            };
            if recipe
                .components
                .iter()
                .flatten()
                .any(|component| component.item == name)
            {
                components.push(entity);
            }
        }
    }
    let starting = components.len();
    let most = ((starting * 3) / 4).max(1);
    let mut destroyed = 0;
    for entity in components.iter().take(most) {
        if skip_one(world, roll) {
            continue;
        }
        world.entity_mut(*entity).despawn();
        destroyed += 1;
    }
    if starting > 0 && destroyed == starting {
        let destroyed = i32::try_from(destroyed).unwrap_or(i32::MAX);
        return Failure {
            destroyed,
            progress_lost: 0,
            catastrophic: true,
        };
    }
    let lost = lost_fraction(world, roll);
    #[allow(clippy::cast_possible_truncation)]
    #[allow(clippy::cast_precision_loss)]
    let progress_f = progress as f64;
    #[allow(clippy::cast_possible_truncation)]
    let progress_lost = (progress_f * lost) as i64;
    Failure {
        destroyed: i32::try_from(destroyed).unwrap_or(i32::MAX),
        progress_lost,
        catastrophic: false,
    }
}

/// Whether a component is skipped, which is the failure roll as a chance.
fn skip_one(world: &mut World, roll: f64) -> bool {
    let mut held = crate::singletons::take_rolls(world);
    let skipped = held
        .as_mut()
        .is_some_and(|dice| dice.x_in_y(roll.max(0.0), 1.0));
    if let Some(dice) = held.take() {
        crate::singletons::put_rolls(world, dice);
    }
    skipped
}

/// The fraction of the progress a failure takes away.
///
/// Reference: `rng_exponential( 0.25, 0.35 ) * ( 1.0 - success_roll )` - a loss whose mean is a quarter and whose floor is a third of that, scaled down by how
/// well the crafter rolled.
fn lost_fraction(world: &mut World, roll: f64) -> f64 {
    let mut held = crate::singletons::take_rolls(world);
    let drawn = held
        .as_mut()
        .map_or(0.0, |dice| dice.rng_exponential(0.25, 0.35));
    if let Some(dice) = held.take() {
        crate::singletons::put_rolls(world, dice);
    }
    drawn * (1.0 - roll).max(0.0)
}
