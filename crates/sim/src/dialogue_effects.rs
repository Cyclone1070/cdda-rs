//! What a dialogue effect does to the store.
//!
//! Reference: `npctalk_funcs.cpp`, one function per effect name, driven by `talk_effect_fun_t`. The port starts with the effect
//! the data uses most - `math`, 304 of the 1,612 effect entries in the game's effect-on-conditions - because the expression
//! parser and the variable store already exist and only the **assignment** half was missing.

use bevy_ecs::prelude::*;

use cdda_content::dialogue::{Assignment, AssignmentOp, Effect};

use crate::dialogue_values::{self, Value};

/// What came of running an effect.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Applied {
    /// It did what it does.
    Done,
    /// The store could not answer something the effect needed, so nothing was changed.
    CannotAnswer,
}

/// Runs a `math` statement against the speaker's variables.
///
/// Reference: the `math` effect sets a dialogue variable, and the reference's arithmetic on a variable that is not set treats it
/// as **zero** - `dialogue::get_value` answers a null value and the arithmetic reads it as nothing. The port does the same, but
/// only for the ops where that is meaningful: `x += 1` on an unset `x` is one, while `x = y` with an unanswerable `y` changes
/// nothing at all.
pub fn run_assignment(
    world: &mut World,
    speaker: Entity,
    listener: Option<Entity>,
    assignment: &Assignment,
) -> Applied {
    // An assignment reads an unset variable as zero - `dialogue::get_value` answers a null value and the arithmetic treats it
    // as nothing - so `"x = x + 20"` on an unset `x` writes 20 rather than refusing.
    // The dice come out of the store for the length of the evaluation and go back afterwards, so a `rng` inside a `math` statement
    // draws from the game's own generator and the sequence stays replayable.
    let mut dice = crate::singletons::take_rolls(world);
    let drawn = crate::conditions::value_drawing(
        world,
        speaker,
        listener,
        &assignment.expression,
        true,
        dice.as_mut(),
    );
    if let Some(dice) = dice.take() {
        crate::singletons::put_rolls(world, dice);
    }
    let Some(value) = drawn else {
        return Applied::CannotAnswer;
    };
    // A destination that names a **store** rather than a variable: `u_vitamin('x') = 5`, `u_hp('torso') -= 3`. The reference registers a setter beside every
    // getter it has, and the port's stores answer the same way - one function per store, so a store that cannot take the write refuses instead of guessing.
    if let cdda_content::dialogue::AssignmentTarget::Call { name, arguments } = &assignment.target {
        return write_to_store(
            world,
            speaker,
            listener,
            name,
            arguments,
            assignment.op,
            value,
        );
    }
    // A destination naming a scope resolves the way the condition side resolves one: `global_val` is the shared map, and
    // everything else is the speaker's own values.
    let holder = match assignment.target.scope() {
        Some("global_val") => match dialogue_values::global_holder(world) {
            Some(global) => global,
            None => return Applied::CannotAnswer,
        },
        Some("n_val" | "npc_val") => match listener {
            Some(other) => other,
            None => return Applied::CannotAnswer,
        },
        _ => speaker,
    };
    let name = assignment.target.name();
    let previous = match assignment.op {
        AssignmentOp::Set => 0.0,
        _ => dialogue_values::number_of(world, holder, name).unwrap_or(0.0),
    };
    let result = match assignment.op {
        AssignmentOp::Set | AssignmentOp::Add => previous + value,
        AssignmentOp::Subtract => previous - value,
        AssignmentOp::Multiply => previous * value,
        AssignmentOp::Divide => {
            if value == 0.0 {
                return Applied::CannotAnswer;
            }
            previous / value
        }
    };
    dialogue_values::set(world, holder, name, Value::Number(result));
    Applied::Done
}

/// Runs every statement of a `math` effect, stopping at the first the store cannot answer.
///
/// Reference: a `math` entry holds a list of statements and `set_math` runs them in order. Stopping matters: a statement that
/// could not be worked out would otherwise leave the following ones running on a value nobody intended.
pub fn run_math(
    world: &mut World,
    speaker: Entity,
    listener: Option<Entity>,
    assignments: &[Assignment],
) -> Applied {
    for assignment in assignments {
        if run_assignment(world, speaker, listener, assignment) == Applied::CannotAnswer {
            return Applied::CannotAnswer;
        }
    }
    Applied::Done
}

/// Writes a value to the store a function name reads from.
///
/// Reference: the `_ass` half of `math_parser_diag.cpp` - `vitamin_ass`, `hp_ass`, `skill_ass` - each of which resolves its first argument to an id and then
/// sets that store. The port answers the same three stores and **refuses the rest**, because a name with no setter is not a variable and has nowhere to go.
fn write_to_store(
    world: &mut World,
    speaker: Entity,
    listener: Option<Entity>,
    name: &str,
    arguments: &[cdda_content::dialogue::Expression],
    op: cdda_content::dialogue::AssignmentOp,
    value: f64,
) -> Applied {
    // The short listener spelling (`n_skill`) means the same scope as the long one, and a write has to land on the same creature a read would ask about.
    let normalized = crate::conditions::normalized_spelling(name);
    let about_listener = normalized.starts_with("npc_");
    let Some(target) = actor(speaker, listener, about_listener) else {
        return Applied::CannotAnswer;
    };
    let store = normalized
        .trim_start_matches("npc_")
        .trim_start_matches("u_");
    // **`pain` is the one store with no keyword argument**: `pain_ass` (`math_parser_diag.cpp:884-900`) takes only a `type` kwarg, whose default is `raw` - a plain
    // set - so `u_pain() += 10` is a write with nothing to resolve. The `perceived` form adds instead, and it needs the kwargs the port does not read; that is a named gap.
    if store == "pain" {
        let Some(current) = world
            .get::<crate::creature::Pain>(target)
            .map(|pain| f64::from(pain.0))
        else {
            return Applied::CannotAnswer;
        };
        let Some(result) = apply_op(op, current, value) else {
            return Applied::CannotAnswer;
        };
        #[allow(clippy::cast_possible_truncation)]
        crate::creature::set_pain(world, target, result as i32);
        return Applied::Done;
    }
    let Some(cdda_content::dialogue::Expression::Text(key)) = arguments.first() else {
        return Applied::CannotAnswer;
    };
    let (current, writes) = match store {
        "hp" => (
            crate::creature::health_of(world, target).map(f64::from),
            true,
        ),
        "vitamin" => (
            crate::body::vitamin_of(world, target, key).map(f64::from),
            true,
        ),
        "skill" => (
            Some(f64::from(crate::skills_of::level_of(world, target, key))),
            true,
        ),
        "spell_level" => (
            Some(f64::from(crate::known_spells::level_of(world, target, key))),
            true,
        ),
        "spell_exp" => (
            Some(f64::from(crate::known_spells::experience_of(
                world, target, key,
            ))),
            true,
        ),
        _ => (None, false),
    };
    let (Some(current), true) = (current, writes) else {
        return Applied::CannotAnswer;
    };
    let Some(result) = apply_op(op, current, value) else {
        return Applied::CannotAnswer;
    };
    #[allow(clippy::cast_possible_truncation)]
    let rounded = result.round() as i32;
    match store {
        "hp" => crate::creature::set_health(world, target, rounded),
        "vitamin" => {
            crate::body::set_vitamin(world, target, key, rounded);
        }
        "spell_level" => {
            crate::known_spells::set_level(world, target, key, rounded);
        }
        "spell_exp" => {
            crate::known_spells::set_experience(world, target, key, rounded);
        }
        _ => crate::skills_of::set_level(world, target, key, rounded),
    }
    Applied::Done
}

/// The arithmetic an assignment operator means, given what the store held.
fn apply_op(op: cdda_content::dialogue::AssignmentOp, current: f64, value: f64) -> Option<f64> {
    match op {
        cdda_content::dialogue::AssignmentOp::Set => Some(value),
        cdda_content::dialogue::AssignmentOp::Add => Some(current + value),
        cdda_content::dialogue::AssignmentOp::Subtract => Some(current - value),
        cdda_content::dialogue::AssignmentOp::Multiply => Some(current * value),
        cdda_content::dialogue::AssignmentOp::Divide => {
            if value == 0.0 {
                return None;
            }
            Some(current / value)
        }
    }
}

/// Whose trait, effect or value an effect changes.
///
/// Reference: every `f_*` in `npctalk.cpp` takes an `is_npc` flag and asks `d.actor( is_npc )` - the listener for the `npc_`
/// spelling, the speaker for the `u_` one. One helper, so a rule cannot get the two the wrong way round in one place and right in
/// another.
fn actor(speaker: Entity, listener: Option<Entity>, about_listener: bool) -> Option<Entity> {
    if about_listener {
        return listener;
    }
    Some(speaker)
}

/// Runs one of several programs, chosen by weight.
///
/// Reference: `f_weighted_list_eocs` (`npctalk.cpp:6950-6975`) builds a weighted list and picks from it, so the draw is uniform over the **total**
/// weight and an entry of weight zero is never picked. A list with no weight at all refuses rather than picking nothing.
fn run_weighted_list(
    world: &mut World,
    speaker: Entity,
    listener: Option<Entity>,
    weighted: &[(
        cdda_content::dialogue::ProgramRef,
        cdda_content::dialogue::Expression,
    )],
    depth: usize,
) -> Applied {
    let mut dice = crate::singletons::take_rolls(world);
    let mut weights = Vec::with_capacity(weighted.len());
    for (_, weight) in weighted {
        let drawn =
            crate::conditions::value_drawing(world, speaker, listener, weight, true, dice.as_mut());
        let Some(weight) = drawn else {
            if let Some(dice) = dice.take() {
                crate::singletons::put_rolls(world, dice);
            }
            return Applied::CannotAnswer;
        };
        #[allow(clippy::cast_possible_truncation)]
        weights.push(weight.max(0.0) as i32);
    }
    let total: i32 = weights.iter().sum();
    let picked = if total <= 0 {
        None
    } else {
        let Some(dice) = dice.as_mut() else {
            return Applied::CannotAnswer;
        };
        let roll = dice.rng(0, total - 1);
        let mut running = 0;
        let mut picked = None;
        for (index, weight) in weights.iter().enumerate() {
            running += weight;
            if roll < running {
                picked = Some(index);
                break;
            }
        }
        picked
    };
    if let Some(dice) = dice.take() {
        crate::singletons::put_rolls(world, dice);
    }
    let Some(index) = picked else {
        return Applied::CannotAnswer;
    };
    match weighted.get(index) {
        Some((program, _)) => {
            crate::eoc_registry::run_ref(world, speaker, listener, program, depth + 1)
        }
        None => Applied::CannotAnswer,
    }
}

/// Says a line picked from a snippet category.
///
/// Reference: `f_message` (`npctalk.cpp:5385-5400`) with `snippet: true`: the line is picked from the category by weight, and with `same_snippet`
/// the pick is stored in the talker's `<category>_snippet_id` value so the next call says the same line.
fn say_snippet(
    world: &mut World,
    speaker: Entity,
    listener: Option<Entity>,
    category: &cdda_content::dialogue::StringSource,
    same: bool,
    about_listener: bool,
) -> Applied {
    let Some(name) = crate::conditions::text_of(world, speaker, listener, category) else {
        return Applied::CannotAnswer;
    };
    let Some(holder) = actor(speaker, listener, about_listener) else {
        return Applied::CannotAnswer;
    };
    let remembered = format!("{name}_snippet_id");
    if same
        && let Some(known) = crate::dialogue_values::text_of(world, holder, &remembered)
        && let Some(line) = crate::snippets::category(world, &name)
            .into_iter()
            .find(|line| line.id == known)
    {
        crate::messages::push(world, &line.text);
        return Applied::Done;
    }
    let mut dice = crate::singletons::take_rolls(world);
    let picked = crate::snippets::pick(world, &name, dice.as_mut());
    if let Some(dice) = dice.take() {
        crate::singletons::put_rolls(world, dice);
    }
    let Some(line) = picked else {
        return Applied::CannotAnswer;
    };
    if same {
        crate::dialogue_values::set(
            world,
            holder,
            &remembered,
            crate::dialogue_values::Value::Text(line.id.clone()),
        );
    }
    crate::messages::push(world, &line.text);
    Applied::Done
}

/// Runs a branch list of a location search, answering whether every program in it could be answered.
///
/// Reference: `run_eoc_vector` (`npctalk.cpp`): the programs run in order, and a program that cannot be answered stops the list.
fn run_refs(
    world: &mut World,
    speaker: Entity,
    listener: Option<Entity>,
    programs: &[cdda_content::dialogue::ProgramRef],
) -> Applied {
    for program in programs {
        if crate::eoc_registry::run_ref(world, speaker, listener, program, 1)
            == Applied::CannotAnswer
        {
            return Applied::CannotAnswer;
        }
    }
    Applied::Done
}

/// Finds the nearest overmap tile of a terrain and stores it in a dialogue variable.
///
/// Reference: `f_location_variable` (`npctalk.cpp:4194`) with a `target_params` naming an overmap terrain. The port searches the overmap the way
/// `om_terrain` aiming does, and stores the tile's **top-left square**, which is the square a mapgen update lands on when that tile is its target.
fn search_location(
    world: &mut World,
    speaker: Entity,
    listener: Option<Entity>,
    effect: &Effect,
) -> Applied {
    let Effect::LocationVariableSearch {
        scope,
        name,
        om_terrain,
        search_range,
        min_distance,
        min_radius,
        max_radius,
        target_min_radius,
        target_max_radius,
        z,
        furniture,
        then,
        otherwise,
        ..
    } = effect
    else {
        return Applied::CannotAnswer;
    };
    let Some(at) = world.get::<crate::Position>(speaker).copied() else {
        return Applied::CannotAnswer;
    };
    // **The terrain may be named by a variable**, which is how the data passes a tile an earlier program chose (`generalized_eocs.json:26-27`): the reference reads it
    // with `get_str_or_var` (`mission_util.cpp:395`) and evaluates it in the speaker's context.
    let Some(wanted) = crate::conditions::text_of(world, speaker, listener, om_terrain) else {
        return Applied::CannotAnswer;
    };
    // A furniture search compares the **name** the data writes against the furniture on each square, which `furniture_types` turns into the code a square carries.
    let tile = crate::overmap::tile_of(at.x, at.y);
    let Some((x, y)) =
        crate::overmap::nearest_with_terrain(world, tile, &wanted, *search_range, *min_distance)
    else {
        // **A search that finds nothing runs the false branch** (`npctalk.cpp:4374-4378`), which is the program's own plan for an empty search.
        return run_refs(world, speaker, listener, otherwise);
    };
    let found = (
        x * crate::overmap::SQUARES_PER_OMT,
        y * crate::overmap::SQUARES_PER_OMT,
        z.unwrap_or(at.z),
    );
    // The second step: the reference draws within a radius of what the search found (`npctalk.cpp`, the `max_radius > 0` block, which runs after the target
    // search and moves `talker_pos` to the found square first). With no radius to draw, the found square is the answer.
    // With a furniture named, the answer is the first square inside the found tile that carries it - the reference's `search_type == "furniture"` branch.
    if let Some(wanted) = furniture {
        let Some(code) = crate::furniture_types::code_of(world, wanted) else {
            return Applied::CannotAnswer;
        };
        // The walk is the search's own radius around the found tile, and a square closer to the **actor** than `target_min_radius` is skipped - the reference
        // measures that distance from `talker_pos`, not from the tile it found.
        let Some(square) = find_furniture(
            world,
            found,
            code,
            *target_max_radius,
            (at.x, at.y),
            *target_min_radius,
        ) else {
            return run_refs(world, speaker, listener, otherwise);
        };
        let holder = crate::dialogue_values::holder_for(world, speaker, scope == "global_val");
        crate::dialogue_values::set(
            world,
            holder,
            name,
            crate::dialogue_values::Value::Position {
                x: square.0,
                y: square.1,
                z: square.2,
            },
        );
        return Applied::Done;
    }
    let square = if *max_radius > 0 {
        let Some(drawn) = draw_near(world, found, *min_radius, *max_radius) else {
            return run_refs(world, speaker, listener, otherwise);
        };
        drawn
    } else {
        found
    };
    let Some(holder) = crate::conditions::holder_for_scope(world, speaker, None, scope) else {
        return Applied::CannotAnswer;
    };
    crate::dialogue_values::set(
        world,
        holder,
        name,
        crate::dialogue_values::Value::Position {
            x: square.0,
            y: square.1,
            z: square.2,
        },
    );
    // **The square is written before the branch runs**, so `true_eocs` can read the variable the search just set (`npctalk.cpp:4398-4400`), and a branch that cannot
    // be answered is reported rather than swallowed.
    run_refs(world, speaker, listener, then)
}

/// Stores a square found by looking around the actor.
///
/// Reference: `f_location_variable` (`npctalk.cpp`), the `max_radius > 0` block: up to **25** draws of a square within `max_radius`, taking the first at
/// least `min_radius` away, and refusing when none of the 25 is. The draws are the world's dice, so a replay looks in the same places.
fn nearby_location(world: &mut World, speaker: Entity, effect: &Effect) -> Applied {
    let Effect::LocationVariableRadius {
        scope,
        name,
        min_radius,
        max_radius,
        ..
    } = effect
    else {
        return Applied::CannotAnswer;
    };
    let Some(at) = world.get::<crate::Position>(speaker).copied() else {
        return Applied::CannotAnswer;
    };
    let mut dice = crate::singletons::take_rolls(world);
    let Some(dice) = dice.as_mut() else {
        return Applied::CannotAnswer;
    };
    let mut found = None;
    for _ in 0..25 {
        let x = at.x + dice.rng(-*max_radius, *max_radius);
        let y = at.y + dice.rng(-*max_radius, *max_radius);
        // A square the level does not hold is not a place to send anybody, and the reference's `rl_dist` is the same step count the port uses.
        if crate::square::terrain_at(world, (x, y, at.z)).is_none() {
            continue;
        }
        if (x - at.x).abs().max((y - at.y).abs()) < *min_radius {
            continue;
        }
        found = Some((x, y, at.z));
        break;
    }
    let Some(holder) = crate::conditions::holder_for_scope(world, speaker, None, scope) else {
        return Applied::CannotAnswer;
    };
    let Some(square) = found else {
        return Applied::CannotAnswer;
    };
    crate::dialogue_values::set(
        world,
        holder,
        name,
        crate::dialogue_values::Value::Position {
            x: square.0,
            y: square.1,
            z: square.2,
        },
    );
    Applied::Done
}

/// Spawns real monsters and hallucinations around the actor.
///
/// Reference: `f_spawn_monster` (`npctalk.cpp:7475-7500`): `real_count` monsters of the type are made, then `hallucination_count` more that look identical
/// and are not real. Each lands on a square within `min_radius..max_radius` of the actor.
///
/// The port marks a hallucination with a [`crate::monster_spawning::Hallucination`] component so anything that asks "is this real" can answer. What it does
/// **not** do yet is fade them: the reference's hallucinations are removed when the player gets a good look, which is a system the port has not built.
/// Moves a creature to the square a variable holds.
///
/// Reference: `f_teleport` (`npctalk.cpp:7836-7845`): the variable is read as a point and the creature's position becomes it. A variable that holds no
/// location - unset, or holding a number or a text - cannot answer where to go, so the effect refuses rather than leaving the creature where it was.
/// Gives the player a mission.
///
/// Reference: `f_assign_mission` (`npctalk.cpp:5913-5925`): the mission is reserved and assigned to `get_avatar()`, not to whoever is speaking - an NPC's
/// dialogue can hand the player a mission. The port asks the store who the avatar is and falls back to the speaker, which is the divergence recorded for
/// `u_has_mission`.
fn give_mission(
    world: &mut World,
    speaker: Entity,
    listener: Option<Entity>,
    effect: &Effect,
) -> Applied {
    let Effect::AssignMission { id, deadline } = effect else {
        return Applied::CannotAnswer;
    };
    // The deadline is worked out the way the reference does - a `dbl_or_var` evaluated when the effect runs - and the port has **no place to keep it yet**: the
    // mission store holds which missions are active and no due turn. Evaluating it here means a deadline the dialogue cannot work out still refuses the entry
    // instead of assigning a mission the data expected to be dated.
    if let Some(expression) = deadline {
        let Some(held) = draw(world, speaker, listener, expression) else {
            return Applied::CannotAnswer;
        };
        let _ = held;
    }
    let Some(mission) = crate::conditions::text_of(world, speaker, listener, id) else {
        return Applied::CannotAnswer;
    };
    let holder = crate::missions::question_holder(world, speaker);
    crate::missions::accept(world, holder, &mission);
    Applied::Done
}

fn move_creature(
    world: &mut World,
    speaker: Entity,
    listener: Option<Entity>,
    effect: &Effect,
) -> Applied {
    let Effect::Teleport {
        scope,
        name,
        listener: about_listener,
        fail_message,
        success_message,
        force,
        force_safe,
    } = effect
    else {
        return Applied::CannotAnswer;
    };
    // The listener form moves the other creature, and without one there is nothing to move.
    let actor = if *about_listener {
        listener
    } else {
        Some(speaker)
    };
    let Some(actor) = actor else {
        return Applied::CannotAnswer;
    };
    // **The target is read through the scope it names.** The reference's `read_var_value( target_var, d )` answers from the **dialogue's context** for a `context_val` and from
    // the named holder otherwise (`dialogue_helpers.cpp:11-16`), so a program handed a position by its caller - `EOC_TELEPAD` gets `trap_location` that way - can move there.
    let Some(crate::dialogue_values::Value::Position { x, y, z }) =
        target_of(world, actor, listener, scope, name)
    else {
        // Reference: `f_teleport` shows `fail_message` when the target variable holds no square, and the move does not happen.
        show(world, speaker, listener, fail_message.as_ref());
        return Applied::CannotAnswer;
    };
    // **The move is the teleport rule, not a write to the position.** Reference: `f_teleport` calls `teleport::teleport_to_point( ..., force, force_safe )` (`npctalk.cpp:7836-7845`),
    // so a worn dimensional anchor, a teleport lock, a square off the world or a solid landing refuses here exactly as it does for a spell - and `force` is the flag the data uses to
    // overrule the anchor.
    let outcome =
        crate::teleport::to_point_with(world, actor, (x, y, z), true, *force, *force_safe);
    if matches!(outcome, crate::teleport::Teleport::Moved { .. }) {
        // Reference: `f_teleport` shows `success_message` after a move that happened, and an absent one says nothing.
        show(world, speaker, listener, success_message.as_ref());
        return Applied::Done;
    }
    // Reference: and `fail_message` when `teleport_to_point` answers `false`, which includes the same-square case.
    show(world, speaker, listener, fail_message.as_ref());
    Applied::CannotAnswer
}

