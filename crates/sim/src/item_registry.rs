//! Which item is which, inside the store.
//!
//! Reference: `item_controller` / `item_factory`'s `itype_id` table, which numbers every item type and answers both
//! directions - the number a stack carries, and the id the data writes in recipes and components.
//!
//! The port kept that table outside the store: callers passed a `name_of` closure around, and the simulation could not
//! answer "what is in my hands" without one. That is a second source of truth wearing a function's clothes, so the
//! names live here, on a singleton entity, and every rule that needs them asks the store.

use bevy_ecs::prelude::*;
use std::collections::BTreeMap;

use crate::pockets::Pocket;
use crate::singletons;
use crate::ItemTypeId;

/// Every item type's id, by the number the store uses for it.
///
/// Index `n` is [`ItemTypeId(n)`]. Index zero is the null item, as it is in the reference's registry.
#[derive(Component, Debug, Clone, Default, PartialEq, Eq)]
pub struct ItemTypes {
    /// The name of each kind, in registry order.
    names: Vec<String>,
    /// The number of each kind, by name.
    by_name: BTreeMap<String, ItemTypeId>,
    /// How much room each kind takes, in millilitres, in registry order.
    ///
    /// Reference: `itype::volume`, which is what `item::volume()` answers for a stack of one. Picking something up costs
    /// fifty moves a litre of it, and the store could not say how big anything was: every caller handed the number in.
    volumes_ml: Vec<i32>,
    /// What each kind weighs, in grams, in registry order.
    ///
    /// Reference: `itype::weight`, which is what `item::weight()` answers for a stack of one. Carrying capacity is
    /// measured against it, so a store that could not say how heavy a thing was could not tell you when you were
    /// overloaded.
    weights_g: Vec<i64>,
    /// How much each kind can hold, in millilitres, in registry order.
    ///
    /// Reference: `item::get_total_capacity`, the sum of the item's pockets. It is what a worn thing adds to
    /// `Character::volume_capacity` (`character.cpp:8883-8889`), so a coat with no pockets adds nothing and a backpack
    /// adds a lot.
    holds_ml: Vec<i32>,
    /// Every kind's pockets, one after another, with [`ItemTypes::pocket_ranges`] saying where each kind's start.
    ///
    /// Reference: `itype::pockets`, which is where a thing goes once a survivor picks it up. The pockets are laid flat
    /// rather than as a list of lists because an architecture gate treats a `Vec<Vec<_>>` inside a component as a
    /// per-square array, and the cheapest way to keep that gate strict is to not write one: a flat list with ranges is
    /// also smaller and reads the same through [`ItemTypes::pockets_of`].
    pockets: Vec<Pocket>,
    /// Where each kind's pockets begin and how many it has, in registry order.
    pocket_ranges: Vec<(u32, u32)>,
}

impl ItemTypes {
    /// The pockets of one kind, as the store holds them.
    #[must_use]
    pub fn pockets_of_in_store(&self, kind: ItemTypeId) -> Vec<Pocket> {
        let Some(&(start, how_many)) = self.pocket_ranges.get(kind.0 as usize) else {
            return Vec::new();
        };
        let start = start as usize;
        self.pockets
            .get(start..start + how_many as usize)
            .map_or_else(Vec::new, <[Pocket]>::to_vec)
    }
}

/// Marks the entity carrying [`ItemTypes`].
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct TheItemTypes;

/// The flags each item kind carries, filed by the number the registry gave it.
///
/// Reference: `item::has_flag` (`item.cpp`), which looks a flag up on the item's type. Kept apart from [`ItemTypes`] because the registry is built from
/// a small [`Kind`] and flags are a separate question with their own loader - and a second installer cannot disagree with the first about a number.
#[derive(Debug, Clone, Default, PartialEq, Component)]
pub struct ItemFlags(pub std::collections::BTreeMap<ItemTypeId, std::collections::BTreeSet<String>>);

/// Marks the entity holding the item flags.
#[derive(Debug, Clone, Copy, Default, Component)]
pub struct TheItemFlags;

