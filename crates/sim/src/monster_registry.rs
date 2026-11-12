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
use std::collections::{BTreeMap, BTreeSet};

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
    /// The speed of each kind, in registry order (`mtype::speed`, `mtype.h`).
    speeds: Vec<i32>,
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
    /// The flags each kind carries (`mtype::has_flag`), in registry order.
    ///
    /// The port keeps these **per kind** as well as per creature, because a rule that reads a kind's flags has only an id in hand: `spell_effect::revive` asks the kind of the
    /// corpse whether it `REVIVES`, whether it is `DORMANT`, and whether it is `NO_NECRO`, and none of those questions have a creature to ask.
    flags: Vec<BTreeSet<String>>,
    /// The species each kind belongs to (`mtype::species`), in registry order. A `true` species answer is `in_species`, which is what a spell naming a species asks for.
    species: Vec<BTreeSet<String>>,
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
    /// The armour each kind has against each damage type, by registry order (`mtype::armor_bash`, `armor_cut`, `armor_stab`, `mtype.h:216-220`).
    armours: Vec<crate::creature::Armour>,
    /// The body each kind has, by registry order, or none when the data names a body type the port could not build.
    ///
    /// Reference: `mtype::bodytype` (`mtype.h:236`) is an **anatomy id**, and `mtype::get_all_body_parts()` asks the anatomy for the parts a blow can land on. The port keeps the
    /// resolved body here, per kind, so that a monster spawned from an id has somewhere for a blow to land - which is what an attack spell asks for.
    bodies: Vec<Option<crate::creature::Body>>,
    /// Every kind's dissection entries, with [`MonsterTypes::dissect_ranges`] saying where each kind's start.
    dissect_entries: Vec<crate::HarvestSpec>,
    /// The item ids those entries name.
    dissect_ids: Vec<String>,
    /// The item each kind leaves behind after a butchery, by registry order.
    leftovers: Vec<String>,
    /// Where each kind's dissection entries start and how many there are, by registry order.
    dissect_ranges: Vec<(u32, u32)>,
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

/// What each kind of body leaves behind after a butchery, by kind: the item id its harvest definition names.
///
/// Reference: `harvest_definition::leftovers`, which every definition has - `ruined_chunks` by default (`harvest.h:102`).
pub fn set_leftovers(world: &mut World, entries: &[(crate::MonsterKindId, String)]) {
    let entity = crate::singletons::singleton::<MonsterTypes, TheMonsterTypes>(world);
    let Some(mut types) = world.get_mut::<MonsterTypes>(entity) else {
        return;
    };
    for (kind, leftover) in entries {
        let at = kind.0 as usize;
        if types.leftovers.len() <= at {
            types.leftovers.resize(at + 1, String::new());
        }
        if let Some(slot) = types.leftovers.get_mut(at) {
            *slot = leftover.clone();
        }
    }
}

/// What one kind leaves behind, or the reference's default when the world did not say.
#[must_use]
pub fn leftovers_of(world: &World, kind: crate::MonsterKindId) -> String {
    world
        .iter_entities()
        .find_map(|entity| entity.get::<MonsterTypes>())
        .and_then(|types| types.leftovers.get(kind.0 as usize).cloned())
        .filter(|leftover| !leftover.is_empty())
        .unwrap_or_else(|| "ruined_chunks".to_owned())
}

/// What dissecting each kind of body yields, by kind.
///
/// Reference: `mtype::dissect`, a second `harvest_definition` that the finish loop walks **instead of** the harvest list when the action is a dissection and the body states one (`butchery.cpp:692-694`). Kept
/// in its own table for the same reason the families are: a body may have neither, either, or both, and a world that installs no dissection lists dissects like any other butchery.
pub fn set_dissections(
    world: &mut World,
    entries: &[(crate::MonsterKindId, Vec<crate::HarvestSpec>, Vec<String>)],
) {
    let entity = crate::singletons::singleton::<MonsterTypes, TheMonsterTypes>(world);
    let Some(mut types) = world.get_mut::<MonsterTypes>(entity) else {
        return;
    };
    for (kind, specs, ids) in entries {
        let at = kind.0 as usize;
        if types.dissect_ranges.len() <= at {
            types.dissect_ranges.resize(at + 1, (0, 0));
        }
        #[allow(clippy::cast_possible_truncation)]
        let start = types.dissect_entries.len() as u32;
        #[allow(clippy::cast_possible_truncation)]
        let how_many = specs.len() as u32;
        types.dissect_entries.extend(specs.iter().copied());
        types.dissect_ids.extend(ids.iter().cloned());
        if let Some(slot) = types.dissect_ranges.get_mut(at) {
            *slot = (start, how_many);
        }
    }
}

