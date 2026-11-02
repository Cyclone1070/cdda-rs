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
    /// An item with a quality the recipe needs is not on hand.
    MissingQualities,
    /// The crafter does not know the recipe.
    UnknownRecipe,
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
    // Reference: `Character::craft_consume_tools` (`crafting.cpp:2352-2400`) spends a tool's charges for the batch - `count * batch_size`, with the start
    // remainder added on the first call. **The amount is exact here; the moment is not:** the reference divides it into twenty 5% instalments as the craft
    // progresses, and the port's craft is a single timer with no progress hook, so it spends the whole lot when the thing is made. A craft abandoned halfway
    // costs nothing in the port and some charges in the reference, which is recorded in `docs/parity.md` rather than papered over.
    spend_tool_charges(world, crafter, &making.recipe, making.batch, names);
    // Reference: `activity_actor.cpp:4232-4238` spreads the craft's practice across its progress and `craft_skill_gain` (`crafting.cpp:981-1033`) lays it on. The
    // port's craft has no progress hook, so the whole craft's worth is laid on when the thing is made - the same tick count the reference finishes with.
    if !making.recipe.skill.is_empty() {
        let ticks = crate::skills_of::craft_practice_ticks(making.recipe.time_moves);
        let cap = crate::skills_of::craft_skill_cap(making.recipe.difficulty);
        crate::skills_of::practice(world, crafter, &making.recipe.skill, ticks, cap);
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

/// Spends what the recipe's charge-hungry tools need for a batch, taking the charges from the stacks that carry them.
///
/// Reference: `Character::consume_tools( map&, tool, batch, ... )` (`crafting.cpp:2458-2482`), which spends `tool.count * batch` charges of the chosen stack. A
/// held tool (`ToolNeed::Held`) is never spent, which is the whole difference between the two encodings.
fn spend_tool_charges(
    world: &mut World,
    crafter: Entity,
    recipe: &Recipe,
    batch: i32,
    names: &crate::item_registry::ItemNames,
) {
    let batch = batch.max(1);
    for alternatives in recipe.tool_needs() {
        // One alternative per group satisfies it, and the first that can pay does: that is the selection the reference caches on the craft.
        for (item, need) in alternatives {
            let cdda_content::recipes::ToolNeed::Charges(count) = need else {
                continue;
            };
            let mut remaining = count.saturating_mul(batch);
            if remaining <= 0 {
                continue;
            }
            let mut stacks: Vec<(Entity, i32)> = Vec::new();
            {
                let mut query = world.query::<(Entity, &crate::creature::Carried, &crate::GroundItem)>();
                for (entity, carried, stack) in query.iter(world) {
                    if carried.0 == crafter && names.name_of(stack.kind).as_deref() == Some(item.as_str()) {
                        stacks.push((entity, stack.charges.max(1)));
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
                } else if let Some(mut stack) = world.get_mut::<crate::GroundItem>(entity) {
                    stack.charges -= used;
                }
                remaining -= used;
            }
            break;
        }
    }
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
    // The same carried stacks answer the tool questions: a held tool is a stack that is there, and its charges are the stack's own. The quality question needs
    // the **kinds** rather than the counts, because a quality belongs to the item type.
    let kinds = crate::inventory::carried_kinds(world, crafter);
    // The qualities are read **before** the craft starts, because asking the store while the world is borrowed for the start is a second borrow of it.
    let levels: Vec<std::collections::BTreeMap<String, i32>> = kinds
        .iter()
        .map(|kind| crate::item_registry::qualities_of(world, *kind))
        .collect();
    let qualities = |quality: &str, level: i32| -> i32 {
        let held = levels
            .iter()
            .filter(|held| held.get(quality).is_some_and(|own| *own >= level));
        held.count().try_into().unwrap_or(0)
    };
    // Knowledge is answered **before** the craft starts, for the same reason the qualities are: the start borrows the world mutably.
    let learns = crate::recipes_known::knows(world, crafter, &recipe.id);
    let known = move |_asked: &str| learns;
    let supplies = Supplies {
        components: &|item| on_hand.get(item).copied().unwrap_or(0),
        held: &|item| i32::from(on_hand.contains_key(item)),
        charges: &|item| on_hand.get(item).copied().unwrap_or(0),
        qualities: &qualities,
        known: &known,
    };
    begin_craft(world, crafter, recipe, batch, &supplies)
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

/// What the crafter has, answered one question at a time.
///
/// The four questions are the four a recipe asks: how many of a component, how many copies of a tool, how many of its charges, and how many items with a quality at
/// a level. They are closures rather than a lookup because the port's inventory is the carrier's own stacks, and a caller - eventually the inventory, today a test or a
/// playtest - is the only thing that knows what is in them.
pub struct Supplies<'a> {
    /// How many of an item id the crafter has.
    pub components: &'a dyn Fn(&str) -> i32,
    /// How many copies of a tool the crafter has.
    pub held: &'a dyn Fn(&str) -> i32,
    /// How many charges of a tool the crafter has.
    pub charges: &'a dyn Fn(&str) -> i32,
    /// How many carried items have a quality at **at least** this level.
    pub qualities: &'a dyn Fn(&str, i32) -> i32,
    /// Whether the crafter knows the recipe, asked by the recipe's id.
    pub known: &'a dyn Fn(&str) -> bool,
}

/// Answers every question with nothing, which is what a caller that only cares about components passes.
#[must_use]
pub fn nothing_on_hand() -> Supplies<'static> {
    static NONE: fn(&str) -> i32 = |_| 0;
    static QUALITIES: fn(&str, i32) -> i32 = |_, _| 0;
    static KNOWN: fn(&str) -> bool = |_| true;
    Supplies {
        components: &NONE,
        held: &NONE,
        charges: &NONE,
        qualities: &QUALITIES,
        known: &KNOWN,
    }
}

/// Starts making a recipe, if the things it needs are on hand.
///
/// Reference: `Character::making_would_work` for the refusal and `Character::start_craft` for the activity, whose
/// length is `recipe::batch_time( guy, batch, 1.0, 0 )`.
///
/// `on_hand` says how many of an item id the crafter has. It is a closure rather than a lookup because the port has no
/// inventory yet: the caller - eventually the inventory, today a test or a playtest - is the only thing that knows.
pub fn begin_craft(world: &mut World, crafter: Entity, recipe: &Recipe, batch: i32, supplies: &Supplies<'_>) -> Craft {
    // Reference: `Character::can_make` refuses when `!has_recipe( r )` (`crafting.cpp:602-616`), and `has_recipe` is `knows_recipe || group_available`
    // (`character_crafting.cpp:23-27`) - a recipe the crafter never learned is not makeable, whatever is on hand.
    if !(supplies.known)(&recipe.id) {
        return Craft::UnknownRecipe;
    }
    if world.get::<Doing>(crafter).is_some() {
        return Craft::Busy;
    }
    if !recipe.components_available(batch, supplies.components) {
        return Craft::MissingComponents;
    }
    // Reference: `character::making_would_work` -> `requirement_data::can_make_with_inventory` (`requirements.cpp:952-990`): **tools are part of the
    // requirement**, a held tool is needed in that many copies and a tool that spends charges needs them per batch. A craft that skipped this started
    // with bare hands and finished a recipe that needed a hammer.
    if !recipe.tools_available(batch, supplies.held, supplies.charges) {
        return Craft::MissingTools;
    }
    // Reference: `quality_requirement::has` (`requirements.cpp:932-940`): an item whose quality is **at least** the required level counts, and there must be that many.
    if !recipe.qualities_available(batch, supplies.qualities) {
        return Craft::MissingQualities;
    }
    let batch = batch.max(1);
    // Reference: `recipe::batch_time` at full speed with no assistants - a recipe that benefits from batching takes less than `time * batch`, and the port
    // multiplied and never asked, which over-charged every one of the data's 1,148 batching recipes.
    let moves = crate::batch_time_moves(
        recipe.time_moves,
        batch,
        recipe.batch_rscale,
        f64::from(recipe.batch_rsize),
        1.0,
        0,
    );
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
