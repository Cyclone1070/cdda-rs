//! The emissions the data defines, by id.
//!
//! Reference: `emit::emit_map` (`emit.cpp`), which the loader fills and `map::emit_field( pos, emit_id )` reads. A spell whose
//! effect is `emit` names one of these ids in its `effect_str`, and the effect puts the field down on each square of the area,
//! asking the emission's chance once per square.

use bevy_ecs::prelude::*;
use cdda_content::emit::Emission;
use std::collections::BTreeMap;

use crate::singletons;

/// Every emission, by the id the data gives it.
#[derive(Component, Debug, Clone, Default)]
pub struct Emits {
    by_id: BTreeMap<String, Vec<Emission>>,
}

/// Marks the entity holding the emissions.
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct TheEmits;

/// Installs the emissions into the store, replacing whatever was there.
pub fn set_emits(world: &mut World, emits: &[(String, Vec<Emission>)]) {
    let mut by_id = BTreeMap::new();
    for (id, emissions) in emits {
        by_id.insert(id.clone(), emissions.clone());
    }
    let holder = singletons::singleton::<Emits, TheEmits>(world);
    if let Some(mut existing) = world.get_mut::<Emits>(holder) {
        existing.by_id = by_id;
        return;
    }
    world.entity_mut(holder).insert(Emits { by_id });
}

/// What an emission puts down, or nothing when the data does not define it.
#[must_use]
pub fn emissions_of(world: &World, id: &str) -> Option<Vec<Emission>> {
    let mut query = world.try_query_filtered::<&Emits, With<TheEmits>>()?;
    let emits = query.iter(world).next()?;
    emits.by_id.get(id).cloned()
}

/// How many emissions the store holds, which is what a test counts.
#[must_use]
pub fn count(world: &mut World) -> usize {
    let holder = singletons::singleton::<Emits, TheEmits>(world);
    world.get::<Emits>(holder).map_or(0, |emits| emits.by_id.len())
}