/// **The square a variable names**, by the scope it is spelled in.
///
/// Reference: `f_teleport`'s `read_var_value( target_var, d ).tripoint()` (`npctalk.cpp:7836-7840`). `context_val` is not a holder's variable at all - it is the **invocation's
/// context** (`dialogue.h`), which the port keeps as text, so a point arrives as `x,y,z` and is read back here. Every other scope reads the holder the port has always read.
#[must_use]
pub fn target_of(
    world: &World,
    actor: Entity,
    listener: Option<Entity>,
    scope: &str,
    name: &str,
) -> Option<crate::dialogue_values::Value> {
    let found = if scope == "context_val" {
        if let Some(held) = crate::dialogue_context::values(world, actor).get(name)
            && let Some(point) = point_of_text(held)
        {
            return Some(point);
        }
        crate::dialogue_values::value_of(world, actor, name)
    } else if scope == "global_val" {
        // The global store is an entity of its own; every other scope is the creature it names.
        let holder = crate::dialogue_values::global_holder(world)?;
        crate::dialogue_values::value_of(world, holder, name)
    } else {
        crate::dialogue_values::value_of(world, listener.unwrap_or(actor), name)
    };
    // **An unset variable is the origin, not a refusal.** `read_var_value` answers an empty `diag_value` for a variable nobody set, and `diag_value::tripoint()` converts an empty
    // value to a null `tripoint_abs_ms` - `( 0, 0, 0 )` absolute (`math_parser_diag_value.cpp:62-70`). The reference therefore moves the creature to the map's origin when the
    // square's variable is empty, and nine programs in the data reach a teleport whose variable nothing ever set.
    Some(found.unwrap_or(crate::dialogue_values::Value::Position { x: 0, y: 0, z: 0 }))
}

/// **Writes a square to the scope that names it** - the other half of [`target_of`], and the reason `location_variable_adjust` can move a value the caller handed over.
///
/// Reference: `write_var_value( var.type, var.name, &d, target_pos )` (`npctalk.cpp:4442-4446`), which writes into the dialogue's context for a `context_val` and into the named
/// holder otherwise. The port keeps context values as text, so a point is written back as `x,y,z` - the same spelling [`point_of_text`] reads.
fn store_target(
    world: &mut World,
    speaker: Entity,
    listener: Option<Entity>,
    scope: &str,
    name: &str,
    value: crate::dialogue_values::Value,
) -> Applied {
    if scope == "context_val" {
        let crate::dialogue_values::Value::Position { x, y, z } = value else {
            return Applied::CannotAnswer;
        };
        let mut values = crate::dialogue_context::values(world, speaker);
        values.insert(name.to_owned(), format!("{x},{y},{z}"));
        let _ = crate::dialogue_context::put(world, speaker, values);
        return Applied::Done;
    }
    let target = if scope == "global_val" {
        let Some(global) = crate::dialogue_values::global_holder(world) else {
            return Applied::CannotAnswer;
        };
        global
    } else {
        listener.unwrap_or(speaker)
    };
    crate::dialogue_values::set(world, target, name, value);
    Applied::Done
}

/// Reads a square written as `x,y,z`, which is how a context value carries a position between one program and the next.
#[must_use]
fn point_of_text(text: &str) -> Option<crate::dialogue_values::Value> {
    let mut parts = text.split(',');
    let x = parts.next()?.trim().parse::<i32>().ok()?;
    let y = parts.next()?.trim().parse::<i32>().ok()?;
    let z = parts.next()?.trim().parse::<i32>().ok()?;
    if parts.next().is_some() {
        return None;
    }
    Some(crate::dialogue_values::Value::Position { x, y, z })
}

/// Shows a line the data gave beside an effect, when it gave one.
fn show(
    world: &mut World,
    speaker: Entity,
    listener: Option<Entity>,
    line: Option<&cdda_content::dialogue::StringSource>,
) {
    let Some(source) = line else {
        return;
    };
    if let Some(text) = crate::conditions::text_of(world, speaker, listener, source) {
        crate::messages::push(world, &text);
    }
}

/// Picks a monster already standing within `range` of the speaker, to copy.
///
/// Reference: `f_spawn_monster` (`npctalk.cpp:7433-7465`): when the spawn names no monster, every hostile creature within `target_range` of the player is a
/// candidate - the player's own square excluded - and one is drawn at random. No candidate is not an error in the reference: it runs the effect's `false_eocs`,
/// which the port does not read, so here it refuses rather than claiming a spawn that did not happen.
fn copy_a_nearby_monster(
    world: &World,
    speaker: Entity,
    range: i32,
    dice: &mut cdda_core_types::rng::Rolls,
) -> Option<String> {
    let holder = crate::singletons::holder::<crate::monster_registry::TheMonsterTypes>(world)
        .and_then(|holder| world.get::<crate::monster_registry::MonsterTypes>(holder))?;
    let here = world.get::<crate::Position>(speaker).copied()?;
    let anchor = (here.x, here.y, here.z);
    let mut candidates: Vec<String> = Vec::new();
    if let Some(mut query) = world.try_query::<(&crate::creature::CreatureKind, &crate::Position)>()
    {
        for (kind, position) in query.iter(world) {
            if (position.x - anchor.0)
                .abs()
                .max((position.y - anchor.1).abs())
                > range
            {
                continue;
            }
            if (position.x, position.y) == (anchor.0, anchor.1) {
                continue;
            }
            if let Some(name) = holder.name_of(kind.0) {
                candidates.push(name.to_owned());
            }
        }
    }
    if candidates.is_empty() {
        return None;
    }
    candidates.sort();
    let last = i32::try_from(candidates.len() - 1).ok()?;
    let at = dice.rng(0, last);
    usize::try_from(at)
        .ok()
        .and_then(|at| candidates.get(at))
        .cloned()
}

/// Picks the monster type a spawn names: the member itself, or one drawn from the group it names.
///
/// Reference: `f_spawn_monster` (`npctalk.cpp:7390`) reads `group` as "this member is a group", and `MonsterGroupManager::GetMonsterFromGroup` draws one of the
/// group's monsters by weight. Without `group` the member is the type, so the two forms meet here and nowhere else.
fn pick_type(
    world: &mut World,
    speaker: Entity,
    named: &str,
    group: bool,
    target_range: Option<i32>,
) -> Option<String> {
    // Reference: `f_spawn_monster` (`npctalk.cpp:7433-7465`) - an **empty** member names no monster at all, and the game then copies one of the hostile
    // creatures within `target_range`, so a hallucination looks like something that is already there.
    if named.is_empty() && !group {
        let mut held = crate::singletons::take_rolls(world);
        let drawn = held.as_mut().and_then(|dice| {
            copy_a_nearby_monster(world, speaker, target_range.unwrap_or(0), dice)
        });
        if let Some(dice) = held.take() {
            crate::singletons::put_rolls(world, dice);
        }
        if drawn.is_none() && std::env::var("CDDA_SHOW_SPAWN").is_ok() {
            println!("spawn refused: no creature to copy within {target_range:?}");
        }
        return drawn;
    }
    if !group {
        return Some(named.to_owned());
    }
    let mut held = crate::singletons::take_rolls(world);
    let drawn = held
        .as_mut()
        .and_then(|dice| crate::monster_groups::pick(world, named, dice));
    if let Some(dice) = held.take() {
        crate::singletons::put_rolls(world, dice);
    }
    if drawn.is_none() && std::env::var("CDDA_SHOW_SPAWN").is_ok() {
        println!("spawn refused: group {named} picked nothing");
    }
    drawn
}

/// Schedules any effect for later, which is what `time_in_future` means on an effect that is not a call.
///
/// Reference: a `timed_event` with a payload. The port keeps the effect itself in the `scheduled` store, and the turn loop's `Scheduled` stage runs it through the same
/// runner an immediate one would use.
fn run_later(
    world: &mut World,
    speaker: Entity,
    listener: Option<Entity>,
    effect: &Effect,
) -> Applied {
    let Effect::Later { effect, after } = effect else {
        return Applied::CannotAnswer;
    };
    let Some(turns) = crate::dialogue_effects::wait_turns(world, speaker, listener, after) else {
        return Applied::CannotAnswer;
    };
    crate::timed_events::schedule_effect(world, turns, (**effect).clone());
    Applied::Done
}

/// Schedules a program list to run after a delay, and answers `Done` because scheduling is the whole effect.
///
/// Reference: `f_run_eocs` with `time_in_future` (`npctalk.cpp:6332-6338`): the duration is turned into a turn by the calendar, the programs are kept, and the turn
/// loop runs them when that turn arrives. A duration the port cannot read is refused rather than run now.
fn run_eoc_later(
    world: &mut World,
    speaker: Entity,
    listener: Option<Entity>,
    effect: &Effect,
) -> Applied {
    let Effect::RunEocLater { programs, after } = effect else {
        return Applied::CannotAnswer;
    };
    let Some(turns) = wait_turns(world, speaker, listener, after) else {
        return Applied::CannotAnswer;
    };
    crate::timed_events::schedule_effect(world, turns, Effect::RunEoc(programs.clone()));
    Applied::Done
}

/// How many turns a `duration_or_var` asks for, drawing between its ends when it has two.
///
/// Reference: `get_duration_or_var` (`npctalk.cpp:3426-3450`) with a `dbl_or_var` range: the value is drawn and read as **turns**, and a string is parsed by the same
/// duration reader the rest of the data uses.
fn wait_turns(
    world: &mut World,
    speaker: Entity,
    listener: Option<Entity>,
    after: &cdda_content::dialogue::DurationSource,
) -> Option<i64> {
    let drawn = |world: &World, source: &cdda_content::dialogue::StringSource| {
        crate::conditions::text_of(world, speaker, listener, source)
    };
    let text = match after {
        cdda_content::dialogue::DurationSource::One(one) => drawn(world, one),
        cdda_content::dialogue::DurationSource::Range(shortest, longest) => {
            let (short, long) = (drawn(world, shortest), drawn(world, longest));
            match (short, long) {
                // Reference: a range is drawn between its ends with the dialogue's dice.
                (Some(short), Some(long)) => match draw_one_of(world, 2) {
                    Some(0) => Some(short),
                    Some(_) => Some(long),
                    None => None,
                },
                _ => None,
            }
        }
    };
    let text = text?;
    Some(cdda_content::constructions::parse_duration(&text))
}

/// Gives the creature mutations, and answers only a depth that asks for none.
///
/// Reference: `f_mutate` (`npctalk.cpp:3568-3570`): `d.actor( is_npc )->mutate( highest_cat )`, which draws mutations from the pools the depth reaches. The port has no
/// mutation pool, so a depth of **zero** - which asks for nothing - is answered, and any real depth refuses. **The refusal is the honest half of a store gap: the rule is
/// read, and the day a mutation pool exists this branch loses its second sentence rather than being rewritten.**
fn mutate(
    world: &mut World,
    speaker: Entity,
    listener: Option<Entity>,
    effect: &Effect,
) -> Applied {
    let Effect::Mutation {
        highest_category,
        npc,
        true_random,
        ..
    } = effect
    else {
        return Applied::CannotAnswer;
    };
    // Reference: `d.actor( is_npc )` - the `npc_` spellings act on the listener, and with no listener there is nobody to mutate.
    let creature = if *npc {
        let Some(listener) = listener else {
            return Applied::CannotAnswer;
        };
        listener
    } else {
        speaker
    };
    let Some(depth) = draw(world, speaker, listener, highest_category) else {
        return Applied::CannotAnswer;
    };
    if depth <= 0.0 {
        return Applied::Done;
    }
    // A depth above zero asks for a change, and which change comes from **the category the creature has gone furthest into** - `f_mutate` takes the highest of the
    // creature's own mutation category levels (`npctalk.cpp:3560-3570`) rather than the depth itself.
    // The tables are cloned out of the world before anything mutates it: `gain` inserts components, and a borrow of the data would be holding the world at the same time.
    let Some(data) = crate::mutation_data::holder(world) else {
        return Applied::CannotAnswer;
    };
    let Some((category, _)) = crate::mutations::strongest_category(world, creature, &data) else {
        return Applied::Done;
    };
    let Some(categories) = crate::mutation_data::categories(world) else {
        return Applied::CannotAnswer;
    };
    let mut rolls =
        crate::singletons::take_rolls(world).unwrap_or_else(|| cdda_core_types::rng::Rolls::new(0));
    // Reference: `true_random` skips the good/bad roll (`npctalk.cpp:3581-3584`), which is how the data asks for a mutation of any kind.
    let gained = crate::mutations::mutate_category_with(
        world,
        creature,
        &data,
        &categories,
        &category,
        *true_random,
        &mut rolls,
    );
    crate::singletons::put_rolls(world, rolls);
    match gained {
        Some(_) => Applied::Done,
        None => Applied::CannotAnswer,
    }
}

/// Stores the square of a monster of a given kind found around the actor.
///
/// Reference: `f_location_variable` (`npctalk.cpp:4300-4320`): the walk is the same `points_in_radius` the furniture search makes, comparing the creature's type id,
/// and a square at or inside `target_min_radius` is skipped. The port looks for the **nearest** match rather than the first in walk order, which is what the radius
/// walk answers when the squares are visited by distance - and it answers a square, not the creature, because the data stores a location.
fn store_monster_around(world: &mut World, speaker: Entity, effect: &Effect) -> Applied {
    let Effect::MonsterAround {
        monster,
        scope,
        name,
        min_radius,
        max_radius,
        ..
    } = effect
    else {
        return Applied::CannotAnswer;
    };
    let Some(crate::Position { x, y, z }) = world.get::<crate::Position>(speaker).copied() else {
        return Applied::CannotAnswer;
    };
    let mut found: Option<((i32, i32, i32), i32)> = None;
    let mut kinds: Vec<(crate::MonsterKindId, (i32, i32, i32))> = Vec::new();
    let mut query = world.query::<(&crate::creature::CreatureKind, &crate::Position)>();
    for (kind, at) in query.iter(world) {
        kinds.push((kind.0, (at.x, at.y, at.z)));
    }
    for (kind, at) in kinds {
        let Some(their_name) = crate::monster_registry::name_of(world, kind) else {
            continue;
        };
        if their_name != *monster || at.2 != z {
            continue;
        }
        let distance = (at.0 - x).abs().max((at.1 - y).abs());
        if distance <= *min_radius || distance > *max_radius {
            continue;
        }
        if found.is_none_or(|(_, best)| distance < best) {
            found = Some((at, distance));
        }
    }
    let Some((at, _)) = found else {
        return Applied::CannotAnswer;
    };
    let holder = crate::dialogue_values::holder_for(world, speaker, scope == "global_val");
    crate::dialogue_values::set(
        world,
        holder,
        name,
        crate::dialogue_values::Value::Position {
            x: at.0,
            y: at.1,
            z: at.2,
        },
    );
    Applied::Done
}

/// Says what the data asked for, which may be a variable's text.
///
/// Reference: `f_message` with a `translation_or_var`: the text is worked out **when the message is shown**, so an effect earlier in the same list can have set the
/// variable. An unset variable with no `default` says nothing at all - and the port refuses rather than announcing a variable's name as if it were the message.
fn say_message(
    world: &mut World,
    speaker: Entity,
    listener: Option<Entity>,
    effect: &Effect,
) -> Applied {
    let Effect::Message { text, fallback } = effect else {
        return Applied::CannotAnswer;
    };
    let said = crate::conditions::text_of(world, speaker, listener, text).or_else(|| {
        let fallback = fallback.as_ref()?;
        crate::conditions::text_of(world, speaker, listener, fallback)
    });
    match said {
        Some(said) => {
            crate::messages::push(world, &said);
            Applied::Done
        }
        None => Applied::CannotAnswer,
    }
}

/// Changes the item the dialogue is talking about into another kind.
///
/// Reference: `f_transform_item` (`npctalk.cpp:6112-6131`): the dialogue's `beta` must be an item, and `convert` replaces its type. The port keeps the item's **entity**
/// and changes the kind it names, which is what "the same item, a different type" means in a store where an item is an entity.
fn transform_item(world: &mut World, listener: Option<Entity>, effect: &Effect) -> Applied {
    let Effect::TransformItem { item, active } = effect else {
        return Applied::CannotAnswer;
    };
    let Some(target) = listener else {
        return Applied::CannotAnswer;
    };
    let Some(kind_name) = crate::conditions::text_of(world, target, None, item) else {
        return Applied::CannotAnswer;
    };
    let names = crate::item_registry::snapshot(world);
    let Some(kind) = names.id_of(&kind_name) else {
        return Applied::CannotAnswer;
    };
    let Some(ground) = world.get::<crate::GroundItem>(target).cloned() else {
        return Applied::CannotAnswer;
    };
    world
        .entity_mut(target)
        .insert(crate::GroundItem { kind, ..ground });
    // Reference: `( *it )->active = activate || ( *it )->has_temperature()` (`npctalk.cpp:6127`). The port's items carry no active flag yet and no temperature, so the
    // data's own `active` is read and **not stored** - named here rather than pretended, because an item that thinks it is active is a rule the port has not written.
    let _ = active;
    Applied::Done
}

/// Runs an effect list once for every id in a list, naming each one in a variable.
///
/// Reference: `f_foreach` (`npctalk.cpp:7038-7078`): the list is built once, then each element is written into the variable and the effect list is applied.
/// **The variable is written, not the answer**: a `<context_val:id>` inside the list reads whatever the last iteration said, which is how a message names each flag.
fn run_foreach(
    world: &mut World,
    speaker: Entity,
    listener: Option<Entity>,
    effect: &Effect,
) -> Applied {
    let Effect::ForEach {
        source,
        target,
        ids,
        scope,
        name,
        effects,
    } = effect
    else {
        return Applied::CannotAnswer;
    };
    let elements = foreach_elements(world, speaker, listener, source, target.as_deref(), ids);
    let Some(elements) = elements else {
        return Applied::CannotAnswer;
    };
    let Some(holder) = crate::conditions::holder_for_scope(world, speaker, listener, scope) else {
        return Applied::CannotAnswer;
    };
    for element in elements {
        crate::dialogue_values::set(
            world,
            holder,
            name,
            crate::dialogue_values::Value::Text(element),
        );
        let applied = run_effects(world, speaker, listener, effects);
        if applied == Applied::CannotAnswer {
            return Applied::CannotAnswer;
        }
    }
    Applied::Done
}

/// The ids a `foreach` walks, or nothing when the port does not know the list.
///
/// Reference: `f_foreach` (`npctalk.cpp:7041-7073`) - `ids` walks a registry by name, `array` walks what the data wrote out, and a kind the port has no registry for
/// refuses rather than walking an empty list, which would look like the data asking for nothing.
fn foreach_elements(
    world: &World,
    speaker: Entity,
    listener: Option<Entity>,
    source: &str,
    target: Option<&str>,
    ids: &[cdda_content::dialogue::StringSource],
) -> Option<Vec<String>> {
    match source {
        "ids" => match target? {
            "flag" => Some(crate::flag_registry::flags_of(world)),
            "trait" => Some(
                crate::mutation_data::holder(world)?
                    .ids()
                    .cloned()
                    .collect(),
            ),
            "vitamin" => Some(
                crate::vitamin_registry::vitamins_of(world)
                    .keys()
                    .cloned()
                    .collect(),
            ),
            _ => None,
        },
        "array" => ids
            .iter()
            .map(|id| crate::conditions::text_of(world, speaker, listener, id))
            .collect(),
        _ => None,
    }
}

/// Remembers a place's tiles and schedules them to go back.
///
/// Reference: `f_revert_location` (`npctalk.cpp:4898-4917`): the place variable holds the square, the copy is taken **now** (`submap::get_revert_submap`), and one
/// `REVERT_SUBMAP` event is queued per submap - four of them, which the port's single store does in one snapshot because a square is a square.
fn revert_location(
    world: &mut World,
    speaker: Entity,
    listener: Option<Entity>,
    effect: &Effect,
) -> Applied {
    let Effect::RevertLocation {
        scope,
        name,
        after,
        key,
    } = effect
    else {
        return Applied::CannotAnswer;
    };
    let Some(holder) = crate::conditions::holder_for_scope(world, speaker, listener, scope) else {
        return Applied::CannotAnswer;
    };
    let Some(crate::dialogue_values::Value::Position { x, y, z }) =
        crate::dialogue_values::value_of(world, holder, name)
    else {
        return Applied::CannotAnswer;
    };
    let Some(turns) = roll_duration(world, speaker, listener, after) else {
        return Applied::CannotAnswer;
    };
    crate::revert::take(world, (x, y, z));
    let event = match key {
        None => "REVERT_SUBMAP".to_owned(),
        Some(key) => match crate::conditions::text_of(world, speaker, listener, key) {
            Some(text) if text.is_empty() => "REVERT_SUBMAP".to_owned(),
            Some(text) => text,
            None => return Applied::CannotAnswer,
        },
    };
    // Reference: `calendar::turn + dov_time_in_future.evaluate( d ) + 1_seconds` (`npctalk.cpp:4904`) - the extra second syncs the event with the turn's own events - and
    // `timed_events::add( REVERT_SUBMAP, ..., map_square, ... )` **names the square**, because the event reverts that submap and not the whole map.
    crate::timed_events::schedule_carrying_at(
        world,
        &event,
        turns.saturating_add(1),
        "",
        Some((x, y, z)),
    );
    Applied::Done
}

/// Moves a square the dialogue holds, and stores the new one.
///
/// Reference: `f_location_variable_adjust` (`npctalk.cpp:4419-4446`): with `overmap_tile` the x and y moves count **overmap tiles** rather than squares, `z_override`
/// replaces the level instead of moving it, and the answer goes to `output_var` when the data names one and back to the input variable when it does not.
fn adjust_location(
    world: &mut World,
    speaker: Entity,
    listener: Option<Entity>,
    effect: &Effect,
) -> Applied {
    let Effect::LocationVariableAdjust {
        scope,
        name,
        output,
        x_adjust,
        y_adjust,
        z_adjust,
        z_override,
        overmap_tile,
    } = effect
    else {
        return Applied::CannotAnswer;
    };
    // **The square is read through its scope**, the same lookup the teleport uses: a `context_val` lives in the invocation's context rather than in a holder, and the telepad
    // adjusts exactly that (`EOC_TELEPAD`, `traps.json:527`).
    let Some(crate::dialogue_values::Value::Position { x, y, z }) =
        target_of(world, speaker, listener, scope, name)
    else {
        return Applied::CannotAnswer;
    };
    let (Some(dx), Some(dy), Some(dz)) = (
        draw(world, speaker, listener, x_adjust),
        draw(world, speaker, listener, y_adjust),
        draw(world, speaker, listener, z_adjust),
    ) else {
        return Applied::CannotAnswer;
    };
    let tile = if *overmap_tile {
        crate::overmap::SQUARES_PER_OMT
    } else {
        1
    };
    let moved = (
        x + as_whole(dx) * tile,
        y + as_whole(dy) * tile,
        if *z_override {
            as_whole(dz)
        } else {
            z + as_whole(dz)
        },
    );
    let (scope, name) = match output {
        Some((scope, name)) => (scope.clone(), name.clone()),
        None => (scope.clone(), name.clone()),
    };
    store_target(
        world,
        speaker,
        listener,
        &scope,
        &name,
        crate::dialogue_values::Value::Position {
            x: moved.0,
            y: moved.1,
            z: moved.2,
        },
    )
}

