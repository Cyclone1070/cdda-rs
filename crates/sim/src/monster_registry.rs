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
#[derive(Component, Debug, Clone, Default)]
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
    /// Every kind's weak-point families, one after another, with [`MonsterTypes::family_ranges`] saying where each kind's start.
    ///
    /// Reference: `mtype::families` (`mtype.h:382`). **Laid flat rather than nested**, the same way the item registry lays pockets flat and for the same reason the architecture gate gives: a component
    /// that holds one list per kind is a second table beside the store, and a range into one list is not. The families are what a **kill** teaches (`weakpoint.cpp:152-155`), and a world that never
    /// installs them teaches nothing rather than pretending a body has none.
    families: Vec<cdda_content::weakpoints::Family>,
    /// Where each kind's families start and how many there are, by registry order.
    family_ranges: Vec<(u32, u32)>,
    /// Every kind's butchering entries, one after another, with [`MonsterTypes::harvest_ranges`] saying where each kind's start.
    harvest_entries: Vec<crate::HarvestSpec>,
    /// The item ids those entries name, in the same order.
    harvest_ids: Vec<String>,
    /// Where each kind's harvest entries start and how many there are, by registry order.
    harvest_ranges: Vec<(u32, u32)>,
    /// What each kind weighed while it was alive, in grams, by registry order.
    weights: Vec<i64>,
    /// How big each kind is, by registry order.
    sizes: Vec<crate::CreatureSize>,
}

/// What killing each kind teaches, by kind.
///
/// Reference: `mtype::families` (`mtype.h:382`), a `weakpoint_families`, installed once from the content the world loaded.
pub fn set_weakpoint_families(
    world: &mut World,
    entries: &[(crate::MonsterKindId, Vec<cdda_content::weakpoints::Family>)],
) {
    let entity = crate::singletons::singleton::<MonsterTypes, TheMonsterTypes>(world);
    let Some(mut types) = world.get_mut::<MonsterTypes>(entity) else {
        return;
    };
    for (kind, families) in entries {
        let at = kind.0 as usize;
        if types.family_ranges.len() <= at {
            types.family_ranges.resize(at + 1, (0, 0));
        }
        // This installer runs once, over kinds whose families nothing else has written, so the list is appended in registry order and each kind's range points into it.
        #[allow(clippy::cast_possible_truncation)]
        let start = types.families.len() as u32;
        #[allow(clippy::cast_possible_truncation)]
        let how_many = families.len() as u32;
        types.families.extend(families.iter().cloned());
        if let Some(slot) = types.family_ranges.get_mut(at) {
            *slot = (start, how_many);
        }
    }
}

/// The weak-point families one kind's body rewards, empty when the world never installed them.
#[must_use]
pub fn families_of(world: &World, kind: crate::MonsterKindId) -> Vec<cdda_content::weakpoints::Family> {
    let Some(types) = world.iter_entities().find_map(|entity| entity.get::<MonsterTypes>()) else {
        return Vec::new();
    };
    let Some(&(start, how_many)) = types.family_ranges.get(kind.0 as usize) else {
        return Vec::new();
    };
    let start = start as usize;
    types
        .families
        .get(start..start + how_many as usize)
        .map_or_else(Vec::new, <[cdda_content::weakpoints::Family]>::to_vec)
}

/// What butchering each kind of body yields, by kind.
///
/// Reference: `mtype::harvest` names a `harvest_definition`, whose entries `butchery.cpp`'s finish loop walks. **Laid flat with a range per kind**, the way the families and the item registry's pockets
/// are: the sim does not own the content crate, so the numbers arrive installed rather than looked up, and a world that never installs them butchers nothing rather than inventing a yield.
pub fn set_harvests(
    world: &mut World,
    entries: &[(crate::MonsterKindId, Vec<crate::HarvestSpec>, Vec<String>)],
) {
    let entity = crate::singletons::singleton::<MonsterTypes, TheMonsterTypes>(world);
    let Some(mut types) = world.get_mut::<MonsterTypes>(entity) else {
        return;
    };
    for (kind, specs, ids) in entries {
        let at = kind.0 as usize;
        if types.harvest_ranges.len() <= at {
            types.harvest_ranges.resize(at + 1, (0, 0));
        }
        #[allow(clippy::cast_possible_truncation)]
        let start = types.harvest_entries.len() as u32;
        #[allow(clippy::cast_possible_truncation)]
        let how_many = specs.len() as u32;
        types.harvest_entries.extend(specs.iter().copied());
        types.harvest_ids.extend(ids.iter().cloned());
        if let Some(slot) = types.harvest_ranges.get_mut(at) {
            *slot = (start, how_many);
        }
    }
}