/// Installs what flags each item kind carries.
pub fn set_item_flags(world: &mut World, flags: &[(ItemTypeId, &[&str])]) {
    let holder = singletons::singleton::<ItemFlags, TheItemFlags>(world);
    let mut by_kind = std::collections::BTreeMap::new();
    for (kind, names) in flags {
        by_kind.insert(
            *kind,
            names.iter().map(|name| (*name).to_owned()).collect(),
        );
    }
    world.entity_mut(holder).insert(ItemFlags(by_kind));
}

/// The qualities each kind has, by kind. An empty table answers "no quality" for everything, which is the state a world that never loaded item data is in.
#[derive(bevy_ecs::component::Component, Debug, Clone, Default)]
pub struct ItemQualities(pub BTreeMap<u32, BTreeMap<String, i32>>);

/// Marks the entity holding [`ItemQualities`].
#[derive(bevy_ecs::component::Component, Debug, Clone, Copy, Default)]
pub struct TheItemQualities;

/// Installs the quality table, replacing whatever was there.
///
/// Reference: `itype::qualities` (`item_factory.cpp`), the map from a quality id to its level that `quality_requirement::has` reads.
pub fn set_item_qualities(world: &mut World, qualities: &[(ItemTypeId, &[(String, i32)])]) {
    let mut table = ItemQualities::default();
    for (kind, held) in qualities {
        table.0.insert(kind.0, held.iter().cloned().collect());
    }
    let entity = singletons::singleton::<ItemQualities, TheItemQualities>(world);
    world.entity_mut(entity).insert(table);
}

/// Every quality a kind has, as the map the requirement rule asks.
#[must_use]
pub fn qualities_of(world: &World, kind: ItemTypeId) -> BTreeMap<String, i32> {
    singletons::holder::<TheItemQualities>(world)
        .and_then(|holder| world.get::<ItemQualities>(holder))
        .and_then(|table| table.0.get(&kind.0).cloned())
        .unwrap_or_default()
}

/// Whether an item kind carries a flag.
#[must_use]
pub fn kind_has_flag(world: &World, kind: ItemTypeId, flag: &str) -> bool {
    singletons::holder::<TheItemFlags>(world)
        .and_then(|holder| world.get::<ItemFlags>(holder))
        .and_then(|flags| flags.0.get(&kind))
        .is_some_and(|names| names.contains(flag))
}

/// Installs the registry the store will answer from.
///
/// Later calls replace it, which is what a second world load does. The null item stays at zero whatever the data says,
/// because a stack with no kind has to have a number that means nothing.
/// One kind of thing, as the registry needs to know it.
///
/// Taken as a struct rather than a tuple because the tuple had grown to four anonymous numbers and was about to grow a
/// fifth: `("boots", 2_000, 800, 1_500)` does not tell a reader which number is which, and a name for each is cheaper
/// than a comment saying so.
#[derive(Debug, Clone, Copy)]
pub struct Kind<'a> {
    /// The id the data uses.
    pub id: &'a str,
    /// How much room it takes, in millilitres.
    pub volume_ml: i64,
    /// What it weighs, in grams.
    pub weight_g: i64,
    /// The pockets it has.
    pub pockets: &'a [Pocket],
}

