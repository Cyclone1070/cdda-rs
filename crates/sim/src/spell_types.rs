//! The spells the game defines, by id.
//!
//! Reference: `spell_type::load_spell` fills a map from `spell_id` to `spell_type` that `magic.cpp` reads by id. The port kept only **which spells a creature
//! knows** (`known_spells`); a dialogue effect that casts a spell names an id, so the prototypes need a home of their own. It is one singleton beside the other
//! prototype tables, and the ids are strings - never pointers into it.

use bevy_ecs::component::Component;
use bevy_ecs::prelude::{Entity, World};
use std::collections::BTreeMap;

/// Every spell the game defines, by id.
#[derive(Component, Debug, Clone, Default)]
pub struct SpellTypes(pub BTreeMap<String, cdda_content::spells::Spell>);

/// Marks the entity holding [`SpellTypes`].
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct TheSpellTypes;

/// Installs the spell table, replacing whatever was there.
pub fn set_types(world: &mut World, spells: Vec<cdda_content::spells::Spell>) {
    let element = crate::singletons::singleton::<SpellTypes, TheSpellTypes>(world);
    let table = spells.into_iter().map(|spell| (spell.id.clone(), spell)).collect();
    world.entity_mut(element).insert(SpellTypes(table));
}

/// The entity holding the spell table, when the world has one.
#[must_use]
pub fn holder(world: &World) -> Option<Entity> {
    crate::singletons::holder::<TheSpellTypes>(world)
}

/// One spell by id, or nothing when the game defines no such spell.
#[must_use]
pub fn find(world: &World, id: &str) -> Option<cdda_content::spells::Spell> {
    let element = holder(world)?;
    world
        .get::<SpellTypes>(element)
        .and_then(|types| types.0.get(id).cloned())
}