/// How big each kind of body is, by kind, which decides the base cost of cutting it up.
pub fn set_sizes(world: &mut World, entries: &[(crate::MonsterKindId, crate::CreatureSize)]) {
    let entity = crate::singletons::singleton::<MonsterTypes, TheMonsterTypes>(world);
    let Some(mut types) = world.get_mut::<MonsterTypes>(entity) else {
        return;
    };
    for (kind, size) in entries {
        let at = kind.0 as usize;
        if types.sizes.len() <= at {
            types.sizes.resize(at + 1, crate::CreatureSize::Medium);
        }
        if let Some(slot) = types.sizes.get_mut(at) {
            *slot = *size;
        }
    }
}

/// How big one kind of body is; a medium body when the world did not say, which is the reference's own default for a size it cannot read (`butchery.cpp:377-392`).
#[must_use]
pub fn size_of(world: &World, kind: crate::MonsterKindId) -> crate::CreatureSize {
    world
        .iter_entities()
        .find_map(|entity| entity.get::<MonsterTypes>())
        .and_then(|types| types.sizes.get(kind.0 as usize).copied())
        .unwrap_or(crate::CreatureSize::Medium)
}

/// What one kind's body yields when butchered: the entries and the ids their drops name, in the definition's order.
///
/// Empty when the world never installed them, which is the honest answer - a body with no recorded harvest butchers to nothing rather than to a guess.
#[must_use]
pub fn harvest_of(world: &World, kind: crate::MonsterKindId) -> (Vec<crate::HarvestSpec>, Vec<String>) {
    let Some(types) = world.iter_entities().find_map(|entity| entity.get::<MonsterTypes>()) else {
        return (Vec::new(), Vec::new());
    };
    let Some(&(start, how_many)) = types.harvest_ranges.get(kind.0 as usize) else {
        return (Vec::new(), Vec::new());
    };
    let start = start as usize;
    let end = start + how_many as usize;
    (
        types.harvest_entries.get(start..end).map_or_else(Vec::new, <[crate::HarvestSpec]>::to_vec),
        types.harvest_ids.get(start..end).map_or_else(Vec::new, <[String]>::to_vec),
    )
}

/// How much each kind of body weighed while it was alive, by kind, in grams.
pub fn set_weights(world: &mut World, entries: &[(crate::MonsterKindId, i64)]) {
    let entity = crate::singletons::singleton::<MonsterTypes, TheMonsterTypes>(world);
    let Some(mut types) = world.get_mut::<MonsterTypes>(entity) else {
        return;
    };
    for (kind, grams) in entries {
        let at = kind.0 as usize;
        if types.weights.len() <= at {
            types.weights.resize(at + 1, 0);
        }
        if let Some(slot) = types.weights.get_mut(at) {
            *slot = *grams;
        }
    }
}

/// How much one kind of body weighed while it was alive, in grams; zero when the world did not say.
#[must_use]
pub fn weight_of(world: &World, kind: crate::MonsterKindId) -> i64 {
    world
        .iter_entities()
        .find_map(|entity| entity.get::<MonsterTypes>())
        .and_then(|types| types.weights.get(kind.0 as usize).copied())
        .unwrap_or(0)
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
#[allow(clippy::indexing_slicing, clippy::assigning_clones)]
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
        families: Vec::new(),
        family_ranges: Vec::new(),
        harvest_entries: Vec::new(),
        harvest_ids: Vec::new(),
        harvest_ranges: Vec::new(),
        weights: Vec::new(),
        sizes: Vec::new(),
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
#[allow(clippy::indexing_slicing, clippy::assigning_clones)]
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
#[allow(clippy::indexing_slicing, clippy::assigning_clones)]
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
#[allow(clippy::indexing_slicing, clippy::assigning_clones)]
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
#[allow(clippy::indexing_slicing, clippy::assigning_clones)]
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
#[allow(clippy::indexing_slicing, clippy::assigning_clones)]
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
