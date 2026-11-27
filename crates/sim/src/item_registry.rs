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

use crate::ItemTypeId;
use crate::pockets::Pocket;
use crate::singletons;

/// Every item type's id, by the number the store uses for it.
///
/// Index `n` is [`ItemTypeId(n)`]. Index zero is the null item, as it is in the reference's registry.
#[derive(Component, Debug, Clone, Default, PartialEq)]
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
    /// How many charges a kind's own volume makes, in registry order - the reference's `itype::stack_size` for a kind that
    /// counts by charges, and 1 for everything else, which makes a single item one charge of itself.
    ///
    /// Reference: `item::charges_per_volume` (`item.cpp:1391-1416`) answers `vol * stack_size / type->volume` for a stack-able
    /// kind and `vol / volume` for anything else. The two numbers are kept apart because the division is done on the volume the
    /// caller asks about, not on a pre-divided "one charge is 1.25 ml" - integer millilitres would round that to 1 and a litre
    /// would hold a thousand charges of gasoline instead of eight hundred.
    charge_stacks: Vec<i64>,
    /// What each kind fetches when **sold**, in cents, in registry order - the reference's `itype::price_post`, which is what `item::price( true )` reads.
    ///
    /// **A different number from what a shop charges**, and the shop restock's value loop (`npc.cpp:2168`) subtracts this one. Filled by
    /// [`set_prices_post`] rather than by [`set_item_types`], because a [`Kind`] is built with the few measures a combat or carry rule needs and a price is not one of them.
    prices_post: Vec<i64>,
    /// Whether each kind is a **liquid**, in registry order - the reference's `made_of( phase_id::LIQUID )`, which a trade refuses to offer and a spill rules on.
    ///
    /// Filled by [`set_liquids`] for the same reason the prices are: a [`Kind`] built by hand in a test has no phase, and a kind that a test never says is liquid is a solid.
    liquids: Vec<bool>,
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
    /// What each kind does when thrown, in points of damage, in registry order.
    ///
    /// Reference: `item::base_damage_thrown` (`item.cpp:7654-7658`) returns the type's `thrown_damage`, which `item_factory.cpp:324-326` fills with
    /// `melee.damage_map[bash] * 0.75` when the data names none. The port holds it as one number per kind because that is all a throw reads.
    thrown_damage: Vec<f64>,
    /// What each kind does when swung, in points of bash damage.
    melee_bash: Vec<f64>,
    /// What each kind does when swung, in points of cutting damage.
    melee_cut: Vec<f64>,
    /// What each kind does when swung, in points of stabbing damage.
    melee_stab: Vec<f64>,
    /// How well each kind is balanced for a swing.
    to_hit: Vec<i32>,
    /// How much damage each kind takes before it is ruined, in registry order, or `None` where the reference's table names no figure.
    ///
    /// Reference: `itype::damage_max` (`corpus/items/damage.json`). Kept beside the kinds because destruction is a question about the kind - `damage + qty > max_damage` - and the
    /// difference between "cannot be damaged" (zero) and "not known" (absent) is one a rule must not blur.
    damage_limits: Vec<Option<i32>>,
}

impl ItemTypes {
    /// How much damage a kind takes before it is ruined, when the reference's table names it.
    #[must_use]
    pub fn max_damage_of(&self, kind: ItemTypeId) -> Option<i32> {
        self.damage_limits
            .get(usize::try_from(kind.0).ok()?)
            .copied()
            .flatten()
    }

    /// The kind a name refers to.
    #[must_use]
    pub fn id_of(&self, name: &str) -> Option<ItemTypeId> {
        self.by_name.get(name).copied()
    }

    /// How much room a kind takes, in millilitres - `itype::volume`, which a stack of one answers.
    #[must_use]
    pub fn volume_of(&self, kind: ItemTypeId) -> i64 {
        self.volumes_ml
            .get(kind.0 as usize)
            .map_or(0, |volume| i64::from(*volume))
    }

    /// What a kind fetches when sold, in cents.
    ///
    /// Reference: `itype::price_post`, read by `item::price( true )` (`item.cpp:7187`). Zero means the content never said, which is the same answer the reference gives for a
    /// thing with no price.
    #[must_use]
    pub fn price_post_of(&self, kind: ItemTypeId) -> i64 {
        self.prices_post.get(kind.0 as usize).copied().unwrap_or(0)
    }

    /// Whether a kind is a liquid.
    #[must_use]
    pub fn is_liquid_of(&self, kind: ItemTypeId) -> bool {
        self.liquids.get(kind.0 as usize).copied().unwrap_or(false)
    }

    /// What a kind weighs, in grams.
    #[must_use]
    pub fn weight_of(&self, kind: ItemTypeId) -> i64 {
        self.weights_g.get(kind.0 as usize).copied().unwrap_or(0)
    }

