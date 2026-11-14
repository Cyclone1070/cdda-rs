//! Looking at the world: what is here, what it carries, and how a turn changed it.
//!
//! A debugging surface, not a rule. The headless binary prints what these answer, and the QA runner
//! compares two worlds by them; a person reading a failed gate needs to see the store, not a stack
//! trace. Nothing here writes to the world.

use crate::Position;
use bevy_ecs::prelude::*;
use std::collections::BTreeMap;
use std::fmt::Write as _;

/// Every component this port names, for a person reading a report.
///
/// The ECS can name a component only when it is built with its debug feature, which pulls a
/// dependency tree this crate does not otherwise need. A table of our own is smaller, and it names
/// exactly the things a reader of a square report is looking for.
macro_rules! named_components {
    ($($name:literal => $type:ty),* $(,)?) => {
        /// The components a report can name, paired with the text it prints.
        const NAMED: &[(&str, std::any::TypeId)] = &[
            $(($name, std::any::TypeId::of::<$type>())),*
        ];

        /// The component's value, written out, or `None` when the thing does not carry it.
        fn describe(world: &World, entity: Entity, id: bevy_ecs::component::ComponentId) -> Option<String> {
            $(
                if world.components().get_id(std::any::TypeId::of::<$type>()) == Some(id) {
                    return world.get::<$type>(entity).map(|value| format!("{value:?}"));
                }
            )*
            None
        }
    };
}

named_components! {
    "Position" => Position,
    // What a creature is, since a square report is where a reader meets one: the arrow that says "this is not scenery" and the numbers that make it move and hurt.
    "Body" => crate::body::Body,
    "Focus" => crate::skills::Focus,
    "Creature" => crate::creature::Creature,
    "CreatureKind" => crate::creature::CreatureKind,
    "Species" => crate::creature::Species,
    "Ally" => crate::npcs::Ally,
    "Health" => crate::creature::Health,
    "Speed" => crate::creature::Speed,
    "Moves" => crate::creature::Moves,
    "Dodge" => crate::creature::Dodge,
    "MeleeAttack" => crate::creature::MeleeAttack,
    "AttackCost" => crate::creature::AttackCost,
    "Exercise" => crate::creature::Exercise,
    "Stamina" => crate::creature::Stamina,
    "Morale" => crate::creature::Morale,
    "Anger" => crate::creature::Anger,
    "Nerve" => crate::creature::Nerve,
    "Armour" => crate::creature::Armour,
    "Goal" => crate::creature::Goal,
    "Wander" => crate::creature::Wander,
    "HeardSound" => crate::hearing::HeardSound,
    "Mending" => crate::regeneration::Mending,
    "WeakPoints" => crate::weakpoints::WeakPoints,
    "Dexterity" => crate::creature::Dexterity,
    "Wielding" => crate::creature::Wielding,
    "Strength" => crate::creature::Strength,
    "CreatureFlags" => crate::creature_flags::CreatureFlags,
    "Materials" => crate::creature::Materials,
    "AppliedEffects" => crate::applied_effects::AppliedEffects,
    "Doing" => crate::creature::Doing,
    "Square" => crate::square::Square,
    "Ground" => crate::square::Ground,
    "Terrain" => crate::square::Terrain,
    "Furniture" => crate::square::Furniture,
    "Field" => crate::Field,
    "GroundItem" => crate::GroundItem,
    "Vehicle" => crate::vehicles::Vehicle,
    "FireFacts" => crate::fire_facts::FireFacts,
    "Radiation" => crate::radiation::Radiation,
    "Scent" => crate::scent::Scent,
    "Memory" => crate::memory::Memory,
    "SupportLost" => crate::SupportLost,
    "ScentSource" => crate::turn::ScentSource,
    "Random" => crate::turn::Random,
    "TurnTrace" => crate::turn::TurnTrace,
    "GasCapacity" => crate::turn::GasCapacity,
    "Light" => crate::Light,
}

/// What a component is called, or `?` for one this port does not name.
#[must_use]
pub fn component_name(world: &World, id: bevy_ecs::component::ComponentId) -> String {
    for (name, type_id) in NAMED {
        if world.components().get_id(*type_id) == Some(id) {
            return (*name).to_string();
        }
    }
    "?".to_string()
}

