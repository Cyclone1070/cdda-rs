//! Whether a dialogue condition holds for a creature.
//!
//! Reference: the callable at the heart of `conditional_t` (`condition.cpp:2660-2760`), which answers a question about one
//! dialoguer and one talker. The port answers from the store: what a creature is wearing, what it has, where it is.

use bevy_ecs::prelude::*;

use cdda_content::dialogue::{Condition, Expression, Operator, StringSource};


/// Whether a condition holds for a creature talking to nobody in particular.
///
/// Reference: a `const_dialogue` always has two actors - the player (`u`) and an NPC (`n`) - and many conditions can be asked
/// of either. A question about the NPC asked with no NPC present is unanswerable, which is false rather than zero.
#[must_use]
pub fn holds(world: &World, creature: Entity, condition: &Condition) -> bool {
    holds_with(world, creature, None, condition)
}

/// Whether a condition holds, in a conversation with someone.
///
/// The two dialoguers are the reference's `actor( false )` and `actor( true )`: `u_val` is the speaker's, `n_val` and
/// `npc_val` are the listener's.
#[must_use]
/// Whether the actor wears anything carrying a flag.
fn worn_flag(world: &World, speaker: Entity, listener: Option<Entity>, condition: &Condition) -> bool {
    let Condition::HasWornWithFlag {
        flag,
        listener: about_listener,
    } = condition
    else {
        return false;
    };
    actor_for(speaker, listener, *about_listener)
        .is_some_and(|actor| crate::worn_flags::worn_with_flag(world, actor, flag))
}

/// Whether the actor carries a flag itself.
fn creature_flag(world: &World, speaker: Entity, listener: Option<Entity>, condition: &Condition) -> bool {
    let Condition::HasFlag {
        flag,
        listener: about_listener,
    } = condition
    else {
        return false;
    };
    actor_for(speaker, listener, *about_listener)
        .is_some_and(|actor| crate::creature_flags::has_flag(world, actor, flag))
}

/// A character's level in a named skill, which is what `skill( 'bashing' )` asks.
///
/// Reference: `skill_eval` (`math_parser_diag.cpp:961-965`): the answer is the character's level, and an unpractised skill is level **0** rather than unknown.
fn answer_skill(
    world: &World,
    speaker: Entity,
    listener: Option<Entity>,
    name: &str,
    arguments: &[Expression],
) -> Option<f64> {
    // Reference: the scope prefix is optional in the data, so `skill`, `u_skill` and `npc_skill` are the same question about three different actors
    // (`math_parser_diag.cpp:961`).
    if !matches!(name, "skill" | "u_skill" | "npc_skill") {
        return None;
    }
    let [Expression::Text(skill)] = arguments else {
        return None;
    };
    let actor = if name == "npc_skill" {
        listener?
    } else {
        speaker
    };
    Some(f64::from(crate::skills_of::level_of(world, actor, skill)))
}

/// Whether the actor is standing under the sky.
///
/// Reference: `is_creature_outside` (`weather.cpp:76-82`): the actor's square is outside **and** the level is at or above street level - a creature below ground
/// is under rock however open the square above it is.
fn is_outside(world: &World, speaker: Entity, listener: Option<Entity>, condition: &Condition) -> bool {
    let Condition::IsOutside {
        listener: about_listener,
    } = condition
    else {
        return false;
    };
    let Some(actor) = actor_for(speaker, listener, *about_listener) else {
        return false;
    };
    let Some(at) = world.get::<crate::Position>(actor).copied() else {
        return false;
    };
    at.z >= 0 && crate::outdoors::is_outside(world, (at.x, at.y, at.z))
}

/// Whether a named overmap terrain is within a range of the actor.
///
/// Reference: `f_near_om_location` (`condition.cpp:1074`): every overmap tile within `range` is compared against the name, and the guard holds on the first
/// match. `oter_no_dir_or_connections` is why the comparison drops the direction - a tile spelled `road_north` answers for `road`.
fn near_om_location(world: &World, speaker: Entity, listener: Option<Entity>, condition: &Condition) -> bool {
    let Condition::NearOmLocation {
        location,
        range,
        listener: about_listener,
    } = condition
    else {
        return false;
    };
    let Some(actor) = actor_for(speaker, listener, *about_listener) else {
        return false;
    };
    let (Some(wanted), Some(at)) = (
        text_of(world, speaker, listener, location),
        world.get::<crate::Position>(actor).copied(),
    ) else {
        return false;
    };
    let tile = crate::overmap::tile_of(at.x, at.y);
    crate::overmap::tiles(world).into_iter().any(|(tile_at, terrain)| {
        let step = (tile_at.0 - tile.0).abs().max((tile_at.1 - tile.1).abs());
        step <= *range && (terrain == wanted || crate::overmap::base_terrain(&terrain) == wanted)
    })
}

/// Whether a condition holds, with a listener to ask about.
pub fn holds_with(
    world: &World,
    speaker: Entity,
    listener: Option<Entity>,
    condition: &Condition,
) -> bool {
    holds_drawing(world, speaker, listener, condition, &mut None)
}


/// Whether a creature stands on an overmap tile of a named terrain.
///
/// Reference: `f_at_om_location` (`condition.cpp:1023-1046`): the tile of the actor's position, its terrain id with direction and connections
/// stripped, compared to the string the data writes.
fn at_om_location(
    world: &World,
    speaker: Entity,
    listener: Option<Entity>,
    place: &cdda_content::dialogue::StringSource,
    about_listener: bool,
) -> bool {
    let Some(creature) = actor_for(speaker, listener, about_listener) else {
        return false;
    };
    let Some(name) = text_of(world, speaker, listener, place) else {
        return false;
    };
    let Some(position) = world.get::<crate::Position>(creature) else {
        return false;
    };
    let tile = crate::overmap::tile_of(position.x, position.y);
    crate::overmap::terrain_at(world, tile)
        .is_some_and(|terrain| crate::overmap::base_terrain(terrain) == name)
}



/// Whether the program a condition names holds.
///
/// Reference: `f_test_eoc` (`condition.cpp`): using a program as a condition means asking its own condition, and running past the depth limit
/// answers no rather than looping.
fn tests_program(
    world: &World,
    speaker: Entity,
    listener: Option<Entity>,
    id: &cdda_content::dialogue::StringSource,
    draws: &mut Option<&mut cdda_core_types::rng::Rolls>,
    depth: usize,
) -> bool {
    if depth >= TEST_EOC_DEPTH_LIMIT {
        return false;
    }
    let Some(name) = text_of(world, speaker, listener, id) else {
        return false;
    };
    let Some(program) = crate::eoc_registry::program_of(world, &name) else {
        return false;
    };
    let Some(inner) = program.condition.clone() else {
        return false;
    };
    holds_within(world, speaker, listener, &inner, draws, depth + 1)
}

