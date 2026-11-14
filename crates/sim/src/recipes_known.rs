//! The recipes a character has been taught.
//!
//! Reference: `learned_recipes` (`src/character_crafting.cpp:28-53`). A character knows the recipes in this set, and nothing else: `knows_recipe` is a membership
//! test, so a recipe the character can make only because it can see the ingredients is **not** known, and the two questions stay apart.
//!
//! **The refusal is the interesting half.** `learn_recipe` (`:32-40`) returns without teaching anything when the recipe carries `never_learn` and the caller did not
//! ask to override it - and the reference's own dump marks **1,114 of 6,291 recipes** that way, practice recipes and companions alike. The flag arrives as an
//! argument because the recipe prototype is the caller's to look up; a `learn` that guessed would be a second opinion about what can be taught.

use bevy_ecs::component::Component;
use bevy_ecs::entity::Entity;
use bevy_ecs::prelude::World;
use std::collections::BTreeSet;

/// The recipes a character has been taught.
#[derive(Component, Debug, Clone, Default, PartialEq, Eq)]
pub struct KnownRecipes(pub BTreeSet<String>);

/// Teaches a character a recipe, and answers whether it was taught.
///
/// Reference: `Character::learn_recipe` (`src/character_crafting.cpp:32-40`):
///
/// ```text
/// if( rec->never_learn && !override_never_learn ) return;
/// learned_recipes->include( rec );
/// ```
///
/// **The refusal is silent and complete**: nothing is added and the set is not created, so a character who was only ever offered never-learn recipes carries no component at all - the same
/// "missing means none" rule [`crate::identification`] follows. `override_never_learn` exists because **reading a book** can teach a recipe the character could not otherwise learn.
pub fn learn(
    world: &mut World,
    creature: Entity,
    recipe: &str,
    never_learn: bool,
    override_never_learn: bool,
) -> bool {
    if never_learn && !override_never_learn {
        return false;
    }
    if let Some(mut known) = world.get_mut::<KnownRecipes>(creature) {
        known.0.insert(recipe.to_owned());
        return true;
    }
    let mut known = KnownRecipes::default();
    known.0.insert(recipe.to_owned());
    world.entity_mut(creature).insert(known);
    true
}

/// Makes a character forget a recipe. Forgetting one it never knew changes nothing, which the reference also allows.
pub fn forget(world: &mut World, creature: Entity, recipe: &str) {
    if let Some(mut known) = world.get_mut::<KnownRecipes>(creature) {
        known.0.remove(recipe);
    }
}

/// Whether a character knows a recipe.
#[must_use]
pub fn knows(world: &World, creature: Entity, recipe: &str) -> bool {
    world
        .get::<KnownRecipes>(creature)
        .is_some_and(|known| known.0.contains(recipe))
}

/// Every recipe a character knows, in id order so a dump of it is stable.
#[must_use]
pub fn known(world: &World, creature: Entity) -> Vec<String> {
    world
        .get::<KnownRecipes>(creature)
        .map(|known| known.0.iter().cloned().collect())
        .unwrap_or_default()
}
