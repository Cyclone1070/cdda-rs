//! Making something from a recipe.
//!
//! Reference: `player_activity` with `ACT_CRAFT`, whose duration is `recipe::batch_time` and whose finish is
//! `activity_handlers::craft_finish` - consume the components, produce the result. The port has the recipe data and the
//! availability rule (`recipe::components_available`); what it does not have is an inventory to draw components from,
//! so a caller hands in what is on hand and the finish produces nothing yet. Both absences are declared here rather
//! than papered over: a craft that consumed from nowhere would be a rule that cannot be caught being wrong.

use bevy_ecs::prelude::*;
use cdda_content::recipes::Recipe;

use crate::creature::{Basis, Doing};

/// What came of trying to start a craft.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Craft {
    /// It has begun, and will take as many turns as the recipe needs.
    Started,
    /// Something on hand is missing: the recipe's components are not all there.
    MissingComponents,
    /// The creature is already busy with something else.
    Busy,
    /// The recipe takes no time at all, which no recipe in the data does - a sign the time was never read.
    NoTimeToMakeIt,
}

/// Starts a craft using what is actually in the crafter's hands.
///
/// The join the port was missing: [`crate::inventory::carried_by_id`] counts the carried items by their data ids, and
/// this hands that count to [`begin_craft`]'s availability rule - so a recipe is satisfied by things the creature is
/// really holding, not by a number a test made up.
pub fn begin_craft_from_carried(
    world: &mut World,
    crafter: Entity,
    recipe: &Recipe,
    batch: i32,
    name_of: impl Fn(crate::ItemTypeId) -> Option<String>,
) -> Craft {
    let on_hand = crate::inventory::carried_by_id(world, crafter, name_of);
    begin_craft(world, crafter, recipe, batch, |item| {
        on_hand.get(item).copied().unwrap_or(0)
    })
}

/// A craft in progress.
///
/// Reference: the `ACT_CRAFT` activity carries the recipe it is making and how many of it. The port keeps that on the
/// creature instead of inside the activity, because the activity component is the same one a pickup uses and the two
/// have nothing else in common.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Making {
    /// What is being made.
    pub recipe: Recipe,
    /// How many at once.
    pub batch: i32,
}

/// Starts making a recipe, if the things it needs are on hand.
///
/// Reference: `Character::making_would_work` for the refusal and `Character::start_craft` for the activity, whose
/// length is `recipe::batch_time( guy, batch, 1.0, 0 )` - one batch's time multiplied by the batch size.
///
/// `on_hand` says how many of an item id the crafter has. It is a closure rather than a lookup because the port has no
/// inventory yet: the caller - eventually the inventory, today a test or a playtest - is the only thing that knows.
pub fn begin_craft(
    world: &mut World,
    crafter: Entity,
    recipe: &Recipe,
    batch: i32,
    on_hand: impl Fn(&str) -> i32,
) -> Craft {
    if world.get::<Doing>(crafter).is_some() {
        return Craft::Busy;
    }
    if !recipe.components_available(batch, on_hand) {
        return Craft::MissingComponents;
    }
    let batch = batch.max(1);
    let moves = recipe.time_moves.saturating_mul(i64::from(batch));
    let Ok(moves_left) = i32::try_from(moves) else {
        return Craft::NoTimeToMakeIt;
    };
    if moves_left <= 0 {
        return Craft::NoTimeToMakeIt;
    }
    world.entity_mut(crafter).insert(Doing {
        activity: "craft".to_owned(),
        moves_left,
        basis: Basis::Speed,
        subject: None,
    });
    world.entity_mut(crafter).insert(Making {
        recipe: recipe.clone(),
        batch,
    });
    Craft::Started
}