/// Whether a creature has the bionic a condition names, reading the name as a variable when it is not one.
///
/// Reference: `f_has_bionics` (`condition.cpp:943`) reads its member with `get_str_or_var(…, true)`, so a member may name a bionic directly or
/// name a dialogue variable holding one - the port tries the literal first and the variable second.
fn has_bionic(
    world: &World,
    speaker: Entity,
    listener: Option<Entity>,
    bionic: &cdda_content::dialogue::StringSource,
    about_listener: bool,
) -> bool {
    let Some(creature) = actor_for(speaker, listener, about_listener) else {
        return false;
    };
    let Some(name) = text_of(world, speaker, listener, bionic) else {
        return false;
    };
    if crate::bionics::has(world, creature, &name) {
        return true;
    }
    crate::dialogue_values::text_of(world, creature, &name)
        .is_some_and(|bound| crate::bionics::has(world, creature, &bound))
}

/// Whether every name a program states is set in the call's context.
///
/// Reference: `f_expects_vars` (`condition.cpp:1111-1131`), which answers false when any named value is missing - it is how a program
/// declares what its caller must hand it.
fn expects_vars(
    world: &World,
    speaker: Entity,
    listener: Option<Entity>,
    names: &[cdda_content::dialogue::StringSource],
) -> bool {
    names.iter().all(|name| {
        text_of(world, speaker, listener, name)
            .is_some_and(|name| crate::dialogue_context::knows(world, speaker, &name))
    })
}

/// Whether the date is in a named season.
///
/// Reference: `f_is_season` (`condition.cpp:1250-1260`), which compares `season_of_year( calendar::turn )` against the string the data
/// writes - so a misspelt name answers false rather than failing to read.
fn is_season(
    world: &World,
    speaker: Entity,
    listener: Option<Entity>,
    season: &cdda_content::dialogue::StringSource,
) -> bool {
    let Some(name) = text_of(world, speaker, listener, season) else {
        return false;
    };
    crate::calendar::season_of_year(world).is_some_and(|now| now.name() == name)
}

/// Whether one of the two chance guards rolls true.
///
/// Reference: `one_in_chance` (`condition.cpp:1760-1766` -> `one_in`, `src/rng.cpp:196-199`) and `x_in_y_chance`
/// (`condition.cpp:1801-1810` -> `x_in_y`, `src/rng.cpp:206-209`). Both need the store's generator; a caller without one gets `false`.
fn rolls_against(
    world: &World,
    speaker: Entity,
    listener: Option<Entity>,
    condition: &Condition,
    draws: &mut Option<&mut cdda_core_types::rng::Rolls>,
) -> bool {
    let Some(dice) = draws.as_deref_mut() else {
        return false;
    };
    match condition {
        Condition::OneInChance(chance) => {
            let Some(n) = value_drawing(world, speaker, listener, chance, false, None) else {
                return false;
            };
            #[allow(clippy::cast_possible_truncation)]
            let chance = n.round() as i32;
            dice.one_in(chance)
        }
        Condition::XInYChance { x, y } => {
            let (Some(x), Some(y)) = (
                value_drawing(world, speaker, listener, x, false, None),
                value_drawing(world, speaker, listener, y, false, None),
            ) else {
                return false;
            };
            dice.x_in_y(x, y)
        }
        _ => false,
    }
}

/// Whether a creature carries enough of an item, in stacks and in charges.
///
/// Reference: `f_has_items` (`condition.cpp:836-865`) - a requirement that states `charges` asks about charges, one that states `count`
/// asks about stacks, and one that states both must satisfy both.
fn carries_enough(
    world: &World,
    speaker: Entity,
    listener: Option<Entity>,
    item: &cdda_content::dialogue::StringSource,
    count: Option<i32>,
    charges: Option<i32>,
) -> bool {
    let Some(name) = text_of(world, speaker, listener, item) else {
        return false;
    };
    let (stacks, total_charges) = crate::inventory::count_of(world, speaker, &name);
    let enough_charges = charges.is_none_or(|wanted| total_charges >= wanted);
    let enough_count = count.is_none_or(|wanted| stacks >= wanted);
    enough_charges && enough_count
}

/// Which creature a guard asks about, or nothing when it asks about a listener that is not there.
fn actor_for(speaker: Entity, listener: Option<Entity>, about_listener: bool) -> Option<Entity> {
    if about_listener { listener } else { Some(speaker) }
}

/// Whether any name in a list answers yes, resolving each name as the data wrote it.
fn any_named(
    world: &World,
    speaker: Entity,
    listener: Option<Entity>,
    names: &[cdda_content::dialogue::StringSource],
    mut holds: impl FnMut(String) -> bool,
) -> bool {
    names.iter().any(|wanted| {
        text_of(world, speaker, listener, wanted).is_some_and(&mut holds)
    })
}

/// Whether a creature carries at least one of an item, by stack or by charge.
///
/// Reference: `f_has_item` (`condition.cpp:826-834`) - `charges_of( id ) > 0 || has_amount( id, 1 )` - so a stack of a charge item
/// counts however few charges are left.
fn carries_one_of(
    world: &World,
    speaker: Entity,
    listener: Option<Entity>,
    item: &cdda_content::dialogue::StringSource,
    about_listener: bool,
) -> bool {
    let creature = if about_listener {
        match listener {
            Some(creature) => creature,
            None => return false,
        }
    } else {
        speaker
    };
    let Some(name) = text_of(world, speaker, listener, item) else {
        return false;
    };
    let (stacks, charges) = crate::inventory::count_of(world, creature, &name);
    stacks > 0 || charges > 0
}

/// Whether a condition holds, taking the store's dice out for the length of the answer.
///
/// This is the entry point for anything holding `&mut World` - the program runner and the effect runner. It exists so that a guard
/// like `one_in_chance` draws from the game's own generator instead of being refused, and it puts the generator back whatever the
/// answer was, so a caller never sees it left behind.
pub fn holds_in_store(
    world: &mut World,
    speaker: Entity,
    listener: Option<Entity>,
    condition: &Condition,
) -> bool {
    let mut dice = crate::singletons::take_rolls(world);
    let held = holds_drawing(world, speaker, listener, condition, &mut dice.as_mut());
    if let Some(dice) = dice {
        crate::singletons::put_rolls(world, dice);
    }
    held
}

/// Whether a condition holds, with dice available for the two guards that roll.
///
/// Reference: `one_in_chance` and `x_in_y_chance` (`condition.cpp:1760-1810`) draw a die, so they are answerable only where the
/// store's generator can be borrowed - the same rule the effect side follows for `rng` and `rand`. A caller with no dice gets `false`
/// rather than an invented answer.
pub fn holds_drawing(
    world: &World,
    speaker: Entity,
    listener: Option<Entity>,
    condition: &Condition,
    draws: &mut Option<&mut cdda_core_types::rng::Rolls>,
) -> bool {
    holds_within(world, speaker, listener, condition, draws, 0)
}