/// Runs programs on the items a creature carries, once per matching item.
///
/// Reference: `f_run_inv_eocs` (`npctalk.cpp:6760-6805`) with the `"all"` mode runs `true_eocs` on every matching item, handing each one to the program through the
/// invocation's context. The port sets the context the data reads - `id` for the item's id and `count` for how many the stack holds - and runs the programs.
fn run_inv_eocs(
    world: &mut World,
    speaker: Entity,
    listener: Option<Entity>,
    effect: &Effect,
) -> Applied {
    let Effect::RunInvEocs {
        search_data,
        random,
        listener: on_listener,
        manual,
        programs,
        false_programs,
    } = effect
    else {
        return Applied::CannotAnswer;
    };
    // A `manual` menu asks the player which items to act on, and the port has no chooser: refusing is the honest answer, where picking for the player would run the
    // program on items they did not choose.
    if *manual {
        return Applied::CannotAnswer;
    }
    // **`npc_run_inv_eocs` works on the listener's items**, which is the only difference between the two members: the same rule, aimed at the other creature. The scope name
    // is the dialogue's own way of saying whose items are being asked about.
    let scope = if *on_listener { "n_val" } else { "u_val" };
    let Some(holder) = crate::conditions::holder_for_scope(world, speaker, listener, scope) else {
        return Applied::CannotAnswer;
    };
    // Every item the creature carries, with how it is held - which is what a filter entry may ask about.
    let mut carried: Vec<(Entity, crate::ItemTypeId, i32, bool, bool)> = Vec::new();
    {
        let mut wielded_query =
            world.query::<(Entity, &crate::creature::WieldedBy, &crate::GroundItem)>();
        let wielded: std::collections::BTreeSet<Entity> = wielded_query
            .iter(world)
            .filter(|(_, who, _)| who.0 == holder)
            .map(|(entity, _, _)| entity)
            .collect();
        let mut worn_query =
            world.query::<(Entity, &crate::creature::WornBy, &crate::GroundItem)>();
        let worn: std::collections::BTreeSet<Entity> = worn_query
            .iter(world)
            .filter(|(_, who, _)| who.0 == holder)
            .map(|(entity, _, _)| entity)
            .collect();
        let mut carried_query =
            world.query::<(Entity, &crate::creature::Carried, &crate::GroundItem)>();
        for (entity, who, item) in carried_query.iter(world) {
            if who.0 == holder {
                carried.push((
                    entity,
                    item.kind,
                    item.charges.max(1),
                    wielded.contains(&entity),
                    worn.contains(&entity),
                ));
            }
        }
    }
    let mut matches: Vec<(Entity, String, i32)> = Vec::new();
    for (entity, kind, charges, is_wielded, is_worn) in carried {
        let Some(name) = crate::item_registry::name_of(world, kind) else {
            continue;
        };
        // Reference: `run_item_eocs` (`npctalk.cpp:3341-3348`) - an empty list matches everything, otherwise **any one** entry matching is enough.
        let accepted = search_data.is_empty()
            || search_data.iter().any(|filter| {
                filter_matches(world, speaker, entity, &name, is_wielded, is_worn, filter)
            });
        if accepted {
            matches.push((entity, name, charges));
        }
    }
    // Reference: **nothing matched runs the false branch once** (`npctalk.cpp:3376-3378`, and the same answer in every mode). It belongs to the dialogue rather than to an
    // item: there is no item to hand the program as its beta, which is why it is run once rather than per item that failed the filter.
    if matches.is_empty() {
        if false_programs.is_empty() {
            return Applied::Done;
        }
        for program in false_programs {
            let _ = crate::eoc_registry::run_ref(world, speaker, listener, program, 1);
        }
        return Applied::Done;
    }
    // Reference: the `"random"` mode picks **one** of the matching items with the game's dice; `"all"` runs on every one.
    if *random {
        let Some(picked) = draw_one_of(world, matches.len()) else {
            return Applied::CannotAnswer;
        };
        matches = matches.get(picked).cloned().into_iter().collect();
    }
    for (item, name, count) in matches {
        let handed = std::collections::BTreeMap::from([
            ("id".to_owned(), name),
            ("count".to_owned(), count.to_string()),
        ]);
        let previous = crate::dialogue_context::put(world, speaker, handed);
        for program in programs {
            let _ = crate::eoc_registry::run_ref(world, speaker, Some(item), program, 1);
        }
        crate::dialogue_context::restore(world, speaker, previous);
    }
    Applied::Done
}

/// Marks an achievement completed on the character the dialogue speaks for.
///
/// Reference: `f_give_achievment` (`npctalk.cpp:5566-5585`) reports the achievement only while it is pending, which
/// [`crate::achievements::complete`] answers with "was it new".
fn give_achievement(
    world: &mut World,
    speaker: Entity,
    listener: Option<Entity>,
    effect: &Effect,
) -> Applied {
    let Effect::GiveAchievement { achievement } = effect else {
        return Applied::CannotAnswer;
    };
    let Some(named) = crate::conditions::text_of(world, speaker, listener, achievement) else {
        return Applied::CannotAnswer;
    };
    let holder = crate::missions::question_holder(world, speaker);
    crate::achievements::complete(world, holder, &named);
    Applied::Done
}

/// Applies a terrain transform to the squares around a creature.
///
/// Reference: `f_transform_radius` (`npctalk.cpp:4483-4493`): with no `time_in_future` the reference calls `map::transform_radius` at once, over the reality bubble
/// around `target_pos`. The port does the same over its loaded level, and the reading and writing of squares lives in
/// [`crate::terrain_transform`], which is where a transform's meaning belongs.
fn transform_radius(
    world: &mut World,
    speaker: Entity,
    listener: Option<Entity>,
    effect: &Effect,
) -> Applied {
    let Effect::TransformRadius {
        transform,
        radius,
        target,
        listener: theirs,
    } = effect
    else {
        return Applied::CannotAnswer;
    };
    // Reference: `f_transform_radius` (`npctalk.cpp:4450-4468`): with a `target_var` the transform is centred on the square the variable holds rather than on the
    // actor, which is how the data drains a site it stored earlier.
    let here = if let Some(target) = target {
        crate::dialogue_values::position_of(world, speaker, listener, target)
            .map(|(x, y, z)| crate::Position { x, y, z })
    } else {
        let Some(actor) = actor(speaker, listener, *theirs) else {
            return Applied::CannotAnswer;
        };
        world.get::<crate::Position>(actor).copied()
    };
    let Some(here) = here else {
        return Applied::CannotAnswer;
    };
    let Some(named) = crate::conditions::text_of(world, speaker, listener, transform) else {
        return Applied::CannotAnswer;
    };
    let Some(reach) = draw(world, speaker, listener, radius) else {
        return Applied::CannotAnswer;
    };
    if crate::terrain_types::holder(world).is_none()
        || crate::terrain_transform::holder(world).is_none()
    {
        return Applied::CannotAnswer;
    }
    let reach = as_whole(reach).clamp(0, cdda_core_types::light::MAX_VIEW_DISTANCE);
    let _ = crate::terrain_transform::apply(world, &named, (here.x, here.y, here.z), reach);
    Applied::Done
}

/// Schedules the light level the data sets, one second after it is due to end.
///
/// Reference: `f_custom_light_level` (`npctalk.cpp:7226-7231`): a `timed_event_type::CUSTOM_LIGHT_LEVEL` is added at
/// `calendar::turn + length + 1_seconds`, carrying the level and the key. The port schedules the same event, under a key that **carries the level**, because its
/// timed-event store holds a name and a due turn - and the level is read here, where the reference reads it, so an entry whose level cannot be worked out refuses
/// instead of scheduling nothing.
fn custom_light_level(
    world: &mut World,
    speaker: Entity,
    listener: Option<Entity>,
    effect: &Effect,
) -> Applied {
    let Effect::CustomLightLevel { level, length, key } = effect else {
        return Applied::CannotAnswer;
    };
    let Some(level) = draw(world, speaker, listener, level) else {
        return Applied::CannotAnswer;
    };
    let Some(turns) = roll_duration(world, speaker, listener, length) else {
        return Applied::CannotAnswer;
    };
    let named = match key {
        None => String::new(),
        Some(source) => match crate::conditions::text_of(world, speaker, listener, source) {
            Some(text) => text,
            None => return Applied::CannotAnswer,
        },
    };
    let whole = as_whole(level);
    // The reference's own `+1_seconds`: the event is ticked on the turn it is applied, before it has an effect.
    let key = format!("custom_light_level:{whole}:{named}");
    crate::timed_events::schedule(world, &key, turns.saturating_add(1));
    Applied::Done
}

/// Puts a field on the squares around a creature.
///
/// Reference: `f_field` (`npctalk.cpp:7707-7725`): `points_in_radius( target_pos, radius )` gets the field at `intensity`. The port refuses an entry with no
/// radius - the reference's default is ten million squares, which the port's loaded level cannot honour - and clamps a stated one to the view, which is as far as a
/// level it holds can reach.
fn set_field(
    world: &mut World,
    speaker: Entity,
    listener: Option<Entity>,
    effect: &Effect,
) -> Applied {
    let Effect::SetField {
        field,
        intensity,
        radius,
        listener: theirs,
    } = effect
    else {
        return Applied::CannotAnswer;
    };
    let Some(actor) = actor(speaker, listener, *theirs) else {
        return Applied::CannotAnswer;
    };
    let (Some(here), Some(named), Some(radius)) = (
        world.get::<crate::Position>(actor).copied(),
        crate::conditions::text_of(world, speaker, listener, field),
        radius.as_ref(),
    ) else {
        return Applied::CannotAnswer;
    };
    let Some(reach) = draw(world, speaker, listener, radius) else {
        return Applied::CannotAnswer;
    };
    let Some(thickness) = draw(world, speaker, listener, intensity) else {
        return Applied::CannotAnswer;
    };
    let Some(id) = crate::field_registry::id_of(world, &named) else {
        return Applied::CannotAnswer;
    };
    let reach = as_whole(reach).clamp(0, cdda_core_types::light::MAX_VIEW_DISTANCE);
    let thickness = as_whole(thickness);
    for dy in -reach..=reach {
        for dx in -reach..=reach {
            crate::add_field_strength(world, (here.x + dx, here.y + dy, here.z), id, thickness);
        }
    }
    Applied::Done
}

/// The first square within `radius` of `centre` that carries a given furniture, or nothing.
///
/// Reference: `f_location_variable` (`npctalk.cpp:4290-4300`): the `points_in_radius` walk, comparing `map::furn`.
fn find_furniture(
    world: &World,
    centre: (i32, i32, i32),
    code: u32,
    radius: i32,
    actor: (i32, i32),
    min_from_actor: i32,
) -> Option<(i32, i32, i32)> {
    let reach = radius.max(0);
    for dy in -reach..=reach {
        for dx in -reach..=reach {
            let at = (centre.0 + dx, centre.1 + dy, centre.2);
            if (at.0 - actor.0).abs().max((at.1 - actor.1).abs()) <= min_from_actor {
                continue;
            }
            if crate::square::furniture_at(world, at) == Some(code) {
                return Some(at);
            }
        }
    }
    None
}

/// Writes the square of a piece of furniture found around the actor into a variable.
///
/// Reference: `f_location_variable` (`npctalk.cpp:4250-4300`), the `search_type = "furniture"` path: the same walk as the terrain search, comparing
/// `map::furn` at each square. Nothing found leaves the actor's own square, because the reference starts the answer there.
fn store_furniture_around(
    world: &mut World,
    speaker: Entity,
    listener: Option<Entity>,
    effect: &Effect,
) -> Applied {
    let Effect::FurnitureAround {
        furniture,
        scope,
        name,
        target_min_radius,
        target_max_radius,
        listener: theirs,
    } = effect
    else {
        return Applied::CannotAnswer;
    };
    let Some(actor) = actor(speaker, listener, *theirs) else {
        return Applied::CannotAnswer;
    };
    let Some(here) = world.get::<crate::Position>(actor).copied() else {
        return Applied::CannotAnswer;
    };
    let Some(wanted) = crate::conditions::text_of(world, speaker, listener, furniture) else {
        return Applied::CannotAnswer;
    };
    let Some(wanted_code) = crate::furniture_types::code_of(world, &wanted) else {
        return Applied::CannotAnswer;
    };
    let mut found = None;
    'search: for dy in -*target_max_radius..*target_max_radius {
        for dx in -*target_max_radius..*target_max_radius {
            let (x, y) = (here.x + dx, here.y + dy);
            if dx.abs().max(dy.abs()) <= *target_min_radius {
                continue;
            }
            if crate::square::furniture_at(world, (x, y, here.z)) == Some(wanted_code) {
                found = Some((x, y));
                break 'search;
            }
        }
    }
    let (x, y) = found.unwrap_or((here.x, here.y));
    let holder = crate::dialogue_values::holder_for(world, actor, scope == "global_val");
    crate::dialogue_values::set(
        world,
        holder,
        name,
        crate::dialogue_values::Value::Position { x, y, z: here.z },
    );
    Applied::Done
}

/// Writes the square of a terrain found around the actor into a variable.
///
/// Reference: `f_location_variable` (`npctalk.cpp:4250-4300`): the walk is `points_in_radius( talker_pos, target_max_radius, 0 )` - z, then y, then x ascending -
/// and the first square whose terrain matches and which is further than `target_min_radius` wins. Nothing found leaves the actor's own square in the variable,
/// because the reference starts the answer there.
fn store_terrain_around(
    world: &mut World,
    speaker: Entity,
    listener: Option<Entity>,
    effect: &Effect,
) -> Applied {
    let Effect::TerrainAround {
        terrain,
        scope,
        name,
        target_min_radius,
        target_max_radius,
        listener: theirs,
    } = effect
    else {
        return Applied::CannotAnswer;
    };
    let Some(actor) = actor(speaker, listener, *theirs) else {
        return Applied::CannotAnswer;
    };
    let Some(here) = world.get::<crate::Position>(actor).copied() else {
        return Applied::CannotAnswer;
    };
    let Some(wanted) = crate::conditions::text_of(world, speaker, listener, terrain) else {
        return Applied::CannotAnswer;
    };
    let Some(wanted_code) = crate::terrain_types::code_of(world, &wanted) else {
        return Applied::CannotAnswer;
    };
    let mut found = None;
    'search: for dy in -*target_max_radius..*target_max_radius {
        for dx in -*target_max_radius..*target_max_radius {
            let (x, y) = (here.x + dx, here.y + dy);
            let distance = dx.abs().max(dy.abs());
            if distance <= *target_min_radius {
                continue;
            }
            if crate::square::terrain_at(world, (x, y, here.z)) == Some(wanted_code) {
                found = Some((x, y));
                break 'search;
            }
        }
    }
    let (x, y) = found.unwrap_or((here.x, here.y));
    let holder = crate::dialogue_values::holder_for(world, actor, scope == "global_val");
    crate::dialogue_values::set(
        world,
        holder,
        name,
        crate::dialogue_values::Value::Position { x, y, z: here.z },
    );
    Applied::Done
}

/// Casts a spell the dialogue names.
///
/// Reference: `f_cast_spell` (`npctalk.cpp`): the member is the spell's id, `hit_self` decides whether it lands on the speaker or the listener, and the level is the
/// caster's own level in that spell unless the data forces one. The port looks the prototype up in [`crate::spell_types`] and hands it to `spell_effects::apply`, which
/// is the same table the CLI casts from - so a dialogue cast and a played cast run the same rule.
fn cast_spell(
    world: &mut World,
    speaker: Entity,
    listener: Option<Entity>,
    effect: &Effect,
) -> Applied {
    let Effect::CastSpell {
        spell,
        hit_self,
        targeted,
        listener: theirs,
    } = effect
    else {
        return Applied::CannotAnswer;
    };
    // A targeted cast asks the player to pick a square (`spell::select_target`, `npctalk.cpp:5650-5657`), and this port has no chooser in a headless world.
    // Refusing is the honest answer: a spell cast at a square nobody chose is not the effect the data asked for.
    if *targeted {
        return Applied::CannotAnswer;
    }
    let Some(caster) = actor(speaker, listener, *theirs) else {
        return Applied::CannotAnswer;
    };
    // The id is a `str_or_var` like every other id in a dialogue: the data may name a spell it read from a context value.
    let Some(named) = crate::conditions::text_of(world, speaker, listener, spell) else {
        return Applied::CannotAnswer;
    };
    let Some(prototype) = crate::spell_types::find(world, &named) else {
        return Applied::CannotAnswer;
    };
    let level = crate::known_spells::level_of(world, caster, &named);
    // `hit_self` puts the spell's centre on the caster; otherwise it is aimed at the other party, which is where the dialogue's own target stands.
    let target = if *hit_self {
        Some(caster)
    } else {
        listener.or(Some(speaker))
    };
    let Some(centre) = target.and_then(|entity| world.get::<crate::Position>(entity).copied())
    else {
        return Applied::CannotAnswer;
    };
    // A cast that reaches nobody still happened: the reference spends the moves and casts, and the effect simply finds nothing. Only an effect the port has no rule
    // for is a refusal, because that one did not happen.
    match crate::spell_effects::apply(
        world,
        caster,
        &prototype,
        level,
        (centre.x, centre.y, centre.z),
    ) {
        crate::spell_effects::Effective::NotPorted => Applied::CannotAnswer,
        crate::spell_effects::Effective::Done
        | crate::spell_effects::Effective::NothingAffected => Applied::Done,
    }
}

/// Starts an activity on a creature.
///
/// Reference: `f_assign_activity` (`npctalk.cpp:5456-5460`): `target->assign_activity( activity_id( ... ), to_moves<int>( duration ) )`. The port's activity is the
/// `Doing` component, and its length is in moves like the reference's, so the duration is multiplied by a hundred - the same conversion `turn_cost` uses.
fn assign_activity(
    world: &mut World,
    speaker: Entity,
    listener: Option<Entity>,
    effect: &Effect,
) -> Applied {
    let Effect::AssignActivity {
        activity,
        duration,
        listener: theirs,
    } = effect
    else {
        return Applied::CannotAnswer;
    };
    let Some(creature) = actor(speaker, listener, *theirs) else {
        return Applied::CannotAnswer;
    };
    let Some(named) = crate::conditions::text_of(world, speaker, listener, activity) else {
        return Applied::CannotAnswer;
    };
    let Some(turns) = roll_duration(world, speaker, listener, duration) else {
        return Applied::CannotAnswer;
    };
    // The duration is a turn count and the activity's length is in moves, so the multiplication is exact; the clamp keeps a silly number from wrapping, and the
    // cast is the one place the two widths meet.
    #[allow(clippy::cast_possible_truncation)]
    let moves_left = turns
        .saturating_mul(100)
        .clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32;
    world.entity_mut(creature).insert(crate::creature::Doing {
        moves_total: 0,
        activity: named,
        moves_left,
        basis: crate::creature::Basis::Time,
        subject: None,
    });
    Applied::Done
}

/// Writes the speaking creature's id into a variable as text.
///
/// Reference: `f_set_talker` (`npctalk.cpp:6943-6947`): the id is **stringified** on the way in, because a later `run_eocs` reads the variable back as a string. The
/// port's id is the entity's index, which is an integer that is never a pointer - the same guarantee the reference's `getID()` gives it.
fn set_talker(
    world: &mut World,
    speaker: Entity,
    listener: Option<Entity>,
    effect: &Effect,
) -> Applied {
    let Effect::SetTalker {
        target,
        listener: theirs,
    } = effect
    else {
        return Applied::CannotAnswer;
    };
    let Some(creature) = actor(speaker, listener, *theirs) else {
        return Applied::CannotAnswer;
    };
    let holder = match target.0.as_str() {
        "global_val" => crate::dialogue_values::holder_for(world, speaker, true),
        _ => crate::dialogue_values::holder_for(world, creature, false),
    };
    let id = creature.index().to_string();
    crate::dialogue_values::set(
        world,
        holder,
        &target.1,
        crate::dialogue_values::Value::Text(id),
    );
    Applied::Done
}

/// Takes a morale point away from a creature.
///
/// Reference: `f_lose_morale` (`npctalk.cpp:7201-7203`): `d.actor( is_npc )->remove_morale( morale_type( ... ) )`, which erases every point of that type. Types the
/// creature is not feeling are left alone, and losing one it never had is not a failure.
fn lose_morale(
    world: &mut World,
    speaker: Entity,
    listener: Option<Entity>,
    effect: &Effect,
) -> Applied {
    let Effect::LoseMorale {
        kind,
        listener: theirs,
    } = effect
    else {
        return Applied::CannotAnswer;
    };
    let Some(creature) = actor(speaker, listener, *theirs) else {
        return Applied::CannotAnswer;
    };
    let Some(named) = crate::conditions::text_of(world, speaker, listener, kind) else {
        return Applied::CannotAnswer;
    };
    crate::morale::remove_kind(world, creature, &named);
    Applied::Done
}

/// Removes every item of a kind from a creature.
///
/// Reference: `f_remove_item_with` (`npctalk.cpp:4048-4054`): `d.actor( is_npc )->remove_items_with( ... )` with a predicate on the type id - all of that kind and
/// nothing else. A kind the creature does not carry is not a failure: the reference's predicate simply matches nothing.
fn remove_item_with(
    world: &mut World,
    speaker: Entity,
    listener: Option<Entity>,
    effect: &Effect,
) -> Applied {
    let Effect::RemoveItemWith {
        item,
        listener: theirs,
    } = effect
    else {
        return Applied::CannotAnswer;
    };
    let Some(creature) = actor(speaker, listener, *theirs) else {
        return Applied::CannotAnswer;
    };
    let Some(named) = crate::conditions::text_of(world, speaker, listener, item) else {
        return Applied::CannotAnswer;
    };
    crate::inventory::remove_all_of(world, creature, &named);
    Applied::Done
}

/// Schedules the place to be renamed when the time is up.
///
/// Reference: `f_place_override` (`npctalk.cpp:4812-4818`): one timed event, `OVERRIDE_PLACE`, due at `calendar::turn + length + 1_seconds`, carrying the new name
/// and the key. The port's turn is a second, so the plus-one is a turn. The renaming itself happens when that event comes due, which is the next round's work - and
/// this function is where it will be found.
fn place_override(
    world: &mut World,
    speaker: Entity,
    listener: Option<Entity>,
    effect: &Effect,
) -> Applied {
    let Effect::PlaceOverride {
        length,
        key,
        place,
        fallback,
    } = effect
    else {
        return Applied::CannotAnswer;
    };
    let Some(turns) = roll_duration(world, speaker, listener, length) else {
        return Applied::CannotAnswer;
    };
    // Reference: a `translation_or_var` reads the variable and falls back to the data's own text when it holds nothing.
    let named = crate::conditions::text_of(world, speaker, listener, place).or_else(|| {
        let fallback = fallback.as_ref()?;
        crate::conditions::text_of(world, speaker, listener, fallback)
    });
    let Some(named) = named else {
        return Applied::CannotAnswer;
    };
    let event = match key {
        None => "OVERRIDE_PLACE".to_owned(),
        Some(key) => match crate::conditions::text_of(world, speaker, listener, key) {
            Some(text) if text.is_empty() => "OVERRIDE_PLACE".to_owned(),
            Some(text) => text,
            None => return Applied::CannotAnswer,
        },
    };
    // Reference: the name rides in the event (`npctalk.cpp:4810-4814`), and whatever reads the place's name reads it back until the event comes due.
    crate::timed_events::schedule_carrying(world, &event, turns.saturating_add(1), &named);
    Applied::Done
}

/// Makes a creature wetter.
///
/// Reference: `f_add_wet` (`npctalk.cpp:5468-5472`): the amount is worked out where the effect is applied and added with `wet_character`, which clamps.
fn add_wet(
    world: &mut World,
    speaker: Entity,
    listener: Option<Entity>,
    effect: &Effect,
) -> Applied {
    let Effect::AddWet {
        amount,
        listener: theirs,
    } = effect
    else {
        return Applied::CannotAnswer;
    };
    let Some(creature) = actor(speaker, listener, *theirs) else {
        return Applied::CannotAnswer;
    };
    let Some(drawn) = crate::conditions::value(world, speaker, listener, amount) else {
        return Applied::CannotAnswer;
    };
    #[allow(clippy::cast_possible_truncation)]
    let whole = drawn.round() as i32;
    crate::wetness::add(world, creature, whole);
    Applied::Done
}

