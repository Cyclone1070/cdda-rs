//! The data's own math functions, where the dialogue can reach them.
//!
//! Reference: `type: "jmath_function"` (`math_parser_jmath.cpp`), read by [`cdda_content::jmath`] and evaluated through [`cdda_content::math_expr`]. The reader and the
//! evaluator both already existed; what did not exist was a **home** for the registry in the world, so the dialogue's math could not see it and every call to a
//! data-defined function refused - fifteen functions, and `addiction_rational` alone is called by three programs.
//!
//! This module is the home: one entity carries the registry, and a system asks it by name.

use bevy_ecs::component::Component;
use bevy_ecs::entity::Entity;
use bevy_ecs::prelude::World;

/// The data-defined math functions this world knows, by name.
#[derive(Component, Debug, Clone, Default)]
pub struct MathFunctions(pub cdda_content::jmath::Registry);

/// Marks the entity carrying [`MathFunctions`].
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct TheMathFunctions;

/// Installs the data-defined math functions, replacing whatever was there.
pub fn set_functions(world: &mut World, registry: cdda_content::jmath::Registry) {
    let holder = crate::singletons::singleton::<MathFunctions, TheMathFunctions>(world);
    if let Some(mut installed) = world.get_mut::<MathFunctions>(holder) {
        installed.0 = registry;
    }
}

/// The functions this world knows, if any have been installed.
#[must_use]
pub fn functions_of(world: &World) -> Option<&cdda_content::jmath::Registry> {
    let holder = crate::singletons::holder::<TheMathFunctions>(world)?;
    world.get::<MathFunctions>(holder).map(|held| &held.0)
}

/// Reads a set of `jmath_function` definitions from a JSON array and installs them.
///
/// # Errors
///
/// Returns the reason the data could not be read.
pub fn install_json(world: &mut World, json: &[u8]) -> Result<usize, String> {
    let registry = cdda_content::jmath::Registry::read_json(json)?;
    let count = registry.len();
    set_functions(world, registry);
    Ok(count)
}

/// The entity carrying the functions, for a caller that wants to read them without a lookup by marker.
#[must_use]
pub fn holder(world: &World) -> Option<Entity> {
    crate::singletons::holder::<TheMathFunctions>(world)
}