/// How deep `test_eoc` may follow one program's condition into another's.
///
/// Reference: `f_test_eoc` has no guard (`condition.cpp:1949-1961`), and a cycle in the data would recurse until the stack ran out.
/// The port follows at most this far and refuses beyond, which is the same deliberate divergence as the program runner's own limit.
pub const TEST_EOC_DEPTH_LIMIT: usize = 16;

/// The body of [`holds_drawing`], with the depth `test_eoc` has reached.
fn holds_within(
    world: &World,
    speaker: Entity,
    listener: Option<Entity>,
    condition: &Condition,
    draws: &mut Option<&mut cdda_core_types::rng::Rolls>,
    depth: usize,
) -> bool {
    match condition {
        Condition::HasBionics {
            bionic,
            listener: about_listener,
        } => has_bionic(world, speaker, listener, bionic, *about_listener),
        Condition::TestEoc { id } => tests_program(world, speaker, listener, id, draws, depth),
        Condition::OneInChance(_) | Condition::XInYChance { .. } => {
            rolls_against(world, speaker, listener, condition, draws)
        }
        Condition::AtOmLocation { place, listener: about_listener } => {
            at_om_location(world, speaker, listener, place, *about_listener)
        }
        Condition::ExpectsVars { names } => expects_vars(world, speaker, listener, names),
        Condition::IsSeason { season } => {
            is_season(world, speaker, listener, season)
        }
        Condition::NearOmLocation { .. } => near_om_location(world, speaker, listener, condition),
        Condition::IsOutside { .. } => is_outside(world, speaker, listener, condition),
        Condition::HasWornWithFlag { .. } => worn_flag(world, speaker, listener, condition),
        Condition::HasFlag { .. } => creature_flag(world, speaker, listener, condition),
        Condition::CanSee { listener: about_listener } => actor_for(speaker, listener, *about_listener)
            .is_some_and(|actor| crate::seeing::can_see(world, actor)),
        Condition::Simple { name } => match name.as_str() {
            // Reference: `has_beta` asks whether a listener exists at all (`d.has_beta`).
            "has_beta" => listener.is_some(),
            // Reference: `u_is_avatar` / `u_is_character` ask what kind of talker the speaker is. The port knows one character
            // kind, so both answer "is this the entity the player drives".
            "u_is_avatar" | "u_is_character" => crate::missions::avatar(world) == Some(speaker),
            _ => false,
        },
        Condition::IsWeather { wanted } => crate::weather::id_of(world)
            .is_some_and(|happening| wanted.contains(&happening)),
        Condition::HasMission { mission } => {
            let holder = crate::missions::question_holder(world, speaker);
            crate::missions::has_mission(world, holder, mission)
        }
        // Reference: `f_u_know_recipe` (`condition.cpp:2034-2042`) - a membership test on the player's learned recipes.
        Condition::KnowsRecipe { recipe } => crate::recipes_known::knows(world, speaker, recipe),
        Condition::HasTrait {
            trait_name,
            listener: about_listener,
        } => match actor_for(speaker, listener, *about_listener) {
            Some(creature) => crate::traits::has_trait(world, creature, trait_name),
            None => false,
        },
        Condition::HasEffect { effect, intensity } => crate::applied_effects::intensity_of(world, speaker, effect)
            .is_some_and(|worn| *intensity <= worn),
        Condition::And(conditions) => {
            conditions_with_dice(world, speaker, listener, conditions, draws, true)
        }
        Condition::Or(conditions) => {
            conditions_with_dice(world, speaker, listener, conditions, draws, false)
        }
        Condition::Not(inner) => !holds_drawing(world, speaker, listener, inner, draws),
        Condition::HasItems {
            item,
            count,
            charges,
        } => carries_enough(world, speaker, listener, item, *count, *charges),
        Condition::CompareString(sources) => {
            let mut seen: Vec<String> = Vec::with_capacity(sources.len());
            for source in sources {
                let Some(text) = text_of(world, speaker, listener, source) else {
                    return false;
                };
                if seen.contains(&text) {
                    return true;
                }
                seen.push(text);
            }
            false
        }
        Condition::HasAnyTrait {
            traits,
            listener: about_listener,
        } => {
            let Some(creature) = actor_for(speaker, listener, *about_listener) else {
                return false;
            };
            let held = crate::traits::traits_held(world, creature);
            any_named(world, speaker, listener, traits, |name| held.contains(&name))
        }
        Condition::HasAnyEffect {
            effects,
            listener: about_listener,
        } => {
            let Some(creature) = actor_for(speaker, listener, *about_listener) else {
                return false;
            };
            any_named(world, speaker, listener, effects, |name| {
                crate::applied_effects::has(world, creature, &name)
            })
        }
        Condition::HasItem { item, listener: about_listener } => {
            carries_one_of(world, speaker, listener, item, *about_listener)
        }
        Condition::Math(expression) => matches!(
            value_drawing(world, speaker, listener, expression, false, draws.as_deref_mut()),
            Some(result) if result != 0.0
        ),
    }
}

/// What an expression works out to, or nothing when something in it cannot be answered.
///
/// Nothing propagates through arithmetic rather than turning into zero: an expression whose variable nobody set has no
/// value, and the reference's null `diag_value` is the same refusal.
#[must_use]
pub fn value(
    world: &World,
    speaker: Entity,
    listener: Option<Entity>,
    expression: &Expression,
) -> Option<f64> {
    value_with(world, speaker, listener, expression, false)
}

/// What a special function had to say about a call.
enum Answer {
    /// It answered.
    Value(f64),
    /// It recognised its own name and could not work out an answer, which is a refusal rather than a zero.
    Refused,
    /// The name is not one of these, so the caller should look in the ordinary function table.
    NotMine,
}

/// The functions that are neither arithmetic nor a variable lookup: `rand`, `rng`, and `time`.
/// One of the four opinion numbers, read from the creature the name's prefix points at.
///
/// Reference: `npc_value_eval` and its three siblings (`math_parser_diag.cpp:1408-1440`): the name's scope prefix says whose opinion it is (`n_` the NPC, `u_`
/// the player), and an actor that does not exist answers **zero** rather than failing.
fn answer_opinion(world: &World, speaker: Entity, listener: Option<Entity>, name: &str) -> Option<Answer> {
    let (which, about_listener) = match name {
        "n_npc_value" | "npc_value" => (crate::npc_opinion::Feeling::Value, true),
        "n_npc_anger" | "npc_anger" => (crate::npc_opinion::Feeling::Anger, true),
        "n_npc_trust" | "npc_trust" => (crate::npc_opinion::Feeling::Trust, true),
        "n_npc_fear" | "npc_fear" => (crate::npc_opinion::Feeling::Fear, true),
        "u_npc_value" => (crate::npc_opinion::Feeling::Value, false),
        "u_npc_anger" => (crate::npc_opinion::Feeling::Anger, false),
        "u_npc_trust" => (crate::npc_opinion::Feeling::Trust, false),
        "u_npc_fear" => (crate::npc_opinion::Feeling::Fear, false),
        _ => return None,
    };
    // Nobody to ask answers zero rather than nothing: that is what lets a program check an opinion in a dialogue that has no listener at all.
    let actor = actor_for(speaker, listener, about_listener);
    let feeling = actor.map_or(0, |actor| crate::npc_opinion::feeling_of(world, actor, which));
    Some(Answer::Value(f64::from(feeling)))
}