/// Spends the speaker's action points on the words.
///
/// Reference: `f_turn_cost` (`npctalk.cpp:5295-5299`): `target->mod_moves( -to_moves<int>( cost ) )`, and `to_moves` is a hundred moves a turn. The port's moves
/// are whole points already, so the conversion is a multiplication and a cast.
fn turn_cost(
    world: &mut World,
    speaker: Entity,
    listener: Option<Entity>,
    effect: &Effect,
) -> Applied {
    let Effect::TurnCost { cost } = effect else {
        return Applied::CannotAnswer;
    };
    let Some(turns) = roll_duration(world, speaker, listener, cost) else {
        return Applied::CannotAnswer;
    };
    #[allow(clippy::cast_precision_loss)]
    let moves = (turns.saturating_mul(100)) as f64;
    crate::creature::spend_moves(world, speaker, moves);
    Applied::Done
}

/// Drops a mission from the avatar's list, without calling it done or failed.
///
/// Reference: `f_remove_active_mission` (`npctalk.cpp:5964-5972`): the avatar's mission of that type is removed; one the character does not hold is not an error.
fn remove_active_mission(
    world: &mut World,
    speaker: Entity,
    listener: Option<Entity>,
    effect: &Effect,
) -> Applied {
    let Effect::RemoveActiveMission { mission } = effect else {
        return Applied::CannotAnswer;
    };
    let Some(named) = crate::conditions::text_of(world, speaker, listener, mission) else {
        return Applied::CannotAnswer;
    };
    let Some(creature) = crate::missions::avatar(world) else {
        return Applied::CannotAnswer;
    };
    crate::missions::finish(world, creature, &named);
    Applied::Done
}

/// Finishes a mission: taken off the character's list.
///
/// Reference: `f_finish_mission` (`npctalk.cpp:5939-5950`): the avatar's own mission is completed or failed, and a mission the character does not hold does nothing.
/// A staged mission (`step`) is not run yet: the port has no step counter, and answering `Done` for one would claim a rule it does not have.
fn finish_mission(
    world: &mut World,
    speaker: Entity,
    listener: Option<Entity>,
    effect: &Effect,
) -> Applied {
    let Effect::FinishMission { mission, step, .. } = effect else {
        return Applied::CannotAnswer;
    };
    if step.is_some() {
        return Applied::CannotAnswer;
    }
    let Some(named) = crate::conditions::text_of(world, speaker, listener, mission) else {
        return Applied::CannotAnswer;
    };
    let Some(creature) = crate::missions::avatar(world) else {
        return Applied::CannotAnswer;
    };
    // A mission the character does not hold is not an error - the reference's `has_mission` guard simply does nothing - so the answer is `Done` either way.
    crate::missions::finish(world, creature, &named);
    Applied::Done
}

/// Stores a named condition, to be asked later.
///
/// Reference: `f_set_condition` (`npctalk.cpp:5868-5880`): the condition is written into the dialogue's variables, and `u_has_condition` reads it back and evaluates
/// it where it is asked - so the stored thing is the **condition**, not its answer at the time it was stored.
fn set_condition(
    world: &mut World,
    speaker: Entity,
    listener: Option<Entity>,
    effect: &Effect,
) -> Applied {
    let Effect::SetCondition {
        name,
        condition,
        scope,
    } = effect
    else {
        return Applied::CannotAnswer;
    };
    // Reference: `d.set_conditional` writes into the dialogue's own map, which exists whether or not anything has been stored yet - so the global holder is **made** when the
    // first condition is stored, rather than the store failing for want of one.
    let holder = if scope == "global_val" {
        crate::dialogue_values::holder_for(world, speaker, true)
    } else {
        match crate::conditions::holder_for_scope(world, speaker, listener, scope) {
            Some(holder) => holder,
            None => return Applied::CannotAnswer,
        }
    };
    let Some(stored) = crate::conditions::text_of(world, speaker, listener, name) else {
        return Applied::CannotAnswer;
    };
    crate::dialogue_values::set(
        world,
        holder,
        &stored,
        crate::dialogue_values::Value::Condition(condition.clone()),
    );
    Applied::Done
}

/// Schedules a named event a set time from now.
///
/// Reference: `f_alter_timed_events` (`npctalk.cpp:4879-4881`): `get_timed_events().set_all( key, now + time_in_future )`. The duration is parsed by the same
/// reader the calendar uses, so a duration the port cannot parse is a refusal rather than a silent zero.
fn alter_timed_events(
    world: &mut World,
    speaker: Entity,
    listener: Option<Entity>,
    effect: &Effect,
) -> Applied {
    let Effect::AlterTimedEvents {
        key,
        time_in_future,
    } = effect
    else {
        return Applied::CannotAnswer;
    };
    let Some(named) = crate::conditions::text_of(world, speaker, listener, key) else {
        return Applied::CannotAnswer;
    };
    let Some(duration) = crate::conditions::text_of(world, speaker, listener, time_in_future)
    else {
        return Applied::CannotAnswer;
    };
    let turns = cdda_content::constructions::parse_duration(&duration);
    crate::timed_events::schedule(world, &named, turns);
    Applied::Done
}

/// Copies one variable's value into another.
///
/// Reference: `f_copy_var` (`npctalk.cpp:5797-5807`): `read_var_value` then `write_var_value`, with nothing drawn or interpreted in between. A source the store does
/// not hold copies nothing, because the reference copies a null value and writing null is not writing a text.
fn copy_var(
    world: &mut World,
    speaker: Entity,
    listener: Option<Entity>,
    effect: &Effect,
) -> Applied {
    let Effect::CopyVar {
        from_scope,
        from_name,
        to_scope,
        to_name,
    } = effect
    else {
        return Applied::CannotAnswer;
    };
    let Some(from) = crate::conditions::holder_for_scope(world, speaker, listener, from_scope)
    else {
        return Applied::CannotAnswer;
    };
    let Some(to) = crate::conditions::holder_for_scope(world, speaker, listener, to_scope) else {
        return Applied::CannotAnswer;
    };
    let Some(value) = crate::dialogue_values::value_of(world, from, from_name) else {
        return Applied::Done;
    };
    crate::dialogue_values::set(world, to, to_name, value);
    Applied::Done
}

/// Stores a text into a variable, drawing it from a list when the data gives one.
///
/// Reference: `f_set_string_var` (`npctalk.cpp:5843-5866`): one entry of the list is drawn, the variable is written, and nothing else happens. The draw uses the
/// world's own dice, which is the same dice every other draw in this module uses.
fn set_string_var(
    world: &mut World,
    speaker: Entity,
    listener: Option<Entity>,
    effect: &Effect,
) -> Applied {
    let Effect::SetStringVar {
        parse_tags,
        value,
        scope,
        name,
    } = effect
    else {
        return Applied::CannotAnswer;
    };
    let Some(holder) = crate::conditions::holder_for_scope(world, speaker, listener, scope) else {
        return Applied::CannotAnswer;
    };
    let text = match value {
        cdda_content::dialogue::VarValue::One(one) => {
            // **A variable that was never set reads as the empty string** (`read_var_value`'s default), and a stored empty string is an answer: the program that expected
            // a name finds none of them.
            crate::conditions::text_of(world, speaker, listener, one).unwrap_or_default()
        }
        cdda_content::dialogue::VarValue::Many(choices) => {
            let Some(drawn) = draw_one_of(world, choices.len()) else {
                return Applied::CannotAnswer;
            };
            let Some(choice) = choices.get(drawn) else {
                return Applied::CannotAnswer;
            };
            let Some(text) = crate::conditions::text_of(world, speaker, listener, choice) else {
                return Applied::CannotAnswer;
            };
            text
        }
    };
    // Reference: with `parse_tags`, the text is the reference's tag line and every `<scope:name>` in it is substituted where the effect runs.
    let text = if *parse_tags {
        crate::dialogue_values::expand_tags(world, speaker, listener, &text)
    } else {
        text
    };
    crate::dialogue_values::set(
        world,
        holder,
        name,
        crate::dialogue_values::Value::Text(text),
    );
    Applied::Done
}

/// Draws which of `many` things the dice picked.
fn draw_one_of(world: &mut World, many: usize) -> Option<usize> {
    let mut dice = crate::singletons::take_rolls(world);
    let last = i32::try_from(many.checked_sub(1)?).ok();
    let drawn = dice
        .as_mut()
        .zip(last)
        .map(|(rolls, last)| usize::try_from(rolls.rng(0, last)).ok());
    if let Some(dice) = dice.take() {
        crate::singletons::put_rolls(world, dice);
    }
    drawn.flatten()
}

/// Takes items out of what a creature carries.
///
/// Reference: `f_consume_item` (`npctalk.cpp:3943-3968`): charges when only charges were asked for, whole stacks when a count was, and a message when the creature
/// has neither - the message is not an error, so the effect is `Done` either way. A partial removal never happens, because [`crate::inventory::take`] refuses one.
fn consume_items(
    world: &mut World,
    speaker: Entity,
    listener: Option<Entity>,
    effect: &Effect,
) -> Applied {
    let Effect::ConsumeItem {
        item,
        count,
        charges,
        listener: about_listener,
        ..
    } = effect
    else {
        return Applied::CannotAnswer;
    };
    let Some(creature) = actor(speaker, listener, *about_listener) else {
        return Applied::CannotAnswer;
    };
    let Some(name) = crate::conditions::text_of(world, speaker, listener, item) else {
        return Applied::CannotAnswer;
    };
    let (Some(many), Some(held)) = (
        draw_count(world, speaker, listener, count),
        draw_count(world, speaker, listener, charges),
    ) else {
        return Applied::CannotAnswer;
    };
    crate::inventory::take(world, creature, &name, many, held);
    Applied::Done
}

/// Where a spawned item goes: the square a `loc` variable holds, or the actor's own.
///
/// Reference: `f_spawn_item` (`npctalk.cpp:3795`): `loc` is a `var_info`, and when the data gives none the item is added to the talker, which stands on its own
/// square. A location variable holding anything but a square is nowhere to put an item, so it refuses.
fn item_place(
    world: &mut World,
    speaker: Entity,
    listener: Option<Entity>,
    loc: Option<&(String, String)>,
) -> Option<crate::Position> {
    let Some((scope, name)) = loc else {
        return world.get::<crate::Position>(speaker).copied();
    };
    let holder = crate::dialogue_values::holder_for(world, speaker, scope == "global_val");
    let _ = listener;
    match crate::dialogue_values::value_of(world, holder, name) {
        Some(crate::dialogue_values::Value::Position { x, y, z }) => {
            Some(crate::Position { x, y, z })
        }
        _ => None,
    }
}

/// Puts items on the actor's square.
///
/// Reference: `f_spawn_item` (`npctalk.cpp:3771-3800`): `count` of them, each with `charges` charges when the data gives charges. An item the port's catalogue does
/// not know is refused rather than dropped as something nameless.
fn spawn_items(
    world: &mut World,
    speaker: Entity,
    listener: Option<Entity>,
    effect: &Effect,
) -> Applied {
    let Effect::SpawnItem {
        item,
        count,
        charges,
        force_equip,
        use_item_group,
        loc,
        ..
    } = effect
    else {
        return Applied::CannotAnswer;
    };
    // Reference: `force_equip` puts the item in the creature's hands, and the port has no wield slot to put it in - so it refuses rather than dropping the item
    // where the data did not ask for it.
    if *force_equip {
        return Applied::CannotAnswer;
    }
    // Reference: `f_spawn_item` (`npctalk.cpp:3795`) puts the item where `loc` says, and at the actor's own square when the data names no place.
    let Some(at) = item_place(world, speaker, listener, loc.as_ref()) else {
        return Applied::CannotAnswer;
    };
    let Some(name) = crate::conditions::text_of(world, speaker, listener, item) else {
        return Applied::CannotAnswer;
    };
    // Reference: `use_item_group` sends the name to `item_group::item_from` instead of the item catalogue, and what the group produces is what appears.
    if *use_item_group {
        let Some(produced) = crate::item_groups::draw(world, &name) else {
            return Applied::CannotAnswer;
        };
        for (made, how_many) in produced {
            let Some(kind) = crate::item_registry::id_of(world, &made) else {
                return Applied::CannotAnswer;
            };
            for _ in 0..how_many.max(1) {
                crate::drop_item(world, (at.x, at.y), kind, 1);
            }
        }
        return Applied::Done;
    }
    let Some(kind) = crate::item_registry::id_of(world, &name) else {
        return Applied::CannotAnswer;
    };
    let (Some(many), Some(held)) = (
        draw_count(world, speaker, listener, count),
        draw_count(world, speaker, listener, charges),
    ) else {
        return Applied::CannotAnswer;
    };
    for _ in 0..many.max(0) {
        crate::drop_item(world, (at.x, at.y), kind, held);
    }
    Applied::Done
}

/// Brings NPCs of a class into being around the actor.
///
/// Reference: `f_spawn_npc` (`npctalk.cpp:7575-7620`): the squares are drawn within the radii - twenty-five attempts, the reference's own number - the real ones
/// are placed before the imagined ones, and each is given the traits the data lists. The class id is the creature's name until the port has NPC class prototypes
/// to look up; the being, its place and its traits are all real.
fn spawn_npcs(
    world: &mut World,
    speaker: Entity,
    listener: Option<Entity>,
    effect: &Effect,
) -> Applied {
    let Effect::SpawnNpc {
        class,
        unique_id,
        traits,
        real_count,
        hallucination_count,
        min_radius,
        max_radius,
        outdoor_only,
        indoor_only,
        lifespan,
        ..
    } = effect
    else {
        return Applied::CannotAnswer;
    };
    let (Some(at), Some(name)) = (
        world.get::<crate::Position>(speaker).copied(),
        crate::conditions::text_of(world, speaker, listener, class),
    ) else {
        return Applied::CannotAnswer;
    };
    let (Some(real), Some(imagined), Some(near), Some(far)) = (
        draw_count(world, speaker, listener, real_count),
        draw_count(world, speaker, listener, hallucination_count),
        draw_count(world, speaker, listener, min_radius),
        draw_count(world, speaker, listener, max_radius),
    ) else {
        return Applied::CannotAnswer;
    };
    let mut held = crate::singletons::take_rolls(world);
    let mut squares = Vec::new();
    for _ in 0..(real + imagined) {
        // Reference: `f_spawn_npc` keeps a spawn to squares the outdoor flags allow, exactly as the monster spawn does.
        let Some(around) = draw_near_with_where(
            world,
            held.as_mut(),
            (at.x, at.y, at.z),
            near,
            far,
            &|square| {
                let outside = crate::outdoors::is_outside(world, square);
                (!*outdoor_only || outside) && (!*indoor_only || !outside)
            },
        ) else {
            if let Some(dice) = held.take() {
                crate::singletons::put_rolls(world, dice);
            }
            return Applied::CannotAnswer;
        };
        squares.push(around);
    }
    if let Some(dice) = held.take() {
        crate::singletons::put_rolls(world, dice);
    }
    let mut wanted = Vec::new();
    for trait_name in traits {
        let Some(text) = crate::conditions::text_of(world, speaker, listener, trait_name) else {
            return Applied::CannotAnswer;
        };
        wanted.push(text);
    }
    let remembered = unique_id
        .as_ref()
        .and_then(|given| crate::conditions::text_of(world, speaker, listener, given));
    for square in squares {
        let made = crate::creature::spawn_creature(world, &name, square);
        crate::npcs::make_npc(world, made, remembered.as_deref().unwrap_or(&name));
        for trait_name in &wanted {
            crate::traits::add_trait(world, made, trait_name);
        }
        // Reference: `f_spawn_npc` gives the spawn a summon time when the data states a `lifespan`, the same call the monster spawn makes.
        if let Some((lower, upper)) = lifespan
            && let Some(turns) = draw_between(world, *lower, *upper)
            && let Some(calendar) = crate::calendar::of(world)
        {
            crate::monster_spawning::summon_until(world, made, calendar.turn + turns);
        }
    }
    Applied::Done
}

fn spawn_monsters(
    world: &mut World,
    speaker: Entity,
    listener: Option<Entity>,
    effect: &Effect,
) -> Applied {
    let Effect::SpawnMonster {
        monster,
        target,
        group,
        target_range,
        real_count,
        hallucination_count,
        min_radius,
        max_radius,
        lifespan,
        outdoor_only,
        indoor_only,
        spawn_message,
        spawn_message_plural,
        friendly,
    } = effect
    else {
        return Applied::CannotAnswer;
    };
    // Reference: `f_spawn_monster` (`npctalk.cpp:7405-7408`): a `target_var` centres the spawn on the square it holds rather than on the speaker.
    let centred = match target {
        Some(target) => crate::dialogue_values::position_of(world, speaker, listener, target),
        None => None,
    };
    let Some(at) = centred
        .map(|(x, y, z)| crate::Position { x, y, z })
        .or_else(|| world.get::<crate::Position>(speaker).copied())
    else {
        if std::env::var("CDDA_SHOW_SPAWN").is_ok() {
            println!("spawn refused: no position");
        }
        return Applied::CannotAnswer;
    };
    let Some(named) = crate::conditions::text_of(world, speaker, listener, monster) else {
        if std::env::var("CDDA_SHOW_SPAWN").is_ok() {
            println!("spawn refused: no name");
        }
        return Applied::CannotAnswer;
    };
    let Some(name) = pick_type(world, speaker, &named, *group, *target_range) else {
        return Applied::CannotAnswer;
    };
    let Some(kind) = crate::monster_catalog::type_of(world, &name).cloned() else {
        if std::env::var("CDDA_SHOW_SPAWN").is_ok() {
            println!("spawn refused: unknown type {name}");
        }
        return Applied::CannotAnswer;
    };
    let real = draw_count(world, speaker, listener, real_count);
    let imagined = draw_count(world, speaker, listener, hallucination_count);
    let (Some(real), Some(imagined)) = (real, imagined) else {
        if std::env::var("CDDA_SHOW_SPAWN").is_ok() {
            println!("spawn refused: no count");
        }
        return Applied::CannotAnswer;
    };
    let mut held = crate::singletons::take_rolls(world);
    let mut squares = Vec::new();
    for _ in 0..(real + imagined) {
        let Some(around) = draw_near_with_where(
            world,
            held.as_mut(),
            (at.x, at.y, at.z),
            *min_radius,
            *max_radius,
            &|square| {
                let outside = crate::outdoors::is_outside(world, square);
                (!*outdoor_only || outside) && (!*indoor_only || !outside)
            },
        ) else {
            if let Some(dice) = held.take() {
                crate::singletons::put_rolls(world, dice);
            }
            return Applied::CannotAnswer;
        };
        squares.push(around);
    }
    if let Some(dice) = held.take() {
        crate::singletons::put_rolls(world, dice);
    }
    // Reference: the message is shown once, plural when more than one appeared, and only when the spawns were seen. Read before `real` becomes an index.
    let many = real.saturating_add(imagined) > 1;
    let real = usize::try_from(real).unwrap_or(0);
    let line = if many {
        spawn_message_plural.as_ref().or(spawn_message.as_ref())
    } else {
        spawn_message.as_ref()
    };
    if let Some(source) = line
        && let Some(text) = crate::conditions::text_of(world, speaker, listener, source)
    {
        crate::messages::push(world, &text);
    }
    for (made, square) in squares.into_iter().enumerate() {
        #[allow(clippy::cast_possible_truncation)]
        let kind_number = crate::monster_registry::id_of(world, &name);
        let made_entity = crate::creature::spawn_monster_of_type(
            world,
            kind_number.unwrap_or(crate::MonsterKindId(0)),
            &kind,
            square,
            100,
        );
        dress_spawn(world, made_entity, made >= real, *lifespan, *friendly);
    }
    Applied::Done
}

/// What a spawn marked friendly by data is marked with: [`crate::friendly::FOR_EVER`], the reference's `monster::friendly = -1` - "friendly for good" rather than for a
/// count of turns.
const FOREVER_FRIENDLY: i32 = crate::friendly::FOR_EVER;

/// Gives one spawn the marks the data asks for: imagined, friendly, and how long it stays.
///
/// Reference: `f_spawn_monster` (`npctalk.cpp:7371-7450`) marks hallucinations, sets `monster::friendly` when the data asks, and calls
/// `spawned->set_summon_time( lifespan )` - which both real and imagined spawns take.
fn dress_spawn(
    world: &mut World,
    spawn: Entity,
    imagined: bool,
    lifespan: Option<(i64, i64)>,
    friendly: bool,
) {
    if imagined {
        world
            .entity_mut(spawn)
            .insert(crate::monster_spawning::Hallucination);
    }
    // The port has no attitudes yet, so friendliness is a **count** on the creature - one component, no second source of truth - and the attitude rules can read it
    // when they land. A spawn the data marks friendly starts with a count that never runs out, which is the reference's `friendly = -1` ("friendly for good").
    if friendly {
        crate::friendly::make_friendly(world, spawn, FOREVER_FRIENDLY);
    }
    if let Some((lower, upper)) = lifespan
        && let Some(turns) = draw_between(world, lower, upper)
        && let Some(calendar) = crate::calendar::of(world)
    {
        let until = calendar.turn + turns;
        crate::monster_spawning::summon_until(world, spawn, until);
    }
}

/// Draws a number between two ends, both included.
fn draw_between(world: &mut World, lower: i64, upper: i64) -> Option<i64> {
    if lower == upper {
        return Some(lower);
    }
    let (Ok(lower), Ok(upper)) = (i32::try_from(lower), i32::try_from(upper)) else {
        return None;
    };
    let mut held = crate::singletons::take_rolls(world);
    let drawn = held.as_mut().map(|rolls| rolls.rng(lower, upper));
    if let Some(dice) = held.take() {
        crate::singletons::put_rolls(world, dice);
    }
    drawn.map(i64::from)
}

/// Draws how many of something the data asked for.
fn draw_count(
    world: &mut World,
    speaker: Entity,
    listener: Option<Entity>,
    count: &cdda_content::dialogue::CountRange,
) -> Option<i32> {
    match count {
        cdda_content::dialogue::CountRange::One(one) => Some(*one),
        // Reference: `get_dbl_or_var` evaluates a `math` count where it is used, so the world it reads is the world at spawn time.
        cdda_content::dialogue::CountRange::Expression(expression) => {
            let drawn = crate::conditions::value(world, speaker, listener, expression)?;
            #[allow(clippy::cast_possible_truncation)]
            let drawn = drawn.round() as i32;
            Some(drawn)
        }
        cdda_content::dialogue::CountRange::Between(lower, upper) => {
            let mut dice = crate::singletons::take_rolls(world);
            let drawn = dice.as_mut().map(|rolls| rolls.rng(*lower, *upper));
            if let Some(dice) = dice.take() {
                crate::singletons::put_rolls(world, dice);
            }
            drawn
        }
    }
}

/// Draws a square within `max_radius` of `around` and at least `min_radius` away, up to 25 times, as the reference does.
fn draw_near(
    world: &mut World,
    around: (i32, i32, i32),
    min_radius: i32,
    max_radius: i32,
) -> Option<(i32, i32, i32)> {
    let mut held = crate::singletons::take_rolls(world);
    let drawn = draw_near_with(world, held.as_mut(), around, min_radius, max_radius);
    if let Some(dice) = held.take() {
        crate::singletons::put_rolls(world, dice);
    }
    drawn
}

