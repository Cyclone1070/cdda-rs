//! Which effect is which, inside the store.
//!
//! Reference: `effect_type`'s factory (`src/effect.cpp`), which answers `efftype_id` by id and hands back the type's caps.
//! [`crate::applied_effects`] keeps what a creature is wearing; what it could not do was look a name up, so a spell that
//! says "apply `adrenaline`" had no type to apply the caps from.
//!
//! The store keeps the types themselves rather than a copy of their numbers: a type is plain data with no identity of its
//! own beyond its id, and a second paraphrase of its fields would be the same drift the item layer removed.

use bevy_ecs::prelude::*;
use std::collections::BTreeMap;

use cdda_content::effects::EffectType;

use crate::singletons;

/// Every effect type the store can apply, by id.
#[derive(Component, Debug, Clone, Default, PartialEq)]
pub struct EffectTypes {
    /// The types, in the order they were installed.
    kinds: Vec<EffectType>,
    /// Where each one is in `kinds`.
    by_id: BTreeMap<String, usize>,
}

/// Marks the entity carrying [`EffectTypes`].
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct TheEffectTypes;

/// Installs the effect types the store will answer from.
pub fn set_effect_types(world: &mut World, kinds: &[EffectType]) {
    let mut by_id = BTreeMap::new();
    for (at, kind) in kinds.iter().enumerate() {
        by_id.insert(kind.id.clone(), at);
    }
    let entity = singletons::singleton::<EffectTypes, TheEffectTypes>(world);
    world.entity_mut(entity).insert(EffectTypes {
        kinds: kinds.to_vec(),
        by_id,
    });
}

/// The type a name refers to.
#[must_use]
pub fn kind_of(world: &mut World, id: &str) -> Option<EffectType> {
    let entity = singletons::singleton::<EffectTypes, TheEffectTypes>(world);
    world
        .get::<EffectTypes>(entity)
        .and_then(|types| types.by_id.get(id).copied())
        .and_then(|at| world.get::<EffectTypes>(entity)?.kinds.get(at).cloned())
}

/// The type a name refers to, without touching the store.
///
/// `kind_of` has to build the singleton when it is missing, which is why it takes the store mutably. A rule that is only **reading** a creature's effects - a speed bonus, a stat bonus -
/// has nothing to build, and giving it a read-only door is what lets such a rule keep `&World` like the rest of the read side.
#[must_use]
pub fn kind_named(world: &World, id: &str) -> Option<EffectType> {
    let types = world
        .iter_entities()
        .find_map(|entity| entity.get::<EffectTypes>())?;
    let at = types.by_id.get(id).copied()?;
    types.kinds.get(at).cloned()
}

/// Every type the store holds, for a rule that needs the whole table at once.
#[must_use]
pub fn types_of(world: &mut World) -> BTreeMap<String, EffectType> {
    let entity = singletons::singleton::<EffectTypes, TheEffectTypes>(world);
    world
        .get::<EffectTypes>(entity)
        .map(|types| {
            types
                .kinds
                .iter()
                .map(|kind| (kind.id.clone(), kind.clone()))
                .collect()
        })
        .unwrap_or_default()
}

/// How many types the store knows.
#[must_use]
pub fn count(world: &mut World) -> usize {
    let entity = singletons::singleton::<EffectTypes, TheEffectTypes>(world);
    world
        .get::<EffectTypes>(entity)
        .map_or(0, |types| types.by_id.len())
}

/// An effect type the data never defined, with the reference's own defaults for everything it says nothing about.
///
/// Reference: `effect::effect` (`src/effect.cpp:1560-1600`): an unknown id still has to answer *something*, and the reference answers a type whose intensity ceiling is one and whose every
/// other field is the default. **This is for the test fixtures and for an id the corpus happens not to carry**; a rule that lands an effect asks [`kind_of`] first.
#[must_use]
pub fn bare(id: &str) -> EffectType {
    EffectType {
        id: id.to_owned(),
        max_intensity: 1,
        max_duration: cdda_content::effects::DEFAULT_MAX_DURATION,
        dur_add_perc: 100,
        int_add_val: 0,
        int_decay_step: -1,
        int_decay_tick: 0,
        int_dur_factor: 0,
        int_decay_remove: false,
        main_parts_only: false,
        show_in_info: false,
        show_intensity: true,
        pain_sizing: false,
        hurt_sizing: false,
        harmful_cough: false,
        impairs_movement: false,
        pkill_addict_reduces: false,
        kill_chance: Vec::new(),
        red_kill_chance: Vec::new(),
        apply_ratings: Vec::new(),
        flags: std::collections::BTreeSet::new(),
        resist_traits: Vec::new(),
        resist_effects: Vec::new(),
        removes_effects: Vec::new(),
        blocks_effects: Vec::new(),
        mods: cdda_content::effects::ModTable::default(),
    }
}