/// The unit a call asked its answer in, when it named one.
fn unit_of(keywords: &[(String, Expression)]) -> Option<&str> {
    keywords.iter().find_map(|(name, value)| match (name.as_str(), value) {
        ("unit", Expression::Text(unit)) => Some(unit.as_str()),
        _ => None,
    })
}

/// How many turns one of a named span is worth.
///
/// Reference: the reference's own clock, one turn to the second, so an hour is 3,600 turns and a day is [`crate::calendar::TURNS_PER_DAY`]. A span the port has
/// no number for answers nothing rather than a wrong scale.
fn unit_turns(unit: &str) -> Option<f64> {
    let per_day = crate::calendar::TURNS_PER_DAY;
    let turns = match unit {
        "turn" | "turns" | "second" | "seconds" => 1,
        "minute" | "minutes" => per_day / (24 * 60),
        "hour" | "hours" => per_day / 24,
        "day" | "days" => per_day,
        "week" | "weeks" => per_day * 7,
        _ => return None,
    };
    #[allow(clippy::cast_precision_loss)]
    let turns = turns as f64;
    Some(turns)
}

/// A call's own arguments: the positional ones, and the named ones.
struct Call<'a> {
    /// The arguments, in the order the data writes them.
    arguments: &'a [Expression],
    /// The named arguments, as `'unit': 'hour'`.
    keywords: &'a [(String, Expression)],
}

/// The functions that need dice, answered while the caller holds them.
///
/// Reference: `rand( n )` and `rng( low, high )` (`math_parser_func.h:57-60`), which draw from the game's own generator rather than from a private one.
fn drawing_function(
    world: &World,
    speaker: Entity,
    listener: Option<Entity>,
    name: &str,
    arguments: &[Expression],
    unset_is_zero: bool,
    dice: &mut cdda_core_types::rng::Rolls,
) -> Answer {
    match (name, arguments) {
        ("rand", [argument]) => {
            let Some(n) = value_drawing(world, speaker, listener, argument, unset_is_zero, None) else {
                return Answer::Refused;
            };
            #[allow(clippy::cast_possible_truncation)]
            let high = n.round() as i32;
            Answer::Value(f64::from(dice.rng(0, high)))
        }
        ("rng", [low, high]) => {
            let (Some(low), Some(high)) = (
                value_drawing(world, speaker, listener, low, unset_is_zero, None),
                value_drawing(world, speaker, listener, high, unset_is_zero, None),
            ) else {
                return Answer::Refused;
            };
            #[allow(clippy::cast_possible_truncation)]
            let (low, high) = (low.round() as i32, high.round() as i32);
            Answer::Value(f64::from(dice.rng(low, high)))
        }
        _ => Answer::NotMine,
    }
}

/// How far apart two points are, named as `distance('u', 'npc')` or as stored locations.
///
/// Reference: `distance_eval` (`math_parser_diag.cpp:185-201`) answers `rl_dist`, which is `square_dist` unless the game's circular-distance option is on: the
/// **largest** axis distance, so a diagonal step is one square and one storey up is one step too (`line.h:190-199`).
fn distance_between(
    world: &World,
    speaker: Entity,
    listener: Option<Entity>,
    first: &Expression,
    second: &Expression,
) -> Answer {
    let Some(one) = point_of(world, speaker, listener, first) else {
        return Answer::Refused;
    };
    let Some(other) = point_of(world, speaker, listener, second) else {
        return Answer::Refused;
    };
    let (dx, dy, dz) = (
        (one.0 - other.0).abs(),
        (one.1 - other.1).abs(),
        (one.2 - other.2).abs(),
    );
    Answer::Value(f64::from(dx.max(dy).max(dz)))
}

/// How many monsters stand within `radius` squares of a point, of the types the list names (all types when it is empty).
///
/// Reference: `_filter_monster` (`math_parser_diag.cpp:781-794`): a creature counts when it **is a monster**, when its type is one the arguments name, and
/// when `radius >= rl_dist(...)` - so the radius is a Chebyshev one, like `distance`, not a walked path.
/// How much energy the body has stored, in kilocalories.
///
/// Reference: `calories_eval` (`math_parser_diag.cpp:1753`) answers `get_stored_kcal()`, which is the port's `stored_calories` - thousandths of a kilocalorie -
/// divided by a thousand. The `percent` form needs the character's healthy weight, which the port has no rule for, so it is refused rather than answered as if
/// two thousand were right for every body.
fn answer_calories(
    world: &World,
    speaker: Entity,
    listener: Option<Entity>,
    name: &str,
    keywords: &[(String, Expression)],
) -> Answer {
    if keyword_text(world, speaker, listener, keywords, "format").is_some_and(|format| format != "raw") {
        return Answer::Refused;
    }
    let creature = match name {
        "npc_calories" => listener.unwrap_or(speaker),
        _ => speaker,
    };
    let Some(body) = crate::body::body_of(world, creature) else {
        return Answer::Refused;
    };
    let kilocalories = i32::try_from(body.stored_calories / 1000).unwrap_or(i32::MAX);
    Answer::Value(f64::from(kilocalories))
}

/// The health **stat**, which is not the creature's hit points.
///
/// Reference: `health_eval` (`math_parser_diag.cpp:150-155`) answers `get_health()`, a number from `-200` to `200` the reference keeps apart from `get_hp()`: a
/// character at full hit points can be at `-200` health, and the dialogue asks about the stat.
fn answer_health(world: &World, speaker: Entity, listener: Option<Entity>, name: &str) -> Answer {
    let creature = match name {
        "npc_health" => listener.unwrap_or(speaker),
        _ => speaker,
    };
    let health = crate::body::body_of(world, creature).map_or(0, |body| body.health);
    Answer::Value(f64::from(health))
}

