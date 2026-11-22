//! What a sign says.
//!
//! Reference: `map::set_signage` (`mapgen.cpp:2720`) stores the text on the square, and `map::signage_at` reads it back; a sign is furniture that carries words, and the
//! words are the part a player reads. Kept as one component on the square's own entity, so nothing beside the store copies the text.

use bevy_ecs::component::Component;

/// The words on a square's sign.
#[derive(Component, Debug, Clone, PartialEq, Eq)]
pub struct Sign {
    /// What it says, already resolved: the data's `signage` is a string, and a `snippet` category is chosen elsewhere.
    pub text: String,
}

/// Writes what a square's sign says, making the sign's record if the square has none.
pub fn set_text(world: &mut bevy_ecs::prelude::World, square: (i32, i32, i32), text: &str) {
    let standing = crate::square::entities_at(world, square)
        .iter()
        .copied()
        .find(|entity| world.get::<Sign>(*entity).is_some());
    if let Some(entity) = standing {
        if let Some(mut sign) = world.get_mut::<Sign>(entity) {
            text.clone_into(&mut sign.text);
        }
        return;
    }
    let entity = world
        .spawn((
            crate::Position {
                x: square.0,
                y: square.1,
                z: square.2,
            },
            Sign {
                text: text.to_owned(),
            },
        ))
        .id();
    crate::singletons::index_mut(world).insert(entity, square);
}

/// What a square's sign says, if it has one.
#[must_use]
pub fn text_at(world: &bevy_ecs::prelude::World, square: (i32, i32, i32)) -> Option<String> {
    crate::square::entities_at(world, square)
        .iter()
        .find_map(|entity| world.get::<Sign>(*entity).map(|sign| sign.text.clone()))
}

/// How many signs the world holds.
///
/// Takes the store mutably because a query's own cache is written on first use; that is `bevy_ecs`'s shape, not a second store.
pub fn census(world: &mut bevy_ecs::prelude::World) -> usize {
    world.query::<&Sign>().iter(world).count()
}
