//! Mutations: what a creature has become, and what it may become next.
//!
//! Reference: `Character::has_trait`, `Character::can_mutate` and `Character::mutate_towards` (`mutation.cpp:520-700`). The creature holds **ids** in one component;
//! the four sets that decide whether a mutation may be gained live in the data table (`cdda_content::mutations`), not on the creature.

use bevy_ecs::prelude::*;
use cdda_content::mutations::{Categories, Mutations as MutationData};

/// The mutations and traits a creature has.
#[derive(Debug, Clone, Default, PartialEq, Eq, Component)]
pub struct Mutations(pub Vec<String>);

/// Why a mutation cannot be gained.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Gain {
    /// It can be gained.
    Yes,
    /// One of its prerequisites is missing.
    MissingPrerequisite,
    /// The creature already holds something the mutation conflicts with.
    Conflicting,
    /// The data does not define it.
    Unknown,
}

/// Whether a creature has a mutation.
#[must_use]
pub fn has(world: &World, creature: Entity, id: &str) -> bool {
    world
        .get::<Mutations>(creature)
        .is_some_and(|held| held.0.iter().any(|known| known == id))
}

/// Whether a creature may gain a mutation, and why not when it may not.
///
/// Reference: `Character::can_mutate` (`mutation.cpp:540-575`): every entry in `prereqs` is an alternative - **one** is enough - and the same holds for `prereqs2`;
/// a mutation in `cancels` that the creature already has blocks the gain outright.
#[must_use]
pub fn can_gain(world: &World, creature: Entity, data: &MutationData, id: &str) -> Gain {
    let Some(rule) = data.get(id) else {
        return Gain::Unknown;
    };
    if has(world, creature, id) {
        return Gain::Yes;
    }
    // Reference: `Character::mutate_towards` (`mutation.cpp:1656-1712`): a conflict is **removed**, not a barrier - unless the trait holding the conflict cannot be
    // taken away at all, in which case the gain is refused (`canceltrait` is not purifiable, and the function returns false).
    let stuck = rule.cancels.iter().any(|blocker| {
        has(world, creature, blocker) && data.get(blocker).is_some_and(|held| !held.purifiable)
    });
    if stuck {
        return Gain::Conflicting;
    }
    if !satisfied(world, creature, &rule.prereqs) || !satisfied(world, creature, &rule.prereqs2) {
        return Gain::MissingPrerequisite;
    }
    // Reference: `mutation_branch::threshreq` (`mutation.h:333`): "Prerequisites; dedicated slot to needing thresholds". Unlike `prereqs`, where **one** member is
    // enough, every threshold in this list must be held - a mutation past a threshold needs the threshold itself, not one of several.
    if !rule.threshreq.iter().all(|need| has(world, creature, need)) {
        return Gain::MissingPrerequisite;
    }
    Gain::Yes
}

/// One of the list must be held, and an empty list asks for nothing.
fn satisfied(world: &World, creature: Entity, needed: &[String]) -> bool {
    needed.is_empty() || needed.iter().any(|one| has(world, creature, one))
}

/// Gives a creature a mutation, taking away what the new one conflicts with.
///
/// Reference: `Character::mutate_towards` (`mutation.cpp:600-700`): the conflicting traits are removed **first**, so the creature never holds two mutations that
/// cancel each other, and the gain is refused when the prerequisites are not met.
pub fn gain(world: &mut World, creature: Entity, data: &MutationData, categories: &Categories, id: &str) -> Gain {
    let verdict = can_gain(world, creature, data, id);
    if verdict != Gain::Yes {
        return verdict;
    }
    let Some(rule) = data.get(id) else {
        return Gain::Unknown;
    };
    let conflicts: Vec<String> = rule.cancels.clone();
    // Reference: `Character::mutate_towards` (`mutation.cpp:1643-1652`): every held mutation **of the same category** is pruned as well, except the ones the new
    // mutation needs as prerequisites - a mutation never eats the trait it grew out of.
    let keep: Vec<String> = prerequisite_closure(data, id);
    let of_new = rule.categories.clone();
    let chance = removal_chance(categories, &of_new);
    let mut rolled = crate::singletons::take_rolls(world).unwrap_or_else(|| cdda_core_types::rng::Rolls::new(0));
    let mut held = world.get::<Mutations>(creature).cloned().unwrap_or_default();
    let mut kept: Vec<String> = Vec::new();
    for known in &held.0 {
        if known == id || keep.contains(known) {
            kept.push(known.clone());
            continue;
        }
        let pruned = conflicts.contains(known) || shares_a_category(data, known, &of_new);
        if !pruned {
            kept.push(known.clone());
            continue;
        }
        // Reference: `Character::mutate_towards` (`mutation.cpp:1666-1674`): a starting trait of the category is only removed when the category's own chance
        // succeeds. When it fails, the trait **stays** and the mutation still happens - the reference bails out of the whole gain (`return true`) instead, and that
        // difference is recorded in `docs/parity.md`.
        let starting = data.get(known).is_some_and(|one| one.startingtrait);
        #[allow(clippy::cast_precision_loss)]
        let odds = f64::from(chance);
        if starting && chance < 100 && !rolled.x_in_y(odds, 100.0) {
            kept.push(known.clone());
        }
    }
    crate::singletons::put_rolls(world, rolled);
    held.0 = kept;
    if !held.0.iter().any(|known| known == id) {
        held.0.push(id.to_owned());
    }
    world.entity_mut(creature).insert(held);
    Gain::Yes
}