/// A creature's hit points: what is left for `u_hp`, and the ceiling for `hp_max`.
///
/// Reference: `hp_eval` and `hp_max_eval` (`math_parser_diag.cpp:1714`) both take a body part; the port has no per-part hit points, so both answer the whole
/// creature, and the two numbers stay apart because a health bar needs both.
fn answer_hit_points(world: &World, speaker: Entity, listener: Option<Entity>, name: &str) -> Answer {
    let Some(actor) = actor_for(speaker, listener, matches!(name, "npc_hp" | "npc_hp_max")) else {
        return Answer::Refused;
    };
    let Some(health) = world.get::<crate::creature::Health>(actor) else {
        return Answer::Value(0.0);
    };
    let ceiling = matches!(name, "hp_max" | "u_hp_max" | "npc_hp_max");
    Answer::Value(f64::from(if ceiling { health.max_hp } else { health.hp }))
}

/// Whether the actor carries a flag, as a number.
///
/// Reference: `has_flag_eval` (`math_parser_diag.cpp:1707-1717`): `actor->has_flag( flag )`, which is one or zero because the data **adds** these together
/// (`u_has_flag(_flag1) + u_has_flag(_flag2) + ...`). The flag name may be written as a variable, which is why the argument is evaluated rather than matched.
fn answer_has_flag(
    world: &World,
    speaker: Entity,
    listener: Option<Entity>,
    name: &str,
    arguments: &[Expression],
) -> Answer {
    let [argument] = arguments else {
        return Answer::Refused;
    };
    let Some(flag) = expression_text(world, speaker, listener, argument) else {
        return Answer::Refused;
    };
    let Some(actor) = actor_for(speaker, listener, name == "npc_has_flag") else {
        return Answer::Refused;
    };
    Answer::Value(if crate::creature_flags::has_flag(world, actor, &flag) {
        1.0
    } else {
        0.0
    })
}

/// How strong a field is on the actor's square.
///
/// Reference: `field_strength_eval` (`math_parser_diag.cpp:330-355`): the intensity of the named field where the actor stands, and **zero** when that square has
/// none of it - the reference's `fp ? fp->get_field_intensity() : 0`. A stored `location` is refused because the port has no stored point to read from yet.
fn answer_field_strength(
    world: &World,
    speaker: Entity,
    listener: Option<Entity>,
    name: &str,
    arguments: &[Expression],
    keywords: &[(String, Expression)],
) -> Answer {
    if keywords.iter().any(|(name, _)| name == "location") {
        return Answer::Refused;
    }
    let [Expression::Text(field)] = arguments else {
        return Answer::Refused;
    };
    let actor = match name {
        "npc_field_strength" => listener.unwrap_or(speaker),
        _ => speaker,
    };
    let Some(here) = world.get::<crate::Position>(actor).copied() else {
        return Answer::Refused;
    };
    let Some(identity) = crate::field_registry::identity_of(world, field) else {
        // A field the world does not know cannot be on the square, so "none of it" is the honest answer rather than a refusal.
        return Answer::Value(0.0);
    };
    Answer::Value(f64::from(crate::field_strength_at(
        world,
        (here.x, here.y, here.z),
        identity,
    )))
}

/// How much of one vitamin a character holds.
///
/// Reference: `vitamin_eval` (`math_parser_diag.cpp:1584-1592`) answers `vitamin_get`, which is the stored amount and **zero** for a vitamin the body has none of
/// (`consumption.cpp:715-723`) - not a refusal, because "none of it" is a real answer.
fn answer_vitamin(world: &World, speaker: Entity, listener: Option<Entity>, name: &str, arguments: &[Expression]) -> Option<f64> {
    let [Expression::Text(vitamin)] = arguments else {
        return None;
    };
    let creature = match name {
        "npc_vitamin" => listener?,
        _ => speaker,
    };
    let amount = crate::body::body_of(world, creature)
        .and_then(|body| body.vitamins.get(vitamin).copied())
        .unwrap_or(0);
    Some(f64::from(amount))
}

/// How much pain a creature is in, for `u_pain` and its listener spelling.
///
/// Reference: `pain_eval` (`math_parser_diag.cpp:869-882`): the actor's `pain_cur`, and `type: 'perceived'` asks for the pain as the character feels it. The port
/// has no masking rule yet, so that form is refused rather than answered with the raw number under another name.
fn answer_pain(
    world: &World,
    speaker: Entity,
    listener: Option<Entity>,
    name: &str,
    keywords: &[(String, Expression)],
) -> Answer {
    if keywords.iter().any(|(name, _)| name == "type") {
        return Answer::Refused;
    }
    let creature = match name {
        "npc_pain" => listener.unwrap_or(speaker),
        _ => speaker,
    };
    let pain = world
        .get::<crate::creature::Pain>(creature)
        .copied()
        .unwrap_or_default();
    Answer::Value(f64::from(pain.0))
}

/// Counts what is near an actor: monsters and characters answer from different queries and the same shape.
fn answer_nearby(
    world: &World,
    speaker: Entity,
    listener: Option<Entity>,
    name: &str,
    arguments: &[Expression],
    keywords: &[(String, Expression)],
) -> Answer {
    match name {
        "u_characters_nearby" | "npc_characters_nearby" => {
            answer_characters_nearby(world, speaker, listener, name, keywords)
        }
        _ => answer_monsters_nearby(world, speaker, listener, name, arguments, keywords),
    }
}

/// Counts the other characters near an actor, for `u_characters_nearby` and its listener spelling.
///
/// Reference: `_characters_nearby_eval` (`math_parser_diag.cpp:688-734`): the characters within `radius` (default **1000**), filtered by `attitude` (default
/// `any`). The port has no attitude between characters - no faction, no ally - so `any` is the only filter it can answer, and the rest are refused rather than
/// answered as if everyone were a stranger.
fn answer_characters_nearby(
    world: &World,
    speaker: Entity,
    listener: Option<Entity>,
    name: &str,
    keywords: &[(String, Expression)],
) -> Answer {
    if keywords.iter().any(|(name, _)| name == "location") {
        return Answer::Refused;
    }
    if let Some(attitude) = keyword_text(world, speaker, listener, keywords, "attitude")
        && attitude != "any"
    {
        return Answer::Refused;
    }
    let Some(radius) = keyword_number(world, speaker, listener, keywords, "radius", 1000.0) else {
        return Answer::Refused;
    };
    let actor = match name {
        "npc_characters_nearby" => listener.unwrap_or(speaker),
        _ => speaker,
    };
    let Some(here) = world.get::<crate::Position>(actor).copied() else {
        return Answer::Refused;
    };
    #[allow(clippy::cast_possible_truncation)]
    let radius = radius.round() as i32;
    let mut found = 0usize;
    for (person, _) in crate::npcs::npcs(world) {
        if person == actor {
            continue;
        }
        let Some(there) = world.get::<crate::Position>(person).copied() else {
            continue;
        };
        let step = (there.x - here.x)
            .abs()
            .max((there.y - here.y).abs())
            .max((there.z - here.z).abs());
        if step <= radius {
            found += 1;
        }
    }
    Answer::Value(f64::from(u32::try_from(found).unwrap_or(0)))
}

