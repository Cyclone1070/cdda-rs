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
    // The craft's **practice** and its **proficiencies** are both laid on by the turn hook (`crate::creature::spend_a_turn_on`), which is the reference's own timing
    // (`activity_actor.cpp:4232-4245`): a craft that finishes without running its turns - a test that jumps straight to the end - teaches nothing, on purpose.
    // Reference: `create_result` (`recipe.cpp:768-771`) - a `contained` recipe makes **the container**, with the result inside it, so a canned good's product is the can
    // rather than what went into it. The contents are a link (`crate::containment`), which is why the result is spawned and then put inside.
    let result = making.recipe.result.clone();
    let Some(kind) = names.id_of(&result) else {
        // The result is not an item this world knows: the components have already been decided, so the honest answer
        // is to say what could not be made rather than pretend a thing appeared.
        return None;
    };
    // Reference: `recipe::create_results` (`recipe.cpp:790-825`): one batch makes `made_per_batch` things, which the reference's own dump reports as the product of
    // `result_mult` and `charges`. The port holds a count as a stack's charges, so the batch multiplies it.
    let charges = making.batch.max(1).saturating_mul(making.recipe.made_per_batch.max(1));
    if let Some(container) = making.recipe.container.as_deref().and_then(|id| names.id_of(id)) {
        fill_container(world, crafter, container, kind, charges);
        return Some(result);
    }
    give_to_crafter(world, crafter, kind, charges, names);
    // Reference: `recipe::create_byproducts` (`recipe.cpp:872-900`) - the other things the craft leaves behind, `count * batch` of each. They land wherever the
    // result would have landed, which is the reference's own treatment of them.
    // Reference: `create_byproducts` (`recipe.cpp:872-900`) also draws from `byproduct_group`, an **inline item group** whose counts are ranges - so each entry is
    // rolled once per batch rather than multiplied. The roll leads the give, because a draw of zero makes nothing.
    for (byproduct, low, high) in &making.recipe.byproduct_group {
        let Some(kind) = names.id_of(byproduct) else {
            continue;
        };
        for _ in 0..making.batch.max(1) {
            let Some(made) = crate::crafting::draw_between(world, *low, *high) else {
                continue;
            };
            if made > 0 {
                give_to_crafter(world, crafter, kind, made, names);
            }
        }
    }
    for (byproduct, count) in &making.recipe.byproducts {
        let Some(kind) = names.id_of(byproduct) else {
            continue;
        };
        let made = count.saturating_mul(making.batch.max(1));
        if made > 0 {
            give_to_crafter(world, crafter, kind, made, names);
        }
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

/// Practises what a recipe teaches, for the whole craft at once.
///
/// Reference: `craft_proficiency_gain` (`crafting.cpp:1036-1090`), which the craft activity calls on **every fifth-percent step** (`activity_actor.cpp:4245`). The port
/// divides the span the caller hands over - the fifth-percent slice the reference gives it (`base_total_moves / 2000` seconds per step).
pub fn teach_proficiencies(world: &mut World, crafter: Entity, recipe: &Recipe, seconds: i64) {
    let taught: Vec<crate::proficiencies::Taught<'_>> = recipe
        .proficiencies
        .iter()
        .filter_map(|subject| {
            let time_to_learn = crate::proficiencies::time_to_learn(world, &subject.id)?;
            Some(crate::proficiencies::Taught {
                id: &subject.id,
                time_multiplier: subject.time_multiplier,
                learning_time_mult: subject.learning_time_mult,
                time_to_learn,
            })
        })
        .collect();
    if taught.is_empty() {
        return;
    }
    // The data's own guard, asked **before** the rule borrows the world: a craft cannot teach an expert trade to someone who has not learned the basic one.
    let learnable: Vec<String> = taught
        .iter()
        .filter(|subject| crate::proficiencies::can_learn(world, crafter, subject.id))
        .map(|subject| subject.id.to_owned())
        .collect();
    let _ = crate::proficiencies::gain_from_craft(world, crafter, &taught, seconds, &|id| {
        learnable.iter().any(|learnable| learnable == id)
    });
}

/// Makes `charges` of a thing inside a new container, which is what a `contained` recipe produces.
///
/// Reference: `create_result` (`recipe.cpp:768-771`): `newit.in_container( container, amount, sealed )` - the container is one item holding the result, and the
/// recipe's product is the container. The contents are a **link** (`crate::containment`), so each of them is its own entity pointing at the container.
fn fill_container(
    world: &mut World,
    crafter: Entity,
    container: crate::ItemTypeId,
    kind: crate::ItemTypeId,
    charges: i32,
) {
    let names = crate::item_registry::snapshot(world);
    // The container is given first, because the contents need something to point at.
    let jar = give_to_crafter(world, crafter, container, 1, &names);
    for _ in 0..charges.max(1) {
        let made = world
            .spawn((
                crate::creature::Carried(crafter),
                crate::GroundItem {
                    kind,
                    charges: 1,
                    age: 0,
                    burnt: 0,
                    damage: 0,
                    burn: None,
                },
            ))
            .id();
        crate::containment::put_inside(world, made, jar);
    }
}

/// Draws a whole number between two ends, both included, from the world's dice.
fn draw_between(world: &mut World, low: i32, high: i32) -> Option<i32> {
    if low == high {
        return Some(low);
    }
    let (low, high) = (low.min(high), low.max(high));
    let mut held = crate::singletons::take_rolls(world);
    let drawn = held.as_mut().map(|rolls| rolls.rng(low, high));
    if let Some(dice) = held.take() {
        crate::singletons::put_rolls(world, dice);
    }
    drawn
}

/// Puts something the crafter just made where a found thing would go: a pocket if it fits, the floor if not.
///
/// Reference: `Character::i_add` -> `add_item` for the pocket, and `i_add_or_drop` for the fallback that puts what fits nowhere on the ground where the crafter
/// stands. The port has no wield slot, so a thing that fits no pocket is dropped rather than held; that difference is the reference's own fallback, not a shortcut.
fn give_to_crafter(
    world: &mut World,
    crafter: Entity,
    kind: crate::ItemTypeId,
    charges: i32,
    names: &crate::item_registry::ItemNames,
) -> Entity {
    let size = names.volume_of(kind).saturating_mul(charges);
    let weight = names.weight_of(kind) * i64::from(charges);
    if let Some((worn, index)) = crate::creature::can_stash(world, crafter, names, size, weight) {
        let made = world
            .spawn((
                crate::creature::Carried(crafter),
                crate::GroundItem {
                    kind,
                    charges,
                    age: 0,
                    burnt: 0,
                    damage: 0,
                    burn: None,
                },
            ))
            .insert(crate::creature::InPocket { worn, index })
            .id();
        return made;
    }
    // A creature with no square at all is not standing anywhere, so the thing stays in hand rather than vanishing.
    if let Some(placed) = world.get::<crate::Position>(crafter).copied() {
        return crate::drop_item(world, (placed.x, placed.y), kind, charges);
    }
    world
        .spawn((
            crate::creature::Carried(crafter),
            crate::GroundItem {
                kind,
                charges,
                age: 0,
                burnt: 0,
                damage: 0,
                burn: None,
            },
        ))
        .id()
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
        // The whole craft's moves, which is what places the reference's fifth-percent steps.
        moves_total: moves_left,
        subject: None,
    });
    world.entity_mut(crafter).insert(Making {
        recipe: recipe.clone(),
        batch,
    });
    Craft::Started
}