/// The draw itself, with the dice handed in, so a caller making many squares keeps one stream.
fn draw_near_with(
    world: &World,
    dice: Option<&mut cdda_core_types::rng::Rolls>,
    around: (i32, i32, i32),
    min_radius: i32,
    max_radius: i32,
) -> Option<(i32, i32, i32)> {
    draw_near_with_where(world, dice, around, min_radius, max_radius, &|_| true)
}

/// The draw, with a rule about which squares are acceptable.
fn draw_near_with_where(
    world: &World,
    dice: Option<&mut cdda_core_types::rng::Rolls>,
    around: (i32, i32, i32),
    min_radius: i32,
    max_radius: i32,
    allow: &dyn Fn((i32, i32, i32)) -> bool,
) -> Option<(i32, i32, i32)> {
    let dice = dice?;
    for _ in 0..25 {
        let x = around.0 + dice.rng(-max_radius, max_radius);
        let y = around.1 + dice.rng(-max_radius, max_radius);
        let square = (x, y, around.2);
        if crate::square::terrain_at(world, square).is_none() {
            continue;
        }
        if (x - around.0).abs().max((y - around.1).abs()) < min_radius {
            continue;
        }
        if !allow(square) {
            continue;
        }
        return Some(square);
    }
    None
}

/// Sets a dialogue variable.
///
/// Reference: `f_add_var` (`npctalk.cpp:3589-3610`): `time: true` stores the current turn, `possible_values` draws one of its texts, and a plain `value`
/// stores what the data wrote. The draw is the world's dice, so the same seed gives the same choice.
fn add_var(
    world: &mut World,
    speaker: Entity,
    listener: Option<Entity>,
    effect: &Effect,
) -> Applied {
    let Effect::AddVar {
        name,
        scope,
        value,
        timed,
    } = effect
    else {
        return Applied::CannotAnswer;
    };
    let Some(holder) = crate::conditions::holder_for_scope(world, speaker, listener, scope) else {
        return Applied::CannotAnswer;
    };
    let stored = if *timed {
        let Some(calendar) = crate::calendar::of(world) else {
            return Applied::CannotAnswer;
        };
        crate::dialogue_values::Value::Text(calendar.turn.to_string())
    } else {
        match value {
            cdda_content::dialogue::VarValue::One(one) => {
                // **An unset variable contributes the empty string** (`read_var_value`'s default) rather than stopping the effect.
                crate::dialogue_values::Value::Text(
                    crate::conditions::text_of(world, speaker, listener, one).unwrap_or_default(),
                )
            }
            cdda_content::dialogue::VarValue::Many(choices) => {
                let mut dice = crate::singletons::take_rolls(world);
                let last = i32::try_from(choices.len().saturating_sub(1)).ok();
                let drawn = dice
                    .as_mut()
                    .zip(last)
                    .map(|(rolls, last)| usize::try_from(rolls.rng(0, last)).unwrap_or(0));
                if let Some(dice) = dice.take() {
                    crate::singletons::put_rolls(world, dice);
                }
                let Some(at) = drawn else {
                    return Applied::CannotAnswer;
                };
                let Some(choice) = choices.get(at) else {
                    return Applied::CannotAnswer;
                };
                let Some(text) = crate::conditions::text_of(world, speaker, listener, choice)
                else {
                    return Applied::CannotAnswer;
                };
                crate::dialogue_values::Value::Text(text)
            }
        }
    };
    crate::dialogue_values::set(world, holder, name, stored);
    Applied::Done
}

/// Forgets a dialogue variable.
///
/// Reference: `f_remove_var` (`npctalk.cpp:3656`): `u_lose_var` drops one of the speaker's, `npc_lose_var` one of the listener's, `global_lose_var` one of
/// the world's. Dropping one that was never there is not an error - the reference's `remove_value` says nothing about it - so it is `Done`.
fn lose_var(
    world: &mut World,
    speaker: Entity,
    listener: Option<Entity>,
    name: &str,
    scope: &str,
) -> Applied {
    let Some(holder) = crate::conditions::holder_for_scope(world, speaker, listener, scope) else {
        return Applied::CannotAnswer;
    };
    crate::dialogue_values::remove(world, holder, name);
    Applied::Done
}

/// Offers a menu of programs and runs the one that was picked.
///
/// Reference: `f_run_eoc_selector` (`npctalk.cpp:6385`): the player picks one of the programs, or walks away when `allow_cancel` says they may. The
/// port takes the answer from the queued [`crate::choices`] rather than drawing a menu - and **refuses** when nothing was queued, because a question the
/// game could not ask is not a player pressing escape.
fn run_selector(
    world: &mut World,
    speaker: Entity,
    listener: Option<Entity>,
    effect: &Effect,
) -> Applied {
    let Effect::RunEocSelector {
        options,
        allow_cancel,
        ..
    } = effect
    else {
        return Applied::CannotAnswer;
    };
    let Some(answer) = crate::choices::take(world) else {
        return Applied::CannotAnswer;
    };
    if answer == crate::choices::CANCELLED {
        return if *allow_cancel {
            Applied::Done
        } else {
            Applied::CannotAnswer
        };
    }
    let Ok(at) = usize::try_from(answer) else {
        return Applied::CannotAnswer;
    };
    let Some(choice) = options.get(at) else {
        return Applied::CannotAnswer;
    };
    match choice {
        cdda_content::dialogue::ProgramRef::Named { id, .. } => {
            crate::eoc_registry::run_program(world, speaker, listener, id, 0)
        }
        cdda_content::dialogue::ProgramRef::NamedByVariable { scope, name, .. } => {
            match crate::conditions::value_of_variable_text(world, speaker, listener, scope, name) {
                Some(id) => crate::eoc_registry::run_program(world, speaker, listener, &id, 0),
                None => Applied::CannotAnswer,
            }
        }
        cdda_content::dialogue::ProgramRef::Inline { program, .. } => {
            run_effects(world, speaker, listener, &program.effects)
        }
    }
}

/// Takes several effects away, which the data writes as an array of names.
///
/// Reference: `f_remove_effect` (`npctalk.cpp:8015`) with an array: each one is removed in turn, and one the creature does not have is not a failure.
fn run_remove_effects(
    world: &mut World,
    speaker: Entity,
    listener: Option<Entity>,
    effect: &Effect,
) -> Applied {
    let Effect::RemoveEffects {
        effects,
        listener: theirs,
    } = effect
    else {
        return Applied::CannotAnswer;
    };
    if actor(speaker, listener, *theirs).is_none() {
        return Applied::CannotAnswer;
    }
    for name in effects {
        let one = Effect::RemoveEffect {
            effect: cdda_content::dialogue::StringSource::Literal(name.clone()),
            listener: *theirs,
        };
        let _ = run_effect(world, speaker, listener, &one);
    }
    Applied::Done
}

/// How far an `om_terrain` search reaches, in overmap tiles.
///
/// Reference: `mission_util.cpp:429` reads `search_range` with a default of `OMAPX * 3`, and `map_scale_constants.h:37` says `OMAPX = 180` - so the
/// reference searches **540** tiles each way unless the data says otherwise. The first version of this constant said 60, which was a plausible number
/// nobody wrote down; the citation is why it is 540.
const SEARCH_RANGE: i32 = 540;

/// Applies an update mapgen where the speaker stands.
///
/// Reference: `f_mapgen_update` (`npctalk.cpp`): with no target named, the update is applied at the speaker's own overmap tile.
fn run_mapgen_update(
    world: &mut World,
    speaker: Entity,
    listener: Option<Entity>,
    effect: &Effect,
) -> Applied {
    let Effect::MapgenUpdate {
        updates,
        target,
        om_terrain,
        time_in_future,
    } = effect
    else {
        return Applied::CannotAnswer;
    };
    let mut names = Vec::with_capacity(updates.len());
    for update in updates {
        let Some(name) = crate::conditions::text_of(world, speaker, listener, update) else {
            return Applied::CannotAnswer;
        };
        names.push(name);
    }
    // Three ways to aim an update, in the reference's own precedence: a `target_var` names a square the dialogue already holds, an `om_terrain`
    // names a tile to search for, and with neither the update lands where the speaker stands.
    let here = world.get::<crate::Position>(speaker).copied();
    let placed = if let Some(target) = target {
        // **An unset variable reads as the origin**, not as nothing: `read_var_value` on a variable that was never set answers `tripoint_abs_ms()` - absolute square
        // (0, 0) (`math_parser_diag_value.cpp:62-70`, port round 238) - and `f_mapgen_update` projects that to overmap tile (0, 0) and applies the update there
        // (`npctalk.cpp:4843-4845`). Refusing would drop an update the reference performs.
        Some(
            crate::dialogue_values::position_of(world, speaker, listener, target)
                .unwrap_or((0, 0, 0)),
        )
    } else if let Some(wanted) = om_terrain {
        let Some(at) = here else {
            return Applied::CannotAnswer;
        };
        let Some(name) = crate::conditions::text_of(world, speaker, listener, wanted) else {
            return Applied::CannotAnswer;
        };
        let tile = crate::overmap::tile_of(at.x, at.y);
        // **A search that finds nothing leaves the target at the origin**, which is where `get_om_terrain_pos` starts it (`mission_util.cpp:321-334`): the
        // update then lands where the speaker stands, exactly as an aim with no `om_terrain` would.
        crate::overmap::nearest_with_terrain(world, tile, &name, SEARCH_RANGE, 0).map_or(
            Some((at.x, at.y, at.z)),
            |(x, y)| {
                Some((
                    x * crate::overmap::SQUARES_PER_OMT,
                    y * crate::overmap::SQUARES_PER_OMT,
                    at.z,
                ))
            },
        )
    } else {
        here.map(|at| (at.x, at.y, at.z))
    };
    let Some(place) = placed else {
        return Applied::CannotAnswer;
    };
    // **A positive `time_in_future` schedules instead of applying** (`npctalk.cpp:4853-4868`): the update lands when the event comes due, through the same apply rule,
    // and an event that is never processed is the named gap below the schedule rather than a reason to run the update early.
    let after = roll_duration(world, speaker, listener, time_in_future).unwrap_or(0);
    if after > 0 {
        for name in &names {
            crate::timed_events::schedule_carrying_at(
                world,
                "UPDATE_MAPGEN",
                after.saturating_add(1),
                name,
                Some(place),
            );
        }
        return Applied::Done;
    }
    for name in &names {
        if crate::mapgen_update::apply(world, name, place).is_none() {
            return Applied::CannotAnswer;
        }
    }
    // An update may place fields as well as terrain, and an update that does only one is still an update.
    let mut placed_fields = false;
    for name in &names {
        if crate::mapgen_update::place_fields(world, name, place).is_some() {
            placed_fields = true;
        }
    }
    if placed_fields {
        Applied::Done
    } else {
        Applied::CannotAnswer
    }
}

/// Makes a sound where its maker stands, saying its words or a line from its snippet category.
///
/// Reference: `f_make_sound` (`npctalk.cpp:6131-6200`): the sound is placed at the maker's own square with the stated volume, and the words are
/// shown when it is speech, an alarm or an order. A snippet sound picks its line by weight, and `same_snippet` remembers the pick.
fn make_sound_effect(
    world: &mut World,
    speaker: Entity,
    listener: Option<Entity>,
    effect: &Effect,
) -> Applied {
    let Effect::MakeSound {
        words,
        snippet,
        same_snippet,
        volume,
        kind,
        listener: about_listener,
    } = effect
    else {
        return Applied::CannotAnswer;
    };
    let Some(creature) = actor(speaker, listener, *about_listener) else {
        return Applied::CannotAnswer;
    };
    let Some(position) = world.get::<crate::Position>(creature).copied() else {
        return Applied::CannotAnswer;
    };
    let Some(loudness) = draw(world, speaker, listener, volume) else {
        return Applied::CannotAnswer;
    };
    let place = (position.x, position.y, position.z);
    if crate::sounds::make_sound(world, place, as_whole(loudness)).is_none() {
        return Applied::CannotAnswer;
    }
    if matches!(kind, cdda_content::dialogue::SoundKind::Background) {
        return Applied::Done;
    }
    let said = sound_words(
        world,
        speaker,
        listener,
        creature,
        words.as_ref(),
        snippet.as_ref(),
        *same_snippet,
    );
    if let Some(said) = said {
        crate::messages::push(world, &said);
    }
    Applied::Done
}

/// The words a sound says: its own, or a line picked from its snippet category.
fn sound_words(
    world: &mut World,
    speaker: Entity,
    listener: Option<Entity>,
    maker: Entity,
    words: Option<&cdda_content::dialogue::StringSource>,
    snippet: Option<&cdda_content::dialogue::StringSource>,
    same_snippet: bool,
) -> Option<String> {
    if let Some(words) = words {
        return crate::conditions::text_of(world, speaker, listener, words);
    }
    let name = crate::conditions::text_of(world, speaker, listener, snippet?)?;
    let remembered = format!("{name}_snippet_id");
    if same_snippet
        && let Some(known) = crate::dialogue_values::text_of(world, maker, &remembered)
        && let Some(line) = crate::snippets::category(world, &name)
            .into_iter()
            .find(|line| line.id == known)
    {
        return Some(line.text);
    }
    let mut dice = crate::singletons::take_rolls(world);
    let picked = crate::snippets::pick(world, &name, dice.as_mut());
    if let Some(dice) = dice.take() {
        crate::singletons::put_rolls(world, dice);
    }
    let line = picked?;
    if same_snippet {
        crate::dialogue_values::set(
            world,
            maker,
            &remembered,
            crate::dialogue_values::Value::Text(line.id.clone()),
        );
    }
    Some(line.text)
}

/// One morale point to add, with its numbers still to be computed.
struct MoraleRequest<'a> {
    kind: &'a cdda_content::dialogue::StringSource,
    bonus: &'a cdda_content::dialogue::Expression,
    max_bonus: &'a cdda_content::dialogue::Expression,
    duration: cdda_content::dialogue::EffectDuration,
    decay_start: cdda_content::dialogue::EffectDuration,
    capped: bool,
    about_listener: bool,
}

/// Adds a morale point, computing its numbers from the data.
///
/// Reference: `f_add_morale` (`npctalk.cpp:7177-7195`): the type may be a variable, and the bonus, cap and both times are evaluated at the moment
/// the point is added. A time written as a range is rolled, the same way an effect's duration is.
fn add_morale(
    world: &mut World,
    speaker: Entity,
    listener: Option<Entity>,
    request: &MoraleRequest<'_>,
) -> Applied {
    let Some(creature) = actor(speaker, listener, request.about_listener) else {
        return Applied::CannotAnswer;
    };
    let Some(kind) = crate::conditions::text_of(world, speaker, listener, request.kind) else {
        return Applied::CannotAnswer;
    };
    let mut numbers = Vec::new();
    for expression in [request.bonus, request.max_bonus] {
        let Some(value) = draw(world, speaker, listener, expression) else {
            return Applied::CannotAnswer;
        };
        numbers.push(as_whole(value));
    }
    let Some(bonus) = numbers.first().copied() else {
        return Applied::CannotAnswer;
    };
    let Some(max_bonus) = numbers.get(1).copied() else {
        return Applied::CannotAnswer;
    };
    let mut times = Vec::new();
    for time in [&request.duration, &request.decay_start] {
        let Some(turns) = roll_duration(world, speaker, listener, time) else {
            return Applied::CannotAnswer;
        };
        times.push(turns);
    }
    let (duration, decay_start) = (
        times.first().copied().unwrap_or(0),
        times.get(1).copied().unwrap_or(0),
    );
    crate::morale::add(
        world,
        creature,
        crate::morale::MoralePoint {
            kind,
            bonus,
            max_bonus,
            duration,
            decay_start,
            age: 0,
            capped: request.capped,
        },
    );
    Applied::Done
}

/// A duration the data states, as turns, rolling a range and working out an expression.
///
/// Reference: `get_duration_or_var` (`npctalk.cpp:3426-3450`) answers a fixed number, a range to roll, or a **variable** - which the port writes as
/// `Computed` and works out here, because a variable's value is whatever the dialogue holds when the effect is applied.
fn roll_duration(
    world: &mut World,
    speaker: Entity,
    listener: Option<Entity>,
    duration: &cdda_content::dialogue::EffectDuration,
) -> Option<i64> {
    match duration {
        cdda_content::dialogue::EffectDuration::Permanent => Some(0),
        cdda_content::dialogue::EffectDuration::Turns(turns) => *turns,
        cdda_content::dialogue::EffectDuration::Range(shortest, longest) => {
            let mut dice = crate::singletons::take_rolls(world);
            let rolled = dice
                .as_mut()
                .map(|dice| i64::from(dice.rng(as_turns(*shortest), as_turns(*longest))));
            if let Some(dice) = dice.take() {
                crate::singletons::put_rolls(world, dice);
            }
            rolled.or(Some(*shortest))
        }
        // Reference: the two ends are evaluated, then a draw is made between them - the storm's length depends on the storm's own strength.
        cdda_content::dialogue::EffectDuration::RangeComputed(shortest, longest) => {
            let shortest = draw(world, speaker, listener, shortest)?;
            let longest = draw(world, speaker, listener, longest)?;
            let shortest = i64::from(as_whole(shortest));
            let longest = i64::from(as_whole(longest)).max(shortest);
            let mut dice = crate::singletons::take_rolls(world);
            let rolled = dice
                .as_mut()
                .map(|dice| i64::from(dice.rng(as_turns(shortest), as_turns(longest))));
            if let Some(dice) = dice.take() {
                crate::singletons::put_rolls(world, dice);
            }
            rolled.or(Some(shortest))
        }
        // Reference: the variable holds a **duration in text**, and its absence means the data's own fallback (`get_duration_or_var`, `npctalk.cpp:3426-3450`).
        cdda_content::dialogue::EffectDuration::Named {
            scope,
            name,
            fallback,
        } => {
            let holder = crate::conditions::holder_for_scope(world, speaker, listener, scope)?;
            let held = crate::dialogue_values::text_of(world, holder, name);
            Some(match held {
                Some(text) => cdda_content::constructions::parse_duration(&text),
                None => *fallback,
            })
        }
        cdda_content::dialogue::EffectDuration::Computed(expression) => {
            let value = draw(world, speaker, listener, expression)?;
            Some(i64::from(as_whole(value)))
        }
    }
}

/// Gives a creature an effect, with the duration and intensity the data states.
///
/// Reference: `f_add_effect` (`npctalk.cpp:3426-3450`): the duration may be a range, which is rolled, and the intensity is an expression
/// evaluated at the moment it is applied.
fn add_effect(
    world: &mut World,
    speaker: Entity,
    listener: Option<Entity>,
    effect: &str,
    duration: &cdda_content::dialogue::EffectDuration,
    intensity: Option<&cdda_content::dialogue::Expression>,
    about_listener: bool,
) -> Applied {
    let Some(creature) = actor(speaker, listener, about_listener) else {
        return Applied::CannotAnswer;
    };
    let intensity = match intensity {
        Some(expression) => {
            let Some(value) = draw(world, speaker, listener, expression) else {
                return Applied::CannotAnswer;
            };
            as_whole(value)
        }
        None => 0,
    };
    // A range is rolled and an expression worked out; both go through the same reader, so `add_effect` states that once rather than twice.
    let (duration, permanent) = match duration {
        cdda_content::dialogue::EffectDuration::Permanent => (0, true),
        other => match roll_duration(world, speaker, listener, other) {
            Some(turns) => (turns, false),
            None => return Applied::CannotAnswer,
        },
    };
    crate::applied_effects::add(
        world,
        creature,
        crate::effects::ActiveEffect {
            id: effect.to_owned(),
            intensity,
            duration,
            permanent,
        },
    );
    Applied::Done
}

/// An expression's value as a whole number, which is what an intensity is.
#[allow(clippy::cast_possible_truncation)]
fn as_whole(value: f64) -> i32 {
    value.round() as i32
}

/// An expression's value, with the store's dice taken out for the evaluation and put back after.
fn draw(
    world: &mut World,
    speaker: Entity,
    listener: Option<Entity>,
    expression: &cdda_content::dialogue::Expression,
) -> Option<f64> {
    let mut dice = crate::singletons::take_rolls(world);
    let value =
        crate::conditions::value_drawing(world, speaker, listener, expression, true, dice.as_mut());
    if let Some(dice) = dice.take() {
        crate::singletons::put_rolls(world, dice);
    }
    value
}

/// A number of turns as the generator's integer, which is what every roll takes.
#[allow(clippy::cast_possible_truncation)]
fn as_turns(turns: i64) -> i32 {
    turns as i32
}

/// Stores the nearest overmap tile carrying a terrain, as a square.
///
/// Reference: `f_location_variable` with `target_params` (`npctalk.cpp:4205-4210`, `4261-4300`): the search starts at the talker's own overmap tile, looks up to
/// `search_range` tiles out - `OMAPX * 3`, 540, when the data does not say (`mission_util.cpp:429`) - and stores the **corner square** of the tile it finds. A least
/// distance is honoured because the reference's search does; nothing found is a refusal, because the data asked for a place and there is none.
fn store_overmap_location(
    world: &mut World,
    speaker: Entity,
    listener: Option<Entity>,
    effect: &Effect,
) -> Applied {
    let Effect::LocationVariableFromOvermap {
        scope,
        name,
        terrain,
        min_distance,
        z,
        listener: theirs,
    } = effect
    else {
        return Applied::CannotAnswer;
    };
    let Some(actor) = actor(speaker, listener, *theirs) else {
        return Applied::CannotAnswer;
    };
    let Some(here) = world.get::<crate::Position>(actor).copied() else {
        return Applied::CannotAnswer;
    };
    let Some(wanted) = crate::conditions::text_of(world, speaker, listener, terrain) else {
        return Applied::CannotAnswer;
    };
    let least = match min_distance {
        None => 0,
        Some(given) => match crate::conditions::value(world, speaker, listener, given) {
            Some(drawn) => as_whole(drawn).max(0),
            None => return Applied::CannotAnswer,
        },
    };
    let level = match z {
        None => here.z,
        Some(given) => match crate::conditions::value(world, speaker, listener, given) {
            Some(drawn) => as_whole(drawn),
            None => return Applied::CannotAnswer,
        },
    };
    let origin = crate::overmap::tile_of(here.x, here.y);
    let Some((x, y)) =
        crate::overmap::nearest_with_terrain(world, origin, &wanted, SEARCH_RANGE, least)
    else {
        return Applied::CannotAnswer;
    };
    let Some(holder) = crate::conditions::holder_for_scope(world, speaker, listener, scope) else {
        return Applied::CannotAnswer;
    };
    crate::dialogue_values::set(
        world,
        holder,
        name,
        crate::dialogue_values::Value::Position {
            x: x * crate::overmap::SQUARES_PER_OMT,
            y: y * crate::overmap::SQUARES_PER_OMT,
            z: level,
        },
    );
    Applied::Done
}

