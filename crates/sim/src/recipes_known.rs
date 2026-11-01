//! The recipes a character has been taught.
//!
//! Reference: `learned_recipes` (`src/character_crafting.cpp:28-53`). A character knows the recipes in this set, and nothing else: `knows_recipe` is a membership
//! test, so a recipe the character can make only because it can see the ingredients is **not** known, and the two questions stay apart.
//!
//! What the reference refuses to teach is the part worth naming: `learn_recipe` (`:32-40`) returns without adding anything when the recipe carries
//! `never_learn`, which the data uses for the seventeen companion recipes (`recipes/recipe_companion.json`). The port has no recipe prototype to ask, so it
//! teaches what it is told - and no effect-on-condition in the data names one of those seventeen, which is why that gap is recorded here rather than guessed at.

use bevy_ecs::component::Component;
use bevy_ecs::entity::Entity;
use bevy_ecs::prelude::World;
use std::collections::BTreeSet;

/// The recipes a character has been taught.
#[derive(Component, Debug, Clone, Default, PartialEq, Eq)]
pub struct KnownRecipes(pub BTreeSet<String>);

/// Teaches a character a recipe, which is adding it to the set it carries.
///
/// Reference: `Character::learn_recipe` (`src/character_crafting.cpp:32-40`). The set has no duplicates: teaching the same recipe twice leaves one copy.
pub fn learn(world: &mut World, creature: Entity, recipe: &str) {
    if let Some(mut known) = world.get_mut::<KnownRecipes>(creature) {
        known.0.insert(recipe.to_owned());
        return;
    }
    let mut known = KnownRecipes::default();
    known.0.insert(recipe.to_owned());
    world.entity_mut(creature).insert(known);
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
