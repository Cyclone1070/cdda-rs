//! **Walking**: a direction becomes a square, the square gets a price, and the step is taken.
//!
//! Reference: `game::walk_move` (`game.cpp`) is a long function that does far more than this - it carries fire, fields, monster attacks, running, climbing and the reality bubble. What
//! the port can state exactly today is the spine of it: where the step lands, what it costs, when it is refused, and that a character walking off a vehicle square leaves the ride.
//!
//! The price has **two halves from two places**: the destination's own cost (terrain, or the vehicle standing on it, which is why [`crate::vehicles::move_cost_at`] is asked first)
//! and the square being left. [`crate::creature::combined_move_cost`] is the reference's arithmetic for joining them, including the diagonal, so the port does not re-derive it here.
//!
//! **Named, not guessed:** the reference charges `Character::run_cost`, which starts at a hundred and adds what the character carries and wears. The port has no carried weight yet - a
//! creature's [`crate::creature::Weight`] is a single number with no composition - so a step costs the terrain's own price. `corpus/walk.json`'s `cost` column records the
//! reference's answers (163 for a floor step by a loaded survivor) and is **not** compared; the direction, the refusal and the boarding are.

use crate::creature::{StepOutcome, combined_move_cost};
use crate::routing::square_cost;
use crate::vehicles::move_cost_at;
use bevy_ecs::prelude::*;

/// Where a step in a direction lands.
#[must_use]
pub fn destination(from: (i32, i32, i32), dx: i32, dy: i32) -> (i32, i32, i32) {
    (from.0.saturating_add(dx), from.1.saturating_add(dy), from.2)
}

/// What entering a square costs, from whichever holds it - a vehicle first, then the terrain - or `None` when it cannot be entered at all.
///
/// Reference: `map::move_cost` (`map.cpp:1322-1338`) asks the vehicle standing on the square before the terrain does, and a cost of zero means the square cannot be walked
/// into. A vehicle square's own cost is `vpart_position::get_movecost` (`vehicle.cpp:2871-2881`), which [`move_cost_at`] answers.
#[must_use]
pub fn entering_cost(world: &World, square: (i32, i32, i32)) -> Option<i32> {
    if let Some(cost) = move_cost_at(world, square) {
        return if cost <= 0 { None } else { Some(cost) };
    }
    square_cost(world, square)
}

/// What a step from one square to another costs, or `None` when it cannot be taken.
///
/// The destination must be enterable and the square being left must have a price of its own; the two are joined by [`combined_move_cost`], which is where the diagonal's surcharge
/// lives. A character standing on a square the data never described cannot walk off it, because there is no number to charge for leaving.
#[must_use]
pub fn step_cost(world: &World, from: (i32, i32, i32), to: (i32, i32, i32)) -> Option<i32> {
    let leaving = entering_cost(world, from)?;
    let entering = entering_cost(world, to)?;
    let axes = usize::from(from.0 != to.0) + usize::from(from.1 != to.1);
    Some(combined_move_cost(leaving, entering, 0, axes))
}

/// **What a creature's own body does to the price of a step**: its two movement limb scores and the flat-ground multiplier from its enchantments.
///
/// Reference: `Character::run_cost` reads all three (`character.cpp:10396-10446`) - "Footing", "Movement" and "Flat Ground Mut." - and the port has no body parts to compute the
/// scores from and no enchantment cache to gather the multiplier from yet, so they are installed where a creature is made. A creature without this component pays the square's own
/// cost, which is what a body in perfect repair on broken ground would pay.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct LimbCost {
    /// The `move_speed` limb score, near one for sound legs and lower as they are hurt or laden.
    pub move_speed: f32,
    /// The `footing` limb score, which counts for a third of the blend.
    pub footing: f32,
    /// The summed `MOVECOST_FLATGROUND_MOD` as a multiplier: one plus the enchantments' multiplies.
    pub flat_ground: f32,
}

