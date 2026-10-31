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
pub fn holds_with(
    world: &World,
    speaker: Entity,
    listener: Option<Entity>,
    condition: &Condition,
) -> bool {
    holds_drawing(world, speaker, listener, condition, &mut None)
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
        Condition::TestEoc { id } => {
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
        Condition::OneInChance(_) | Condition::XInYChance { .. } => {
            rolls_against(world, speaker, listener, condition, draws)
        }
        Condition::ExpectsVars { names } => expects_vars(world, speaker, listener, names),
        Condition::IsSeason { season } => {
            is_season(world, speaker, listener, season)
        }
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
        Condition::HasTrait {
            trait_name,
            listener: about_listener,
        } => {
            let Some(creature) = actor_for(speaker, listener, *about_listener) else {
                return false;
            };
            crate::traits::has_trait(world, creature, trait_name)
        }
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
fn special_function(
    world: &World,
    speaker: Entity,
    listener: Option<Entity>,
    name: &str,
    arguments: &[Expression],
    unset_is_zero: bool,
    draws: &mut Option<&mut cdda_core_types::rng::Rolls>,
) -> Answer {
    // A reborrow, so the caller keeps ownership of the dice: `as_deref_mut` on this type is already a reborrow in everything but
    // name, and clippy reads it as one.
    if let Some(dice) = draws.as_mut().map(|dice| &mut **dice) {
        match (name, arguments) {
            ("rand", [argument]) => {
                let dice: &mut cdda_core_types::rng::Rolls = dice;
                let Some(n) = value_drawing(world, speaker, listener, argument, unset_is_zero, None)
                else {
                    return Answer::Refused;
                };
                #[allow(clippy::cast_possible_truncation)]
                let high = n.round() as i32;
                return Answer::Value(f64::from(dice.rng(0, high)));
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
                return Answer::Value(f64::from(dice.rng(low, high)));
            }
            _ => {}
        }
    }
    if name == "time"
        && let [Expression::Text(text)] = arguments
    {
        #[allow(clippy::cast_precision_loss)]
        let turns = cdda_content::constructions::parse_duration(text) as f64;
        return Answer::Value(turns);
    }
    Answer::NotMine
}

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
            let holder = holder_of(world, speaker, listener, scope)?;
            match crate::dialogue_values::number_of(world, holder, name) {
                Some(number) => Some(number),
                None if unset_is_zero => Some(0.0),
                None => None,
            }
        }
        Expression::Text(_) => None,
        Expression::Call { name, arguments } => {
            match special_function(
                world,
                speaker,
                listener,
                name,
                arguments,
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
fn holder_of(
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
            let holder = holder_of(world, speaker, listener, scope)?;
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