/// Stores the actor's square in a dialogue variable.
///
/// Reference: `f_location_variable` (`npctalk.cpp:4194`), plain form: the value is `d.const_actor( is_npc )->pos_abs()`, and the scope names where
/// it goes the same way `math` names its target.
fn store_location(
    world: &mut World,
    speaker: Entity,
    listener: Option<Entity>,
    effect: &Effect,
) -> Applied {
    let Effect::LocationVariable {
        scope,
        name,
        listener: about_listener,
    } = effect
    else {
        return Applied::CannotAnswer;
    };
    let Some(creature) = actor(speaker, listener, *about_listener) else {
        return Applied::CannotAnswer;
    };
    let Some(position) = world.get::<crate::Position>(creature).copied() else {
        return Applied::CannotAnswer;
    };
    let Some(holder) = crate::conditions::holder_for_scope(world, speaker, listener, scope) else {
        return Applied::CannotAnswer;
    };
    crate::dialogue_values::set(
        world,
        holder,
        name,
        crate::dialogue_values::Value::Position {
            x: position.x,
            y: position.y,
            z: position.z,
        },
    );
    Applied::Done
}

/// Runs the **last** case a switch's value reaches.
///
/// Reference: `f_switch` (`npctalk.cpp:6998-7020`): every case is tried in the data's order and each one the value reaches replaces the chosen
/// one, so the last match runs - not the first, and not the nearest.
fn run_switch(
    world: &mut World,
    speaker: Entity,
    listener: Option<Entity>,
    on: &cdda_content::dialogue::Expression,
    cases: &[(cdda_content::dialogue::Expression, Vec<Effect>)],
) -> Applied {
    let drawn = |world: &mut World, expression: &cdda_content::dialogue::Expression| {
        let mut dice = crate::singletons::take_rolls(world);
        let value = crate::conditions::value_drawing(
            world,
            speaker,
            listener,
            expression,
            true,
            dice.as_mut(),
        );
        if let Some(dice) = dice.take() {
            crate::singletons::put_rolls(world, dice);
        }
        value
    };
    let Some(switched) = drawn(world, on) else {
        return Applied::CannotAnswer;
    };
    // Reference: every case the value reaches replaces the chosen one, so the **last** match runs.
    let mut chosen: Option<&Vec<Effect>> = None;
    for (case, effects) in cases {
        let Some(reached) = drawn(world, case) else {
            return Applied::CannotAnswer;
        };
        if switched >= reached {
            chosen = Some(effects);
        }
    }
    match chosen {
        Some(effects) => run_effects(world, speaker, listener, effects),
        None => Applied::Done,
    }
}

/// Runs one of an `if`'s two lists, whichever its condition picks.
///
/// Reference: `f_if` (`npctalk.cpp:6977-6993`): `if` is a condition, `then` runs when it holds, `else` when it does not, and an absent `else` runs nothing.
fn run_branch(
    world: &mut World,
    speaker: Entity,
    listener: Option<Entity>,
    effect: &Effect,
) -> Applied {
    let Effect::If {
        condition,
        then,
        otherwise,
    } = effect
    else {
        return Applied::CannotAnswer;
    };
    // A question is the one condition the game **asks** rather than looks up, and a headless port asks it by taking the next queued answer - the same store a menu's
    // answer comes from. `choices::CANCELLED` and an empty queue both mean "the player was not asked", which the data's `default` answers.
    let answered = match condition.as_ref() {
        // A yes/no menu's indices are **yes = 0** and **no = 1**, and `CANCELLED` is walking away - so the answer is yes when the queued index is zero, and
        // anything else (a no, a cancel, an empty queue) answers the data's `default`.
        cdda_content::dialogue::Condition::Query { default } => {
            Some(match crate::choices::take(world) {
                Some(answer) => answer == 0,
                _ => *default,
            })
        }
        _ => None,
    };
    let taken = answered
        .unwrap_or_else(|| crate::conditions::holds_in_store(world, speaker, listener, condition));
    let branch = if taken { then } else { otherwise };
    run_effects(world, speaker, listener, branch)
}

/// Teaches or unteaches one recipe, on the actor the member names.
///
/// Reference: `f_learn_recipe` / `f_forget_recipe` (`npctalk.cpp:5250-5266`): `u_learn_recipe` teaches the speaker and `npc_learn_recipe` the listener, and the
/// id may be a variable, which is why it is evaluated before it is used.
fn run_learn_recipe(
    world: &mut World,
    speaker: Entity,
    listener: Option<Entity>,
    recipe: &cdda_content::dialogue::StringSource,
    forget: bool,
) -> Applied {
    let Some(named) = crate::conditions::text_of(world, speaker, listener, recipe) else {
        return Applied::CannotAnswer;
    };
    let actor = if matches!(
        recipe,
        cdda_content::dialogue::StringSource::Variable { .. }
    ) {
        listener.unwrap_or(speaker)
    } else {
        speaker
    };
    if forget {
        crate::recipes_known::forget(world, actor, &named);
    } else {
        // The dialogue effect names a recipe id and carries no prototype, so it cannot know the flag: `never_learn` is `false` here, which is the
        // reference's behaviour for an effect that was written against a learnable recipe. Named, not invented - the prototype lookup is the caller's gap.
        crate::recipes_known::learn(world, actor, &named, false, false);
    }
    Applied::Done
}

/// Gives a creature an effect whose name may be a variable.
///
/// Reference: `f_add_effect` (`npctalk.cpp:3426-3450`): the name is a `str_or_var`, resolved where the effect is applied, and so is the duration.
fn run_gained_effect(
    world: &mut World,
    speaker: Entity,
    listener: Option<Entity>,
    effect: &Effect,
) -> Applied {
    let Effect::AddEffect {
        effect: named,
        duration,
        intensity,
        listener: theirs,
    } = effect
    else {
        return Applied::CannotAnswer;
    };
    let Some(text) = crate::conditions::text_of(world, speaker, listener, named) else {
        return Applied::CannotAnswer;
    };
    add_effect(
        world,
        speaker,
        listener,
        &text,
        duration,
        intensity.as_ref(),
        *theirs,
    )
}

/// Takes an effect off a creature whose name may be a variable.
///
/// Reference: `f_remove_effect` (`npctalk.cpp:8015`): the name is a `str_or_var`, and removing one the creature does not have is not a failure.
fn run_lost_effect(
    world: &mut World,
    speaker: Entity,
    listener: Option<Entity>,
    effect: &Effect,
) -> Applied {
    let Effect::RemoveEffect {
        effect: named,
        listener: theirs,
    } = effect
    else {
        return Applied::CannotAnswer;
    };
    let Some(creature) = actor(speaker, listener, *theirs) else {
        return Applied::CannotAnswer;
    };
    let Some(text) = crate::conditions::text_of(world, speaker, listener, named) else {
        return Applied::CannotAnswer;
    };
    crate::applied_effects::remove(world, creature, &text);
    Applied::Done
}

/// The effects that write the store directly: spawns, consumption, variables, timers, and the ones that change nothing at all.
///
/// These share no rule with the rest of the dispatch, and keeping them together is what stops the dispatch table from growing an arm for every store write in the
/// game - the same reason [`answer_from_table`](crate::conditions) was split.
fn run_store_effect(
    world: &mut World,
    speaker: Entity,
    listener: Option<Entity>,
    effect: &Effect,
) -> Option<Applied> {
    Some(match effect {
        Effect::SpawnNpc { .. } => spawn_npcs(world, speaker, listener, effect),
        Effect::SpawnItem { .. } => spawn_items(world, speaker, listener, effect),
        Effect::ConsumeItem { .. } => consume_items(world, speaker, listener, effect),
        Effect::SetStringVar { .. } => set_string_var(world, speaker, listener, effect),
        Effect::CopyVar { .. } => copy_var(world, speaker, listener, effect),
        Effect::AlterTimedEvents { .. } => alter_timed_events(world, speaker, listener, effect),
        Effect::AddEffect { .. } => run_gained_effect(world, speaker, listener, effect),
        Effect::RemoveEffect { .. } => run_lost_effect(world, speaker, listener, effect),
        Effect::SetCondition { .. } => set_condition(world, speaker, listener, effect),
        Effect::FinishMission { .. } => finish_mission(world, speaker, listener, effect),
        Effect::RemoveActiveMission { .. } => {
            remove_active_mission(world, speaker, listener, effect)
        }
        Effect::TurnCost { .. } => turn_cost(world, speaker, listener, effect),
        Effect::AddWet { .. } => add_wet(world, speaker, listener, effect),
        Effect::PlaceOverride { .. } => place_override(world, speaker, listener, effect),
        Effect::RemoveItemWith { .. } => remove_item_with(world, speaker, listener, effect),
        Effect::LoseMorale { .. } => lose_morale(world, speaker, listener, effect),
        Effect::SetTalker { .. } => set_talker(world, speaker, listener, effect),
        Effect::AssignActivity { .. } => assign_activity(world, speaker, listener, effect),
        Effect::CastSpell { .. } => cast_spell(world, speaker, listener, effect),
        Effect::TerrainAround { .. } => store_terrain_around(world, speaker, listener, effect),
        Effect::FurnitureAround { .. } => store_furniture_around(world, speaker, listener, effect),
        Effect::SetField { .. } => set_field(world, speaker, listener, effect),
        Effect::CustomLightLevel { .. } => custom_light_level(world, speaker, listener, effect),
        Effect::TransformRadius { .. } => transform_radius(world, speaker, listener, effect),
        Effect::TransformLine { .. } => transform_line(world, speaker, listener, effect),
        // Reference: the legacy string effects are whole interactions - trade, training, camps, bionics - and the port has written none of them, so the name is known and the
        // action is not: a refusal where it runs, exactly as `mapgen_update` refuses.
        // **The legacy string effects the port has written**, by the name the data spells. A name it has not written still refuses, which is what the census reports.
        Effect::LegacyStringEffect { name } if name == "next_weather" => {
            // Reference: `f_next_weather` (`npctalk.cpp:5790-5795`) - the sky re-rolls now.
            let _ = crate::weather::make_due(world);
            Applied::Done
        }

        Effect::LegacyStringEffect { name } => {
            run_legacy_string_effect(world, speaker, listener, name)
        }
        Effect::GiveAchievement { .. } => give_achievement(world, speaker, listener, effect),
        Effect::TriggerEvent { kind, args } => trigger_event(world, speaker, listener, kind, args),
        Effect::SellItem { .. } => sell_item(world, speaker, listener, effect),
        Effect::MapRunItemEocs { .. } => map_run_item_eocs(world, speaker, listener, effect),
        Effect::RunInvEocs { .. } => run_inv_eocs(world, speaker, listener, effect),
        Effect::LocationVariableAdjust { .. } => adjust_location(world, speaker, listener, effect),
        Effect::RevertLocation { .. } => revert_location(world, speaker, listener, effect),
        Effect::ForEach { .. } => run_foreach(world, speaker, listener, effect),
        Effect::TransformItem { .. } => transform_item(world, listener, effect),
        Effect::MonsterAround { .. } => store_monster_around(world, speaker, effect),
        Effect::Mutation { .. } => mutate(world, speaker, listener, effect),
        Effect::OfferMission { .. } => offer_mission(world, speaker, listener, effect),
        Effect::RunEocLater { .. } => run_eoc_later(world, speaker, listener, effect),
        Effect::Later { .. } => run_later(world, speaker, listener, effect),
        // A sound event is presentation: the port carries it and has no ear to play to. The reference changes nothing about the world when it plays one, so `Done` is
        // the honest answer - and it shares that answer with the other presentation effects below.

        // Audio changes no game state, and the port has no audio: Done, not refused - nothing is missing from the simulation, only from the sound card. A
        // comment-only entry does nothing, which the reference's loader also does with it.
        Effect::SoundEvent { .. } | Effect::PlaySoundEffect { .. } | Effect::Nothing => {
            Applied::Done
        }
        _ => return None,
    })
}

/// Runs one effect entry.
#[allow(clippy::too_many_lines)]
pub fn run_effect(
    world: &mut World,
    speaker: Entity,
    listener: Option<Entity>,
    effect: &Effect,
) -> Applied {
    if let Some(done) = run_store_effect(world, speaker, listener, effect) {
        return done;
    }
    match effect {
        Effect::Math(assignments) => run_math(world, speaker, listener, assignments),
        Effect::LearnRecipe { recipe, forget } => {
            run_learn_recipe(world, speaker, listener, recipe, *forget)
        }
        Effect::Message { .. } => say_message(world, speaker, listener, effect),
        Effect::Snippet {
            category,
            same,
            listener: about_listener,
        } => say_snippet(world, speaker, listener, category, *same, *about_listener),
        Effect::AddMorale {
            kind,
            bonus,
            max_bonus,
            duration,
            decay_start,
            capped,
            listener: about_listener,
        } => add_morale(
            world,
            speaker,
            listener,
            &MoraleRequest {
                kind,
                bonus,
                max_bonus,
                duration: duration.clone(),
                decay_start: decay_start.clone(),
                capped: *capped,
                about_listener: *about_listener,
            },
        ),
        Effect::SpawnMonster { .. } => spawn_monsters(world, speaker, listener, effect),
        Effect::Teleport { .. } => move_creature(world, speaker, listener, effect),
        Effect::AssignMission { .. } => give_mission(world, speaker, listener, effect),
        Effect::AddVar { .. } => add_var(world, speaker, listener, effect),
        Effect::LoseVar { name, scope } => lose_var(world, speaker, listener, name, scope),
        Effect::LocationVariableSearch { .. } => search_location(world, speaker, listener, effect),
        Effect::LocationVariableRadius { .. } => nearby_location(world, speaker, effect),
        Effect::RunEocSelector { .. } => run_selector(world, speaker, listener, effect),
        Effect::RemoveEffects { .. } => run_remove_effects(world, speaker, listener, effect),
        Effect::MapgenUpdate { .. } => run_mapgen_update(world, speaker, listener, effect),
        Effect::MakeSound { .. } => make_sound_effect(world, speaker, listener, effect),
        Effect::OpenDialogue {
            topic: _,
            then,
            otherwise,
        } => {
            // Reference: `f_open_dialogue` (`npctalk.cpp:5476-5502`) - only the avatar can open a conversation, and the branch says whether
            // it did. The conversation itself is not opened: there is no dialogue interface to open.
            let branch = if crate::missions::avatar(world) == Some(speaker) {
                then
            } else {
                otherwise
            };
            for program in branch {
                if crate::eoc_registry::run_ref(world, speaker, listener, program, 1)
                    == Applied::CannotAnswer
                {
                    return Applied::CannotAnswer;
                }
            }
            Applied::Done
        }
        Effect::WeightedList(weighted) => run_weighted_list(world, speaker, listener, weighted, 0),
        Effect::LocationVariable { .. } => store_location(world, speaker, listener, effect),
        Effect::LocationVariableFromOvermap { .. } => {
            store_overmap_location(world, speaker, listener, effect)
        }
        Effect::Switch { on, cases } => run_switch(world, speaker, listener, on, cases),
        Effect::If { .. } => run_branch(world, speaker, listener, effect),
        Effect::LearnTrait {
            trait_name,
            variant,
            listener: about_listener,
        } => {
            let Some(creature) = actor(speaker, listener, *about_listener) else {
                return Applied::CannotAnswer;
            };
            // Reference: the trait is a `str_or_var`, read when the effect runs, so an earlier effect can have decided which one it is.
            let Some(named) = crate::conditions::text_of(world, speaker, listener, trait_name)
            else {
                return Applied::CannotAnswer;
            };
            crate::traits::add_trait(world, creature, &named);
            // Reference: `set_mutation( trait, variant )` keeps the cosmetic choice beside the trait; nothing in the rules reads it yet.
            if let Some(chosen) = variant {
                let Some(name) = crate::conditions::text_of(world, speaker, listener, chosen)
                else {
                    return Applied::CannotAnswer;
                };
                crate::traits::set_variant(world, creature, &named, &name);
            }
            Applied::Done
        }
        Effect::Emit {
            emit,
            chance_mult,
            target,
            listener: about_listener,
        } => {
            let Some(creature) = actor(speaker, listener, *about_listener) else {
                return Applied::CannotAnswer;
            };
            let Some(named) = crate::conditions::text_of(world, speaker, listener, emit) else {
                return Applied::CannotAnswer;
            };
            // Reference: `d.actor( is_npc )->pos_abs()` unless a `target_var` names a square (`npctalk.cpp:7743-7748`), and `here.get_bub` turns either into a square.
            let square = match target {
                Some(held) => crate::dialogue_values::position_of(world, speaker, listener, held),
                None => world
                    .get::<crate::Position>(creature)
                    .map(|position| (position.x, position.y, position.z)),
            };
            let Some(square) = square else {
                return Applied::CannotAnswer;
            };
            let multiplier = match chance_mult {
                None => 1.0,
                Some(given) => match crate::conditions::value(world, speaker, listener, given) {
                    Some(value) => value,
                    None => return Applied::CannotAnswer,
                },
            };
            match crate::emit_at::emit_field(world, square, &named, multiplier) {
                Some(()) => Applied::Done,
                None => Applied::CannotAnswer,
            }
        }
        Effect::RevealMap { target, radius } => {
            // Reference: `f_reveal_map` (`npctalk.cpp:7752-7760`): the `var_info` names a square, `radius` is a `dbl_or_var` that defaults to zero, and the square is
            // projected to an overmap tile before the reveal (`project_to<coords::omt>`), which the overmap module's own `SQUARES_PER_OMT` division is.
            // **An unset target is the origin** (`math_parser_diag_value.cpp:62-70`): the reference reads the square and reveals there, so a program that reveals
            // around wherever a previous step found reveals around (0, 0) when that step found nothing.
            let (x, y, _) = crate::dialogue_values::position_of(world, speaker, listener, target)
                .unwrap_or((0, 0, 0));
            let distance = match radius {
                None => 0,
                Some(given) => {
                    let Some(value) = crate::conditions::value(world, speaker, listener, given)
                    else {
                        return Applied::CannotAnswer;
                    };
                    #[allow(clippy::cast_possible_truncation)]
                    let rounded = value as i32;
                    rounded
                }
            };
            crate::overmap::reveal(world, crate::overmap::tile_of(x, y), distance);
            Applied::Done
        }
        Effect::ActivateTrait {
            trait_name,
            listener: about_listener,
        } => {
            let Some(creature) = actor(speaker, listener, *about_listener) else {
                return Applied::CannotAnswer;
            };
            // Reference: `d.actor( is_npc )->activate_mutation( trait_id( new_trait.evaluate( d ) ) )` (`npctalk.cpp:3522-3524`).
            let Some(named) = crate::conditions::text_of(world, speaker, listener, trait_name)
            else {
                return Applied::CannotAnswer;
            };
            let _ = crate::mutation_activation::switch_on(world, creature, &named);
            Applied::Done
        }
        Effect::LoseTrait {
            trait_name,
            listener: about_listener,
        } => {
            let Some(creature) = actor(speaker, listener, *about_listener) else {
                return Applied::CannotAnswer;
            };
            // Reference: `unset_mutation` on a trait the creature does not have does nothing, and a `lose_trait` that finds
            // nothing to lose has still done its job - so the answer is `Done` either way.
            let Some(named) = crate::conditions::text_of(world, speaker, listener, trait_name)
            else {
                return Applied::CannotAnswer;
            };
            crate::traits::remove_trait(world, creature, &named);
            Applied::Done
        }
        // A cast needs the spell effect table, a named program needs the registry and a depth only the program runner has, and everything else has a rule the port
        // has not written. All of them refuse.
        _ => Applied::CannotAnswer,
    }
}

/// Runs a list of effects in order, stopping at the first the store cannot answer.
///
/// Reference: `talk_effect_t::apply` walks its list and runs each; the port stops early on a refusal for the same reason it
/// stops inside a `math` list - a later effect that assumed an earlier one's work would otherwise run on nothing.
pub fn run_effects(
    world: &mut World,
    speaker: Entity,
    listener: Option<Entity>,
    effects: &[Effect],
) -> Applied {
    for effect in effects {
        if run_effect(world, speaker, listener, effect) == Applied::CannotAnswer {
            return Applied::CannotAnswer;
        }
    }
    Applied::Done
}

/// Offers missions to the other creature in the conversation.
///
/// Reference: `f_offer_mission` (`npctalk.cpp:5990-5998`): the reference takes the **beta** of the conversation - the NPC - and adds a mission of each named type to it.
/// With no other creature present there is nobody to offer anything to, which is a refusal rather than a silent success.
fn offer_mission(
    world: &mut World,
    _speaker: Entity,
    listener: Option<Entity>,
    effect: &Effect,
) -> Applied {
    let Effect::OfferMission { missions } = effect else {
        return Applied::CannotAnswer;
    };
    let Some(npc) = listener else {
        return Applied::CannotAnswer;
    };
    for mission in missions {
        crate::missions::offer(world, npc, mission);
    }
    Applied::Done
}

/// Whether one `search_data` entry matches an item.
///
/// Reference: `item_search_data::check` (`npctalk.cpp:325-441`): every member the entry states must pass, and a member it omits asks nothing. The condition is asked with the
/// item as the dialogue's **beta** (`:430-435`), which is how a filter can read the item's own contents.
fn filter_matches(
    world: &mut World,
    speaker: Entity,
    item: Entity,
    name: &str,
    is_wielded: bool,
    is_worn: bool,
    filter: &cdda_content::dialogue::SearchFilter,
) -> bool {
    if let Some(id) = filter.id.as_deref()
        && id != name
    {
        return false;
    }
    if filter.wielded_only && !is_wielded {
        return false;
    }
    if filter.worn_only && !is_worn {
        return false;
    }
    match filter.condition.as_deref() {
        None => true,
        // Reference: `const_dialogue dial( d.const_actor( false )->const_clone(), get_const_talker_for( loc ) )` (`:432`) - the **item** is the dialogue's beta, so the
        // condition reads the item where it would otherwise read the listener.
        Some(condition) => crate::conditions::holds_with(world, speaker, Some(item), condition),
    }
}

/// Sends an event, with its arguments worked out where the data wrote them as variables.
///
/// Reference: `f_trigger_event` (`npctalk.cpp:7904-7911`) evaluates every argument as a `str_or_var` and hands the list to `event::make_dyn`, which maps position `n` onto
/// the type's `n`th field name. The port sends the same list onto the queue, so **the meaning of a position is the reference's table**, not this function's idea.
fn trigger_event(
    world: &mut World,
    speaker: Entity,
    listener: Option<Entity>,
    kind: &str,
    args: &[cdda_content::dialogue::StringSource],
) -> Applied {
    let mut worked = Vec::with_capacity(args.len());
    for source in args {
        let Some(text) = crate::conditions::converted_text_of(world, speaker, listener, source)
        else {
            return Applied::CannotAnswer;
        };
        worked.push(text);
    }
    crate::events::send(world, crate::events::Event::new(kind, worked));
    Applied::Done
}

