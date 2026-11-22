//! What a computer terminal is, where it stands and what it offers.
//!
//! Reference: `map::add_computer( pos, name, security )` (`mapgen.cpp:3882-3905`) stores a `computer` record on the square and every mapgen'd console gets one; the record
//! holds the terminal's **name**, its **security** level, the options a user may pick and the messages a failed attempt shows. The port keeps it as one component on the
//! square's own entity - the store is the record, and nothing beside it copies the text.

use bevy_ecs::component::Component;

/// One thing a terminal offers, as the data writes it: a **name** a user reads and an **action** the game performs.
///
/// Reference: `computer_option::from_json` (`computer.cpp`): the name and the action, plus parameters the port does not carry yet (the action's own `ter_set`/`furn_set`,
/// an elevator's target floor). Storing the pair as text is what keeps this a record rather than a pointer to behaviour.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalOption {
    /// What the menu says.
    pub name: String,
    /// What picking it does, as the data spells it.
    pub action: String,
}

/// A computer terminal standing on a square.
#[derive(Component, Debug, Clone, PartialEq, Eq)]
pub struct Computer {
    /// What the terminal calls itself.
    pub name: String,
    /// What it says when access is refused.
    pub access_denied: String,
    /// The security level a hack has to beat.
    pub security: i32,
    /// The actions it offers, in the data's order.
    pub options: Vec<TerminalOption>,
    /// The effect-on-conditions the data attaches.
    pub eocs: Vec<String>,
    /// The chat topics it can start.
    pub chat_topics: Vec<String>,
}

/// Puts a terminal on a square, replacing any record already standing there.
pub fn set(world: &mut bevy_ecs::prelude::World, square: (i32, i32, i32), computer: Computer) {
    let standing = crate::square::entities_at(world, square)
        .iter()
        .copied()
        .find(|entity| world.get::<Computer>(*entity).is_some());
    if let Some(entity) = standing {
        world.entity_mut(entity).insert(computer);
        return;
    }
    let entity = world
        .spawn((
            crate::Position {
                x: square.0,
                y: square.1,
                z: square.2,
            },
            computer,
        ))
        .id();
    crate::singletons::index_mut(world).insert(entity, square);
}

/// The terminal standing on a square, if it has one.
#[must_use]
pub fn computer_at(world: &bevy_ecs::prelude::World, square: (i32, i32, i32)) -> Option<&Computer> {
    crate::square::entities_at(world, square)
        .iter()
        .find_map(|entity| world.get::<Computer>(*entity))
}

/// How many terminals the world holds.
pub fn census(world: &mut bevy_ecs::prelude::World) -> usize {
    world.query::<&Computer>().iter(world).count()
}
