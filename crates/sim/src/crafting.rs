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
pub fn finish_craft(
    world: &mut World,
    crafter: Entity,
    names: &crate::item_registry::ItemNames,
) -> Option<String> {
    let making = world.get::<Making>(crafter).cloned()?;
    world.entity_mut(crafter).remove::<Making>();
    let on_hand = crate::inventory::carried_by_id(world, crafter, |kind| names.name_of(kind));
    if !making
        .recipe
        .components_available(making.batch, |item| on_hand.get(item).copied().unwrap_or(0))
    {
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
                let mut query =
                    world.query::<(Entity, &crate::creature::Carried, &crate::GroundItem)>();
                for (entity, carried, item) in query.iter(world) {
                    if carried.0 == crafter
                        && names.name_of(item.kind).as_deref() == Some(component.item.as_str())
                    {
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
    // Reference: the step that completes the craft is not charged (`activity_actor.cpp:4250-4253`), so the last twentieth is owed here - which is exactly what the
    // start's `% 20` remainder was holding.
    spend_tool_charges(
        world,
        crafter,
        &making.recipe,
        making.batch,
        names,
        ChargeShare::Rest,
    );
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
    let charges = making
        .batch
        .max(1)
        .saturating_mul(making.recipe.made_per_batch.max(1));
    if let Some(container) = making
        .recipe
        .container
        .as_deref()
        .and_then(|id| names.id_of(id))
    {
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
/// The level a craft's roll is centred on.
///
/// Reference: `get_recipe_weighted_skill_average` (`crafting.cpp:1110-1116`): a recipe that names **no** skill counts as `MAX_SKILL`, not as zero - which is why a
/// recipe anybody can make is never at risk, and why reading an absent skill as level zero made every simple craft fail its check every turn.
#[must_use]
pub fn skill_for(world: &World, crafter: Entity, recipe: &Recipe) -> i32 {
    if recipe.skill.is_empty() {
        crate::skills::MAX_SKILL
    } else {
        crate::skills_of::level_of(world, crafter, &recipe.skill)
    }
}

/// Which part of a craft's tool charges a spend covers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChargeShare {
    /// The twentieth taken when the craft starts, plus the remainder that would otherwise be lost.
    Start,
    /// One twentieth for each fifth-percent step the turn crossed.
    Steps(i32),
    /// Everything still owed: what a finished craft has left, which is what the start's remainder by twenty was for.
    Rest,
}

/// Spends the charges for the fifth-percent steps a turn crossed.
pub fn spend_craft_charges(
    world: &mut World,
    crafter: Entity,
    recipe: &Recipe,
    batch: i32,
    names: &crate::item_registry::ItemNames,
    steps: i32,
) {
    spend_tool_charges(
        world,
        crafter,
        recipe,
        batch,
        names,
        ChargeShare::Steps(steps),
    );
}

/// Spends a share of the tool charges a craft needs.
///
/// Reference: `craft_consume_tools` (`crafting.cpp:2362-2381`): `calc_charges` is `charges * batch * multiplier / 20`, and when the craft **starts** the remainder
/// `( charges * batch ) % 20` is added so nothing is lost. The start's share is one twentieth, every fifth-percent step is one twentieth, and the step that
/// **completes** the craft is skipped (`activity_actor.cpp:4250-4253`) - so a finished craft has spent exactly `charges * batch`, which the round before this one
/// spent in one go at the end.
fn spend_tool_charges(
    world: &mut World,
    crafter: Entity,
    recipe: &Recipe,
    batch: i32,
    names: &crate::item_registry::ItemNames,
    share: ChargeShare,
) {
    let batch = batch.max(1);
    for alternatives in recipe.tool_needs() {
        // One alternative per group satisfies it, and the first that can pay does: that is the selection the reference caches on the craft.
        for (item, need) in alternatives {
            let cdda_content::recipes::ToolNeed::Charges(count) = need else {
                continue;
            };
            let whole = count.saturating_mul(batch);
            let mut remaining = match share {
                ChargeShare::Start => whole / 20 + whole % 20,
                ChargeShare::Steps(steps) => whole.saturating_mul(steps.max(0)) / 20,
                ChargeShare::Rest => whole,
            };
            if remaining <= 0 {
                continue;
            }
            let mut stacks: Vec<(Entity, i32)> = Vec::new();
            {
                let mut query =
                    world.query::<(Entity, &crate::creature::Carried, &crate::GroundItem)>();
                for (entity, carried, stack) in query.iter(world) {
                    if carried.0 == crafter
                        && names.name_of(stack.kind).as_deref() == Some(item.as_str())
                    {
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
    let started = begin_craft(world, crafter, recipe, batch, &supplies);
    if started == Craft::Started {
        // Reference: `craft_command.cpp:491` spends the start's share **before** any work is done - a twentieth plus the remainder that would otherwise be lost.
        spend_tool_charges(world, crafter, recipe, batch, names, ChargeShare::Start);
    }
    started
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
    /// When the work will next be checked for failure, in ten-millionths of the craft.
    ///
    /// Reference: `item::set_next_failure_point` (`crafting.cpp:1328-1344`), which the craft sets when it starts and again after every failure. `i64::MAX` means
    /// "never", which is what a craft whose roll leaves nothing to check gets.
    pub next_failure_point: i64,
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
pub fn begin_craft(
    world: &mut World,
    crafter: Entity,
    recipe: &Recipe,
    batch: i32,
    supplies: &Supplies<'_>,
) -> Craft {
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
    // Reference: `item::set_next_failure_point` (`crafting.cpp:1328-1344`) - the first check is placed from the crafter's **own** success roll, so someone who
    // outclasses the recipe is rarely checked at all and someone who does not is checked almost at once.
    let data =
        crate::craft_roll::success_data(recipe.difficulty, skill_for(world, crafter, recipe));
    let first_roll = crate::craft_roll::success_roll(world, data);
    world.entity_mut(crafter).insert(Making {
        recipe: recipe.clone(),
        batch,
        next_failure_point: crate::craft_roll::next_failure_point(0, first_roll),
    });
    Craft::Started
}

/// How much a crafter's mood slows their work.
///
/// Reference: `Character::morale_crafting_speed_multiplier` (`crafting.cpp:209-233`):
///
/// ```text
/// if( morale >= 0 ) return 1.0f;                                   // no bonus for being happy yet
/// float morale_mult = max( 1.0f, 2.0f * rec.difficulty / max( 1.0f, get_skill_level( rec.skill_used ) ) );
/// for( const auto &pr : rec.required_skills )
///     morale_mult *= max( 1.0f, 2.0f * pr.second / max( 1.0f, get_skill_level( pr.first ) ) );
/// float morale_effect = 1.0f + ( morale_mult * morale ) / -50.0f;
/// return 1.0f / morale_effect;
/// ```
///
/// **The reference's comment states the ladder** - *"Halve speed at -50 effective morale, quarter at -150"* - and the arithmetic reaches it from two sides: a crafter whose skill **meets** the
/// difficulty has `morale_mult` of one and the same −50 halves their speed, while a crafter whose skill is **below** it has a multiplier above one, so **the same misery costs them more**. That is
/// why being bad at a job while unhappy is worse than either alone, and it is the half a port would lose by only reading the comment.
///
/// `required_skills` is the recipe's other skills as **(difficulty, the crafter's level)** pairs, in the data's order - the products do not care which is which, but the caller does.
#[must_use]
pub fn morale_speed_multiplier(
    morale: i32,
    difficulty: i32,
    skill_level: i32,
    required_skills: &[(i32, i32)],
) -> f32 {
    /// The reference's `-50.0f`: the drop that halves the speed at a multiplier of one.
    const HALVING_MORALE: f32 = -50.0;
    if morale >= 0 {
        return 1.0;
    }
    #[allow(clippy::cast_precision_loss)]
    let as_float = |value: i32| value as f32;
    let mut morale_mult = (2.0 * as_float(difficulty) / as_float(skill_level.max(1))).max(1.0);
    for &(required_difficulty, level) in required_skills {
        morale_mult *= (2.0 * as_float(required_difficulty) / as_float(level.max(1))).max(1.0);
    }
    let morale_effect = 1.0 + (morale_mult * as_float(morale)) / HALVING_MORALE;
    1.0 / morale_effect
}

/// How a recipe tolerates being worked without light.
///
/// The two flags are mutually exclusive in practice, and where a record carried both the reference would take the `BLIND_EASY` branch first - so the port models them as one choice rather than two flags,
/// which also keeps `Checker::No` distinct from "could not be crafted in the dark at all".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Blindness {
    /// Neither flag: the recipe cannot be worked in the dark.
    No,
    /// `BLIND_EASY`: possible in the dark, at a skill cost that two levels above the requirement erase.
    Easy,
    /// `BLIND_HARD`: possible in the dark once the crafter is two levels past the requirement, and free six levels past it.
    Hard,
}

/// The facts about a recipe that decide whether it can be worked in the dark.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DarkRecipe {
    /// The crafter carries `CRAFT_IN_DARKNESS`, so no recipe is a problem in the dark.
    pub craft_in_darkness: bool,
    /// Which of the two blindness flags the recipe carries, if either.
    pub blindness: Blindness,
    /// The recipe trains and gates on a skill at all.
    pub skill_used: bool,
}

/// How much the light slows a craft down - and whether it forbids it.
///
/// Reference: `Character::lighting_craft_speed_multiplier` (`crafting.cpp:170-207`):
///
/// ```text
/// float darkness = fine_detail_vision_mod( p ) - 4.0f;   // negative is bright, +7.0f is pitch black
/// if( darkness <= 0 ) return 1.0f;
/// if( has_flag( CRAFT_IN_DARKNESS ) ) return 1.0f;
/// if( darkness > 0 && !( BLIND_HARD || BLIND_EASY ) ) return 0.0f;
/// if( BLIND_EASY ) return skill ? 1 - ( darkness / ( 7.0 / 0.75 ) ) * max( 0, 2 - exceeds ) / 2 : 1.0f;
/// if( BLIND_HARD && ( !skill || exceeds >= 2 ) )
///                  return skill ? 1 - ( darkness / ( 7.0 / 0.75 ) ) * max( 0, 8 - exceeds ) / 6 : 0.5f;
/// return 0.0f;
/// ```
///
/// **The last `return 0.0f` is a refusal, not a slow craft**: a `BLIND_HARD` recipe whose crafter is below requirement-plus-two gives **zero**, the same answer as never carrying the flag. The flag
/// only helps a crafter skilled enough for it to help, and that is precisely the case a port would get wrong by treating "blind hard" as "possible in the dark".
///
/// `vision_modifier` is [`crate::sight::fine_detail_vision_mod`]'s answer for the square; `exceeds` is how far the crafter's skill is past the recipe's requirement
/// ([`exceeds_recipe_requirements`]). The recipe's four facts travel together because they **are** one recipe's flags - taking them apart into four arguments invited a caller to pass them in the wrong
/// order and say nothing.
#[must_use]
pub fn lighting_speed_multiplier(vision_modifier: f32, recipe: &DarkRecipe, exceeds: i32) -> f32 {
    /// `fine_detail_vision_mod` treats 4 as "bright enough"; above it the reference counts darkness.
    const BRIGHT_ENOUGH: f32 = 4.0;
    /// The reference's own scale: pitch black is `+7`, and the ladders divide by `7 / 0.75`.
    const PITCH_DARK_DIVISOR: f32 = 7.0 / 0.75;
    let DarkRecipe {
        craft_in_darkness,
        blindness,
        skill_used,
    } = *recipe;
    #[allow(clippy::cast_precision_loss)]
    let as_float = |value: i32| value as f32;
    let darkness = vision_modifier - BRIGHT_ENOUGH;
    if darkness <= 0.0 {
        return 1.0;
    }
    if craft_in_darkness {
        return 1.0;
    }
    let shortfall = |allowed: i32| (as_float(allowed) - as_float(exceeds)).max(0.0);
    match blindness {
        Blindness::No => 0.0,
        Blindness::Easy if !skill_used => {
            // No skill is involved, so there is nothing to be bad at.
            1.0
        }
        Blindness::Easy => 1.0 - (darkness / PITCH_DARK_DIVISOR) * shortfall(2) / 2.0,
        Blindness::Hard if !skill_used => {
            // The reference's own comment: "No skill involved, so apply arbitrary penalty."
            0.5
        }
        Blindness::Hard if exceeds < 2 => 0.0,
        Blindness::Hard => 1.0 - (darkness / PITCH_DARK_DIVISOR) * shortfall(8) / 6.0,
    }
}

/// What pain costs a craft, one percent of speed per point.
///
/// Reference: `crafting_speed_multiplier`'s pain term (`crafting.cpp:303-306`):
///
/// ```text
/// const float pain_multi = rec.has_flag( flag_AFFECTED_BY_PAIN ) ? std::max( 0.0f, 1.0f - ( get_perceived_pain() / 100.0f ) ) : 1.0f;
/// ```
///
/// **The clamp is a floor, not a slope that continues:** a hundred pain costs everything and two hundred cannot cost more than everything. A recipe without `AFFECTED_BY_PAIN` ignores the number
/// entirely, which is why the flag is the caller's argument rather than a branch buried here.
#[must_use]
pub fn pain_speed_multiplier(perceived_pain: i32) -> f32 {
    #[allow(clippy::cast_precision_loss)]
    let pain = perceived_pain as f32;
    (1.0 - pain / 100.0).max(0.0)
}

/// The whole craft speed: morale, light, limb and pain multiplied together, floored at zero.
///
/// Reference: `Character::crafting_speed_multiplier` (`crafting.cpp:303-320`):
///
/// ```text
/// float crafting_speed = morale_crafting_speed_multiplier( rec ) * lighting_craft_speed_multiplier( rec ) * limb_score * pain_multi;
/// const float result = enchantment_cache->modify_value( CRAFTING_SPEED_MULTIPLIER, crafting_speed );
/// return std::max( result, 0.0f );
/// ```
///
/// **Any zero term stops the work**, and the floor at the end means the enchantment cannot push the answer negative either. Each factor is ported and gated on its own -
/// [`morale_speed_multiplier`], [`lighting_speed_multiplier`], [`pain_speed_multiplier`] - so this function is only the product the reference does, not a second copy of the rules.
#[must_use]
pub fn crafting_speed_multiplier(morale: f32, lighting: f32, limb_score: f32, pain: f32) -> f32 {
    (morale * lighting * limb_score * pain).max(0.0)
}

/// A workbench's help, from full speed at its allowed mass or volume down to a quarter at the limit.
///
/// Reference: `lerped_multiplier` (`crafting.cpp:235-246`):
///
/// ```text
/// if( value <= low ) return 1.0f;     // no effect if less than allowed
/// if( value >= high ) return 0.25f;   // bottom out at 25% speed
/// return 1.0f + ( value - low ) * ( 0.25f - 1.0f ) / ( high - low );
/// ```
///
/// **The two ends behave differently:** at or under the allowed value there is **no bonus** - one is the ceiling - and at the far end it **bottoms out at a quarter rather than reaching zero**, so an
/// oversized workpiece is slow rather than impossible. The reference calls it twice, once on mass against the workbench's allowance and once on volume against a tile.
#[must_use]
pub fn lerped_multiplier(value: f32, low: f32, high: f32) -> f32 {
    /// The reference's floor: a workpiece past the bench's allowance still gets a quarter of the speed.
    const BOTTOM: f32 = 0.25;
    if value <= low {
        return 1.0;
    }
    if value >= high {
        return BOTTOM;
    }
    1.0 + (value - low) * (BOTTOM - 1.0) / (high - low)
}

/// How many levels past a recipe's requirements a crafter is - the number both darkness ladders read.
///
/// Reference: `SkillLevelMap::exceeds_recipe_requirements` (`skill.cpp:606-613`):
///
/// ```text
/// int over = rec.skill_used ? get_skill_level( rec.skill_used ) - rec.difficulty : 0;
/// for( const auto &elem : compare_skill_requirements( rec.required_skills ) ) over = std::min( over, elem.second );
/// ```
///
/// **The subtlety is in `compare_skill_requirements`, not in the `min`** (`skill.cpp:591-604`): it drops every entry whose difference is **zero** (`if( diff != 0 )`). A required skill met **exactly**
/// therefore contributes **nothing** instead of a zero, so it cannot cap a crafter who is well past the main skill. A port that compared against zero would answer 0 where the reference answers 2 - and
/// both callers in [`lighting_speed_multiplier`] would then refuse a craft the reference allows.
///
/// `main_skill` is `None` when the recipe uses no skill; `required` is `(the crafter's level, the required level)` per extra skill.
#[must_use]
pub fn exceeds_recipe_requirements(
    main_skill: Option<i32>,
    difficulty: i32,
    required: &[(i32, i32)],
) -> i32 {
    let mut over = main_skill.map_or(0, |level| level - difficulty);
    for &(level, needed) in required {
        let difference = level - needed;
        if difference != 0 {
            over = over.min(difference);
        }
    }
    over
}

/// Whether a crafter knows the **theory** of a recipe, regardless of how rusty their hands are.
///
/// Reference: `SkillLevelMap::theoretical_recipe_requirements` (`skill.cpp:615-626`): every skill - the main one **and** each required one - is asked about its **knowledge level**, not its practical
/// level. A recipe with no main skill only asks about the required ones.
#[must_use]
pub fn theoretical_recipe_requirements(
    main_knowledge: Option<i32>,
    difficulty: i32,
    required: &[(i32, i32)],
) -> bool {
    if main_knowledge.is_some_and(|level| level < difficulty) {
        return false;
    }
    required.iter().all(|&(level, needed)| level >= needed)
}

/// Whether a recipe can be attempted at all: the hands know it **or** the head does.
///
/// Reference: `SkillLevelMap::has_recipe_requirements` (`skill.cpp:630-632`):
///
/// ```text
/// return exceeds_recipe_requirements( rec ) >= 0 || theoretical_recipe_requirements( rec );
/// ```
///
/// **The `||` is the whole rule.** A character whose practical skill has fallen below the requirement - rusted, injured, or simply never practised - can still craft it if their **knowledge** is intact.
/// That is what a survivor who read the book but has not practised lately relies on, and a port that asked only the practical question would lock them out of work they legitimately know.
///
/// The two answers are taken as arguments because they are computed from different skill numbers; [`exceeds_recipe_requirements`] and [`theoretical_recipe_requirements`] produce them.
#[must_use]
pub fn has_recipe_requirements(practical_excess: i32, theory_known: bool) -> bool {
    practical_excess >= 0 || theory_known
}

/// A practice recipe's difficulty and teaching cap, which live beside the recipe they describe.
///
/// Reference: `recipe::get_difficulty` and `recipe::get_skill_cap` (`recipe.cpp:61-77`). They are re-exported here because the crafting rules are read from this module, and because a second copy of the
/// clamp and the `1.25` truncation would be a second place to disagree with the reference.
pub use cdda_content::recipes::{practice_difficulty, skill_cap};

/// How much practice an assistant or a watcher earns from someone else's craft.
///
/// Reference: `Character::craft_skill_gain` (`crafting.cpp:993-1033`):
///
/// ```text
/// if( helper->get_skill_level( making.skill_used ) >= making.difficulty )
///     helper->practice( skill, roll_remainder( num_practice_ticks / 2.0 ), skill_cap );
/// else
///     helper->practice( skill, roll_remainder( num_practice_ticks / 10.0 ), skill_cap );
/// ```
///
/// **The factor of five between the two is the whole reward for understanding the work**, and `roll_remainder` (`rng.cpp:222-233`) keeps the fraction as a **chance** rather than rounding it away - which is
/// why the answer is a draw and not a division. The crafter's own share is the full `num_practice_ticks` and is applied without this function.
#[must_use]
pub fn helper_practice_ticks(
    ticks: i32,
    understands: bool,
    rolls: &mut cdda_core_types::rng::Rolls,
) -> i32 {
    let share = if understands {
        f64::from(ticks) / 2.0
    } else {
        f64::from(ticks) / 10.0
    };
    crate::roll_remainder(share, rolls)
}

/// How many practice ticks a stretch of crafting progress is worth.
///
/// Reference: `craft_activity_actor::do_turn` (`activity_actor.cpp:4229-4240`):
///
/// ```text
/// const double total_practice_ticks = rec.time_to_craft_moves( crafter, recipe_time_flag::ignore_proficiencies ) / 100.0;
/// const int ticks_per_practice = 10000000.0 / total_practice_ticks;
/// int num_practice_ticks = craft.item_counter / ticks_per_practice - old_counter / ticks_per_practice;
/// ```
///
/// **Three integer divisions stacked on a double**, and each one is load-bearing:
///
/// * moves are hundredths of a turn, so a recipe's moves over 100 are how many ticks it is worth;
/// * the progress counter runs to ten million, so `ticks_per_practice` is the counter steps between ticks - **truncated to an integer**, which is what makes an hour's craft answer 3,601 rather than
///   3,600;
/// * and the two counter readings are divided **as integers** before subtracting, so the answer is a count of whole ticks and a partial tick is carried to the next turn rather than lost.
///
/// A recipe with no time would divide by zero; the reference never meets one (the loader rejects a recipe without a time), and the port answers zero rather than a panic.
#[must_use]
pub fn practice_ticks(time_moves: i64, old_counter: i64, new_counter: i64) -> i32 {
    /// The progress counter's full scale (`item::item_counter` reaching 10,000,000 at one craft).
    const COUNTER_SCALE: f64 = 10_000_000.0;
    /// Moves in a turn: practice is counted in turns.
    const MOVES_PER_TURN: f64 = 100.0;
    #[allow(clippy::cast_precision_loss)]
    let total_ticks = time_moves as f64 / MOVES_PER_TURN;
    if total_ticks <= 0.0 {
        return 0;
    }
    #[allow(clippy::cast_possible_truncation)]
    let ticks_per_practice = (COUNTER_SCALE / total_ticks) as i64;
    if ticks_per_practice <= 0 {
        return 0;
    }
    #[allow(clippy::cast_possible_truncation)]
    let ticks = (new_counter / ticks_per_practice - old_counter / ticks_per_practice) as i32;
    ticks
}

/// What one proficiency learns from a stretch of crafting time.
///
/// Reference: `Character::craft_proficiency_gain` (`crafting.cpp:1057-1078`):
///
/// ```text
/// const time_duration learn_time = time / subjects.size();
/// int helper_bonus = 1;
/// for( other : all_crafters ) if( p != other && other->has_proficiency( subject ) ) helper_bonus = 2;
/// p->practice_proficiency( subject, subject.time_multiplier * learn_time * helper_bonus, subject.max_experience );
/// ```
///
/// **The time is divided before the multiplier, not after**, so four subjects on a hundred seconds each practise from a **twenty-five-second** share. A port that multiplied first and divided last would
/// agree on the even cases and disagree on every uneven one.
///
/// **The helper bonus is a flat doubling, not a count**: the loop only ever assigns 2, so two knowledgeable assistants are worth exactly as much as one. That is the sort of "obviously additive" rule a port
/// would improve on - and improving it is inventing behaviour.
#[must_use]
pub fn proficiency_share(
    seconds: i64,
    subject_count: usize,
    time_multiplier: f32,
    helper_knows_it: bool,
) -> i64 {
    if subject_count == 0 {
        return 0;
    }
    #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
    let share = seconds / subject_count as i64;
    #[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)]
    let scaled = (share as f32 * time_multiplier * if helper_knows_it { 2.0 } else { 1.0 }) as i64;
    scaled
}