/// What this creature pays to enter a square whose entry costs `combined`.
///
/// Reference: `game::walk_move` charges `u.run_cost( mcost, diag )` (`game.cpp:10809`), where `mcost` is the map's combined cost - a floor step is a hundred - so the price of a
/// step is the square's own number through the body's two factors. A creature with no [`LimbCost`] pays the square's number unchanged.
#[must_use]
pub fn price_of_step(world: &World, creature: Entity, combined: i32) -> i32 {
    let Some(limbs) = world.get::<LimbCost>(creature) else {
        return combined;
    };
    let blend = crate::run_cost::limb_run_cost_modifier(
        crate::run_cost::limb_cost_modifier(limbs.footing),
        crate::run_cost::limb_cost_modifier(limbs.move_speed),
    );
    #[expect(
        clippy::cast_precision_loss,
        reason = "a square's combined cost is a small count of movement points"
    )]
    let base = combined as f32;
    crate::run_cost::run_cost(
        base,
        blend,
        limbs.flat_ground,
        false,
        crate::regeneration::stamina_ratio(world, creature),
    )
}

/// **One step in a direction.** Answers whether the creature moved.
///
/// This is the port's whole walk loop: a direction, a destination, a price, and [`crate::vehicles::step_from_vehicle`], which owns the riding part of the rule - unboard before the
/// step, board again if the square it lands on has a seat, exactly as `avatar_action::move` does (`avatar_action.cpp:653-672`).
pub fn walk(world: &mut World, creature: Entity, dx: i32, dy: i32) -> StepOutcome {
    let Some(position) = world.get::<crate::Position>(creature).copied() else {
        return StepOutcome::TooSlow;
    };
    let from = (position.x, position.y, position.z);
    let to = destination(from, dx, dy);
    let Some(cost) = step_cost(world, from, to) else {
        return StepOutcome::TooSlow;
    };
    // **The square's cost, then the body's.** The reference charges `run_cost(mcost)`, not `mcost`, so a step is never simply what the map would charge.
    let charge = price_of_step(world, creature, cost);
    let outcome = crate::vehicles::step_from_vehicle(world, creature, to, charge);
    if outcome == StepOutcome::Moved {
        // **A step costs breath as well as movement points** (`game.cpp` calls `u.burn_move_stamina( mcost )` with the moves the step cost).
        crate::regeneration::burn(world, creature, charge);
        // **And the square entered may be trapped.** Reference: `game::walk_move` ends with `here.creature_on_trap( u )` (`game.cpp:11356-11362`), which reaches
        // `trap::trigger` - so the trap under the square the creature moved into fires, and one it merely walked past does not. The port's springing covers the `eocs`
        // action, which is the one the data's teleporters use.
        crate::trap_spring::spring(world, creature, to);
    }
    outcome
}

/// **A creature that has been told to step, and has not yet.** The port's own seam between input and simulation: the reference's UI calls `avatar_action::move` directly from
/// the key handler, and an ECS cannot - input arrives as data, a system reads it, and the step happens inside the turn where the schedule can order it.
///
/// One step per turn: the system takes the component out, walks, and does not put it back, so a held key is a new component each frame rather than a creature that runs.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct WantsToStep {
    /// Which way, along the columns.
    pub dx: i32,
    /// Which way, along the rows.
    pub dy: i32,
}

/// **Takes every pending step.** Runs in the creatures stage of the turn, before the monsters act.
///
/// The order is the entity's own, sorted, so two creatures wanting the same square resolve the same way in every replay; the reference's turn order is by speed and is not
/// ported, which is why this says what it does instead of claiming to be it.
pub fn take_steps(world: &mut World) {
    let mut wanting: Vec<Entity> = world
        .iter_entities()
        .filter(|entity| entity.get::<WantsToStep>().is_some())
        .map(|entity| entity.id())
        .collect();
    wanting.sort();
    for entity in wanting {
        let Some(intent) = world.entity_mut(entity).take::<WantsToStep>() else {
            continue;
        };
        walk(world, entity, intent.dx, intent.dy);
    }
}
