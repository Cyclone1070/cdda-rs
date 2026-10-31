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
        z,
        ..
    } = effect
    else {
        return Applied::CannotAnswer;
    };
    let Some(at) = world.get::<crate::Position>(speaker).copied() else {
        return Applied::CannotAnswer;
    };
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
fn spawn_monsters(world: &mut World, speaker: Entity, listener: Option<Entity>, effect: &Effect) -> Applied {
    let Effect::SpawnMonster {
        monster,
        real_count,
        hallucination_count,
        min_radius,
        max_radius,
    } = effect
    else {
        return Applied::CannotAnswer;
    };
    let Some(at) = world.get::<crate::Position>(speaker).copied() else {
        if std::env::var("CDDA_SHOW_SPAWN").is_ok() { println!("spawn refused: no position"); }
        return Applied::CannotAnswer;
    };
    let Some(name) = crate::conditions::text_of(world, speaker, listener, monster) else {
        if std::env::var("CDDA_SHOW_SPAWN").is_ok() { println!("spawn refused: no name"); }
        return Applied::CannotAnswer;
    };
    let Some(kind) = crate::monster_catalog::type_of(world, &name).cloned() else {
        if std::env::var("CDDA_SHOW_SPAWN").is_ok() { println!("spawn refused: unknown type {name}"); }
        return Applied::CannotAnswer;
    };
    let real = draw_count(world, real_count);
    let imagined = draw_count(world, hallucination_count);
    let (Some(real), Some(imagined)) = (real, imagined) else {
        if std::env::var("CDDA_SHOW_SPAWN").is_ok() { println!("spawn refused: no count"); }
        return Applied::CannotAnswer;
    };
    let mut held = crate::singletons::take_rolls(world);
    let mut squares = Vec::new();
    for _ in 0..(real + imagined) {
        let Some(around) = draw_near_with(world, held.as_mut(), (at.x, at.y, at.z), *min_radius, *max_radius)
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
    let real = usize::try_from(real).unwrap_or(0);
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
        if made >= real {
            world
                .entity_mut(made_entity)
                .insert(crate::monster_spawning::Hallucination);
        }
    }
    Applied::Done
}

/// Draws how many of something the data asked for.
fn draw_count(world: &mut World, count: &cdda_content::dialogue::CountRange) -> Option<i32> {
    match count {
        cdda_content::dialogue::CountRange::One(one) => Some(*one),
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
    let dice = dice?;
    for _ in 0..25 {
        let x = around.0 + dice.rng(-max_radius, max_radius);
        let y = around.1 + dice.rng(-max_radius, max_radius);
        if crate::square::terrain_at(world, (x, y, around.2)).is_none() {
            continue;
        }
        if (x - around.0).abs().max((y - around.1).abs()) < min_radius {
            continue;
        }
        return Some((x, y, around.2));
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
            effect: name.clone(),
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
        update,
        target,
        om_terrain,
    } = effect
    else {
        return Applied::CannotAnswer;
    };
    let Some(name) = crate::conditions::text_of(world, speaker, listener, update) else {
        return Applied::CannotAnswer;
    };
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
    let Some(_) = crate::mapgen_update::apply(world, &name, place) else {
        return Applied::CannotAnswer;
    };
    // An update may place fields as well as terrain, and an update that does only one is still an update.
    match crate::mapgen_update::place_fields(world, &name, place) {
        Some(_) => Applied::Done,
        None => Applied::CannotAnswer,
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
    let branch = if crate::conditions::holds_in_store(world, speaker, listener, condition) {
        then
    } else {
        otherwise
    };
    run_effects(world, speaker, listener, branch)
}

/// Runs one effect entry.
pub fn run_effect(
    world: &mut World,
    speaker: Entity,
    listener: Option<Entity>,
    effect: &Effect,
) -> Applied {
    match effect {
        Effect::Math(assignments) => run_math(world, speaker, listener, assignments),
        Effect::Message(text) => {
            crate::messages::push(world, text);
            Applied::Done
        }
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
        Effect::Switch { on, cases } => run_switch(world, speaker, listener, on, cases),
        Effect::If { .. } => run_branch(world, speaker, listener, effect),
        Effect::AddEffect {
            effect,
            duration,
            intensity,
            listener: about_listener,
        } => add_effect(world, speaker, listener, effect, duration, intensity.as_ref(), *about_listener),
        Effect::RemoveEffect { effect, listener: about_listener } => {
            let Some(creature) = actor(speaker, listener, *about_listener) else {
                return Applied::CannotAnswer;
            };
            crate::applied_effects::remove(world, creature, effect);
            Applied::Done
        }
        Effect::LearnTrait { trait_name, listener: about_listener } => {
            let Some(creature) = actor(speaker, listener, *about_listener) else {
                return Applied::CannotAnswer;
            };
            crate::traits::add_trait(world, creature, trait_name);
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
        // A named program needs the registry and a depth, which the program runner (`eoc_registry::run_program`) supplies. Reaching
        // here means a list ran a program without one, which is a caller mistake, not a rule - so it refuses rather than guessing.
        Effect::RunEoc(_) => Applied::CannotAnswer,
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
