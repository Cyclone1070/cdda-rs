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
    /// What each kind bleeds, by registry order (`mtype::bloodType`, `mtype.cpp:431-454`).
    blood: Vec<Option<String>>,
    /// What each kind splatters into, by registry order (`mtype::gibType`, `mtype.cpp:456-470`).
    gibs: Vec<Option<String>>,
    /// Each kind's own death sentence, or empty (`mdeath_effect::death_message`, `mtype.h:280`).
    death_messages: Vec<String>,
    /// What each kind leaves behind (`mdeath_effect::corpse_type`, `mtype.h:285`).
    corpse_types: Vec<String>,
    /// The item a `broken` kind leaves when the data names it itself (`mtype::broken_itype`, `mtype.h:367`).
    broken_itypes: Vec<String>,
    /// The item group a kind's body drops (`mtype::death_drops`, `mtype.h:404`).
    death_drops: Vec<String>,
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
        blood: Vec::new(),
        gibs: Vec::new(),
        death_messages: Vec::new(),
        corpse_types: Vec::new(),
        broken_itypes: Vec::new(),
        death_drops: Vec::new(),
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

/// Says what a kind bleeds and splatters into, which the death rules need and the type's own data answers.
///
/// Reference: `mtype::bloodType` and `mtype::gibType` (`mtype.cpp:431-470`), whose answers are computed from the monster's flags, materials and species by
/// [`cdda_content::blood`]. This keeps the **answer** rather than the inputs, because a death happens long after the data was loaded and the ladder's two functions are pure.
pub fn set_remains(world: &mut World, kind: MonsterKindId, blood: Option<&str>, gibs: Option<&str>) {
    let entity = singletons::singleton::<MonsterTypes, TheMonsterTypes>(world);
    let Some(mut registry) = world.get_mut::<MonsterTypes>(entity) else {
        return;
    };
    let at = kind.0 as usize;
    if registry.blood.len() <= at {
        registry.blood.resize(at + 1, None);
        registry.gibs.resize(at + 1, None);
    }
    registry.blood[at] = blood.map(str::to_owned);
    registry.gibs[at] = gibs.map(str::to_owned);
}

/// What a kind bleeds, as the field's id.
#[must_use]
pub fn blood_of(world: &mut World, kind: MonsterKindId) -> Option<String> {
    let entity = singletons::singleton::<MonsterTypes, TheMonsterTypes>(world);
    world
        .get::<MonsterTypes>(entity)
        .and_then(|registry| registry.blood.get(kind.0 as usize).cloned().flatten())
}

/// What a kind splatters into, as the field's id.
#[must_use]
pub fn gibs_of(world: &mut World, kind: MonsterKindId) -> Option<String> {
    let entity = singletons::singleton::<MonsterTypes, TheMonsterTypes>(world);
    world
        .get::<MonsterTypes>(entity)
        .and_then(|registry| registry.gibs.get(kind.0 as usize).cloned().flatten())
}

/// Says what a kind's data writes when it dies.
///
/// Reference: `mdeath_effect::death_message` (`mtype.h:280`), printed by `monster::die` (`monster.cpp:3086`) **before** the normal branch's own line. Most kinds state nothing, which is
/// why the registry stores an empty string rather than an `Option` - absent and empty are the same thing here, and the reference treats them the same way.
pub fn set_death_message(world: &mut World, kind: MonsterKindId, message: &str) {
    let entity = singletons::singleton::<MonsterTypes, TheMonsterTypes>(world);
    let Some(mut registry) = world.get_mut::<MonsterTypes>(entity) else {
        return;
    };
    let at = kind.0 as usize;
    if registry.death_messages.len() <= at {
        registry.death_messages.resize(at + 1, String::new());
    }
    registry.death_messages[at] = message.to_owned();
}