/// Counts the monsters near an actor, for `u_monsters_nearby` and its listener spelling.
///
/// Reference: `_monsters_nearby_eval` (`math_parser_diag.cpp:800-840`): the monsters within `radius` (default **1000**), of any type the arguments name (none
/// means any), filtered by `attitude` (default `hostile`). The port has no friendly monster, so `both` and `hostile` answer the same and `friendly` is refused
/// rather than answered with a number it does not have; `location` is refused too, because counting from a stored point is a rule the port has no square for.
fn answer_monsters_nearby(
    world: &World,
    speaker: Entity,
    listener: Option<Entity>,
    name: &str,
    arguments: &[Expression],
    keywords: &[(String, Expression)],
) -> Answer {
    if keywords.iter().any(|(name, _)| name == "location") {
        return Answer::Refused;
    }
    if let Some(attitude) = keyword_text(world, speaker, listener, keywords, "attitude")
        && attitude != "hostile"
        && attitude != "both"
    {
        return Answer::Refused;
    }
    let Some(radius) = keyword_number(world, speaker, listener, keywords, "radius", 1000.0) else {
        return Answer::Refused;
    };
    let actor = match name {
        "npc_monsters_nearby" => listener.unwrap_or(speaker),
        _ => speaker,
    };
    let Some(here) = world.get::<crate::Position>(actor).copied() else {
        return Answer::Refused;
    };
    let mut wanted = Vec::new();
    for argument in arguments {
        let Some(one) = expression_text(world, speaker, listener, argument) else {
            return Answer::Refused;
        };
        wanted.push(one);
    }
    #[allow(clippy::cast_possible_truncation)]
    let radius = radius.round() as i32;
    Answer::Value(count_monsters_within(
        world,
        (here.x, here.y, here.z),
        radius,
        &wanted,
    ))
}

fn count_monsters_within(
    world: &World,
    at: (i32, i32, i32),
    radius: i32,
    wanted: &[String],
) -> f64 {
    let kinds = crate::singletons::holder::<crate::monster_registry::TheMonsterTypes>(world)
        .and_then(|holder| world.get::<crate::monster_registry::MonsterTypes>(holder));
    let Some(mut query) = world.try_query::<(&crate::creature::CreatureKind, &crate::Position)>() else {
        return 0.0;
    };
    let mut found = 0usize;
    for (kind, position) in query.iter(world) {
        let step = (position.x - at.0)
            .abs()
            .max((position.y - at.1).abs())
            .max((position.z - at.2).abs());
        if step > radius {
            continue;
        }
        if wanted.is_empty() {
            found += 1;
            continue;
        }
        let named = kinds.and_then(|known| known.name_of(kind.0));
        if named.is_some_and(|named| wanted.iter().any(|wanted| wanted == named)) {
            found += 1;
        }
    }
    f64::from(u32::try_from(found).unwrap_or(0))
}

/// The text an expression stands for, when it is text: a quoted string or a variable holding one.
fn expression_text(
    world: &World,
    speaker: Entity,
    listener: Option<Entity>,
    expression: &Expression,
) -> Option<String> {
    match expression {
        Expression::Text(text) => Some(text.clone()),
        Expression::Variable { scope, name } => {
            let holder = holder_for_scope(world, speaker, listener, scope)?;
            match crate::dialogue_values::value_of(world, holder, name) {
                Some(crate::dialogue_values::Value::Text(text)) => Some(text),
                _ => None,
            }
        }
        _ => None,
    }
}

/// The number a keyword holds, or the default when the keyword is absent.
fn keyword_number(
    world: &World,
    speaker: Entity,
    listener: Option<Entity>,
    keywords: &[(String, Expression)],
    name: &str,
    fallback: f64,
) -> Option<f64> {
    for (keyword, expression) in keywords {
        if keyword == name {
            return value(world, speaker, listener, expression);
        }
    }
    Some(fallback)
}

/// The text a keyword holds, when the keyword is there.
fn keyword_text(
    world: &World,
    speaker: Entity,
    listener: Option<Entity>,
    keywords: &[(String, Expression)],
    name: &str,
) -> Option<String> {
    for (keyword, expression) in keywords {
        if keyword == name {
            return expression_text(world, speaker, listener, expression);
        }
    }
    None
}

/// One of `distance`'s two points: a named actor (`'u'`, `'npc'`) or a location a variable holds.
///
/// Reference: `distance_eval`'s `get_pos` (`math_parser_diag.cpp:188-199`), which accepts a string naming an actor and throws for any other string.
fn point_of(
    world: &World,
    speaker: Entity,
    listener: Option<Entity>,
    expression: &Expression,
) -> Option<(i32, i32, i32)> {
    if let Expression::Text(named) = expression {
        let actor = match named.as_str() {
            "u" => speaker,
            "npc" => listener?,
            _ => return None,
        };
        let position = world.get::<crate::Position>(actor)?;
        return Some((position.x, position.y, position.z));
    }
    if let Expression::Variable { scope, name } = expression {
        let holder = holder_for_scope(world, speaker, listener, scope)?;
        if let Some(crate::dialogue_values::Value::Position { x, y, z }) =
            crate::dialogue_values::value_of(world, holder, name)
        {
            return Some((x, y, z));
        }
    }
    None
}

fn special_function(
    world: &World,
    speaker: Entity,
    listener: Option<Entity>,
    name: &str,
    call: &Call<'_>,
    unset_is_zero: bool,
    draws: &mut Option<&mut cdda_core_types::rng::Rolls>,
) -> Answer {
    // Named arguments are passed through: `math_parser` hands each function the names it knows (`kwargs.kwarg_or`) and one that reads none ignores them.
    let (arguments, keywords) = (call.arguments, call.keywords);
    if let Some(answer) = answer_opinion(world, speaker, listener, name) {
        return answer;
    }
    // A reborrow, so the caller keeps ownership of the dice: `as_deref_mut` on this type is already a reborrow in everything but name.
    if let Some(dice) = draws.as_mut().map(|dice| &mut **dice)
        && let Answer::Value(drawn) =
            drawing_function(world, speaker, listener, name, arguments, unset_is_zero, dice)
    {
        return Answer::Value(drawn);
    }
    // The creature's own numbers, which the data asks for constantly and the port already keeps on the creature.
    answer_from_table(world, speaker, listener, name, arguments, keywords)
}