/// The percent chance a starting trait of these categories is removed at all.
///
/// Reference: `mutation_category_trait::base_removal_chance` (`mutation.h:541`), a hundred unless the data says otherwise.
#[must_use]
pub fn removal_chance(categories: &Categories, of_new: &[String]) -> i32 {
    of_new
        .iter()
        .filter_map(|cat| categories.get(cat))
        .map(|cat| cat.base_removal_chance)
        .next()
        .unwrap_or(100)
        .clamp(0, 100)
}

/// Whether a held mutation belongs to one of the categories the new one does.
fn shares_a_category(data: &MutationData, held: &str, categories: &[String]) -> bool {
    data.get(held)
        .is_some_and(|one| one.categories.iter().any(|cat| categories.contains(cat)))
}

/// The new mutation's prerequisites, and theirs, and so on.
///
/// Reference: `get_all_mutation_prereqs` (`mutation.cpp:1578-1593`), the function `mutate_towards` uses to decide which same-category traits to spare.
#[must_use]
pub fn prerequisite_closure(data: &MutationData, id: &str) -> Vec<String> {
    let mut all: Vec<String> = Vec::new();
    let mut queue: Vec<String> = vec![id.to_owned()];
    while let Some(next) = queue.pop() {
        let Some(rule) = data.get(&next) else {
            continue;
        };
        for need in rule.prereqs.iter().chain(rule.prereqs2.iter()) {
            if all.contains(need) {
                continue;
            }
            all.push(need.clone());
            queue.push(need.clone());
        }
    }
    all
}

/// Every mutation a creature has.
#[must_use]
pub fn mutations_of(world: &World, creature: Entity) -> Vec<String> {
    world
        .get::<Mutations>(creature)
        .map_or_else(Vec::new, |held| held.0.clone())
}

/// How far down its own branch a mutation sits: one at the root, and one more for every prerequisite above it.
///
/// Reference: `Character::mutation_height` (`character.cpp:7626-7639`): the height is one more than the tallest prerequisite chain, taken over `prereqs` and
/// `prereqs2` alike, and it is a property of the **data** rather than of what the character holds.
#[must_use]
pub fn height_of(data: &MutationData, id: &str) -> i32 {
    let Some(rule) = data.get(id) else {
        return 1;
    };
    let mut tallest = 0;
    for need in rule.prereqs.iter().chain(rule.prereqs2.iter()) {
        tallest = tallest.max(height_of(data, need));
    }
    tallest + 1
}

/// How strongly a creature has become one thing.
///
/// Reference: `Character::calc_mutation_levels` (`character.cpp:7641-7672`): a threshold is worth **ten** to each of its categories, and any other mutation is worth its
/// **height plus two** to each category it belongs to - so a root trait is three, its child four, and its grandchild five. This is the strength a threshold is gated on
/// and the strength a category's potion threshold reads.
#[must_use]
pub fn category_level(world: &World, creature: Entity, data: &MutationData, category: &str) -> i32 {
    let held = mutations_of(world, creature);
    let mut level = 0;
    for id in &held {
        let Some(rule) = data.get(id) else {
            continue;
        };
        if !rule.categories.iter().any(|cat| cat == category) {
            continue;
        }
        if rule.threshold {
            level += 10;
        } else {
            level += height_of(data, id) + 2;
        }
    }
    level
}

/// Every mutation a creature could become next in one category, in the data's own id order.
#[must_use]
pub fn candidates(world: &World, creature: Entity, data: &MutationData, category: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for id in data.ids() {
        let Some(rule) = data.get(id) else {
            continue;
        };
        if !rule.valid || !rule.categories.iter().any(|cat| cat == category) {
            continue;
        }
        if has(world, creature, id) || can_gain(world, creature, data, id) != Gain::Yes {
            continue;
        }
        out.push(id.clone());
    }
    out
}

/// The category a creature has gone furthest into, and how strong it is.
///
/// Reference: `f_mutate` (`npctalk.cpp:3560-3570`) asks `Character::mutation_category_level` for every category the creature touches and mutates towards the **highest**.
#[must_use]
pub fn strongest_category(world: &World, creature: Entity, data: &MutationData) -> Option<(String, i32)> {
    let mut best: Option<(String, i32)> = None;
    for id in mutations_of(world, creature) {
        let Some(rule) = data.get(&id) else {
            continue;
        };
        for category in &rule.categories {
            let level = category_level(world, creature, data, category);
            if best.as_ref().is_none_or(|(_, champion)| level > *champion) {
                best = Some((category.clone(), level));
            }
        }
    }
    best
}

/// Mutates a creature towards one category, trying twice.
///
/// Reference: `Character::mutate_category` (`mutation.cpp:1354-1404`) builds the category's valid list and calls `mutate_towards( valid, cat, 2, ... )` - **two tries**,
/// each a random pick from the list, and the first that succeeds ends it.
///
/// **Declared:** the reference also refuses good mutations to a creature on a bad roll (`roll_bad_mutation`) and bad ones on a good roll, and filters on the vitamin each
/// mutation costs. The port draws from the whole valid list, so a character can gain either kind - a difference that shows up only where the reference would have said no.
pub fn mutate_category(
    world: &mut World,
    creature: Entity,
    data: &MutationData,
    categories: &Categories,
    category: &str,
    rolls: &mut cdda_core_types::rng::Rolls,
) -> Option<String> {
    let mut valid = candidates(world, creature, data, category);
    for _ in 0..2 {
        if valid.is_empty() {
            return None;
        }
        let last = i32::try_from(valid.len()).unwrap_or(i32::MAX) - 1;
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let pick = rolls.rng(0, last) as usize;
        let chosen = valid.get(pick)?.clone();
        if gain(world, creature, data, categories, &chosen) == Gain::Yes {
            return Some(chosen);
        }
        valid.retain(|id| id != &chosen);
    }
    None
}