/// What a kind's data writes when it dies, or empty.
#[must_use]
pub fn death_message_of(world: &mut World, kind: MonsterKindId) -> String {
    let entity = singletons::singleton::<MonsterTypes, TheMonsterTypes>(world);
    world
        .get::<MonsterTypes>(entity)
        .and_then(|registry| registry.death_messages.get(kind.0 as usize).cloned())
        .unwrap_or_default()
}

/// Says what a kind leaves behind when it dies.
///
/// Reference: `mdeath_effect::corpse_type` (`mtype.h:285`), which `monster::die` switches on (`monster.cpp:3093-3106`): a normal corpse, a broken one, a splatter, or **nothing at all**. The
/// port keeps the data's word rather than a bool per branch, because the four answers are the reference's and collapsing them would lose which one the data gave.
pub fn set_corpse_type(world: &mut World, kind: MonsterKindId, corpse_type: &str) {
    let entity = singletons::singleton::<MonsterTypes, TheMonsterTypes>(world);
    let Some(mut registry) = world.get_mut::<MonsterTypes>(entity) else {
        return;
    };
    let at = kind.0 as usize;
    if registry.corpse_types.len() <= at {
        registry.corpse_types.resize(at + 1, String::new());
    }
    registry.corpse_types[at] = corpse_type.to_owned();
}

/// What a kind leaves behind when it dies, or empty for a kind never registered.
#[must_use]
pub fn corpse_type_of(world: &mut World, kind: MonsterKindId) -> String {
    let entity = singletons::singleton::<MonsterTypes, TheMonsterTypes>(world);
    world
        .get::<MonsterTypes>(entity)
        .and_then(|registry| registry.corpse_types.get(kind.0 as usize).cloned())
        .unwrap_or_default()
}

/// Says which item a `broken` kind leaves, when the data names it rather than leaving it to the id rule.
pub fn set_broken_itype(world: &mut World, kind: MonsterKindId, item: &str) {
    let entity = singletons::singleton::<MonsterTypes, TheMonsterTypes>(world);
    let Some(mut registry) = world.get_mut::<MonsterTypes>(entity) else {
        return;
    };
    let at = kind.0 as usize;
    if registry.broken_itypes.len() <= at {
        registry.broken_itypes.resize(at + 1, String::new());
    }
    registry.broken_itypes[at] = item.to_owned();
}

/// The item a `broken` kind leaves when the data names one, or empty.
#[must_use]
pub fn broken_itype_of(world: &mut World, kind: MonsterKindId) -> String {
    let entity = singletons::singleton::<MonsterTypes, TheMonsterTypes>(world);
    world
        .get::<MonsterTypes>(entity)
        .and_then(|registry| registry.broken_itypes.get(kind.0 as usize).cloned())
        .unwrap_or_default()
}

/// Says which item group a kind's body drops when it dies.
///
/// Reference: `mtype::death_drops` (`mtype.h:404`), drawn by `drop_items_on_death` (`monster.cpp:3248-3272`). **339 of the 1,154 types name one**; the rest leave their loot to an inline
/// group, which the dump does not name and the port therefore does not have.
pub fn set_death_drops(world: &mut World, kind: MonsterKindId, group: &str) {
    let entity = singletons::singleton::<MonsterTypes, TheMonsterTypes>(world);
    let Some(mut registry) = world.get_mut::<MonsterTypes>(entity) else {
        return;
    };
    let at = kind.0 as usize;
    if registry.death_drops.len() <= at {
        registry.death_drops.resize(at + 1, String::new());
    }
    registry.death_drops[at] = group.to_owned();
}

/// The item group a kind's body drops, or empty.
#[must_use]
pub fn death_drops_of(world: &mut World, kind: MonsterKindId) -> String {
    let entity = singletons::singleton::<MonsterTypes, TheMonsterTypes>(world);
    world
        .get::<MonsterTypes>(entity)
        .and_then(|registry| registry.death_drops.get(kind.0 as usize).cloned())
        .unwrap_or_default()
}
