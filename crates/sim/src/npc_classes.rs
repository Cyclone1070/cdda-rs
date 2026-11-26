//! The NPC classes the data defines, so a rule can ask what a person's class says about them.
//!
//! Reference: `npc_class` (`npc_class.cpp`) is a registry keyed by id, and every `npc` holds a pointer into it (`myclass`). The port keeps the definitions in one component on one
//! entity and puts the class's **id** on the person (`npcs::NpcClass`), which is the same fact without the pointer.

use bevy_ecs::prelude::{Component, Entity, World};

/// Every NPC class the data loaded, by id.
#[derive(Component, Debug, Clone, Default, PartialEq)]
pub struct NpcClasses(pub Vec<cdda_content::npc_classes::NpcClass>);

/// Marks the one entity that holds the world's NPC classes.
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct TheNpcClasses;

/// Installs the classes the data loaded, replacing any that were there.
pub fn set_classes(world: &mut World, classes: Vec<cdda_content::npc_classes::NpcClass>) {
    let entity = crate::singletons::singleton::<NpcClasses, TheNpcClasses>(world);
    if let Some(mut held) = world.get_mut::<NpcClasses>(entity) {
        held.0 = classes;
    }
}

/// The classes, when the world has loaded any.
#[must_use]
pub fn holder(world: &World) -> Option<Entity> {
    crate::singletons::holder::<TheNpcClasses>(world)
}

/// The class with this id, when the world has it.
#[must_use]
pub fn class_named(world: &World, id: &str) -> Option<cdda_content::npc_classes::NpcClass> {
    let entity = holder(world)?;
    let held = world.get::<NpcClasses>(entity)?;
    held.0.iter().find(|class| class.id == id).cloned()
}
