//! Which monster is which, inside the store.
//!
//! Reference: `MonsterGenerator`'s `mtype` table (`monstergenerator.cpp`), which numbers every monster type and answers by
//! id. The port had [`crate::MonsterKindId`] with nothing in the store to say what a number meant: a caller kept the
//! `Vec<MonsterType>` and passed it in, which is the same second source of truth the item registry removed.
//!
//! What a kind carries here is deliberately small - a name and the hit points it starts with. A monster's dice, armour
//! and behaviour are `cdda-content`'s `MonsterType` and arrive through [`crate::creature::spawn_monster_of_type`]; this
//! table only has to answer "which kind is number seven" and "what is a `mon_zombie`", which is what a spell that
//! summons one needs.

use bevy_ecs::prelude::*;
use std::collections::BTreeMap;

use crate::singletons;
use crate::MonsterKindId;

/// One monster kind, as the store needs to know it.
#[derive(Debug, Clone, Copy)]
pub struct MonsterKind<'a> {
    /// The id the data uses.
    pub id: &'a str,
    /// The hit points it starts with.
    pub hp: i32,
}

/// Every monster kind's id, by the number the store uses for it.
#[derive(Component, Debug, Clone, Default, PartialEq, Eq)]
pub struct MonsterTypes {
    /// The name of each kind, in registry order.
    names: Vec<String>,
    /// The hit points of each kind, in registry order.
    hit_points: Vec<i32>,
    /// The number of each kind, by name.
    by_name: BTreeMap<String, MonsterKindId>,
}

impl MonsterTypes {
    /// The id of a kind, which is what a rule that names monster types compares against.
    #[must_use]
    pub fn name_of(&self, kind: crate::MonsterKindId) -> Option<&str> {
        usize::try_from(kind.0)
            .ok()
            .and_then(|at| self.names.get(at))
            .map(String::as_str)
    }
}

/// Marks the entity carrying [`MonsterTypes`].
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct TheMonsterTypes;

/// Installs the registry the store will answer from.
pub fn set_monster_types(world: &mut World, kinds: &[MonsterKind<'_>]) {
    let mut names = vec![String::new()];
    let mut hit_points = vec![0];
    let mut by_name = BTreeMap::new();
    for kind in kinds {
        #[allow(clippy::cast_possible_truncation)]
        let number = MonsterKindId(names.len() as u32);
        by_name.insert(kind.id.to_owned(), number);
        names.push(kind.id.to_owned());
        hit_points.push(kind.hp);
    }
    let entity = singletons::singleton::<MonsterTypes, TheMonsterTypes>(world);
    world.entity_mut(entity).insert(MonsterTypes {
        names,
        hit_points,
        by_name,
    });
}

/// The kind a name refers to.
#[must_use]
pub fn id_of(world: &mut World, name: &str) -> Option<MonsterKindId> {
    let entity = singletons::singleton::<MonsterTypes, TheMonsterTypes>(world);
    world
        .get::<MonsterTypes>(entity)
        .and_then(|registry| registry.by_name.get(name).copied())
}

/// The name of a kind, as the data spells it.
#[must_use]
pub fn name_of(world: &mut World, kind: MonsterKindId) -> Option<String> {
    let entity = singletons::singleton::<MonsterTypes, TheMonsterTypes>(world);
    world
        .get::<MonsterTypes>(entity)
        .and_then(|registry| registry.names.get(kind.0 as usize).cloned())
        .filter(|name| !name.is_empty())
}

/// How many hit points a kind starts with.
#[must_use]
pub fn hit_points(world: &mut World, kind: MonsterKindId) -> i32 {
    let entity = singletons::singleton::<MonsterTypes, TheMonsterTypes>(world);
    world
        .get::<MonsterTypes>(entity)
        .and_then(|registry| registry.hit_points.get(kind.0 as usize).copied())
        .unwrap_or(0)
}

/// How many kinds the registry knows, not counting the null kind at zero.
#[must_use]
pub fn count(world: &mut World) -> usize {
    let entity = singletons::singleton::<MonsterTypes, TheMonsterTypes>(world);
    world
        .get::<MonsterTypes>(entity)
        .map_or(0, |registry| registry.by_name.len())
}
