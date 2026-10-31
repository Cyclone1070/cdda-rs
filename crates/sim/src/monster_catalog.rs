//! The monster types the world knows, so a rule can make one.
//!
//! Reference: `MonsterGenerator::get_mtype( const mtype_id & )` (`monstergenerator.cpp`), which the whole game reads a monster's numbers from. The port's
//! registry (`monster_registry`) numbers the kinds; this holds the types themselves, because spawning needs more than a number: hit points, speed, melee
//! dice and the name a player sees.

use std::collections::BTreeMap;

use bevy_ecs::prelude::*;

/// Every monster type, by the id the data spells.
#[derive(Component, Debug, Clone, Default)]
pub struct MonsterCatalog(pub BTreeMap<String, cdda_content::monsters::MonsterType>);

/// Marks the entity holding the catalog.
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct TheMonsterCatalog;

/// Installs the types the world will spawn from. Later calls replace them, which is what a second world load does.
pub fn set_types(world: &mut World, types: Vec<cdda_content::monsters::MonsterType>) {
    let holder = crate::singletons::singleton::<MonsterCatalog, TheMonsterCatalog>(world);
    let mut by_id = BTreeMap::new();
    for kind in types {
        by_id.insert(kind.id.clone(), kind);
    }
    world.entity_mut(holder).insert(MonsterCatalog(by_id));
}

/// Every monster type the world knows, or nothing when none were installed.
#[must_use]
pub fn catalog(world: &World) -> Option<&MonsterCatalog> {
    crate::singletons::holder::<TheMonsterCatalog>(world)
        .and_then(|holder| world.get::<MonsterCatalog>(holder))
}

/// One monster type, if the world knows it.
#[must_use]
pub fn type_of<'a>(world: &'a World, id: &str) -> Option<&'a cdda_content::monsters::MonsterType> {
    catalog(world).and_then(|catalog| catalog.0.get(id))
}