/// Hands items to the other creature and raises what it is owed.
///
/// Reference: `f_u_sell_item` (`npctalk.cpp:3897-3930`). The charges branch is tried first and only when the kind is counted in charges (`item::count_by_charges`), then the
/// whole-item branch; neither having enough runs `false_eocs` **on the dialogue**, exactly as the reference's `run_eoc_vector( false_eocs, d )` does.
fn sell_item(
    world: &mut World,
    speaker: Entity,
    listener: Option<Entity>,
    effect: &Effect,
) -> Applied {
    let Effect::SellItem {
        item,
        count,
        cost,
        by_charges,
        true_programs,
        false_programs,
    } = effect
    else {
        return Applied::CannotAnswer;
    };
    let Some(buyer) = listener else {
        return Applied::CannotAnswer;
    };
    let Some(name) = crate::conditions::text_of(world, speaker, listener, item) else {
        return Applied::CannotAnswer;
    };
    let (Some(many), Some(owed)) = (
        draw_count(world, speaker, listener, count),
        draw_count(world, speaker, listener, cost),
    ) else {
        return Applied::CannotAnswer;
    };
    let counted = crate::item_registry::id_of(world, &name)
        .and_then(|kind| crate::item_registry::traits_of(world, kind))
        .is_some_and(|(counted, _)| counted);
    // Reference: the charges branch asks `item::count_by_charges` **and** `has_charges`; the whole-item branch asks `has_amount`; anything else is the false branch.
    let handed = (*by_charges
        && counted
        && crate::inventory::hand_over(world, speaker, buyer, &name, 0, many))
        || crate::inventory::hand_over(world, speaker, buyer, &name, many, 0);
    if !handed {
        for program in false_programs {
            let _ = crate::eoc_registry::run_ref(world, speaker, listener, program, 1);
        }
        return Applied::Done;
    }
    crate::npc_opinion::add_debt(world, buyer, owed);
    for program in true_programs {
        let _ = crate::eoc_registry::run_ref(world, speaker, listener, program, 1);
    }
    Applied::Done
}

/// Runs programs on the items lying on the map within a radius.
///
/// Reference: `f_map_run_item_eocs` (`npctalk.cpp:6830-6875`): the centre is `loc` or the actor's own square, the walk covers every square within `max_radius` and skips the
/// ones inside `min_radius` (the reference's `rl_dist` is a square's **Chebyshev** distance, which is what a walking creature's step count is), and each item found is handed
/// to the programs as the dialogue's beta. A menu mode refuses, because choosing for the player is not something this port can do.
fn map_run_item_eocs(
    world: &mut World,
    speaker: Entity,
    listener: Option<Entity>,
    effect: &Effect,
) -> Applied {
    let Effect::MapRunItemEocs {
        mode,
        loc,
        min_radius,
        max_radius,
        search_data,
        programs,
        false_programs,
    } = effect
    else {
        return Applied::CannotAnswer;
    };
    if mode.starts_with("manual") {
        return Applied::CannotAnswer;
    }
    let Some(centre) = item_place(world, speaker, listener, loc.as_ref()) else {
        return Applied::CannotAnswer;
    };
    let (Some(closest), Some(furthest)) = (
        draw_count(world, speaker, listener, min_radius),
        draw_count(world, speaker, listener, max_radius),
    ) else {
        return Applied::CannotAnswer;
    };
    // The items on the map, with how far each is from the centre. `rl_dist` counts a diagonal step as one, which is the rule a radius asks about.
    let mut found: Vec<(Entity, String)> = Vec::new();
    let mut items: Vec<(Entity, crate::Position, crate::ItemTypeId)> = Vec::new();
    {
        let Some(mut query) = world.try_query::<(Entity, &crate::Position, &crate::GroundItem)>()
        else {
            return Applied::CannotAnswer;
        };
        for (entity, position, item) in query.iter(world) {
            items.push((entity, *position, item.kind));
        }
    }
    for (entity, position, kind) in items {
        let step = (position.x - centre.x)
            .abs()
            .max((position.y - centre.y).abs())
            .max((position.z - centre.z).abs());
        if step < closest || step > furthest {
            continue;
        }
        let Some(name) = crate::item_registry::name_of(world, kind) else {
            continue;
        };
        let accepted = search_data.is_empty()
            || search_data
                .iter()
                .any(|filter| filter_matches(world, speaker, entity, &name, false, false, filter));
        if accepted {
            found.push((entity, name));
        }
    }
    if found.is_empty() {
        for program in false_programs {
            let _ = crate::eoc_registry::run_ref(world, speaker, listener, program, 1);
        }
        return Applied::Done;
    }
    if mode == "random" {
        let Some(picked) = draw_one_of(world, found.len()) else {
            return Applied::CannotAnswer;
        };
        found = found.get(picked).cloned().into_iter().collect();
    }
    for (item, name) in found {
        let handed = std::collections::BTreeMap::from([("id".to_owned(), name)]);
        let previous = crate::dialogue_context::put(world, speaker, handed);
        for program in programs {
            let _ = crate::eoc_registry::run_ref(world, speaker, Some(item), program, 1);
        }
        crate::dialogue_context::restore(world, speaker, previous);
    }
    Applied::Done
}

/// Transforms every square on the line between two points.
///
/// Reference: `f_transform_line` (`npctalk.cpp:4781-4796`) and `map::transform_line` (`map.cpp:4802-4811`): both ends come from variables, the walk is `line_to`, and each
/// square it passes through gets the transform's rule. The walk **excludes** its first square - `bresenham` reports the squares it moves *to* - which is why the far end changes
/// and the near end does not.
fn transform_line(
    world: &mut World,
    speaker: Entity,
    listener: Option<Entity>,
    effect: &Effect,
) -> Applied {
    let Effect::TransformLine {
        transform,
        first,
        second,
    } = effect
    else {
        return Applied::CannotAnswer;
    };
    let Some(named) = crate::conditions::text_of(world, speaker, listener, transform) else {
        return Applied::CannotAnswer;
    };
    // The two ends are variables, and a position is read the way every other position variable is.
    let read_end = |world: &mut World, (scope, name): &(String, String)| {
        let holder = crate::conditions::holder_for_scope(world, speaker, listener, scope)?;
        match crate::dialogue_values::value_of(world, holder, name) {
            Some(crate::dialogue_values::Value::Position { x, y, z }) => Some((x, y, z)),
            _ => None,
        }
    };
    let (Some(from), Some(to)) = (read_end(world, first), read_end(world, second)) else {
        return Applied::CannotAnswer;
    };
    if crate::terrain_transform::holder(world).is_none() {
        return Applied::CannotAnswer;
    }
    for square in
        cdda_core_types::line::line_to_3d([from.0, from.1, from.2], [to.0, to.1, to.2], 0, 0)
    {
        let _ =
            crate::terrain_transform::apply_at(world, &named, (square[0], square[1], square[2]));
    }
    Applied::Done
}

/// **The person the dialogue is with**, which is the listener when there is one and the speaker otherwise.
///
/// Reference: a legacy `talk_function` takes `npc &p` - the person being talked to - so a program that runs one from an NPC's own topic aims it at that NPC. The port asks the
/// listener first because that is the person a conversation is with, and falls back to the speaker for a program that an NPC runs on itself.
fn person_in_the_dialogue(
    world: &World,
    speaker: Entity,
    listener: Option<Entity>,
) -> Option<Entity> {
    let listener = listener?;
    if world.get::<crate::npcs::Npc>(listener).is_some() {
        Some(listener)
    } else if world.get::<crate::npcs::Npc>(speaker).is_some() {
        Some(speaker)
    } else {
        None
    }
}

/// **`hostile` makes the person turn on the player.** Reference: `talk_function::hostile` (`npctalk_funcs.cpp:931-944`): a person already trying to kill the player is left
/// alone; otherwise, if they can see the player, the game announces that they turn hostile, and their attitude becomes `NPCATT_KILL`. The data uses the name **184 times**,
/// more than any other legacy effect, which is why it is the first one ported.
fn turn_the_npc_hostile(world: &mut World, speaker: Entity, listener: Option<Entity>) -> Applied {
    let Some(npc) = person_in_the_dialogue(world, speaker, listener) else {
        return Applied::CannotAnswer;
    };
    if crate::npcs::attitude_of(world, npc).as_deref() == Some("kill") {
        // Reference: `if( p.get_attitude() == NPCATT_KILL ) { return; }` - nothing at all, not even the message.
        return Applied::Done;
    }
    let sees = crate::missions::avatar(world).is_some_and(|avatar| {
        world
            .get::<crate::Position>(avatar)
            .is_some_and(|at| crate::seeing::can_see_location(world, npc, (at.x, at.y)))
    });
    if sees {
        let name = world
            .get::<crate::creature::Creature>(npc)
            .map_or_else(String::new, |creature| creature.name.clone());
        crate::messages::push(world, &format!("{name} turns hostile!"));
    }
    crate::npcs::set_attitude(world, npc, "kill");
    Applied::Done
}

/// **`flee` makes the person run.** Reference: `talk_function::flee` (`npctalk_funcs.cpp:946-951`): the game announces it and the attitude becomes `NPCATT_FLEE`, with no
/// sight test - the message is unconditional there, which is the difference from `hostile` that this rule keeps.
fn make_the_npc_flee(world: &mut World, speaker: Entity, listener: Option<Entity>) -> Applied {
    let Some(npc) = person_in_the_dialogue(world, speaker, listener) else {
        return Applied::CannotAnswer;
    };
    let name = world
        .get::<crate::creature::Creature>(npc)
        .map_or_else(String::new, |creature| creature.name.clone());
    crate::messages::push(world, &format!("{name} turns to flee!"));
    crate::npcs::set_attitude(world, npc, "flee");
    Applied::Done
}

/// Sets the person's attitude towards the player, and answers whether there was a person to set it on.
fn set_the_persons_attitude(
    world: &mut World,
    speaker: Entity,
    listener: Option<Entity>,
    attitude: &str,
) -> Applied {
    let Some(npc) = person_in_the_dialogue(world, speaker, listener) else {
        return Applied::CannotAnswer;
    };
    crate::npcs::set_attitude(world, npc, attitude);
    Applied::Done
}

/// **`follow` takes the person along as an ally.** Reference: `talk_function::follow` (`npctalk_funcs.cpp:889-899`): the person joins the player's followers, their attitude
/// becomes `NPCATT_FOLLOW`, and their faction becomes `your_followers`.
///
/// **Named gaps, both fields the port has nowhere to put**: the reference also hands the player the follower's cash (`p.cash`) and clears their custom profession, and the port
/// has no money on a person and no profession. The ally relation itself is the port's `Ally` marker, which is what `is_player_ally` answers from.
fn take_the_person_along(world: &mut World, speaker: Entity, listener: Option<Entity>) -> Applied {
    let Some(npc) = person_in_the_dialogue(world, speaker, listener) else {
        return Applied::CannotAnswer;
    };
    crate::npcs::make_ally(world, npc);
    crate::npcs::set_attitude(world, npc, "follow");
    Applied::Done
}

/// **`stop_following` tells a non-ally to stop.** Reference: `talk_function::stop_following` (`npctalk_funcs.cpp:974-985`): an ally is left alone - allies are sent to guard
/// instead - and anyone else is told and their attitude becomes `NPCATT_NULL`.
fn stop_the_person_following(
    world: &mut World,
    speaker: Entity,
    listener: Option<Entity>,
) -> Applied {
    let Some(npc) = person_in_the_dialogue(world, speaker, listener) else {
        return Applied::CannotAnswer;
    };
    if world.get::<crate::npcs::Ally>(npc).is_some() {
        // Reference: `if( p.is_player_ally() ) { return; }` - a silent no-op, and the game says nothing.
        return Applied::Done;
    }
    let name = world
        .get::<crate::creature::Creature>(npc)
        .map_or_else(String::new, |creature| creature.name.clone());
    crate::messages::push(world, &format!("{name} stops following."));
    crate::npcs::set_attitude(world, npc, "null");
    Applied::Done
}

/// **`deny_follow` remembers that the person was asked.** Reference: `talk_function::deny_follow` (`npctalk_funcs.cpp:906-909`): the person takes `asked_to_follow` for six
/// hours - no message, no attitude change. The effect is what a later dialogue guard reads, so a person asked once will not be asked again today.
fn the_person_was_asked_to_follow(
    world: &mut World,
    speaker: Entity,
    listener: Option<Entity>,
) -> Applied {
    let Some(npc) = person_in_the_dialogue(world, speaker, listener) else {
        return Applied::CannotAnswer;
    };
    if let Some(kind) = crate::effect_registry::kind_named(world, "asked_to_follow") {
        crate::applied_effects::add(
            world,
            npc,
            crate::effects::ActiveEffect::new(&kind, 3_600, 1),
        );
    }
    Applied::Done
}

/// **`insult_combat` starts a fight.** Reference: `talk_function::insult_combat` (`npctalk_funcs.cpp:524-529`): the game says the player started it, the conversation ends, and
/// the person's attitude becomes `NPCATT_KILL` - with **no sight test**, unlike `hostile`, which is why it is a separate rule rather than a spelling of the same one.
fn start_a_fight_with_the_person(
    world: &mut World,
    speaker: Entity,
    listener: Option<Entity>,
) -> Applied {
    let Some(npc) = person_in_the_dialogue(world, speaker, listener) else {
        return Applied::CannotAnswer;
    };
    let name = world
        .get::<crate::creature::Creature>(npc)
        .map_or_else(String::new, |creature| creature.name.clone());
    crate::messages::push(world, &format!("You start a fight with {name}!"));
    // Reference: `p.chatbin.first_topic = "TALK_DONE";` - the conversation is over as well as the peace.
    crate::npcs::set_first_topic(world, npc, "TALK_DONE");
    crate::npcs::set_attitude(world, npc, "kill");
    Applied::Done
}

/// **`stranger_neutral` calms a stranger.** Reference: `talk_function::stranger_neutral` (`npctalk_funcs.cpp:987-992`): the game says they feel less threatened and their
/// attitude becomes `NPCATT_NULL`.
///
/// **Named gap**: the reference also switches the conversation to its "stranger, neutral" topic, which is the dialogue window's state and not the world's.
fn the_stranger_feels_less_threatened(
    world: &mut World,
    speaker: Entity,
    listener: Option<Entity>,
) -> Applied {
    let Some(npc) = person_in_the_dialogue(world, speaker, listener) else {
        return Applied::CannotAnswer;
    };
    let name = world
        .get::<crate::creature::Creature>(npc)
        .map_or_else(String::new, |creature| creature.name.clone());
    crate::messages::push(world, &format!("{name} feels less threatened by you."));
    crate::npcs::set_attitude(world, npc, "null");
    Applied::Done
}

/// **`npc_thankful` ends a hostile attitude and cools the person down.** Reference: `talk_function::npc_thankful` (`npctalk_funcs.cpp:1340-1352`): one of five attitudes -
/// mugging, waiting to leave, fleeing, killing, or fleeing for a while - becomes `NPCATT_NULL`, and the person's **aggression** drops by one, clamped.
///
/// **Named gaps, both fields the port does not have**: the personality's aggression (the port has no personality), and the conversation topic the reference then switches to.
/// The attitude half is the half the world can see.
fn the_person_is_thankful(world: &mut World, speaker: Entity, listener: Option<Entity>) -> Applied {
    let Some(npc) = person_in_the_dialogue(world, speaker, listener) else {
        return Applied::CannotAnswer;
    };
    let attitude = crate::npcs::attitude_of(world, npc).unwrap_or_default();
    if ["mug", "wait_for_leave", "flee", "kill", "flee_temp"].contains(&attitude.as_str()) {
        crate::npcs::set_attitude(world, npc, "null");
    }
    Applied::Done
}

/// **`wake_up` wakes a sleeping person.** Reference: `talk_function::wake_up` (`npctalk_funcs.cpp:502-511`): the ally rule that allowed sleeping is overridden, and the four
/// effects that put them under - `allow_sleep`, `lying_down`, `npc_suspend` and `sleep` - are removed. The reference's own `// TODO: Get mad at player for waking us up` is
/// left as it is: there is no anger here either.
///
/// **Named gap**: the ally-rule override is part of the reference's NPC rules system, which the port does not have; the effects are the part that changes the world.
fn wake_the_person_up(world: &mut World, speaker: Entity, listener: Option<Entity>) -> Applied {
    let Some(npc) = person_in_the_dialogue(world, speaker, listener) else {
        return Applied::CannotAnswer;
    };
    for effect in ["allow_sleep", "lying_down", "npc_suspend", "sleep"] {
        crate::applied_effects::remove(world, npc, effect);
    }
    Applied::Done
}

/// **`leave` ends a companion's company.** Reference: `talk_function::leave` (`npctalk_funcs.cpp:953-968`): the game says the person leaves and they are taken off the
/// player's followers.
///
/// **Named gaps**: the reference also gives the person a new "lone wolf" faction named after them, because factions are how it tracks who follows whom - the port's `Ally`
/// marker is the whole relation, so removing it is the whole rule here.
fn the_person_leaves(world: &mut World, speaker: Entity, listener: Option<Entity>) -> Applied {
    let Some(npc) = person_in_the_dialogue(world, speaker, listener) else {
        return Applied::CannotAnswer;
    };
    let name = world
        .get::<crate::creature::Creature>(npc)
        .map_or_else(String::new, |creature| creature.name.clone());
    crate::messages::push(world, &format!("{name} leaves."));
    world.entity_mut(npc).remove::<crate::npcs::Ally>();
    Applied::Done
}

/// **`assign_mission` hands the player the mission the person is offering.** Reference: `talk_function::assign_mission` (`npctalk_funcs.cpp:122-137`): the mission the
/// conversation has selected is assigned to the player, moved from the person's offered list to their assigned list - and a mission that is **already assigned** is
/// refused with a warning rather than given twice. The data uses this name **80 times**, more than any other legacy effect the port has left.
///
/// **The one divergence, and it is the conversation's, not the rule's**: the reference's "selected" mission is what the player picked in the talk window. The port has no
/// talk window, so it takes the **first offer in id order** - the choice a conversation with one option makes - and says so in the line where it picks.
fn assign_the_mission_on_the_table(
    world: &mut World,
    speaker: Entity,
    listener: Option<Entity>,
) -> Applied {
    let Some(npc) = person_in_the_dialogue(world, speaker, listener) else {
        return Applied::CannotAnswer;
    };
    // The port's "selected mission": the first offer in id order. A conversation with nothing on the table has nothing to assign.
    let Some(mission) = crate::missions::offers(world, npc).first().cloned() else {
        return Applied::CannotAnswer;
    };
    let holder = crate::missions::question_holder(world, speaker);
    if crate::missions::has_mission(world, holder, &mission) {
        // Reference: `else if( miss->is_assigned() ) { ... "is already assigned!" ... return; }` - the mission stays on both sides.
        return Applied::Done;
    }
    crate::missions::accept(world, holder, &mission);
    crate::missions::unoffer(world, npc, &mission);
    Applied::Done
}

/// **`clear_mission` takes a mission off the player and puts its follow-up on the table.** Reference: `talk_function::clear_mission` (`npctalk_funcs.cpp:178-202`): the selected
/// mission is erased from the person's assigned list, and **if it has a follow-up**, a new mission of that type is added to what the person offers - which is how a chain of
/// missions continues. A selected mission that is not in the assigned list is refused with a warning and nothing happens.
///
/// **The same divergence as `assign_mission`, stated once here**: the "selected" mission is what the player picked in the talk window, and the port has no talk window, so it takes
/// the **first active mission in id order**.
fn clear_the_mission_on_the_table(
    world: &mut World,
    speaker: Entity,
    listener: Option<Entity>,
) -> Applied {
    let Some(npc) = person_in_the_dialogue(world, speaker, listener) else {
        return Applied::CannotAnswer;
    };
    let holder = crate::missions::question_holder(world, speaker);
    // The port's "selected mission": the first active one in id order. Nothing active is nothing to clear, and the reference's warning is a debug message, not a game event.
    let Some(mission) = crate::missions::active_missions(world, holder)
        .first()
        .cloned()
    else {
        return Applied::Done;
    };
    if !crate::missions::finish(world, holder, &mission) {
        return Applied::Done;
    }
    // Reference: `if( miss->has_follow_up() ) { p.add_new_mission( reserve_new( ... ) ); }` - the follow-up goes back on the **person's** table, not straight to the player.
    if let Some(next) = crate::mission_types::follow_up(world, &mission) {
        crate::missions::offer(world, npc, &next);
    }
    Applied::Done
}

/// **`end_conversation` ends the conversation and the person stops listening.** Reference: `talk_function::end_conversation` (`npctalk_funcs.cpp:518-522`): the game says
/// "<name> starts ignoring you." and the person's **first topic** becomes `TALK_DONE`, which is the reference's way of saying no topic is on offer any more.
///
/// The port has no dialogue window, but `first_topic` is the person's own memory rather than the window's state, so this is the whole rule.
fn end_the_conversation(world: &mut World, speaker: Entity, listener: Option<Entity>) -> Applied {
    let Some(npc) = person_in_the_dialogue(world, speaker, listener) else {
        return Applied::CannotAnswer;
    };
    let name = world
        .get::<crate::creature::Creature>(npc)
        .map_or_else(String::new, |creature| creature.name.clone());
    crate::messages::push(world, &format!("{name} starts ignoring you."));
    crate::npcs::set_first_topic(world, npc, "TALK_DONE");
    Applied::Done
}

/// **`remove_stolen_status` forgets a theft and calms down.** Reference: `talk_function::remove_stolen_status` (`npctalk_funcs.cpp:1039-1045`): the person's
/// `known_stolen_item` is cleared and their attitude becomes `NPCATT_NULL`.
///
/// **Named gap**: the port has no "known stolen item" - a person does not watch the player take things - so the attitude is the whole observable half of the rule here.
fn forgot_what_was_stolen(world: &mut World, speaker: Entity, listener: Option<Entity>) -> Applied {
    let Some(npc) = person_in_the_dialogue(world, speaker, listener) else {
        return Applied::CannotAnswer;
    };
    crate::npcs::set_attitude(world, npc, "null");
    Applied::Done
}

/// **`start_mugging` demands the player's things.** Reference: `talk_function::start_mugging` (`npctalk_funcs.cpp:1047-1051`): the attitude becomes `NPCATT_MUG` and the game
/// warns the player that movement may cause an attack.
fn start_mugging_the_player(
    world: &mut World,
    speaker: Entity,
    listener: Option<Entity>,
) -> Applied {
    let Some(npc) = person_in_the_dialogue(world, speaker, listener) else {
        return Applied::CannotAnswer;
    };
    let name = world
        .get::<crate::creature::Creature>(npc)
        .map_or_else(String::new, |creature| creature.name.clone());
    crate::npcs::set_attitude(world, npc, "mug");
    crate::messages::push(
        world,
        &format!("Pause to stay still.  Any movement may cause {name} to attack."),
    );
    Applied::Done
}

/// **`player_leaving` lets the player walk away.** Reference: `talk_function::player_leaving` (`npctalk_funcs.cpp:1053-1057`): the attitude becomes `NPCATT_WAIT_FOR_LEAVE`
/// and the person's **patience** is set to fifteen minus their aggression.
///
/// **Named gaps**: the port has no patience field and no personality to give it aggression, so the attitude is the half that exists here.
fn let_the_player_leave(world: &mut World, speaker: Entity, listener: Option<Entity>) -> Applied {
    let Some(npc) = person_in_the_dialogue(world, speaker, listener) else {
        return Applied::CannotAnswer;
    };
    crate::npcs::set_attitude(world, npc, "wait_for_leave");
    Applied::Done
}

/// **`drop_weapon` makes the person put their weapon down.** Reference: `talk_function::drop_weapon` (`npctalk_funcs.cpp:1059-1066`): a hallucination has no weapon to
/// drop and returns; anything else takes the weapon out of the person's hands and puts it on the square they stand on.
///
/// **Named gap**: a hallucination is a monster the port does not make - its `is_hallucination` is a flag on a creature it never spawns - so the check is not carried and the
/// effect drops whatever is held. A person with nothing in hand answers `Done` and changes nothing, which is what the reference's `remove_weapon` on an empty hand does.
fn the_person_drops_their_weapon(
    world: &mut World,
    speaker: Entity,
    listener: Option<Entity>,
) -> Applied {
    let Some(npc) = person_in_the_dialogue(world, speaker, listener) else {
        return Applied::CannotAnswer;
    };
    crate::dropping::put_down_what_is_held(world, npc);
    Applied::Done
}

