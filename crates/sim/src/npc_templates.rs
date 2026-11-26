//! The NPC templates the data writes by hand, so a placed person can be made from one.
//!
//! Reference: `npc_templates` is the reference's registry (`npc.cpp`), keyed by id, and `npc::load_npc_template` (`npc.cpp:520-568`) copies a template's class, attitude, mission
//! and chat topic onto the person being made. The port keeps the definitions in one component on one entity, and a person keeps their own **copy** of the values that matter,
//! because a person is not a template.

use bevy_ecs::prelude::{Component, Entity, World};

/// Every NPC template the data loaded, by id.
#[derive(Component, Debug, Clone, Default, PartialEq)]
pub struct NpcTemplates(pub Vec<cdda_content::npc_templates::NpcTemplate>);

/// Marks the one entity that holds the world's NPC templates.
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct TheNpcTemplates;

/// Installs the templates the data loaded, replacing any that were there.
pub fn set_templates(world: &mut World, templates: Vec<cdda_content::npc_templates::NpcTemplate>) {
    let entity = crate::singletons::singleton::<NpcTemplates, TheNpcTemplates>(world);
    if let Some(mut held) = world.get_mut::<NpcTemplates>(entity) {
        held.0 = templates;
    }
}

/// The templates, when the world has loaded any.
#[must_use]
pub fn holder(world: &World) -> Option<Entity> {
    crate::singletons::holder::<TheNpcTemplates>(world)
}

/// The template with this id, when the world has it.
#[must_use]
pub fn template_named(world: &World, id: &str) -> Option<cdda_content::npc_templates::NpcTemplate> {
    let entity = holder(world)?;
    let held = world.get::<NpcTemplates>(entity)?;
    held.0.iter().find(|template| template.id == id).cloned()
}
