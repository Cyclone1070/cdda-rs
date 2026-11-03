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
        return write_to_store(world, speaker, listener, name, arguments, assignment.op, value);
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
    let Some(cdda_content::dialogue::Expression::Text(key)) = arguments.first() else {
        return Applied::CannotAnswer;
    };
    // The short listener spelling (`n_skill`) means the same scope as the long one, and a write has to land on the same creature a read would ask about.
    let normalized = crate::conditions::normalized_spelling(name);
    let about_listener = normalized.starts_with("npc_");
    let Some(target) = actor(speaker, listener, about_listener) else {
        return Applied::CannotAnswer;
    };
    let store = normalized.trim_start_matches("npc_").trim_start_matches("u_");
    let (current, writes) = match store {
        "hp" => (crate::creature::health_of(world, target).map(f64::from), true),
        "vitamin" => (crate::body::vitamin_of(world, target, key).map(f64::from), true),
        "skill" => (Some(f64::from(crate::skills_of::level_of(world, target, key))), true),
        "spell_level" => (Some(f64::from(crate::known_spells::level_of(world, target, key))), true),
        "spell_exp" => (Some(f64::from(crate::known_spells::experience_of(world, target, key))), true),
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
    weighted: &[(cdda_content::dialogue::ProgramRef, cdda_content::dialogue::Expression)],
    depth: usize,
) -> Applied {
    let mut dice = crate::singletons::take_rolls(world);
    let mut weights = Vec::with_capacity(weighted.len());
    for (_, weight) in weighted {
        let drawn = crate::conditions::value_drawing(world, speaker, listener, weight, true, dice.as_mut());
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
        Some((program, _)) => crate::eoc_registry::run_ref(world, speaker, listener, program, depth + 1),
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


/// Stores the nearest overmap tile of a terrain in a dialogue variable.
///
/// Reference: `f_location_variable` (`npctalk.cpp:4194`) with a `target_params` naming an overmap terrain. The port searches the overmap the way
/// `om_terrain` aiming does, and stores the tile's **top-left square**, which is the square a mapgen update lands on when that tile is its target.
fn search_location(world: &mut World, speaker: Entity, effect: &Effect) -> Applied {
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
        ..
    } = effect
    else {
        return Applied::CannotAnswer;
    };
    let Some(at) = world.get::<crate::Position>(speaker).copied() else {
        return Applied::CannotAnswer;
    };
    // A furniture search compares the **name** the data writes against the furniture on each square, which `furniture_types` turns into the code a square carries.
    let tile = crate::overmap::tile_of(at.x, at.y);
    let Some((x, y)) =
        crate::overmap::nearest_with_terrain(world, tile, om_terrain, *search_range, *min_distance)
    else {
        return Applied::CannotAnswer;
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
        let Some(square) = find_furniture(world, found, code, *target_max_radius, (at.x, at.y), *target_min_radius) else {
            return Applied::CannotAnswer;
        };
        let holder = crate::dialogue_values::holder_for(world, speaker, scope == "global_val");
        crate::dialogue_values::set(
            world,
            holder,
            name,
            crate::dialogue_values::Value::Position { x: square.0, y: square.1, z: square.2 },
        );
        return Applied::Done;
    }
    let square = if *max_radius > 0 {
        let Some(drawn) = draw_near(world, found, *min_radius, *max_radius) else {
            return Applied::CannotAnswer;
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
    Applied::Done
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
fn give_mission(world: &mut World, speaker: Entity, listener: Option<Entity>, effect: &Effect) -> Applied {
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

fn move_creature(world: &mut World, speaker: Entity, listener: Option<Entity>, effect: &Effect) -> Applied {
    let Effect::Teleport {
        scope,
        name,
        listener: about_listener,
        fail_message,
        success_message,
        ..
    } = effect
    else {
        return Applied::CannotAnswer;
    };
    // The listener form moves the other creature, and without one there is nothing to move.
    let actor = if *about_listener { listener } else { Some(speaker) };
    let Some(actor) = actor else {
        return Applied::CannotAnswer;
    };
    let holder = crate::dialogue_values::holder_for(world, actor, scope == "global_val");
    let Some(crate::dialogue_values::Value::Position { x, y, z }) =
        crate::dialogue_values::value_of(world, holder, name)
    else {
        // Reference: `f_teleport` shows `fail_message` when the target variable holds no square, and the move does not happen.
        show(world, speaker, listener, fail_message.as_ref());
        return Applied::CannotAnswer;
    };
    if let Some(mut position) = world.get_mut::<crate::Position>(actor) {
        position.x = x;
        position.y = y;
        position.z = z;
    }
    // Reference: `f_teleport` shows `success_message` after a move that happened, and an absent one says nothing.
    show(world, speaker, listener, success_message.as_ref());
    Applied::Done
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
    if let Some(mut query) = world.try_query::<(&crate::creature::CreatureKind, &crate::Position)>() {
        for (kind, position) in query.iter(world) {
            if (position.x - anchor.0).abs().max((position.y - anchor.1).abs()) > range {
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
    usize::try_from(at).ok().and_then(|at| candidates.get(at)).cloned()
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
        let drawn = held
            .as_mut()
            .and_then(|dice| copy_a_nearby_monster(world, speaker, target_range.unwrap_or(0), dice));
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
fn run_later(world: &mut World, speaker: Entity, listener: Option<Entity>, effect: &Effect) -> Applied {
    let Effect::Later { effect, after } = effect else {
        return Applied::CannotAnswer;
    };
    let Some(turns) = crate::dialogue_effects::wait_turns(world, speaker, listener, after) else {
        return Applied::CannotAnswer;
    };
    crate::scheduled::schedule_effect(world, turns, (**effect).clone());
    Applied::Done
}

/// Schedules a program list to run after a delay, and answers `Done` because scheduling is the whole effect.
///
/// Reference: `f_run_eocs` with `time_in_future` (`npctalk.cpp:6332-6338`): the duration is turned into a turn by the calendar, the programs are kept, and the turn
/// loop runs them when that turn arrives. A duration the port cannot read is refused rather than run now.
fn run_eoc_later(world: &mut World, speaker: Entity, listener: Option<Entity>, effect: &Effect) -> Applied {
    let Effect::RunEocLater { programs, after } = effect else {
        return Applied::CannotAnswer;
    };
    let Some(turns) = wait_turns(world, speaker, listener, after) else {
        return Applied::CannotAnswer;
    };
    crate::scheduled::schedule_effect(world, turns, Effect::RunEoc(programs.clone()));
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
fn mutate(world: &mut World, speaker: Entity, listener: Option<Entity>, effect: &Effect) -> Applied {
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
    let mut rolls = crate::singletons::take_rolls(world).unwrap_or_else(|| cdda_core_types::rng::Rolls::new(0));
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
    let Effect::MonsterAround { monster, scope, name, min_radius, max_radius, .. } = effect else {
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
        crate::dialogue_values::Value::Position { x: at.0, y: at.1, z: at.2 },
    );
    Applied::Done
}

/// Moves a square the dialogue holds, and stores the new one.
///
/// Reference: `f_location_variable_adjust` (`npctalk.cpp:4419-4446`): with `overmap_tile` the x and y moves count **overmap tiles** rather than squares, `z_override`
/// replaces the level instead of moving it, and the answer goes to `output_var` when the data names one and back to the input variable when it does not.
fn adjust_location(world: &mut World, speaker: Entity, listener: Option<Entity>, effect: &Effect) -> Applied {
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
    let Some(holder) = crate::conditions::holder_for_scope(world, speaker, listener, scope) else {
        return Applied::CannotAnswer;
    };
    let Some(crate::dialogue_values::Value::Position { x, y, z }) = crate::dialogue_values::value_of(world, holder, name) else {
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
        if *z_override { as_whole(dz) } else { z + as_whole(dz) },
    );
    let (scope, name) = match output {
        Some((scope, name)) => (scope.clone(), name.clone()),
        None => (scope.clone(), name.clone()),
    };
    let Some(target) = crate::conditions::holder_for_scope(world, speaker, listener, &scope) else {
        return Applied::CannotAnswer;
    };
    crate::dialogue_values::set(
        world,
        target,
        &name,
        crate::dialogue_values::Value::Position { x: moved.0, y: moved.1, z: moved.2 },
    );
    Applied::Done
}

/// Runs programs on the items a creature carries, once per matching item.
///
/// Reference: `f_run_inv_eocs` (`npctalk.cpp:6760-6805`) with the `"all"` mode runs `true_eocs` on every matching item, handing each one to the program through the
/// invocation's context. The port sets the context the data reads - `id` for the item's id and `count` for how many the stack holds - and runs the programs.
fn run_inv_eocs(world: &mut World, speaker: Entity, listener: Option<Entity>, effect: &Effect) -> Applied {
    let Effect::RunInvEocs { search_data, wielded_only, worn_only, random, programs } = effect else {
        return Applied::CannotAnswer;
    };
    let Some(holder) = crate::conditions::holder_for_scope(world, speaker, listener, "u_val") else {
        return Applied::CannotAnswer;
    };
    let mut matches: Vec<(String, i32)> = Vec::new();
    // The names are read **before** the query borrows the world mutably: `name_of` needs `&mut World` and a query's iterator holds it for the loop.
    let mut carried: Vec<(crate::ItemTypeId, i32)> = Vec::new();
    let wielded = if *wielded_only {
        world
            .try_query::<(&crate::creature::WieldedBy, &crate::GroundItem)>()
            .and_then(|mut query| {
                query
                    .iter(world)
                    .find(|(who, _)| who.0 == holder)
                    .map(|(_, item)| (item.kind, item.charges.max(1)))
            })
    } else {
        None
    };
    if *worn_only {
        let mut query = world.query::<(&crate::creature::WornBy, &crate::GroundItem)>();
        for (who, item) in query.iter(world) {
            if who.0 == holder {
                carried.push((item.kind, item.charges.max(1)));
            }
        }
    } else if let Some(held) = wielded {
        carried.push(held);
    } else if !*wielded_only {
        let mut query = world.query::<(&crate::creature::Carried, &crate::GroundItem)>();
        for (who, item) in query.iter(world) {
            if who.0 == holder {
                carried.push((item.kind, item.charges.max(1)));
            }
        }
    }
    for (kind, charges) in carried {
        let Some(name) = crate::item_registry::name_of(world, kind) else {
            continue;
        };
        if !search_data.is_empty() && !search_data.contains(&name) {
            continue;
        }
        matches.push((name, charges));
    }
    if matches.is_empty() {
        return Applied::Done;
    }
    // Reference: the `"random"` mode picks **one** of the matching items with the game's dice; `"all"` runs on every one.
    if *random {
        let Some(picked) = draw_one_of(world, matches.len()) else {
            return Applied::CannotAnswer;
        };
        matches = matches.get(picked).cloned().into_iter().collect();
    }
    for (name, count) in matches {
        let handed = std::collections::BTreeMap::from([
            ("id".to_owned(), name),
            ("count".to_owned(), count.to_string()),
        ]);
        let previous = crate::dialogue_context::put(world, speaker, handed);
        for program in programs {
            let _ = crate::eoc_registry::run_ref(world, speaker, listener, program, 1);
        }
        crate::dialogue_context::restore(world, speaker, previous);
    }
    Applied::Done
}

/// Marks an achievement completed on the character the dialogue speaks for.
///
/// Reference: `f_give_achievment` (`npctalk.cpp:5566-5585`) reports the achievement only while it is pending, which
/// [`crate::achievements::complete`] answers with "was it new".
fn give_achievement(world: &mut World, speaker: Entity, listener: Option<Entity>, effect: &Effect) -> Applied {
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
fn transform_radius(world: &mut World, speaker: Entity, listener: Option<Entity>, effect: &Effect) -> Applied {
    let Effect::TransformRadius { transform, radius, target, listener: theirs } = effect else {
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
    if crate::terrain_types::holder(world).is_none() || crate::terrain_transform::holder(world).is_none() {
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
fn custom_light_level(world: &mut World, speaker: Entity, listener: Option<Entity>, effect: &Effect) -> Applied {
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
fn set_field(world: &mut World, speaker: Entity, listener: Option<Entity>, effect: &Effect) -> Applied {
    let Effect::SetField { field, intensity, radius, listener: theirs } = effect else {
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
fn store_furniture_around(world: &mut World, speaker: Entity, listener: Option<Entity>, effect: &Effect) -> Applied {
    let Effect::FurnitureAround { furniture, scope, name, target_min_radius, target_max_radius, listener: theirs } = effect
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
    crate::dialogue_values::set(world, holder, name, crate::dialogue_values::Value::Position { x, y, z: here.z });
    Applied::Done
}

/// Writes the square of a terrain found around the actor into a variable.
///
/// Reference: `f_location_variable` (`npctalk.cpp:4250-4300`): the walk is `points_in_radius( talker_pos, target_max_radius, 0 )` - z, then y, then x ascending -
/// and the first square whose terrain matches and which is further than `target_min_radius` wins. Nothing found leaves the actor's own square in the variable,
/// because the reference starts the answer there.
fn store_terrain_around(world: &mut World, speaker: Entity, listener: Option<Entity>, effect: &Effect) -> Applied {
    let Effect::TerrainAround { terrain, scope, name, target_min_radius, target_max_radius, listener: theirs } = effect
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
    crate::dialogue_values::set(world, holder, name, crate::dialogue_values::Value::Position { x, y, z: here.z });
    Applied::Done
}

/// Casts a spell the dialogue names.
///
/// Reference: `f_cast_spell` (`npctalk.cpp`): the member is the spell's id, `hit_self` decides whether it lands on the speaker or the listener, and the level is the
/// caster's own level in that spell unless the data forces one. The port looks the prototype up in [`crate::spell_types`] and hands it to `spell_effects::apply`, which
/// is the same table the CLI casts from - so a dialogue cast and a played cast run the same rule.
fn cast_spell(world: &mut World, speaker: Entity, listener: Option<Entity>, effect: &Effect) -> Applied {
    let Effect::CastSpell { spell, hit_self, listener: theirs } = effect else {
        return Applied::CannotAnswer;
    };
    let Some(caster) = actor(speaker, listener, *theirs) else {
        return Applied::CannotAnswer;
    };
    // The data spells the id as a literal, so the reader keeps one - unlike most strings in a dialogue, which may be a variable.
    let named = spell.clone();
    let Some(prototype) = crate::spell_types::find(world, &named) else {
        return Applied::CannotAnswer;
    };
    let level = crate::known_spells::level_of(world, caster, &named);
    // `hit_self` puts the spell's centre on the caster; otherwise it is aimed at the other party, which is where the dialogue's own target stands.
    let target = if *hit_self { Some(caster) } else { listener.or(Some(speaker)) };
    let Some(centre) = target.and_then(|entity| world.get::<crate::Position>(entity).copied()) else {
        return Applied::CannotAnswer;
    };
    // A cast that reaches nobody still happened: the reference spends the moves and casts, and the effect simply finds nothing. Only an effect the port has no rule
    // for is a refusal, because that one did not happen.
    match crate::spell_effects::apply(world, caster, &prototype, level, (centre.x, centre.y, centre.z)) {
        crate::spell_effects::Effective::NotPorted => Applied::CannotAnswer,
        crate::spell_effects::Effective::Done | crate::spell_effects::Effective::NothingAffected => Applied::Done,
    }
}

/// Starts an activity on a creature.
///
/// Reference: `f_assign_activity` (`npctalk.cpp:5456-5460`): `target->assign_activity( activity_id( ... ), to_moves<int>( duration ) )`. The port's activity is the
/// `Doing` component, and its length is in moves like the reference's, so the duration is multiplied by a hundred - the same conversion `turn_cost` uses.
fn assign_activity(world: &mut World, speaker: Entity, listener: Option<Entity>, effect: &Effect) -> Applied {
    let Effect::AssignActivity { activity, duration, listener: theirs } = effect else {
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
    let moves_left = turns.saturating_mul(100).clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32;
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
fn set_talker(world: &mut World, speaker: Entity, listener: Option<Entity>, effect: &Effect) -> Applied {
    let Effect::SetTalker { target, listener: theirs } = effect else {
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
    crate::dialogue_values::set(world, holder, &target.1, crate::dialogue_values::Value::Text(id));
    Applied::Done
}

/// Takes a morale point away from a creature.
///
/// Reference: `f_lose_morale` (`npctalk.cpp:7201-7203`): `d.actor( is_npc )->remove_morale( morale_type( ... ) )`, which erases every point of that type. Types the
/// creature is not feeling are left alone, and losing one it never had is not a failure.
fn lose_morale(world: &mut World, speaker: Entity, listener: Option<Entity>, effect: &Effect) -> Applied {
    let Effect::LoseMorale { kind, listener: theirs } = effect else {
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
fn remove_item_with(world: &mut World, speaker: Entity, listener: Option<Entity>, effect: &Effect) -> Applied {
    let Effect::RemoveItemWith { item, listener: theirs } = effect else {
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
fn place_override(world: &mut World, speaker: Entity, listener: Option<Entity>, effect: &Effect) -> Applied {
    let Effect::PlaceOverride { length, key, place, fallback } = effect else {
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
fn add_wet(world: &mut World, speaker: Entity, listener: Option<Entity>, effect: &Effect) -> Applied {
    let Effect::AddWet { amount, listener: theirs } = effect else {
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
fn turn_cost(world: &mut World, speaker: Entity, listener: Option<Entity>, effect: &Effect) -> Applied {
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
fn remove_active_mission(world: &mut World, speaker: Entity, listener: Option<Entity>, effect: &Effect) -> Applied {
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
fn finish_mission(world: &mut World, speaker: Entity, listener: Option<Entity>, effect: &Effect) -> Applied {
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
fn set_condition(world: &mut World, speaker: Entity, listener: Option<Entity>, effect: &Effect) -> Applied {
    let Effect::SetCondition { name, condition, scope } = effect else {
        return Applied::CannotAnswer;
    };
    let Some(holder) = crate::conditions::holder_for_scope(world, speaker, listener, scope) else {
        return Applied::CannotAnswer;
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
fn alter_timed_events(world: &mut World, speaker: Entity, listener: Option<Entity>, effect: &Effect) -> Applied {
    let Effect::AlterTimedEvents { key, time_in_future } = effect else {
        return Applied::CannotAnswer;
    };
    let Some(named) = crate::conditions::text_of(world, speaker, listener, key) else {
        return Applied::CannotAnswer;
    };
    let Some(duration) = crate::conditions::text_of(world, speaker, listener, time_in_future) else {
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
fn copy_var(world: &mut World, speaker: Entity, listener: Option<Entity>, effect: &Effect) -> Applied {
    let Effect::CopyVar { from_scope, from_name, to_scope, to_name } = effect else {
        return Applied::CannotAnswer;
    };
    let Some(from) = crate::conditions::holder_for_scope(world, speaker, listener, from_scope) else {
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
fn set_string_var(world: &mut World, speaker: Entity, listener: Option<Entity>, effect: &Effect) -> Applied {
    let Effect::SetStringVar { parse_tags, value, scope, name } = effect else {
        return Applied::CannotAnswer;
    };
    let Some(holder) = crate::conditions::holder_for_scope(world, speaker, listener, scope) else {
        return Applied::CannotAnswer;
    };
    let text = match value {
        cdda_content::dialogue::VarValue::One(one) => {
            let Some(text) = crate::conditions::text_of(world, speaker, listener, one) else {
                return Applied::CannotAnswer;
            };
            text
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
    crate::dialogue_values::set(world, holder, name, crate::dialogue_values::Value::Text(text));
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
fn consume_items(world: &mut World, speaker: Entity, listener: Option<Entity>, effect: &Effect) -> Applied {
    let Effect::ConsumeItem { item, count, charges, listener: about_listener, .. } = effect else {
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
        Some(crate::dialogue_values::Value::Position { x, y, z }) => Some(crate::Position { x, y, z }),
        _ => None,
    }
}

/// Puts items on the actor's square.
///
/// Reference: `f_spawn_item` (`npctalk.cpp:3771-3800`): `count` of them, each with `charges` charges when the data gives charges. An item the port's catalogue does
/// not know is refused rather than dropped as something nameless.
fn spawn_items(world: &mut World, speaker: Entity, listener: Option<Entity>, effect: &Effect) -> Applied {
    let Effect::SpawnItem { item, count, charges, force_equip, use_item_group, loc, .. } = effect else {
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
fn spawn_npcs(world: &mut World, speaker: Entity, listener: Option<Entity>, effect: &Effect) -> Applied {
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
        let Some(around) = draw_near_with_where(world, held.as_mut(), (at.x, at.y, at.z), near, far, &|square| {
            let outside = crate::outdoors::is_outside(world, square);
            (!*outdoor_only || outside) && (!*indoor_only || !outside)
        })
        else {
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

fn spawn_monsters(world: &mut World, speaker: Entity, listener: Option<Entity>, effect: &Effect) -> Applied {
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
        if std::env::var("CDDA_SHOW_SPAWN").is_ok() { println!("spawn refused: no position"); }
        return Applied::CannotAnswer;
    };
    let Some(named) = crate::conditions::text_of(world, speaker, listener, monster) else {
        if std::env::var("CDDA_SHOW_SPAWN").is_ok() { println!("spawn refused: no name"); }
        return Applied::CannotAnswer;
    };
    let Some(name) = pick_type(world, speaker, &named, *group, *target_range) else {
        return Applied::CannotAnswer;
    };
    let Some(kind) = crate::monster_catalog::type_of(world, &name).cloned() else {
        if std::env::var("CDDA_SHOW_SPAWN").is_ok() { println!("spawn refused: unknown type {name}"); }
        return Applied::CannotAnswer;
    };
    let real = draw_count(world, speaker, listener, real_count);
    let imagined = draw_count(world, speaker, listener, hallucination_count);
    let (Some(real), Some(imagined)) = (real, imagined) else {
        if std::env::var("CDDA_SHOW_SPAWN").is_ok() { println!("spawn refused: no count"); }
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
        )
        else {
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
        world.entity_mut(spawn).insert(crate::monster_spawning::Hallucination);
    }
    // The port has no attitudes yet, so friendliness is a **marker** on the creature - one component, no second source of truth - and the attitude rules can
    // read it when they land.
    if friendly {
        world.entity_mut(spawn).insert(crate::friendly::Friendly);
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
fn draw_near(world: &mut World, around: (i32, i32, i32), min_radius: i32, max_radius: i32) -> Option<(i32, i32, i32)> {
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
fn add_var(world: &mut World, speaker: Entity, listener: Option<Entity>, effect: &Effect) -> Applied {
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
                let Some(text) = crate::conditions::text_of(world, speaker, listener, one) else {
                    return Applied::CannotAnswer;
                };
                crate::dialogue_values::Value::Text(text)
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
                let Some(text) = crate::conditions::text_of(world, speaker, listener, choice) else {
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
fn lose_var(world: &mut World, speaker: Entity, listener: Option<Entity>, name: &str, scope: &str) -> Applied {
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
fn run_selector(world: &mut World, speaker: Entity, listener: Option<Entity>, effect: &Effect) -> Applied {
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
fn run_remove_effects(world: &mut World, speaker: Entity, listener: Option<Entity>, effect: &Effect) -> Applied {
    let Effect::RemoveEffects { effects, listener: theirs } = effect else {
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
fn run_mapgen_update(world: &mut World, speaker: Entity, listener: Option<Entity>, effect: &Effect) -> Applied {
    let Effect::MapgenUpdate {
        updates,
        target,
        om_terrain,
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
        crate::dialogue_values::position_of(world, speaker, listener, target)
    } else if let Some(wanted) = om_terrain {
        let Some(at) = here else {
            return Applied::CannotAnswer;
        };
        let Some(name) = crate::conditions::text_of(world, speaker, listener, wanted) else {
            return Applied::CannotAnswer;
        };
        let tile = crate::overmap::tile_of(at.x, at.y);
        crate::overmap::nearest_with_terrain(world, tile, &name, SEARCH_RANGE, 0).map(|(x, y)| {
            (
                x * crate::overmap::SQUARES_PER_OMT,
                y * crate::overmap::SQUARES_PER_OMT,
                at.z,
            )
        })
    } else {
        here.map(|at| (at.x, at.y, at.z))
    };
    let Some(place) = placed else {
        return Applied::CannotAnswer;
    };
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
fn make_sound_effect(world: &mut World, speaker: Entity, listener: Option<Entity>, effect: &Effect) -> Applied {
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
    let said = sound_words(world, speaker, listener, creature, words.as_ref(), snippet.as_ref(), *same_snippet);
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
    let (duration, decay_start) = (times.first().copied().unwrap_or(0), times.get(1).copied().unwrap_or(0));
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
    let value = crate::conditions::value_drawing(world, speaker, listener, expression, true, dice.as_mut());
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
fn store_overmap_location(world: &mut World, speaker: Entity, listener: Option<Entity>, effect: &Effect) -> Applied {
    let Effect::LocationVariableFromOvermap { scope, name, terrain, min_distance, z, listener: theirs } = effect else {
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
    let Some((x, y)) = crate::overmap::nearest_with_terrain(world, origin, &wanted, SEARCH_RANGE, least) else {
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
fn store_location(world: &mut World, speaker: Entity, listener: Option<Entity>, effect: &Effect) -> Applied {
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
fn run_branch(world: &mut World, speaker: Entity, listener: Option<Entity>, effect: &Effect) -> Applied {
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
        cdda_content::dialogue::Condition::Query { default } => Some(match crate::choices::take(world) {
            Some(answer) => answer == 0,
            _ => *default,
        }),
        _ => None,
    };
    let taken = answered.unwrap_or_else(|| crate::conditions::holds_in_store(world, speaker, listener, condition));
    let branch = if taken {
        then
    } else {
        otherwise
    };
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
    let actor = if matches!(recipe, cdda_content::dialogue::StringSource::Variable { .. }) {
        listener.unwrap_or(speaker)
    } else {
        speaker
    };
    if forget {
        crate::recipes_known::forget(world, actor, &named);
    } else {
        crate::recipes_known::learn(world, actor, &named);
    }
    Applied::Done
}

/// Gives a creature an effect whose name may be a variable.
///
/// Reference: `f_add_effect` (`npctalk.cpp:3426-3450`): the name is a `str_or_var`, resolved where the effect is applied, and so is the duration.
fn run_gained_effect(world: &mut World, speaker: Entity, listener: Option<Entity>, effect: &Effect) -> Applied {
    let Effect::AddEffect { effect: named, duration, intensity, listener: theirs } = effect else {
        return Applied::CannotAnswer;
    };
    let Some(text) = crate::conditions::text_of(world, speaker, listener, named) else {
        return Applied::CannotAnswer;
    };
    add_effect(world, speaker, listener, &text, duration, intensity.as_ref(), *theirs)
}

/// Takes an effect off a creature whose name may be a variable.
///
/// Reference: `f_remove_effect` (`npctalk.cpp:8015`): the name is a `str_or_var`, and removing one the creature does not have is not a failure.
fn run_lost_effect(world: &mut World, speaker: Entity, listener: Option<Entity>, effect: &Effect) -> Applied {
    let Effect::RemoveEffect { effect: named, listener: theirs } = effect else {
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
        Effect::RemoveActiveMission { .. } => remove_active_mission(world, speaker, listener, effect),
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
        Effect::GiveAchievement { .. } => give_achievement(world, speaker, listener, effect),
        Effect::RunInvEocs { .. } => run_inv_eocs(world, speaker, listener, effect),
        Effect::LocationVariableAdjust { .. } => adjust_location(world, speaker, listener, effect),
        Effect::MonsterAround { .. } => store_monster_around(world, speaker, effect),
        Effect::Mutation { .. } => mutate(world, speaker, listener, effect),
        Effect::OfferMission { .. } => offer_mission(world, speaker, listener, effect),
        Effect::RunEocLater { .. } => run_eoc_later(world, speaker, listener, effect),
        Effect::Later { .. } => run_later(world, speaker, listener, effect),
        // A sound event is presentation: the port carries it and has no ear to play to. The reference changes nothing about the world when it plays one, so `Done` is
        // the honest answer - and it shares that answer with the other presentation effects below.

        // Audio changes no game state, and the port has no audio: Done, not refused - nothing is missing from the simulation, only from the sound card. A
        // comment-only entry does nothing, which the reference's loader also does with it.
        Effect::SoundEvent { .. } | Effect::PlaySoundEffect { .. } | Effect::Nothing => Applied::Done,
        _ => return None,
    })
}

/// Runs one effect entry.
pub fn run_effect(
    world: &mut World,
    speaker: Entity,
    listener: Option<Entity>,
    effect: &Effect,
) -> Applied {
    if let Some(done) = run_store_effect(world, speaker, listener, effect) { return done; }
    match effect {
        Effect::Math(assignments) => run_math(world, speaker, listener, assignments),
        Effect::LearnRecipe { recipe, forget } => run_learn_recipe(world, speaker, listener, recipe, *forget),
        Effect::Message(text) => { crate::messages::push(world, text); Applied::Done }
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
        Effect::LocationVariableSearch { .. } => search_location(world, speaker, effect),
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
        Effect::LocationVariableFromOvermap { .. } => store_overmap_location(world, speaker, listener, effect),
        Effect::Switch { on, cases } => run_switch(world, speaker, listener, on, cases),
        Effect::If { .. } => run_branch(world, speaker, listener, effect),
        Effect::LearnTrait { trait_name, variant, listener: about_listener } => {
            let Some(creature) = actor(speaker, listener, *about_listener) else {
                return Applied::CannotAnswer;
            };
            crate::traits::add_trait(world, creature, trait_name);
            // Reference: `set_mutation( trait, variant )` keeps the cosmetic choice beside the trait; nothing in the rules reads it yet.
            if let Some(chosen) = variant {
                let Some(name) = crate::conditions::text_of(world, speaker, listener, chosen) else {
                    return Applied::CannotAnswer;
                };
                crate::traits::set_variant(world, creature, trait_name, &name);
            }
            Applied::Done
        }
        Effect::LoseTrait { trait_name, listener: about_listener } => {
            let Some(creature) = actor(speaker, listener, *about_listener) else {
                return Applied::CannotAnswer;
            };
            // Reference: `unset_mutation` on a trait the creature does not have does nothing, and a `lose_trait` that finds
            // nothing to lose has still done its job - so the answer is `Done` either way.
            crate::traits::remove_trait(world, creature, trait_name);
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
fn offer_mission(world: &mut World, _speaker: Entity, listener: Option<Entity>, effect: &Effect) -> Applied {
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