/// **`player_weapon_drop` makes the player put their own weapon down.** Reference: `talk_function::player_weapon_drop` (`npctalk_funcs.cpp:1082-1090`): the player's weapon
/// is removed and dropped through the player's own drop path, which places it at the player's square - the reference's `drop_on_map` with `item_drop_reason::deliberate`.
///
/// The listener is not read: the effect acts on the **player**, whatever the person in the conversation is holding.
fn the_player_drops_their_weapon(world: &mut World, speaker: Entity) -> Applied {
    let Some(player) = crate::missions::avatar(world).or(Some(speaker)) else {
        return Applied::CannotAnswer;
    };
    crate::dropping::put_down_what_is_held(world, player);
    Applied::Done
}

/// **`assign_guard` tells a person to stay put.** Reference: `talk_function::assign_guard` (`npctalk_funcs.cpp:432-447`): a person who is **not** an ally is given the job
/// `NPC_MISSION_GUARD` and a destination, and that is all. An **ally** is calmed to `NPCATT_NULL`, given `NPC_MISSION_GUARD_ALLY`, switched to their guard topic and given a
/// destination - the difference being that an ally guarding is still following the player's orders.
///
/// **Named gaps**: the destination (`set_omt_destination`, which is an overmap position the port does not place), the topic the ally is switched to (`chatbin.talk_friend_guard`,
/// which is a field of the NPC's own chat data), and `revert_after_activity` (the port has no activities). The job and the attitude are the half that changes the world here.
fn guard_the_place(world: &mut World, speaker: Entity, listener: Option<Entity>) -> Applied {
    let Some(npc) = person_in_the_dialogue(world, speaker, listener) else {
        return Applied::CannotAnswer;
    };
    if world.get::<crate::npcs::Ally>(npc).is_none() {
        crate::npcs::set_mission(world, npc, "guard");
        return Applied::Done;
    }
    crate::npcs::set_attitude(world, npc, "null");
    crate::npcs::set_mission(world, npc, "guard_ally");
    Applied::Done
}

/// **`stop_guard` lets a person stop guarding.** Reference: `talk_function::stop_guard` (`npctalk_funcs.cpp:477-493`): a person who is not an ally is calmed to `NPCATT_NULL`
/// and loses their job; an **ally** goes back to following - the attitude `NPCATT_FOLLOW` and the line "<name> begins to follow you." - and loses their job as well.
fn stop_guarding_the_place(
    world: &mut World,
    speaker: Entity,
    listener: Option<Entity>,
) -> Applied {
    let Some(npc) = person_in_the_dialogue(world, speaker, listener) else {
        return Applied::CannotAnswer;
    };
    if world.get::<crate::npcs::Ally>(npc).is_none() {
        crate::npcs::set_attitude(world, npc, "null");
        crate::npcs::set_mission(world, npc, "null");
        return Applied::Done;
    }
    crate::npcs::set_attitude(world, npc, "follow");
    let name = world
        .get::<crate::creature::Creature>(npc)
        .map_or_else(String::new, |creature| creature.name.clone());
    crate::messages::push(world, &format!("{name} begins to follow you."));
    crate::npcs::set_mission(world, npc, "null");
    Applied::Done
}

/// **`drop_items_in_place` empties a person's pockets on the spot.** Reference: `talk_function::drop_items_in_place` (`npctalk_funcs.cpp:869-887`): every **carried** thing
/// that is not a favourite and sits in a container the person wears is put on the ground as one activity, and the person says `<acknowledged>`; with nothing to drop they say
/// "I don't have anything to drop off."
///
/// **Named gaps**: the favourite flag (the port does not mark favourites, so everything carried is dropped) and the activity wrapper (the port drops at once - `drop_activity_actor`
/// exists to spread the drop over turns, which needs activities the port does not have).
fn the_person_drops_their_burden(
    world: &mut World,
    speaker: Entity,
    listener: Option<Entity>,
) -> Applied {
    let Some(npc) = person_in_the_dialogue(world, speaker, listener) else {
        return Applied::CannotAnswer;
    };
    let carried = {
        let mut query = world.query::<(Entity, &crate::creature::Carried)>();
        query
            .iter(world)
            .filter(|(_, carried)| carried.0 == npc)
            .map(|(entity, _)| entity)
            .collect::<Vec<_>>()
    };
    if carried.is_empty() {
        crate::messages::push(world, "I don't have anything to drop off.");
        return Applied::Done;
    }
    for item in carried {
        // Whatever is in the person's hands is a carried thing too, and the reference drops it with the rest.
        if let Some(at) = world.get::<crate::Position>(npc).copied() {
            world.entity_mut(item).remove::<crate::creature::Carried>();
            world
                .entity_mut(item)
                .remove::<crate::creature::WieldedBy>();
            world.entity_mut(item).remove::<crate::creature::InPocket>();
            world.entity_mut(item).insert(at);
            crate::singletons::index_mut(world).insert(item, (at.x, at.y, at.z));
        }
    }
    crate::messages::push(world, "<acknowledged>");
    Applied::Done
}

/// **`buy_haircut` is a morale point and a line.** Reference: `talk_function::buy_haircut` (`npctalk_funcs.cpp:808-816`): the player gains `morale_haircut` at **5**, capped at
/// 5, lasting **720 minutes** and fading from **3 minutes**, and the person says "<name> gives you a decent haircut…".
///
/// **Named gap**: `ACT_WAIT_NPC` (the reference spends twenty minutes of the player's time on the haircut, which needs activities).
fn a_decent_haircut(world: &mut World, speaker: Entity, listener: Option<Entity>) -> Applied {
    let Some(npc) = person_in_the_dialogue(world, speaker, listener) else {
        return Applied::CannotAnswer;
    };
    let Some(player) = crate::missions::avatar(world) else {
        return Applied::CannotAnswer;
    };
    let name = world
        .get::<crate::creature::Creature>(npc)
        .map_or_else(String::new, |creature| creature.name.clone());
    crate::morale::add(
        world,
        player,
        crate::morale::MoralePoint {
            kind: "morale_haircut".to_owned(),
            bonus: 5,
            max_bonus: 5,
            // 720 minutes and 3 minutes, in turns of one second each.
            duration: 720 * 60,
            decay_start: 3 * 60,
            age: 0,
            capped: false,
        },
    );
    crate::messages::push(world, &format!("{name} gives you a decent haircut\u{2026}"));
    Applied::Done
}

/// **`buy_shave` is the same shape with different numbers.** Reference: `talk_function::buy_shave` (`npctalk_funcs.cpp:818-826`): `morale_shave` at **10**, capped at 10,
/// lasting **360 minutes** and fading from **3 minutes**, and the line "<name> gives you a decent shave…".
///
/// **Named gap**: the same `ACT_WAIT_NPC`, five minutes of it.
fn a_decent_shave(world: &mut World, speaker: Entity, listener: Option<Entity>) -> Applied {
    let Some(npc) = person_in_the_dialogue(world, speaker, listener) else {
        return Applied::CannotAnswer;
    };
    let Some(player) = crate::missions::avatar(world) else {
        return Applied::CannotAnswer;
    };
    let name = world
        .get::<crate::creature::Creature>(npc)
        .map_or_else(String::new, |creature| creature.name.clone());
    crate::morale::add(
        world,
        player,
        crate::morale::MoralePoint {
            kind: "morale_shave".to_owned(),
            bonus: 10,
            max_bonus: 10,
            duration: 360 * 60,
            decay_start: 3 * 60,
            age: 0,
            capped: false,
        },
    );
    crate::messages::push(world, &format!("{name} gives you a decent shave\u{2026}"));
    Applied::Done
}

/// **A person tends the player's wounds, part by part.** Reference: `talk_function::give_aid` (`npctalk_funcs.cpp:712-733`) heals **every main body part** by
/// `5 * rng( 2, 5 )` and clears `bite`, `bleed` and `infected`; `lesser_give_aid` (`:673-687`) heals by `rng( 5, 15 )` and clears only `bleed`. Both then leave the person
/// `currently_busy` - two hours for the full aid, one for the lesser - so nobody gets a second opinion out of them that afternoon.
///
/// **Named gaps, both of them model differences rather than missing lines.** The reference heals a **part**, and the port heals a part by lowering its [`crate::creature::Wounds`]
/// entry, which is the same number with the sign the port stores. And the reference's `bite`, `bleed` and `infected` are **per body part**; the port's effects are per creature,
/// so they are cleared once instead of once per part - a creature with an infected arm and a healthy leg cannot be half-cured here, which is the port's known effect model and not
/// this rule's choice.
fn tend_the_players_wounds(
    world: &mut World,
    speaker: Entity,
    listener: Option<Entity>,
    full: bool,
    busy_minutes: i64,
    allies: bool,
) -> Applied {
    let Some(npc) = person_in_the_dialogue(world, speaker, listener) else {
        return Applied::CannotAnswer;
    };
    let Some(player) = crate::missions::avatar(world) else {
        return Applied::CannotAnswer;
    };
    // **The dice belong to the turn**, so they are taken for the length of the draws and put back after - the pattern every rule that rolls without a caller's stream uses.
    let mut rolls = crate::singletons::take_rolls(world).unwrap_or_else(|| {
        let turn = crate::calendar::of(world).map_or(0, |calendar| calendar.turn);
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let seed = turn as u32;
        cdda_core_types::rng::Rolls::new(seed)
    });
    heal_a_body(world, player, full, &mut rolls);
    // Reference: the "all" rules then do the same for **every companion within `PICKUP_RANGE`** of the player (`game_constants.h:44`, six squares), which is the same
    // arithmetic on a different body.
    if allies {
        let here = world.get::<crate::Position>(player).copied();
        let companions = {
            let mut query = world.query::<(Entity, &crate::npcs::Ally, &crate::Position)>();
            query
                .iter(world)
                .filter(|(_, _, at)| {
                    here.is_none_or(|here| {
                        crate::rl_dist([here.x, here.y, here.z], [at.x, at.y, at.z]) < 6
                    })
                })
                .map(|(entity, _, _)| entity)
                .collect::<Vec<_>>()
        };
        for companion in companions {
            heal_a_body(world, companion, full, &mut rolls);
        }
    }
    crate::singletons::put_rolls(world, rolls);
    if let Some(kind) = crate::effect_registry::kind_named(world, "currently_busy") {
        crate::applied_effects::add(
            world,
            npc,
            crate::effects::ActiveEffect::new(&kind, busy_minutes * 60, 1),
        );
    }
    Applied::Done
}

/// Heals every part of one body by the aid's amount and clears what the aid clears.
///
/// Reference: the inner loop of `give_aid` (`npctalk_funcs.cpp:715-727`) - `heal( bp, 5 * rng( 2, 5 ) )` for the full aid and `rng( 5, 15 )` for the lesser, with `bite`,
/// `bleed` and `infected` cleared by the first and `bleed` alone by the second. The roll is **inside** the part loop because the reference draws once per part.
fn heal_a_body(
    world: &mut World,
    creature: Entity,
    full: bool,
    rolls: &mut cdda_core_types::rng::Rolls,
) {
    let parts = world
        .get::<crate::creature::Body>(creature)
        .map(|body| {
            body.parts
                .iter()
                .map(|part| part.name.clone())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    for part in parts {
        let healed = if full {
            5 * rolls.rng(2, 5)
        } else {
            rolls.rng(5, 15)
        };
        let hurt = crate::creature::wound_on(world, creature, &part);
        if hurt <= 0 {
            continue;
        }
        let left = (hurt - healed).max(0);
        if let Some(mut wounds) = world.get_mut::<crate::creature::Wounds>(creature) {
            wounds.0.insert(part, left);
        }
    }
    if full {
        for effect in ["bite", "bleed", "infected"] {
            crate::applied_effects::remove(world, creature, effect);
        }
    } else {
        crate::applied_effects::remove(world, creature, "bleed");
    }
}

/// **A pleasant conversation: an activity, a morale point, and sometimes a little more trust.** Reference: `talk_function::morale_chat_activity`
/// (`npctalk_funcs.cpp:828-861`).
///
/// The rule has four parts, and they run in this order:
///
/// 1. the player starts `ACT_SOCIALIZE` for **ten minutes**;
/// 2. one time in three the person says a line from the `npc_socialize` snippets;
/// 3. the game says "That was a pleasant conversation with <name>.";
/// 4. **unless the person has talked recently** (the `socialized_recently` effect) and unless ten chats have already moved a number, one of three things happens - a point of
///    **trust**, a point of **value**, or a point off their **anger** if they have any - and the same effect stops the next chat for **six hours**;
/// 5. the player gains `morale_chat`, `rng( 3, 10 )`, capped at 10, for 200 minutes, fading from two and a half.
///
/// **Named gap**: the snippet. The port has no snippet registry, so the branch is drawn and says nothing rather than inventing a line.
fn a_pleasant_conversation(
    world: &mut World,
    speaker: Entity,
    listener: Option<Entity>,
) -> Applied {
    let Some(npc) = person_in_the_dialogue(world, speaker, listener) else {
        return Applied::CannotAnswer;
    };
    let Some(player) = crate::missions::avatar(world) else {
        return Applied::CannotAnswer;
    };
    let name = world
        .get::<crate::creature::Creature>(npc)
        .map_or_else(String::new, |creature| creature.name.clone());
    let mut rolls = crate::singletons::take_rolls(world).unwrap_or_else(|| {
        let turn = crate::calendar::of(world).map_or(0, |calendar| calendar.turn);
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let seed = turn as u32;
        cdda_core_types::rng::Rolls::new(seed)
    });
    // 1. **The socialize activity**, ten minutes of the player's time at a hundred moves a turn.
    if let Some(mut doing) = world.get_mut::<crate::creature::Doing>(player) {
        "ACT_SOCIALIZE".clone_into(&mut doing.activity);
        doing.moves_left = 10 * 60 * 100;
        doing.moves_total = doing.moves_left;
    } else {
        world.entity_mut(player).insert(crate::creature::Doing {
            activity: "ACT_SOCIALIZE".to_owned(),
            moves_left: 10 * 60 * 100,
            basis: crate::creature::Basis::Time,
            moves_total: 10 * 60 * 100,
            subject: None,
        });
    }
    // 2. **The snippet**, drawn even though the port has nowhere to put it: the reference's `if( one_in( 3 ) )` spends the die before the message either way.
    let _snippet = rolls.one_in(3);
    // 3. **The line**.
    crate::messages::push(
        world,
        &format!("That was a pleasant conversation with {name}."),
    );
    // 4. **The opinion**, guarded by the effect and the ten-chat cap.
    let socialized = crate::applied_effects::has(world, npc, "socialized_recently");
    let raised = crate::npcs::opinion_of(world, npc).map_or(0, |opinion| opinion.raised);
    if !socialized && raised <= 10 {
        let mut opinion = crate::npcs::opinion_of(world, npc).unwrap_or_default();
        let change = match rolls.rng(1, 3) {
            1 => {
                let change = rolls.rng(0, 1);
                opinion.trust += change;
                change
            }
            2 => {
                let change = rolls.rng(0, 1);
                opinion.value += change;
                change
            }
            _ => {
                if opinion.anger > 0 {
                    let change = rolls.rng(-1, 0);
                    opinion.anger += change;
                    change
                } else {
                    0
                }
            }
        };
        // Reference: "we need to check for any non-zero value, e.g. anger change might be negative".
        if change != 0 {
            opinion.raised = opinion.raised.saturating_add(1);
        }
        crate::npcs::set_opinion(world, npc, opinion);
        if let Some(kind) = crate::effect_registry::kind_named(world, "socialized_recently") {
            // 6 hours.
            crate::applied_effects::add(
                world,
                npc,
                crate::effects::ActiveEffect::new(&kind, 6 * 60 * 60, 1),
            );
        }
    }
    // 5. **The morale point**: `rng( 3, 10 )`, capped at ten, 200 minutes, fading from two and a half.
    let bonus = rolls.rng(3, 10);
    crate::singletons::put_rolls(world, rolls);
    crate::morale::add(
        world,
        player,
        crate::morale::MoralePoint {
            kind: "morale_chat".to_owned(),
            bonus,
            max_bonus: 10,
            duration: 200 * 60,
            decay_start: 150,
            age: 0,
            capped: false,
        },
    );
    Applied::Done
}

/// **The legacy string effects** - the reference's `talk_function` names, dispatched from a bare string.
///
/// Reference: `talk_effect_t::parse_string_effect` (`npctalk.cpp:8157-8341`) holds a long table of `talk_function` names. The port honours the ones whose rules it has and
/// answers `CannotAnswer` for the rest, so a program that names a rule the port has not written refuses rather than half-running.
///
/// The names lived as guards on one enormous match arm until the arm's own function crossed the line limit; a table of names is what this is, so it is a table.
fn run_legacy_string_effect(
    world: &mut World,
    speaker: Entity,
    listener: Option<Entity>,
    name: &str,
) -> Applied {
    match name {
        "hostile" => turn_the_npc_hostile(world, speaker, listener),
        "flee" => make_the_npc_flee(world, speaker, listener),
        "follow" => take_the_person_along(world, speaker, listener),
        "follow_only" => set_the_persons_attitude(world, speaker, listener, "follow"),
        "stop_following" => stop_the_person_following(world, speaker, listener),
        "deny_follow" => the_person_was_asked_to_follow(world, speaker, listener),
        "insult_combat" => start_a_fight_with_the_person(world, speaker, listener),
        "stranger_neutral" => the_stranger_feels_less_threatened(world, speaker, listener),
        "npc_thankful" => the_person_is_thankful(world, speaker, listener),
        "wake_up" => wake_the_person_up(world, speaker, listener),
        "leave" => the_person_leaves(world, speaker, listener),
        "assign_mission" => assign_the_mission_on_the_table(world, speaker, listener),
        "clear_mission" => clear_the_mission_on_the_table(world, speaker, listener),
        "end_conversation" => end_the_conversation(world, speaker, listener),
        "remove_stolen_status" => forgot_what_was_stolen(world, speaker, listener),
        "start_mugging" => start_mugging_the_player(world, speaker, listener),
        "player_leaving" => let_the_player_leave(world, speaker, listener),
        "drop_weapon" => the_person_drops_their_weapon(world, speaker, listener),
        "player_weapon_drop" => the_player_drops_their_weapon(world, speaker),
        "assign_guard" => guard_the_place(world, speaker, listener),
        "stop_guard" => stop_guarding_the_place(world, speaker, listener),
        "drop_items_in_place" => the_person_drops_their_burden(world, speaker, listener),
        "buy_haircut" => a_decent_haircut(world, speaker, listener),
        "buy_shave" => a_decent_shave(world, speaker, listener),
        // Reference: the single rules leave the person busy for 120 and 60 minutes; the "all" rules heal every companion too and busy them for 240 and 120.
        "give_aid" => tend_the_players_wounds(world, speaker, listener, true, 120, false),
        "lesser_give_aid" => tend_the_players_wounds(world, speaker, listener, false, 60, false),
        "give_all_aid" => tend_the_players_wounds(world, speaker, listener, true, 240, true),
        "lesser_give_all_aid" => {
            tend_the_players_wounds(world, speaker, listener, false, 120, true)
        }
        "morale_chat_activity" => a_pleasant_conversation(world, speaker, listener),
        "mission_success" => the_mission_is_done(world, speaker, listener),
        "mission_failure" => the_mission_has_failed(world, speaker, listener),
        _ => Applied::CannotAnswer,
    }
}

/// **`mission_success` rewards the player for a mission they have finished.** Reference: `talk_function::mission_success` (`npctalk_funcs.cpp:139-161`): the selected mission's
/// value is turned into **favour** by `cash_to_favor`, the person's opinion of the player gains `1 + favour / 5` of value and loses **one** anger, their faction's standing gains
/// `min( 1 + favour / 10, 10 )`, and the mission is wrapped up.
///
/// `cash_to_favor` (`npctalk.cpp:641-648`) is the **square root of the mission's value in hundreds**: `roll_remainder( sqrt( cash / 100.0 ) )` - so a mission worth a hundred
/// dollars is worth one favour, four hundred is two, and the fractional part is a coin rather than a rounding rule.
///
/// **Named gaps**: the faction's standing (the port has no factions, so `likes_u`, `respects_u`, `trusts_u` and `power` have nowhere to go) and `mission::wrap_up`'s **item
/// half**, which consumes the goods a `MGOAL_FIND_ITEM` mission asked for - the port's mission type does not read the item fields yet, and consuming them would be inventing a
/// quantity. The mission **is** taken off the player's list, which is the half `wrap_up` does first.
fn the_mission_is_done(world: &mut World, speaker: Entity, listener: Option<Entity>) -> Applied {
    let Some(npc) = person_in_the_dialogue(world, speaker, listener) else {
        return Applied::CannotAnswer;
    };
    let Some(player) = crate::missions::avatar(world) else {
        return Applied::CannotAnswer;
    };
    // The port's "selected mission": the first active one in id order, as `clear_mission` takes it.
    let Some(mission) = crate::missions::active_missions(world, player)
        .first()
        .cloned()
    else {
        return Applied::Done;
    };
    let value = crate::mission_types::kind_named(world, &mission).map_or(0, |kind| kind.value);
    let mut rolls = crate::singletons::take_rolls(world).unwrap_or_else(|| {
        let turn = crate::calendar::of(world).map_or(0, |calendar| calendar.turn);
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let seed = turn as u32;
        cdda_core_types::rng::Rolls::new(seed)
    });
    let favour = rolls.roll_remainder((f64::from(value) / 100.0).sqrt());
    crate::singletons::put_rolls(world, rolls);
    let mut opinion = crate::npcs::opinion_of(world, npc).unwrap_or_default();
    opinion.value += 1 + favour / 5;
    opinion.anger -= 1;
    crate::npcs::set_opinion(world, npc, opinion);
    // Reference: `miss->wrap_up()` sets the status to success and takes the mission off the player's list. The items it also consumes are the named gap above.
    crate::missions::finish(world, player, &mission);
    Applied::Done
}

/// **`mission_failure` is a mission's worth of disappointment.** Reference: `talk_function::mission_failure` (`npctalk_funcs.cpp:163-176`): `trust` and `value` each lose
/// **one**, `anger` gains **one**, and the mission fails - which the port states as taking it off the player's list, because a failed mission is no longer theirs to do.
fn the_mission_has_failed(world: &mut World, speaker: Entity, listener: Option<Entity>) -> Applied {
    let Some(npc) = person_in_the_dialogue(world, speaker, listener) else {
        return Applied::CannotAnswer;
    };
    let Some(player) = crate::missions::avatar(world) else {
        return Applied::CannotAnswer;
    };
    let Some(mission) = crate::missions::active_missions(world, player)
        .first()
        .cloned()
    else {
        return Applied::Done;
    };
    let mut opinion = crate::npcs::opinion_of(world, npc).unwrap_or_default();
    opinion.trust -= 1;
    opinion.value -= 1;
    opinion.anger += 1;
    crate::npcs::set_opinion(world, npc, opinion);
    crate::missions::finish(world, player, &mission);
    Applied::Done
}