/// Installs the registry the store will answer from.
///
/// Later calls replace it, which is what a second world load does. The null item stays at zero whatever the data says,
/// because a stack with no kind has to have a number that means nothing.
pub fn set_item_types(world: &mut World, kinds: &[Kind<'_>]) {
    let mut names = vec![String::new()];
    let mut by_name = BTreeMap::new();
    let mut volumes_ml = vec![0];
    let mut weights_g = vec![0];
    let mut holds_ml = vec![0];
    let mut pockets: Vec<Pocket> = Vec::new();
    let mut pocket_ranges: Vec<(u32, u32)> = vec![(0, 0)];
    // Named apart from the loop variable so the borrow of the pocket slice outlives the numbers taken from it.
    for each in kinds {
        let (id, volume, weight) = (each.id, each.volume_ml, each.weight_g);
        #[allow(clippy::cast_possible_truncation)]
        let number = ItemTypeId(names.len() as u32);
        by_name.insert(id.to_owned(), number);
        names.push(id.to_owned());
        #[allow(clippy::cast_possible_truncation)]
        volumes_ml.push(volume as i32);
        weights_g.push(weight);
        // Reference: `item::get_total_capacity` - a thing can hold as much as its pockets can.
        holds_ml.push(each.pockets.iter().map(|pocket| pocket.capacity_ml).sum());
        #[allow(clippy::cast_possible_truncation)]
        let start = pockets.len() as u32;
        #[allow(clippy::cast_possible_truncation)]
        let how_many = each.pockets.len() as u32;
        pockets.extend_from_slice(each.pockets);
        pocket_ranges.push((start, how_many));
    }
    let entity = singletons::singleton::<ItemTypes, TheItemTypes>(world);
    world.entity_mut(entity).insert(ItemTypes {
        names,
        by_name,
        volumes_ml,
        weights_g,
        holds_ml,
        pockets,
        pocket_ranges,
    });
}

/// How much room a kind takes, in millilitres.
///
/// A kind nobody numbered takes no room, which is the same answer the reference gives for the null item.
#[must_use]
pub fn volume_of(world: &mut World, kind: ItemTypeId) -> i32 {
    let entity = singletons::singleton::<ItemTypes, TheItemTypes>(world);
    world
        .get::<ItemTypes>(entity)
        .and_then(|registry| registry.volumes_ml.get(kind.0 as usize).copied())
        .unwrap_or(0)
}

/// A copy of the registry that rules can hold while they work on the store.
///
/// Taken as a value because a rule that mutates the store cannot also be holding a borrow of it - and a rule that has
/// to ask the store for every name it meets would have to.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ItemNames {
    /// The name of each kind, in registry order.
    pub names: Vec<String>,
    /// The number of each kind, by name.
    pub by_name: BTreeMap<String, ItemTypeId>,
    /// How much room each kind takes, in millilitres, in registry order.
    pub volumes_ml: Vec<i32>,
    /// What each kind weighs, in grams, in registry order.
    pub weights_g: Vec<i64>,
    /// How much each kind can hold, in millilitres, in registry order.
    pub holds_ml: Vec<i32>,
    /// Every kind's pockets, one after another.
    pub pockets: Vec<Pocket>,
    /// Where each kind's pockets begin and how many it has, in registry order.
    pub pocket_ranges: Vec<(u32, u32)>,
}

impl ItemNames {
    /// The name of a kind, as the data spells it.
    #[must_use]
    pub fn name_of(&self, kind: ItemTypeId) -> Option<String> {
        self.names
            .get(kind.0 as usize)
            .filter(|name| !name.is_empty())
            .cloned()
    }

    /// The name of a kind, asked without needing to change the store - the same answer as [`name_of`], for callers that only
/// hold the world by reference.
#[must_use]
pub fn name_in(world: &World, kind: ItemTypeId) -> Option<String> {
    let mut query = world.try_query_filtered::<&ItemTypes, With<TheItemTypes>>()?;
    let registry = query.iter(world).next()?;
    registry
        .names
        .get(kind.0 as usize)
        .filter(|name| !name.is_empty())
        .cloned()
}

/// The kind a name refers to.
    #[must_use]
    pub fn id_of(&self, name: &str) -> Option<ItemTypeId> {
        self.by_name.get(name).copied()
    }

    /// How much room a kind takes, in millilitres.
    #[must_use]
    pub fn volume_of(&self, kind: ItemTypeId) -> i32 {
        self.volumes_ml.get(kind.0 as usize).copied().unwrap_or(0)
    }

    /// What a kind weighs, in grams.
    #[must_use]
    pub fn weight_of(&self, kind: ItemTypeId) -> i64 {
        self.weights_g.get(kind.0 as usize).copied().unwrap_or(0)
    }

