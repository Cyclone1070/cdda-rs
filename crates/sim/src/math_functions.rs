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

/// Installs every `jmath_function` found in a set of parsed data files.
///
/// The fifteen definitions are spread across five files (`weight_functions.json`, `weather_type.json`, `mutations/mutation_jmath.json`, two effects-on-condition
/// files), so the caller hands over everything it read and this picks the definitions out - the same way the reference's loader visits every file in the data and
/// takes the `jmath_function` objects it finds.
///
/// Returns how many were installed.
///
/// # Errors
///
/// Returns the reason an entry could not be read; a definition the reference would refuse is refused here too.
pub fn install_from_values(world: &mut World, values: &[cdda_json::Value]) -> Result<usize, String> {
    let mut definitions = Vec::new();
    for value in values {
        collect_definitions(value, &mut definitions);
    }
    let registry = cdda_content::jmath::read_entries(&definitions)?;
    let count = registry.len();
    set_functions(world, registry);
    Ok(count)
}

/// Walks a parsed file and collects the objects that carry `type: "jmath_function"`.
fn collect_definitions(value: &cdda_json::Value, found: &mut Vec<cdda_json::Object>) {
    match value {
        cdda_json::Value::Object(object) => {
            if object.get("type").and_then(|kind| match kind {
                cdda_json::Value::Str(name) => Some(name.as_str()),
                _ => None,
            }) == Some("jmath_function")
            {
                found.push(object.clone());
            }
            for (_, member) in object.members() {
                collect_definitions(member, found);
            }
        }
        cdda_json::Value::Array(entries) => {
            for entry in entries {
                collect_definitions(entry, found);
            }
        }
        _ => {}
    }
}
