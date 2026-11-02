//! Making something from a recipe.
//!
//! Reference: `player_activity` with `ACT_CRAFT`, whose duration is `recipe::batch_time` and whose finish is
//! `activity_handlers::craft_finish` - consume the components, produce the result. The port has the recipe data and the
//! availability rule (`recipe::components_available`); what it does not have is an inventory to draw components from,
//! so a caller hands in what is on hand and the finish produces nothing yet. Both absences are declared here rather
//! than papered over: a craft that consumed from nowhere would be a rule that cannot be caught being wrong.

use bevy_ecs::prelude::*;
use cdda_content::recipes::Recipe;

use crate::creature::{Basis, Doing};

/// What came of trying to start a craft.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Craft {
    /// It has begun, and will take as many turns as the recipe needs.
    Started,
    /// Something on hand is missing: the recipe's components are not all there.
    MissingComponents,
    /// A tool the recipe needs is not on hand, or not with enough charges.
    MissingTools,
    /// The creature is already busy with something else.
    Busy,
    /// The recipe takes no time at all, which no recipe in the data does - a sign the time was never read.
    NoTimeToMakeIt,
}

/// Takes the components a recipe asks for out of a crafter's hands, and puts the result in them.
///
/// Reference: `activity_handlers::craft_finish` -> `Character::complete_craft` -> `recipe::create_results`, which
/// consumes one alternative of each component group and makes `batch` of the result. The port removes the entities it
/// consumes - a stack with more charges than the recipe needs loses them, a stack with exactly as many is despawned -
/// and spawns the result in the crafter's hands, because a carried thing is an entity with a `Carried` link and no
/// square.
///
/// Answers the item id that was made, or [`None`] when nothing could be made - in which case nothing is consumed
/// either, because a craft that ate its components and produced nothing is worse than one that refused.
pub fn finish_craft(world: &mut World, crafter: Entity, names: &crate::item_registry::ItemNames) -> Option<String> {
    let making = world.get::<Making>(crafter).cloned()?;
    world.entity_mut(crafter).remove::<Making>();
    let on_hand = crate::inventory::carried_by_id(world, crafter, |kind| names.name_of(kind));
    if !making.recipe.components_available(making.batch, |item| {
        on_hand.get(item).copied().unwrap_or(0)
    }) {
        return None;
    }
    // Which entity satisfies which group, decided before anything is removed: consuming while searching would let an
    // earlier group eat what a later one needs.
    for alternatives in &making.recipe.components {
        let mut took_any = false;
        for component in alternatives {
            let mut remaining = component.count.saturating_mul(making.batch.max(1));
            if remaining <= 0 {
                continue;
            }
            let mut stacks: Vec<(Entity, i32)> = Vec::new();
            {
                let mut query = world.query::<(Entity, &crate::creature::Carried, &crate::GroundItem)>();
                for (entity, carried, item) in query.iter(world) {
                    if carried.0 == crafter && names.name_of(item.kind).as_deref() == Some(component.item.as_str()) {
                        stacks.push((entity, item.charges.max(1)));
                    }
                }
            }
            let available: i32 = stacks.iter().map(|(_, charges)| charges).sum();
            if available < remaining {
                continue;
            }
            for (entity, charges) in stacks {
                if remaining <= 0 {
                    break;
                }
                let used = charges.min(remaining);
                if used == charges {
                    world.despawn(entity);
                } else if let Some(mut item) = world.get_mut::<crate::GroundItem>(entity) {
                    item.charges -= used;
                }
                remaining -= used;
            }
            took_any = true;
            break;
        }
        if !took_any {
            return None;
        }
    }
    let result = making.recipe.result.clone();
    let Some(kind) = names.id_of(&result) else {
        // The result is not an item this world knows: the components have already been decided, so the honest answer
        // is to say what could not be made rather than pretend a thing appeared.
        return None;
    };
    let charges = making.batch.max(1);
    // Reference: `Character::i_add` -> `add_item` puts the result in a pocket, and only what fits nowhere is held
    // instead. The port asks `can_stash` the same question a pickup asks, so a crafted thing lands where a found thing
    // would; the surviving difference is what happens when nothing fits - the reference drops it at your feet, and the
    // port has no wield slot to put it in, so it stays in hand and says so by carrying no pocket.
    let size = names.volume_of(kind).saturating_mul(charges);
    let weight = names.weight_of(kind) * i64::from(charges);
    let pocket = crate::creature::can_stash(world, crafter, names, size, weight);
    if pocket.is_none() {
        // Reference: `i_add_or_drop` - what will not go in a pocket is put on the ground where the crafter stands, not
        // into the character's hands. Every creature in the port has a square, so there is always somewhere to put it.
        let placed = world.get::<crate::Position>(crafter).copied();
        match placed {
            Some(placed) => {
                crate::drop_item(world, (placed.x, placed.y), kind, charges);
            }
            None => {
                // A creature with no square at all is not standing anywhere, so the thing stays in hand rather than
                // vanishing. Nothing in the port can be in this state; the branch exists so a missing position is a
                // thing that does not disappear.
                world.spawn((
                    crate::creature::Carried(crafter),
                    crate::GroundItem {
                        kind,
                        charges,
                        age: 0,
                        burnt: 0,
                        damage: 0,
                        burn: None,
                    },
                ));
            }
        }
        return Some(result);
    }
    let mut spawned = world.spawn((
        crate::creature::Carried(crafter),
        crate::GroundItem {
            kind,
            charges,
            age: 0,
            burnt: 0,
            damage: 0,
            burn: None,
        },
    ));
    if let Some((worn, index)) = pocket {
        spawned.insert(crate::creature::InPocket { worn, index });
    }
    Some(result)
}