    /// How much a kind can hold, in millilitres.
    #[must_use]
    pub fn holds_ml(&self, kind: ItemTypeId) -> i32 {
        self.holds_ml.get(kind.0 as usize).copied().unwrap_or(0)
    }

    /// The pockets of a kind.
    #[must_use]
    pub fn pockets_of(&self, kind: ItemTypeId) -> &[Pocket] {
        let Some(&(start, how_many)) = self.pocket_ranges.get(kind.0 as usize) else {
            return &[];
        };
        let start = start as usize;
        self.pockets
            .get(start..start + how_many as usize)
            .unwrap_or(&[])
    }
}

/// Takes a copy of the registry.
#[must_use]
pub fn snapshot(world: &mut World) -> ItemNames {
    let entity = singletons::singleton::<ItemTypes, TheItemTypes>(world);
    world.get::<ItemTypes>(entity).map_or_else(ItemNames::default, |registry| ItemNames {
        names: registry.names.clone(),
        by_name: registry.by_name.clone(),
        volumes_ml: registry.volumes_ml.clone(),
        weights_g: registry.weights_g.clone(),
        holds_ml: registry.holds_ml.clone(),
        pockets: registry.pockets.clone(),
        pocket_ranges: registry.pocket_ranges.clone(),
    })
}

/// The name of a kind, as the data spells it.
#[must_use]
pub fn name_of(world: &mut World, kind: ItemTypeId) -> Option<String> {
    let entity = singletons::singleton::<ItemTypes, TheItemTypes>(world);
    let registry = world.get::<ItemTypes>(entity)?;
    registry.names.get(kind.0 as usize).filter(|name| !name.is_empty()).cloned()
}

/// The name of a kind, asked without needing to change the store - the same answer as [`name_of`], for callers that only
/// hold the world by reference.
#[must_use]
pub fn name_in(world: &World, kind: ItemTypeId) -> Option<String> {
    let mut query = world.try_query_filtered::<&ItemTypes, With<TheItemTypes>>()?;
    let registry = query.iter(world).next()?;
    registry
        .names
        .get(kind.0 as usize)
        .filter(|name| !name.is_empty())
        .cloned()
}

/// The kind a name refers to.
#[must_use]
pub fn id_of(world: &mut World, name: &str) -> Option<ItemTypeId> {
    let entity = singletons::singleton::<ItemTypes, TheItemTypes>(world);
    let registry = world.get::<ItemTypes>(entity)?;
    registry.by_name.get(name).copied()
}

/// What a kind weighs, in grams.
#[must_use]
pub fn weight_of(world: &mut World, kind: ItemTypeId) -> i64 {
    let entity = singletons::singleton::<ItemTypes, TheItemTypes>(world);
    world
        .get::<ItemTypes>(entity)
        .and_then(|registry| registry.weights_g.get(kind.0 as usize).copied())
        .unwrap_or(0)
}

/// How much a kind can hold, in millilitres.
#[must_use]
pub fn holds_ml(world: &mut World, kind: ItemTypeId) -> i32 {
    let entity = singletons::singleton::<ItemTypes, TheItemTypes>(world);
    world
        .get::<ItemTypes>(entity)
        .and_then(|registry| registry.holds_ml.get(kind.0 as usize).copied())
        .unwrap_or(0)
}

/// The pockets of a kind.
#[must_use]
pub fn pockets_of(world: &mut World, kind: ItemTypeId) -> Vec<Pocket> {
    let entity = singletons::singleton::<ItemTypes, TheItemTypes>(world);
    world
        .get::<ItemTypes>(entity)
        .map_or_else(Vec::new, |registry| registry.pockets_of_in_store(kind))
}

/// How many kinds the registry knows, not counting the null item.
#[must_use]
pub fn count(world: &mut World) -> usize {
    let entity = singletons::singleton::<ItemTypes, TheItemTypes>(world);
    world.get::<ItemTypes>(entity).map_or(0, |registry| registry.by_name.len())
}