/// The table of functions that answer from their arguments rather than from a body or a roll.
///
/// Reference: `math_parser_diag.cpp`'s function tables (`:1550-1760`) plus the arithmetic in `math_parser_func.h` - one name, one meaning, no scope.
fn answer_from_table(
    world: &World,
    speaker: Entity,
    listener: Option<Entity>,
    name: &str,
    arguments: &[Expression],
    keywords: &[(String, Expression)],
) -> Answer {
    match (name, arguments) {
        // Reference: `u_hp` (`math_expressions` in `dialogue_helpers`), which takes a body part the port does not model and answers the whole
        // creature's remaining hit points.
        ("distance", [first, second]) => return distance_between(world, speaker, listener, first, second),
        ("u_monsters_nearby" | "npc_monsters_nearby" | "u_characters_nearby" | "npc_characters_nearby", _) => {
            return answer_nearby(world, speaker, listener, name, arguments, keywords);
        }
        // Reference: `{ "pain", { "un", 0, pain_eval, pain_ass } }` (`math_parser_diag.cpp:869-882`) - the actor's `pain_cur`, with `type: "perceived"` asking
        // for the pain as the character feels it instead. The port has no masking rule yet, so it refuses that form rather than answering the raw number for it.
        ("health" | "u_health" | "npc_health", []) => return answer_health(world, speaker, listener, name),
        ("has_flag" | "u_has_flag" | "npc_has_flag", _) => {
            return answer_has_flag(world, speaker, listener, name, arguments);
        }
        ("field_strength" | "u_field_strength" | "npc_field_strength", _) => {
            return answer_field_strength(world, speaker, listener, name, arguments, keywords);
        }
        // Reference: `calories_eval` (`math_parser_diag.cpp:1753`) answers `get_stored_kcal()`: the stored energy in kilocalories, with `format: percent`
        // refused because it needs a healthy weight the port has no rule for.
        ("calories" | "u_calories" | "npc_calories", _) => {
            return answer_calories(world, speaker, listener, name, keywords);
        }
        ("vitamin" | "u_vitamin" | "npc_vitamin", _) => {
            return answer_vitamin(world, speaker, listener, name, arguments).map_or(Answer::Refused, Answer::Value);
        }
        ("u_pain" | "npc_pain", []) => {
            return answer_pain(world, speaker, listener, name, keywords);
        }
        ("u_hp" | "npc_hp" | "hp_max" | "u_hp_max" | "npc_hp_max", _) => {
            return answer_hit_points(world, speaker, listener, name);
        }
        // Reference: `u_effect_intensity(name)` answers how strong an effect is, and **zero** when it is not worn at all.
        ("u_effect_intensity" | "npc_effect_intensity", [Expression::Text(effect)]) => {
            let creature = match name {
                "npc_effect_intensity" => listener.unwrap_or(speaker),
                _ => speaker,
            };
            let intensity = crate::applied_effects::intensity_of(world, creature, effect).unwrap_or(0);
            return Answer::Value(f64::from(intensity));
        }
        // Reference: `moon_phase()` (`math_parser_diag.cpp:862`) answers the phase **index** at the current turn.
        ("moon_phase", []) => {
            let Some(phase) = crate::calendar::moon_phase_now(world) else {
                return Answer::Refused;
            };
            return Answer::Value(f64::from(phase));
        }
        // Reference: `has_var(name)` answers 1 when the dialogue holds that variable at all, and 0 when it holds none - which is how the data
        // tells "never set" apart from "set to zero".
        ("has_var", [argument]) => {
            // The data writes the name **bare** as often as quoted - `has_var(u_dialogue_hub_rnd_u_project_ongoing)` - and a bare name reads as a variable
            // lookup, so its spelling is the name being asked about rather than a value to evaluate.
            let variable = match argument {
                Expression::Text(name) => name.as_str(),
                Expression::Variable { scope, name } if scope == "var_val" => name.as_str(),
                _ => return Answer::NotMine,
            };
            let held = crate::dialogue_values::value_of(world, speaker, variable).is_some();
            return Answer::Value(if held { 1.0 } else { 0.0 });
        }
        _ => {}
    }
    if let Some(level) = answer_skill(world, speaker, listener, name, arguments) {
        return Answer::Value(level);
    }
    if name == "time_since"
        && let [point] = arguments
    {
        // Reference: `time_since_eval` (`math_parser_diag.cpp:1173-1200`): a name counts from that point on the clock, and a number counts from the turn
        // it spells. The answer is in turns unless a `'unit'` names another one, which `spells.json` does pass (`'unit': 'hour'`) - the earlier note here
        // that no call passes a unit was wrong, and the census's refused list is what showed it.
        let turns: Option<f64> = match point {
            Expression::Text(named) => match named.as_str() {
                "cataclysm" => crate::calendar::turns_since_cataclysm(world),
                "midnight" => crate::calendar::turns_since_midnight(world),
                "noon" => crate::calendar::turns_since_noon(world),
                _ => None,
            }
            .map(|turns| {
                #[allow(clippy::cast_precision_loss)]
                let turns = turns as f64;
                turns
            }),
            other => crate::calendar::of(world)
                .zip(value(world, speaker, listener, other))
                .map(|(calendar, from)| {
                    #[allow(clippy::cast_precision_loss)]
                    let now = calendar.turn as f64;
                    now - from
                }),
        };
        // A unit divides the turns into the span it names. `turn` is the default and a second is one turn, which is the reference's own clock.
        let scale = match unit_of(keywords) {
            Some(unit) => unit_turns(unit),
            None => Some(1.0),
        };
        return turns
            .zip(scale)
            .map_or(Answer::NotMine, |(turns, scale)| Answer::Value(turns / scale));
    }
    if name == "time"
        && let [Expression::Text(text)] = arguments
    {
        #[allow(clippy::cast_precision_loss)]
        let turns = cdda_content::constructions::parse_duration(text) as f64;
        return Answer::Value(turns);
    }
    Answer::NotMine}

/// What an expression works out to, with a choice about what an unset variable means.
///
/// The two callers disagree, and the reference is on both sides. A **condition** that names a variable nobody set has no answer
/// (`maybe_get_value` answers a null pointer, `npctalk.cpp:2659-2662`), which the port reads as "cannot say" and a condition
/// treats as false. An **assignment** reads it as zero, because `dialogue::get_value` answers a null value that the arithmetic
/// treats as nothing - which is why `"u_val('x') = u_val('x') + 20"` on an unset `x` writes 20.
#[must_use]
pub fn value_with(
    world: &World,
    speaker: Entity,
    listener: Option<Entity>,
    expression: &Expression,
    unset_is_zero: bool,
) -> Option<f64> {
    value_drawing(world, speaker, listener, expression, unset_is_zero, None)
}