/// Finishes a craft using the store's own knowledge of what items are.
///
/// The same rule as [`finish_craft`], with the two lookups taken from [`crate::item_registry`] instead of from a
/// caller. That is what lets the turn loop, a playtest or the headless binary finish a craft without being handed the
/// item table first.
pub fn finish_craft_in_store(world: &mut World, crafter: Entity) -> Option<String> {
    let names = crate::item_registry::snapshot(world);
    finish_craft(world, crafter, &names)
}

/// Starts a craft using what is actually in the crafter's hands.
///
/// The join the port was missing: [`crate::inventory::carried_by_id`] counts the carried items by their data ids, and
/// this hands that count to [`begin_craft`]'s availability rule - so a recipe is satisfied by things the creature is
/// really holding, not by a number a test made up.
pub fn begin_craft_from_carried(
    world: &mut World,
    crafter: Entity,
    recipe: &Recipe,
    batch: i32,
    names: &crate::item_registry::ItemNames,
) -> Craft {
    let on_hand = crate::inventory::carried_by_id(world, crafter, |kind| names.name_of(kind));
    // The same carried stacks answer the tool questions: a held tool is a stack that is there, and its charges are the stack's own.
    let charges = crate::inventory::carried_by_id(world, crafter, |kind| names.name_of(kind));
    begin_craft(
        world,
        crafter,
        recipe,
        batch,
        |item| on_hand.get(item).copied().unwrap_or(0),
        |item| i32::from(charges.contains_key(item)),
        |item| charges.get(item).copied().unwrap_or(0),
    )
}

/// A craft in progress.
///
/// Reference: the `ACT_CRAFT` activity carries the recipe it is making and how many of it. The port keeps that on the
/// creature instead of inside the activity, because the activity component is the same one a pickup uses and the two
/// have nothing else in common.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Making {
    /// What is being made.
    pub recipe: Recipe,
    /// How many at once.
    pub batch: i32,
}

/// Starts making a recipe, if the things it needs are on hand.
///
/// Reference: `Character::making_would_work` for the refusal and `Character::start_craft` for the activity, whose
/// length is `recipe::batch_time( guy, batch, 1.0, 0 )` - one batch's time multiplied by the batch size.
///
/// `on_hand` says how many of an item id the crafter has. It is a closure rather than a lookup because the port has no
/// inventory yet: the caller - eventually the inventory, today a test or a playtest - is the only thing that knows.
pub fn begin_craft(
    world: &mut World,
    crafter: Entity,
    recipe: &Recipe,
    batch: i32,
    on_hand: impl Fn(&str) -> i32,
    tools_held: impl Fn(&str) -> i32,
    tool_charges: impl Fn(&str) -> i32,
) -> Craft {
    if world.get::<Doing>(crafter).is_some() {
        return Craft::Busy;
    }
    if !recipe.components_available(batch, on_hand) {
        return Craft::MissingComponents;
    }
    // Reference: `character::making_would_work` -> `requirement_data::can_make_with_inventory` (`requirements.cpp:952-990`): **tools are part of the
    // requirement**, a held tool is needed in that many copies and a tool that spends charges needs them per batch. A craft that skipped this started
    // with bare hands and finished a recipe that needed a hammer.
    if !recipe.tools_available(batch, tools_held, tool_charges) {
        return Craft::MissingTools;
    }
    let batch = batch.max(1);
    let moves = recipe.time_moves.saturating_mul(i64::from(batch));
    let Ok(moves_left) = i32::try_from(moves) else {
        return Craft::NoTimeToMakeIt;
    };
    if moves_left <= 0 {
        return Craft::NoTimeToMakeIt;
    }
    world.entity_mut(crafter).insert(Doing {
        activity: "craft".to_owned(),
        moves_left,
        basis: Basis::Speed,
        subject: None,
    });
    world.entity_mut(crafter).insert(Making {
        recipe: recipe.clone(),
        batch,
    });
    Craft::Started
}