    /// What a kind does when swung, in points of bash damage.
    #[must_use]
    pub fn melee_bash_of(&self, kind: ItemTypeId) -> f64 {
        self.melee_bash.get(kind.0 as usize).copied().unwrap_or(0.0)
    }

    /// What a kind does when swung, in points of cutting damage.
    #[must_use]
    pub fn melee_cut_of(&self, kind: ItemTypeId) -> f64 {
        self.melee_cut.get(kind.0 as usize).copied().unwrap_or(0.0)
    }

    /// What a kind does when swung, in points of stabbing damage.
    #[must_use]
    pub fn melee_stab_of(&self, kind: ItemTypeId) -> f64 {
        self.melee_stab.get(kind.0 as usize).copied().unwrap_or(0.0)
    }

    /// How well a kind is balanced for a swing.
    #[must_use]
    pub fn to_hit_of(&self, kind: ItemTypeId) -> i32 {
        self.to_hit.get(kind.0 as usize).copied().unwrap_or(-2)
    }

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
pub struct ItemFlags(
    pub std::collections::BTreeMap<ItemTypeId, std::collections::BTreeSet<String>>,
);

/// Marks the entity holding the item flags.
#[derive(Debug, Clone, Copy, Default, Component)]
pub struct TheItemFlags;

/// Installs what flags each item kind carries.
pub fn set_item_flags(world: &mut World, flags: &[(ItemTypeId, &[&str])]) {
    let holder = singletons::singleton::<ItemFlags, TheItemFlags>(world);
    let mut by_kind = std::collections::BTreeMap::new();
    for (kind, names) in flags {
        by_kind.insert(*kind, names.iter().map(|name| (*name).to_owned()).collect());
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
    /// What it does when thrown, in points of damage, before the arm's own share.
    pub thrown_damage: f64,
    /// What it does when swung, in points of bash damage.
    pub melee_bash: f64,
    /// What it does when swung, in points of cutting damage.
    pub melee_cut: f64,
    /// What it does when swung, in points of stabbing damage.
    pub melee_stab: f64,
    /// How well it is balanced for a swing, which is the last term of a character's aiming.
    pub to_hit: i32,
}

/// Installs the registry the store will answer from.
///
/// Later calls replace it, which is what a second world load does. The null item stays at zero whatever the data says,
/// because a stack with no kind has to have a number that means nothing.
pub fn set_item_types(world: &mut World, kinds: &[Kind<'_>]) {
    let mut names = vec![String::new()];
    let mut by_name = BTreeMap::new();
    let mut damage_limits: Vec<Option<i32>> = vec![None];
    let mut volumes_ml = vec![0];
    let mut charge_stacks = vec![1];
    let mut weights_g = vec![0];
    let mut prices_post: Vec<i64> = vec![0];
    let mut liquids: Vec<bool> = vec![false];
    let mut holds_ml = vec![0];
    let mut pockets: Vec<Pocket> = Vec::new();
    let mut pocket_ranges: Vec<(u32, u32)> = vec![(0, 0)];
    let mut thrown_damage = vec![0.0];
    let mut melee_bash = vec![0.0];
    let mut melee_cut = vec![0.0];
    let mut melee_stab = vec![0.0];
    let mut to_hit = vec![-2];
    // Named apart from the loop variable so the borrow of the pocket slice outlives the numbers taken from it.
    for each in kinds {
        let (id, volume, weight) = (each.id, each.volume_ml, each.weight_g);
        #[allow(clippy::cast_possible_truncation)]
        let number = ItemTypeId(names.len() as u32);
        by_name.insert(id.to_owned(), number);
        names.push(id.to_owned());
        #[allow(clippy::cast_possible_truncation)]
        volumes_ml.push(volume as i32);
        charge_stacks.push(1);
        weights_g.push(weight);
        prices_post.push(0);
        liquids.push(false);
        thrown_damage.push(each.thrown_damage);
        melee_bash.push(each.melee_bash);
        melee_cut.push(each.melee_cut);
        melee_stab.push(each.melee_stab);
        to_hit.push(each.to_hit);
        damage_limits.push(None);
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
        charge_stacks,
        prices_post,
        liquids,
        holds_ml,
        pockets,
        pocket_ranges,
        thrown_damage,
        melee_bash,
        melee_cut,
        melee_stab,
        to_hit,
        damage_limits,
    });
}

/// How many charges a kind's own volume makes, by the data's own name for it.
///
/// Reference: `itype::stack_size` (`itype.h`), which only means something for a kind that counts by charges - a litre of
/// gasoline is eight hundred charges because a charge of it is 1.25 ml, and a rock is one charge of itself. The item data
/// carries the number on the type it read, and this is how it reaches the store: by name, because the registry's numbers are
/// assigned in data order and a caller that computed a list would have to know that order.
pub fn set_charge_stacks(world: &mut World, stacks: &std::collections::BTreeMap<String, i64>) {
    let entity = singletons::singleton::<ItemTypes, TheItemTypes>(world);
    let Some(mut registry) = world.get_mut::<ItemTypes>(entity) else {
        return;
    };
    for (name, stack) in stacks {
        if let Some(number) = registry.by_name.get(name).copied()
            && let Some(slot) = registry.charge_stacks.get_mut(number.0 as usize)
        {
            *slot = (*stack).max(1);
        }
    }
}

/// How many charges of a kind fit in a volume - the reference's `item::charges_per_volume` (`item.cpp:1391-1416`).
///
/// A kind that counts by charges answers `volume * stack_size / the kind's own volume`, so the arithmetic stays in whole
/// millilitres for as long as possible; everything else answers `volume / the kind's volume`. A kind with no volume answers
/// the reference's `INFINITE_CHARGES`.
#[must_use]
pub fn charges_per_volume(world: &World, kind: ItemTypeId, volume_ml: i64) -> i64 {
    let Some(registry) = world
        .try_query_filtered::<&ItemTypes, With<TheItemTypes>>()
        .and_then(|mut query| query.iter(world).next().cloned())
    else {
        return 0;
    };
    let item_volume = registry.volume_of(kind);
    if item_volume == 0 {
        return i64::from(i32::MAX);
    }
    let stack = registry
        .charge_stacks
        .get(kind.0 as usize)
        .copied()
        .unwrap_or(1)
        .max(1);
    (volume_ml * stack / item_volume).min(i64::from(i32::MAX))
}

/// Installs what each kind fetches when sold, in cents, by the id the data uses.
///
/// This is the one column a [`Kind`] does not carry, because a `Kind` is built by hand in a hundred tests that have no shop in them. The prices come from the same content
/// types every other measure does, and they land in the same registry, so there is still one table and no second place to drift.
pub fn set_prices_post(world: &mut World, prices: &[(String, i64)]) {
    let entity = singletons::singleton::<ItemTypes, TheItemTypes>(world);
    let Some(mut held) = world.get_mut::<ItemTypes>(entity) else {
        return;
    };
    for (id, cents) in prices {
        let Some(kind) = held.by_name.get(id).copied() else {
            continue;
        };
        if let Some(slot) = held.prices_post.get_mut(kind.0 as usize) {
            *slot = *cents;
        }
    }
}

/// Installs which kinds are liquids, by the id the data uses.
///
/// The same shape as [`set_prices_post`]: the column lives in the one registry, and a test that never calls this has no liquids rather than an unknown.
pub fn set_liquids(world: &mut World, liquids: &[(String, bool)]) {
    let entity = singletons::singleton::<ItemTypes, TheItemTypes>(world);
    let Some(mut held) = world.get_mut::<ItemTypes>(entity) else {
        return;
    };
    for (id, is_liquid) in liquids {
        let Some(kind) = held.by_name.get(id).copied() else {
            continue;
        };
        if let Some(slot) = held.liquids.get_mut(kind.0 as usize) {
            *slot = *is_liquid;
        }
    }
}

/// Whether a kind is a liquid.
#[must_use]
pub fn is_liquid(world: &World, kind: ItemTypeId) -> bool {
    world
        .try_query_filtered::<&ItemTypes, With<TheItemTypes>>()
        .and_then(|mut query| {
            query
                .iter(world)
                .next()
                .map(|types| types.is_liquid_of(kind))
        })
        .unwrap_or(false)
}

/// What a kind fetches when sold, in cents.
#[must_use]
pub fn price_post_of(world: &mut World, kind: ItemTypeId) -> i64 {
    let entity = singletons::singleton::<ItemTypes, TheItemTypes>(world);
    world
        .get::<ItemTypes>(entity)
        .map_or(0, |types| types.price_post_of(kind))
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
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ItemNames {
    /// The name of each kind, in registry order.
    pub names: Vec<String>,
    /// The number of each kind, by name.
    pub by_name: BTreeMap<String, ItemTypeId>,
    /// How much room each kind takes, in millilitres, in registry order.
    pub volumes_ml: Vec<i32>,
    /// What each kind weighs, in grams, in registry order.
    pub weights_g: Vec<i64>,
    /// How many charges a kind's own volume makes, in registry order - the reference's `itype::stack_size` for a kind that
    /// counts by charges, and 1 for everything else, which makes a single item one charge of itself.
    ///
    /// Reference: `item::charges_per_volume` (`item.cpp:1391-1416`) answers `vol * stack_size / type->volume` for a stack-able
    /// kind and `vol / volume` for anything else. The two numbers are kept apart because the division is done on the volume the
    /// caller asks about, not on a pre-divided "one charge is 1.25 ml" - integer millilitres would round that to 1 and a litre
    /// would hold a thousand charges of gasoline instead of eight hundred.
    /// How much each kind can hold, in millilitres, in registry order.
    pub holds_ml: Vec<i32>,
    /// Every kind's pockets, one after another.
    pub pockets: Vec<Pocket>,
    /// Where each kind's pockets begin and how many it has, in registry order.
    pub pocket_ranges: Vec<(u32, u32)>,
    /// What each kind does when thrown, in points of damage, in registry order.
    ///
    /// Reference: `item::base_damage_thrown` (`item.cpp:7654-7658`) returns the type's `thrown_damage`, which `item_factory.cpp:324-326` fills with
    /// `melee.damage_map[bash] * 0.75` when the data names none. The port holds it as one number per kind because that is all a throw reads.
    pub thrown_damage: Vec<f64>,
    /// What each kind does when swung, in points of bash damage, in registry order.
    ///
    /// Reference: the `bash` entry of `itype::melee_damage` (`item_factory.cpp:4109`). A character's blow carries it; a throw does not read it.
    pub melee_bash: Vec<f64>,
    /// What each kind does when swung, in points of cutting damage, in registry order.
    pub melee_cut: Vec<f64>,
    /// What each kind does when swung, in points of stabbing damage, in registry order.
    pub melee_stab: Vec<f64>,
    /// How well each kind is balanced for a swing, in registry order.
    ///
    /// Reference: `itype::m_to_hit` (`itype.h:1540`), read by `item::get_to_hit`. It is the last term of `Character::get_hit_weapon` (`melee.cpp:370`).
    pub to_hit: Vec<i32>,
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

    /// What a kind does when thrown, in points of damage, before the arm's own share.
    #[must_use]
    pub fn thrown_of(&self, kind: ItemTypeId) -> f64 {
        self.thrown_damage
            .get(kind.0 as usize)
            .copied()
            .unwrap_or(0.0)
    }

    /// What a kind does when swung, in points of bash damage.
    ///
    /// Reference: the `bash` entry of `itype::melee_damage` (`item_factory.cpp:4109`). A character's own blow carries this plus the weapon's cut, which is why the two are kept
    /// apart rather than summed here: armour resists them differently.
    #[must_use]
    pub fn melee_bash_of(&self, kind: ItemTypeId) -> f64 {
        self.melee_bash.get(kind.0 as usize).copied().unwrap_or(0.0)
    }

    /// What a kind does when swung, in points of cutting damage.
    #[must_use]
    pub fn melee_cut_of(&self, kind: ItemTypeId) -> f64 {
        self.melee_cut.get(kind.0 as usize).copied().unwrap_or(0.0)
    }

    /// What a kind does when swung, in points of stabbing damage.
    #[must_use]
    pub fn melee_stab_of(&self, kind: ItemTypeId) -> f64 {
        self.melee_stab.get(kind.0 as usize).copied().unwrap_or(0.0)
    }

    /// How well a kind is balanced for a swing, as `item::get_to_hit` answers.
    #[must_use]
    pub fn to_hit_of(&self, kind: ItemTypeId) -> i32 {
        self.to_hit.get(kind.0 as usize).copied().unwrap_or(-2)
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

/// What a kind weighs, in grams, read without a mutable handle.
#[must_use]
pub fn weight_in(world: &World, kind: ItemTypeId) -> i64 {
    peek(world).map_or(0, |registry| registry.weight_of(kind))
}

/// **Installs how much damage each kind takes**, from the reference's own table.
///
/// Reference: `itype::damage_max`, reported by `--dump-damage`. Kept apart from [`set_item_types`] because the two come from different recordings and a world may install either
/// alone: a kind with no figure answers `None`, and a rule that needs the figure treats that as "unknown" rather than "cannot be damaged".
pub fn set_damage_limits(world: &mut World, limits: &std::collections::BTreeMap<String, i32>) {
    let entity = singletons::singleton::<ItemTypes, TheItemTypes>(world);
    let Some(mut types) = world.get_mut::<ItemTypes>(entity) else {
        return;
    };
    for (name, limit) in limits {
        let Some(slot) = types
            .by_name
            .get(name)
            .and_then(|kind| usize::try_from(kind.0).ok())
        else {
            continue;
        };
        if let Some(cell) = types.damage_limits.get_mut(slot) {
            *cell = Some(*limit);
        }
    }
}

/// **How much damage a kind takes before it is ruined**, from the reference's own table.
///
/// Reference: `itype::damage_max`, reported by `--dump-damage`. A kind the table does not name answers `None` rather than zero, because a rock's zero means "any damage destroys it"
/// and an unnamed kind means "the port does not know" - and a rule that conflated them would destroy what the reference keeps.
#[must_use]
pub fn max_damage_in(world: &World, kind: ItemTypeId) -> Option<i32> {
    peek(world).and_then(|registry| registry.max_damage_of(kind))
}

/// The kind a name refers to, read without a mutable handle.
#[must_use]
pub fn id_in(world: &World, name: &str) -> Option<ItemTypeId> {
    peek(world).and_then(|registry| registry.id_of(name))
}

/// How much room a kind takes, in millilitres, read without a mutable handle.
#[must_use]
pub fn volume_in(world: &World, kind: ItemTypeId) -> i64 {
    peek(world).map_or(0, |registry| registry.volume_of(kind))
}

/// A look at the registry without taking a copy of it.
///
/// The melee rules ask four things of one kind at a time, and `snapshot` clones every name and every table to answer them. This hands back a reference to the tables themselves;
/// the borrow is read-only, so nothing can change the registry while it is held.
#[must_use]
pub fn peek(world: &World) -> Option<&ItemTypes> {
    singletons::holder::<TheItemTypes>(world).and_then(|holder| world.get::<ItemTypes>(holder))
}

/// Takes a copy of the registry.
#[must_use]
pub fn snapshot(world: &mut World) -> ItemNames {
    let entity = singletons::singleton::<ItemTypes, TheItemTypes>(world);
    world
        .get::<ItemTypes>(entity)
        .map_or_else(ItemNames::default, |registry| ItemNames {
            names: registry.names.clone(),
            by_name: registry.by_name.clone(),
            volumes_ml: registry.volumes_ml.clone(),
            weights_g: registry.weights_g.clone(),
            holds_ml: registry.holds_ml.clone(),
            pockets: registry.pockets.clone(),
            pocket_ranges: registry.pocket_ranges.clone(),
            thrown_damage: registry.thrown_damage.clone(),
            melee_bash: registry.melee_bash.clone(),
            melee_cut: registry.melee_cut.clone(),
            melee_stab: registry.melee_stab.clone(),
            to_hit: registry.to_hit.clone(),
        })
}

/// The name of a kind, as the data spells it.
#[must_use]
pub fn name_of(world: &mut World, kind: ItemTypeId) -> Option<String> {
    let entity = singletons::singleton::<ItemTypes, TheItemTypes>(world);
    let registry = world.get::<ItemTypes>(entity)?;
    registry
        .names
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
    world
        .get::<ItemTypes>(entity)
        .map_or(0, |registry| registry.by_name.len())
}

/// How well each kind resists being chipped by use, filed by the number the registry gave it.
///
/// Reference: `item::chip_resistance()` (`item.cpp:9375-9409`), the figure `Character::handle_melee_wear` divides a weapon's wear chance by. **Kept beside the flags and for the same reason**:
/// it is a question asked *of a kind*, not part of the kind's identity, and a table that nothing has installed answers the reference's own fallback - **2**, the number it returns for an item
/// made of nothing (`:9400-9402`).
#[derive(Debug, Clone, Default, PartialEq, Component)]
pub struct ItemChipResistance(pub std::collections::BTreeMap<ItemTypeId, i32>);

/// Marks the entity holding [`ItemChipResistance`].
#[derive(Debug, Clone, Copy, Default, Component)]
pub struct TheItemChipResistance;

/// Installs how well each kind resists chipping.
pub fn set_item_chip_resistance(world: &mut World, resistance: &[(ItemTypeId, i32)]) {
    let holder = singletons::singleton::<ItemChipResistance, TheItemChipResistance>(world);
    world
        .entity_mut(holder)
        .insert(ItemChipResistance(resistance.iter().copied().collect()));
}

/// How well a kind resists chipping, or **2** for a kind nobody has described - the reference's own fallback.
#[must_use]
pub fn chip_resistance_of(world: &mut World, kind: ItemTypeId) -> i32 {
    let holder = singletons::singleton::<ItemChipResistance, TheItemChipResistance>(world);
    world
        .get::<ItemChipResistance>(holder)
        .and_then(|table| table.0.get(&kind).copied())
        .unwrap_or(2)
}

/// Sets how much room a kind takes, for a store whose types were installed without one.
///
/// The volume table is filled by `set_item_types`; this is the targeted way to say what a kind measures, for a caller that built a world from facts rather than from the data.
pub fn set_item_volumes(world: &mut World, volumes: &[(ItemTypeId, i32)]) {
    let holder = singletons::singleton::<ItemTypes, TheItemTypes>(world);
    if let Some(mut types) = world.get_mut::<ItemTypes>(holder) {
        for (kind, volume) in volumes {
            #[allow(clippy::cast_possible_truncation)]
            let at = kind.0 as usize;
            if types.volumes_ml.len() <= at {
                // **Grow rather than skip**: a store built from facts has a short table, and a silently skipped kind would answer a volume of zero.
                types.volumes_ml.resize(at + 1, 0);
            }
            if let Some(slot) = types.volumes_ml.get_mut(at) {
                *slot = *volume;
            }
        }
        return;
    }
    let mut types = ItemTypes::default();
    for (kind, volume) in volumes {
        #[allow(clippy::cast_possible_truncation)]
        let at = kind.0 as usize;
        if types.volumes_ml.len() <= at {
            types.volumes_ml.resize(at + 1, 0);
        }
        if let Some(slot) = types.volumes_ml.get_mut(at) {
            *slot = *volume;
        }
    }
    world.entity_mut(holder).insert(types);
}

/// Which faults each kind can catch, with their weights - the column `item::set_random_fault_of_type` picks from (`item.cpp:7854-7868`).
///
/// Reference: `itype::faults`, a weighted list the item factory fills from the data. The **fault type** is a property of the fault, not of this list, so the filter "mechanical damage only" is
/// applied by the caller; what lives here is what the kind can catch and how likely each one is.
#[derive(Debug, Clone, Default, PartialEq, Component)]
pub struct ItemFaultWeights(pub std::collections::BTreeMap<ItemTypeId, Vec<(String, i32)>>);

/// Marks the entity holding [`ItemFaultWeights`].
#[derive(Debug, Clone, Copy, Default, Component)]
pub struct TheItemFaultWeights;

/// Installs which faults each kind can catch.
pub fn set_item_fault_weights(world: &mut World, weights: &[(ItemTypeId, Vec<(String, i32)>)]) {
    let holder = singletons::singleton::<ItemFaultWeights, TheItemFaultWeights>(world);
    world
        .entity_mut(holder)
        .insert(ItemFaultWeights(weights.iter().cloned().collect()));
}

/// The faults a kind can catch, or nothing at all for a kind nobody has described.
#[must_use]
pub fn fault_weights_of(world: &mut World, kind: ItemTypeId) -> Vec<(String, i32)> {
    let holder = singletons::singleton::<ItemFaultWeights, TheItemFaultWeights>(world);
    world
        .get::<ItemFaultWeights>(holder)
        .and_then(|table| table.0.get(&kind).cloned())
        .unwrap_or_default()
}

/// Everything the store needs to know about one item kind, in one value.
///
/// **Four tables, one pass.** The flags, the qualities, the chip resistance and the fault weights are all questions asked of a *kind*, and they are filled from the same walk over the prototypes -
/// four installers fed by four separate walks is how two of them end up disagreeing about what kind seven is.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct KindFacts {
    /// The flags the kind carries.
    pub flags: Vec<String>,
    /// Its qualities and their levels.
    pub qualities: Vec<(String, i32)>,
    /// How well it resists being chipped, `item::chip_resistance()`.
    pub chip_resistance: i32,
    /// The faults it can catch, with weights, in the data's order.
    pub faults: Vec<(String, i32)>,
    /// Whether the kind is measured in charges rather than in units - `item::count_by_charges`.
    pub count_by_charges: bool,
    /// Whether it can be eaten or drunk, which is the branch `item::ammo_sufficient` answers `true` for.
    pub is_comestible: bool,
}

/// What a kind says about **how it is counted and used**, which is one table because the two questions are read together.
///
/// Reference: `itype::count_by_charges` (`itype.h:1592`) and `itype::comestible`, both read by `item::ammo_sufficient` (`item.cpp:11569-11582`).
#[derive(Debug, Clone, Default, PartialEq, Component)]
pub struct KindTraits(pub std::collections::BTreeMap<ItemTypeId, (bool, bool)>);

/// Marks the entity holding [`KindTraits`].
#[derive(Debug, Clone, Copy, Default, Component)]
pub struct TheKindTraits;

/// Installs the counting and edible tables.
pub fn set_kind_traits(world: &mut World, kinds: &[(ItemTypeId, bool, bool)]) {
    let holder = singletons::singleton::<KindTraits, TheKindTraits>(world);
    let by_kind = kinds
        .iter()
        .map(|(kind, counted, edible)| (*kind, (*counted, *edible)))
        .collect();
    world.entity_mut(holder).insert(KindTraits(by_kind));
}

/// Fills every kind table from one list, so no two of them can disagree about what a kind is.
pub fn install_kind_facts(world: &mut World, kinds: &[(ItemTypeId, KindFacts)]) {
    let flags: Vec<(ItemTypeId, Vec<&str>)> = kinds
        .iter()
        .map(|(kind, facts)| (*kind, facts.flags.iter().map(String::as_str).collect()))
        .collect();
    let flag_slices: Vec<(ItemTypeId, &[&str])> = flags
        .iter()
        .map(|(kind, names)| (*kind, names.as_slice()))
        .collect();
    set_item_flags(world, &flag_slices);
    let qualities: Vec<(ItemTypeId, Vec<(String, i32)>)> = kinds
        .iter()
        .map(|(kind, facts)| (*kind, facts.qualities.clone()))
        .collect();
    let quality_slices: Vec<(ItemTypeId, &[(String, i32)])> = qualities
        .iter()
        .map(|(kind, levels)| (*kind, levels.as_slice()))
        .collect();
    set_item_qualities(world, &quality_slices);
    let chip: Vec<(ItemTypeId, i32)> = kinds
        .iter()
        .map(|(kind, facts)| (*kind, facts.chip_resistance))
        .collect();
    set_item_chip_resistance(world, &chip);
    let faults: Vec<(ItemTypeId, Vec<(String, i32)>)> = kinds
        .iter()
        .map(|(kind, facts)| (*kind, facts.faults.clone()))
        .collect();
    set_item_fault_weights(world, &faults);
    let traits: Vec<(ItemTypeId, bool, bool)> = kinds
        .iter()
        .map(|(kind, facts)| (*kind, facts.count_by_charges, facts.is_comestible))
        .collect();
    set_kind_traits(world, &traits);
}

/// What kind of damage each fault is, filed by fault id: `Fault::fault_type` from the catalogue, which is what `set_random_fault_of_type` filters on.
///
/// This is the **catalogue's** half of the fault pick: `ItemFaultWeights` says which faults a kind can catch and how likely, and this says which of those are mechanical damage. A fault whose
/// id is not here has no type, so it is never eligible - the reference's `f.obj.obj().type() == fault_type` simply fails to match.
#[derive(Debug, Clone, Default, PartialEq, Component)]
pub struct FaultTypes(pub std::collections::BTreeMap<String, String>);

/// Marks the entity holding [`FaultTypes`].
#[derive(Debug, Clone, Copy, Default, Component)]
pub struct TheFaultTypes;

/// Installs the fault catalogue's types.
pub fn set_fault_types(world: &mut World, faults: &[(String, String)]) {
    let holder = singletons::singleton::<FaultTypes, TheFaultTypes>(world);
    world
        .entity_mut(holder)
        .insert(FaultTypes(faults.iter().cloned().collect()));
}

/// What kind of fault an id names, or nothing when the catalogue has never been installed or does not name it.
#[must_use]
pub fn fault_type_of(world: &mut World, fault: &str) -> Option<String> {
    let holder = singletons::singleton::<FaultTypes, TheFaultTypes>(world);
    world
        .get::<FaultTypes>(holder)
        .and_then(|table| table.0.get(fault).cloned())
}

/// Which faults each fault prevents: `Fault::block_faults` from the catalogue, which `set_fault` and the pick both read.
///
/// The reference's rule is that **a thing cannot have a chipped blade when the blade is gone** - so a broken fault blocks a chipped one, and the pick must not offer a fault the item's existing faults
/// have already ruled out.
#[derive(Debug, Clone, Default, PartialEq, Component)]
pub struct FaultBlocks(pub std::collections::BTreeMap<String, std::collections::BTreeSet<String>>);

/// Marks the entity holding [`FaultBlocks`].
#[derive(Debug, Clone, Copy, Default, Component)]
pub struct TheFaultBlocks;

/// Installs what each fault prevents.
pub fn set_fault_blocks(
    world: &mut World,
    blocks: &[(String, std::collections::BTreeSet<String>)],
) {
    let holder = singletons::singleton::<FaultBlocks, TheFaultBlocks>(world);
    world
        .entity_mut(holder)
        .insert(FaultBlocks(blocks.iter().cloned().collect()));
}

/// What one fault prevents, or nothing when the catalogue has never been installed or the fault blocks nothing.
#[must_use]
pub fn blocks_of(world: &mut World, fault: &str) -> std::collections::BTreeSet<String> {
    let holder = singletons::singleton::<FaultBlocks, TheFaultBlocks>(world);
    world
        .get::<FaultBlocks>(holder)
        .and_then(|table| table.0.get(fault).cloned())
        .unwrap_or_default()
}

/// The line each fault prints when an item catches it: `Fault::message()`, printed by `item::set_fault` when it is told to speak (`item.cpp:7840-7842`).
///
/// Kept beside the types and the blocks, from the same catalogue read: **a fault's type decides whether it is eligible, its blocks decide what it takes away, and its message is what the player is
/// told** - three questions, one record, one install.
#[derive(Debug, Clone, Default, PartialEq, Component)]
pub struct FaultMessages(pub std::collections::BTreeMap<String, String>);

/// Marks the entity holding [`FaultMessages`].
#[derive(Debug, Clone, Copy, Default, Component)]
pub struct TheFaultMessages;

/// Installs the line each fault prints.
pub fn set_fault_messages(world: &mut World, messages: &[(String, String)]) {
    let holder = singletons::singleton::<FaultMessages, TheFaultMessages>(world);
    world
        .entity_mut(holder)
        .insert(FaultMessages(messages.iter().cloned().collect()));
}

/// What a fault says when it is caught, or nothing when the catalogue has never been installed or the fault is silent.
#[must_use]
pub fn message_of(world: &mut World, fault: &str) -> Option<String> {
    let holder = singletons::singleton::<FaultMessages, TheFaultMessages>(world);
    world
        .get::<FaultMessages>(holder)
        .and_then(|table| table.0.get(fault).cloned())
        .filter(|line| !line.is_empty())
}

/// Which kinds of weapon each item type is (`itype::weapon_category`).
#[derive(Component, Debug, Clone, Default, PartialEq, Eq)]
pub struct ItemWeaponCategories(pub BTreeMap<ItemTypeId, Vec<String>>);

/// Marks the entity carrying the weapon categories.
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct TheItemWeaponCategories;

/// Installs which kinds of weapon each item type is.
///
/// Reference: `itype::weapon_category`, read by `wielded_weapon_categories` (`melee.cpp:570-578`). **A weapon that names none gets an empty list rather than a made-up category**: the
/// reference answers `UNARMED` for an empty hand, which is the caller's decision rather than this table's.
pub fn set_item_weapon_categories(world: &mut World, categories: &[(ItemTypeId, Vec<String>)]) {
    let entity = singletons::singleton::<ItemWeaponCategories, TheItemWeaponCategories>(world);
    let table: BTreeMap<ItemTypeId, Vec<String>> = categories.iter().cloned().collect();
    world.entity_mut(entity).insert(ItemWeaponCategories(table));
}

/// The kinds of weapon an item type is, or an empty list.
#[must_use]
pub fn weapon_categories_of(world: &mut World, kind: ItemTypeId) -> Vec<String> {
    let entity = singletons::singleton::<ItemWeaponCategories, TheItemWeaponCategories>(world);
    world
        .get::<ItemWeaponCategories>(entity)
        .and_then(|table| table.0.get(&kind).cloned())
        .unwrap_or_default()
}

/// Whether an item has enough charges to be used once.
///
/// Reference: `item::ammo_sufficient( carrier, qty = 1 )` (`item.cpp:11569-11582`), which is the rule `f_has_ammo` asks (`condition.cpp:1963-1974`):
/// a **charge-counted** kind compares its charges, a **comestible** is always sufficient (one bite spends one charge, and eating never fails for want of ammunition), and
/// anything else answers from its remaining shots - which the port reads as its charges, because it has no magazines and a magazine-fed gun's rounds are its charges until it does.
#[must_use]
pub fn has_enough_charges(world: &World, item: Entity) -> bool {
    let Some(ground) = world.get::<crate::GroundItem>(item) else {
        return false;
    };
    let kind = ground.kind;
    let charges = ground.charges;
    let (counted, edible) = traits_of(world, kind).unwrap_or((false, false));
    if edible {
        return true;
    }
    // A charge-counted stack and a plain item both answer from their charges; the reference's third branch (`shots_remaining`) is the same number for a magazine the port does
    // not have, and a magazine-fed gun whose rounds live elsewhere is a named gap.
    let _ = counted;
    charges >= 1
}

/// Whether a kind is counted in charges, and whether it is edible - or nothing when the table has never seen it.
#[must_use]
pub fn traits_of(world: &World, kind: ItemTypeId) -> Option<(bool, bool)> {
    let entity = singletons::holder::<TheKindTraits>(world)?;
    world.get::<KindTraits>(entity)?.0.get(&kind).copied()
}

/// The kind a name refers to, read without taking the store by the hand.
///
/// [`id_of`] needs a `&mut World` because a world that never loaded item data gets an empty registry made for it. A caller that only wants to ask a question about data it
/// knows is there - the vehicle rules, for instance - can ask this and get `None` rather than a store it did not want.
#[must_use]
pub fn kind_by_name(world: &World, name: &str) -> Option<ItemTypeId> {
    let holder = singletons::holder::<TheItemTypes>(world)?;
    world.get::<ItemTypes>(holder)?.by_name.get(name).copied()
}