/// How many things carry each kind of component, by component name.
///
/// The name is the ECS's own, so a component that exists but is never spawned shows as absent rather
/// than as a quiet zero.
#[must_use]
pub fn component_counts(world: &World) -> BTreeMap<String, usize> {
    let mut counts = BTreeMap::new();
    for entity in world.iter_entities() {
        for id in entity.archetype().components() {
            *counts.entry(component_name(world, *id)).or_insert(0) += 1;
        }
    }
    counts
}

/// What stands on one square, as entity ids and the components each carries.
#[must_use]
pub fn what_is_at(world: &World, square: (i32, i32, i32)) -> Vec<(Entity, Vec<String>)> {
    let mut found = Vec::new();
    let Some(index) = crate::singletons::index(world) else {
        return found;
    };
    for entity in index.at_level(square.0, square.1, square.2) {
        let components = world
            .inspect_entity(*entity)
            .map(|inspection| {
                inspection
                    .map(|(id, _info)| {
                        let name = component_name(world, id);
                        match describe(world, *entity, id) {
                            Some(value) => format!("{name} = {value}"),
                            None => name,
                        }
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        found.push((*entity, components));
    }
    found
}

/// A square, written out for a person to read: the things standing there and what each of them is.
#[must_use]
pub fn square_report(world: &World, square: (i32, i32, i32)) -> String {
    let here = what_is_at(world, square);
    let mut out = format!(
        "square ({}, {}, {}) holds {} thing(s)\n",
        square.0,
        square.1,
        square.2,
        here.len()
    );
    for (entity, components) in here {
        let _ = writeln!(out, "  {entity:?}: {}", components.join(", "));
    }
    out
}

/// Everything the store holds, written out by kind, largest first.
#[must_use]
pub fn store_report(world: &World) -> String {
    let mut counts: Vec<(String, usize)> = component_counts(world).into_iter().collect();
    counts.sort_by(|left, right| right.1.cmp(&left.1).then(left.0.cmp(&right.0)));
    let mut out = format!("{} entities\n", world.entities().len());
    for (name, count) in counts {
        let _ = writeln!(out, "  {count:>7}  {name}");
    }
    out
}

/// A short number that changes when anything about a square changes, so two turns can be compared.
///
/// FNV-1a over the square's entity ids and the component names each carries: cheap, stable within a
/// run, and enough for "did this turn touch that square".
#[must_use]
pub fn square_fingerprint(world: &World, square: (i32, i32, i32)) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for (entity, components) in what_is_at(world, square) {
        for byte in entity.to_bits().to_le_bytes() {
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        }
        for name in components {
            for byte in name.bytes() {
                hash ^= u64::from(byte);
                hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
            }
        }
    }
    hash
}

/// Every square that holds something, in a stable order, with its fingerprint.
#[must_use]
pub fn world_fingerprints(world: &World) -> BTreeMap<(i32, i32, i32), u64> {
    let mut squares = BTreeMap::new();
    let mut query = world.try_query::<&Position>();
    let Some(query) = query.as_mut() else {
        return squares;
    };
    let mut seen: std::collections::BTreeSet<(i32, i32, i32)> = std::collections::BTreeSet::new();
    for position in query.iter(world) {
        seen.insert((position.x, position.y, position.z));
    }
    for square in seen {
        squares.insert(square, square_fingerprint(world, square));
    }
    squares
}

/// The squares whose fingerprint differs between two worlds, which is what a turn report lists.
#[must_use]
pub fn squares_that_differ(
    before: &BTreeMap<(i32, i32, i32), u64>,
    after: &BTreeMap<(i32, i32, i32), u64>,
) -> Vec<(i32, i32, i32)> {
    let mut changed = Vec::new();
    for (square, hash) in after {
        if before.get(square) != Some(hash) {
            changed.push(*square);
        }
    }
    for square in before.keys() {
        if !after.contains_key(square) {
            changed.push(*square);
        }
    }
    changed.sort_unstable();
    changed.dedup();
    changed
}