/// What one kind's body yields when dissected: the entries and their drop ids, empty when the data states none.
#[must_use]
pub fn dissect_of(world: &World, kind: crate::MonsterKindId) -> (Vec<crate::HarvestSpec>, Vec<String>) {
    let Some(types) = world.iter_entities().find_map(|entity| entity.get::<MonsterTypes>()) else {
        return (Vec::new(), Vec::new());
    };
    let Some(&(start, how_many)) = types.dissect_ranges.get(kind.0 as usize) else {
        return (Vec::new(), Vec::new());
    };
    let start = start as usize;
    let end = start + how_many as usize;
    (
        types.dissect_entries.get(start..end).map_or_else(Vec::new, <[crate::HarvestSpec]>::to_vec),
        types.dissect_ids.get(start..end).map_or_else(Vec::new, <[String]>::to_vec),
    )
}

/// Which list a butchery of a body walks: its dissection list when the action is a dissection and the body states one, and its harvest list otherwise.
///
/// Reference: `( action == butcher_type::DISSECT && !mt.dissect.is_empty() ) ? *mt.dissect : *mt.harvest` (`butchery.cpp:692-694`). The rule is the reference's `&&`: a dissection of a body that states no
/// dissection list walks the harvest list like any other butchery, and a butchery that is not a dissection never looks at the dissection list at all.
#[must_use]
pub fn list_for(
    world: &World,
    kind: crate::MonsterKindId,
    action: crate::ButcherAction,
) -> (Vec<crate::HarvestSpec>, Vec<String>) {
    if action == crate::ButcherAction::Dissect {
        let (entries, ids) = dissect_of(world, kind);
        if !entries.is_empty() {
            return (entries, ids);
        }
    }
    harvest_of(world, kind)
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

/// Gives each kind the body its data type names, by kind.
///
/// The caller resolves `bodytype` through [`crate::creature::body_named`] because that is where the anatomies and their parts live; this table only remembers the answer, so a
/// monster spawned from an id can carry it.
pub fn set_bodies(world: &mut World, entries: &[(crate::MonsterKindId, crate::creature::Body)]) {
    let entity = crate::singletons::singleton::<MonsterTypes, TheMonsterTypes>(world);
    let Some(mut types) = world.get_mut::<MonsterTypes>(entity) else {
        return;
    };
    for (kind, body) in entries {
        let at = kind.0 as usize;
        if types.bodies.len() <= at {
            types.bodies.resize(at + 1, None);
        }
        if let Some(slot) = types.bodies.get_mut(at) {
            *slot = Some(body.clone());
        }
    }
}

/// Gives each kind the three armour figures its type states.
///
/// Reference: `monster::get_armor_type( dt, bp )` (`monster.cpp`) answers from `mtype::armor_bash`, `armor_cut` and `armor_stab`, **per damage type** - which is why a zombie that
/// shrugs off a club can still be cut. A kind the world never resolved has no armour at all rather than zero of each, so a born-bodiless fixture does not silently look armoured.
pub fn set_armours(world: &mut World, entries: &[(crate::MonsterKindId, crate::creature::Armour)]) {
    let entity = crate::singletons::singleton::<MonsterTypes, TheMonsterTypes>(world);
    let Some(mut types) = world.get_mut::<MonsterTypes>(entity) else {
        return;
    };
    for (kind, armour) in entries {
        let at = kind.0 as usize;
        if types.armours.len() <= at {
            types
                .armours
                .resize(at + 1, crate::creature::Armour { bash: 0, cut: 0, stab: 0 });
        }
        if let Some(slot) = types.armours.get_mut(at) {
            *slot = *armour;
        }
    }
}

/// The armour one kind has, or `None` when the world never installed its figures.
#[must_use]
pub fn armour_of(world: &World, kind: crate::MonsterKindId) -> Option<crate::creature::Armour> {
    world
        .iter_entities()
        .find_map(|entity| entity.get::<MonsterTypes>())
        .and_then(|types| types.armours.get(kind.0 as usize).copied())
}

/// The body one kind has, or `None` when the world never resolved its body type.
///
/// `None` is the honest answer rather than an empty body: a creature with no parts is a creature a blow cannot land on, which is what a monster whose anatomy the port could not
/// build really is. A monster whose parts are known but few says so with a body of few parts.
#[must_use]
pub fn body_of(world: &World, kind: crate::MonsterKindId) -> Option<crate::creature::Body> {
    world
        .iter_entities()
        .find_map(|entity| entity.get::<MonsterTypes>())
        .and_then(|types| types.bodies.get(kind.0 as usize).cloned().flatten())
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
        bodies: Vec::new(),
        armours: Vec::new(),
        dissect_entries: Vec::new(),
        dissect_ids: Vec::new(),
        leftovers: Vec::new(),
        dissect_ranges: Vec::new(),
        names,
        hit_points,
        by_name,
        speeds: Vec::new(),
        blood: Vec::new(),
        gibs: Vec::new(),
        death_messages: Vec::new(),
        corpse_types: Vec::new(),
        broken_itypes: Vec::new(),
        death_drops: Vec::new(),
        flags: Vec::new(),
        species: Vec::new(),
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

/// The registry, when the world holds one, without creating it.
///
/// **A read-only question needs a read-only lookup**: [`singleton`] would make the entity to answer for its absence, which a rule that only wants to know a name must not do.
#[must_use]
pub fn registry(world: &World) -> Option<&MonsterTypes> {
    let entity = singletons::holder::<TheMonsterTypes>(world)?;
    world.get::<MonsterTypes>(entity)
}

/// The name of a kind, as the data spells it.
#[must_use]
pub fn name_of(world: &World, kind: MonsterKindId) -> Option<String> {
    registry(world)
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

/// Records every kind's speed, which a polymorph resets its victim to.
///
/// Reference: `monster::poly` sets `Creature::set_speed_base( type->speed )` (`monster.cpp:371`) - the new body moves at the new body's own pace, so a slow thing that
/// becomes a fast thing is fast immediately. The number is the data's own and the registry is where the port keeps it.
pub fn set_speeds(world: &mut World, entries: &[(MonsterKindId, i32)]) {
    let entity = singletons::singleton::<MonsterTypes, TheMonsterTypes>(world);
    if let Some(mut registry) = world.get_mut::<MonsterTypes>(entity) {
        for (kind, speed) in entries {
            let index = kind.0 as usize;
            if registry.speeds.len() <= index {
                registry.speeds.resize(index + 1, 0);
            }
            registry.speeds[index] = *speed;
        }
    }
}

/// Says what flags and species a kind has, which the rules that read a kind rather than a creature need.
pub fn set_flags_and_species(
    world: &mut World,
    entries: &[(MonsterKindId, BTreeSet<String>, BTreeSet<String>)],
) {
    let entity = singletons::singleton::<MonsterTypes, TheMonsterTypes>(world);
    let Some(mut registry) = world.get_mut::<MonsterTypes>(entity) else {
        return;
    };
    for (kind, flags, species) in entries {
        let at = kind.0 as usize;
        if registry.flags.len() <= at {
            registry.flags.resize(at + 1, BTreeSet::new());
            registry.species.resize(at + 1, BTreeSet::new());
        }
        registry.flags[at].clone_from(flags);
        registry.species[at].clone_from(species);
    }
}

/// Whether a kind has a flag, which is `mtype::has_flag`.
#[must_use]
pub fn kind_has_flag(world: &mut World, kind: MonsterKindId, flag: &str) -> bool {
    let entity = singletons::singleton::<MonsterTypes, TheMonsterTypes>(world);
    world
        .get::<MonsterTypes>(entity)
        .and_then(|registry| registry.flags.get(kind.0 as usize))
        .is_some_and(|flags| flags.iter().any(|held| held == flag))
}

/// Whether a kind belongs to a species, which is `mtype::in_species`.
#[must_use]
pub fn kind_in_species(world: &World, kind: MonsterKindId, species: &str) -> bool {
    registry(world)
        .and_then(|registry| registry.species.get(kind.0 as usize))
        .is_some_and(|held| held.iter().any(|name| name == species))
}

/// How fast a kind moves, or zero when the registry never said.
#[must_use]
pub fn speed_of(world: &mut World, kind: MonsterKindId) -> i32 {
    let entity = singletons::singleton::<MonsterTypes, TheMonsterTypes>(world);
    world
        .get::<MonsterTypes>(entity)
        .and_then(|registry| registry.speeds.get(kind.0 as usize).copied())
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