/// What an expression works out to, with dice available.
///
/// Reference: `rng( lo, hi )` and `rand( n )` are in the reference's math function table (`math_parser_func.h:141-163`), and they are the
/// only two that change the store while answering: a draw advances the game's generator. The port cannot draw through `&World`, so the
/// dice are **taken out** of the store by the caller, handed in here, and put back - the same shape every other rule that rolls uses.
/// A caller with no dice to offer gets a refusal rather than a zero, because a `rng` that quietly answered nothing would make a
/// condition false for a reason nobody could see.
#[must_use]
pub fn value_drawing(
    world: &World,
    speaker: Entity,
    listener: Option<Entity>,
    expression: &Expression,
    unset_is_zero: bool,
    mut draws: Option<&mut cdda_core_types::rng::Rolls>,
) -> Option<f64> {
    match expression {
        Expression::Number(number) => Some(*number),
        Expression::Variable { scope, name } => {
            // Reference: `context_val( '…' )` reads the invocation's context (`dialogue.h`), which a `run_eocs` call fills from its
            // `variables` - so it is not a per-holder scope and has no holder to look up.
            if scope == "context_val" {
                let held = crate::dialogue_context::values(world, speaker).remove(name)?;
                return held.parse::<f64>().ok().or(if unset_is_zero { Some(0.0) } else { None });
            }
            let holder = holder_for_scope(world, speaker, listener, scope)?;
            // Reference: `read_var_value` (`dialogue_helpers.cpp:11-16`) answers an empty `diag_value` for a variable nobody set, and
            // `diag_value::dbl()` (`math_parser_diag_value.cpp:168`) converts an empty value to **zero**. So an unset variable is zero in dialogue
            // math - and a program whose condition reads one can finally be evaluated instead of failing to answer at all.
            let _ = unset_is_zero;
            Some(crate::dialogue_values::number_of(world, holder, name).unwrap_or(0.0))
        }
        Expression::Text(_) => None,
        Expression::Call {
            name,
            arguments,
            keywords,
        } => {
            match special_function(
                world,
                speaker,
                listener,
                name,
                &Call {
                    arguments,
                    keywords,
                },
                unset_is_zero,
                &mut draws,
            ) {
                Answer::Value(number) => return Some(number),
                Answer::Refused => return None,
                Answer::NotMine => {}
            }
            let arguments = arguments
                .iter()
                .map(|argument| {
                    value_drawing(
                        world,
                        speaker,
                        listener,
                        argument,
                        unset_is_zero,
                        draws.as_deref_mut(),
                    )
                })
                .collect::<Option<Vec<f64>>>()?;
            apply_function(name, &arguments)
        }
        Expression::Binary { op, left, right } => {
            // Two operands, one borrow: the dice move into a local and are reborrowed for each side.
            let mut dice = draws;
            let left = value_drawing(
                world,
                speaker,
                listener,
                left,
                unset_is_zero,
                dice.as_deref_mut(),
            )?;
            let right = value_drawing(
                world,
                speaker,
                listener,
                right,
                unset_is_zero,
                dice,
            )?;
            Some(match op {
                Operator::Add => left + right,
                Operator::Subtract => left - right,
                Operator::Multiply => left * right,
                Operator::Divide => left / right,
                Operator::Remainder => left % right,
                Operator::Equal => f64::from(u8::from((left - right).abs() < f64::EPSILON)),
                Operator::NotEqual => f64::from(u8::from((left - right).abs() >= f64::EPSILON)),
                Operator::Less => f64::from(u8::from(left < right)),
                Operator::AtMost => f64::from(u8::from(left <= right)),
                Operator::Greater => f64::from(u8::from(left > right)),
                Operator::AtLeast => f64::from(u8::from(left >= right)),
                // The left side of a logical not is a placeholder zero; the answer is one when the operand is zero.
                Operator::Not => f64::from(u8::from(right.abs() < f64::EPSILON)),
            })
        }
    }
}

/// Which entity a variable's scope reads, or nothing when the scope names somebody who is not here.
///
/// Reference: `read_var_info` (`condition.cpp:408-437`) classes a variable by the member that names it, and
/// `const_dialogue` then reads it from `actor( false )` or `actor( true )`. A bare identifier is looked up in the dialogue's
/// own context (`const_dialogue::maybe_get_value`, `npctalk.cpp:2659-2662`), which the port keeps on the speaker - the one
/// dialoguer that always exists.
pub fn holder_for_scope(
    world: &World,
    speaker: Entity,
    listener: Option<Entity>,
    scope: &str,
) -> Option<Entity> {
    match scope {
        "global_val" => crate::dialogue_values::global_holder(world),
        "n_val" | "npc_val" => listener,
        _ => Some(speaker),
    }
}

/// The text a string source names, or nothing when the store cannot answer for it.
pub fn text_of(
    world: &World,
    speaker: Entity,
    listener: Option<Entity>,
    source: &StringSource,
) -> Option<String> {
    match source {
        StringSource::Literal(text) => Some(text.clone()),
        StringSource::Variable { scope, name } => {
            let holder = holder_for_scope(world, speaker, listener, scope)?;
            crate::dialogue_values::text_of(world, holder, name)
        }
    }
}

/// What one of the reference's math functions answers.
///
/// Reference: `math_parser_func.h:141-163`, transcribed function for function. The temperature conversions are the ones in
/// `units.h:397-424`: `celsius` and `fahrenheit` turn kelvin into the other scale, and `from_celsius`/`from_fahrenheit` turn
/// the other scale into kelvin, with the same constants the reference spells (`273.150`, `1.8`, `459.67`).
fn apply_function(name: &str, arguments: &[f64]) -> Option<f64> {
    let first = *arguments.first()?;
    Some(match name {
        "abs" => first.abs(),
        "floor" => first.floor(),
        "trunc" => first.trunc(),
        "ceil" => first.ceil(),
        "round" => first.round(),
        "sqrt" => first.sqrt(),
        "log" => first.ln(),
        "sin" => first.sin(),
        "cos" => first.cos(),
        "tan" => first.tan(),
        "clamp" => {
            let low = *arguments.get(1)?;
            let high = *arguments.get(2)?;
            first.max(low).min(high)
        }
        "max" => arguments.iter().copied().fold(f64::NEG_INFINITY, f64::max),
        "min" => arguments.iter().copied().fold(f64::INFINITY, f64::min),
        "celsius" => first - 273.150,
        "fahrenheit" => first * 1.8 - 459.67,
        "from_celsius" => first + 273.150,
        "from_fahrenheit" => (first + 459.67) / 1.8,
        _ => return None,
    })
}

/// Every one of a list of conditions, or any one of them, with the dice passed along.
fn conditions_with_dice(
    world: &World,
    speaker: Entity,
    listener: Option<Entity>,
    conditions: &[Condition],
    draws: &mut Option<&mut cdda_core_types::rng::Rolls>,
    all: bool,
) -> bool {
    for condition in conditions {
        let held = holds_drawing(world, speaker, listener, condition, draws);
        if held != all {
            return held;
        }
    }
    all
}
