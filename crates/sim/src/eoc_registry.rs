//! The effect-on-conditions the data defines, by id, and how to run one.
//!
//! Reference: `effect_on_conditions::load` fills `effect_on_conditions::eoc_map`, and `dialogue::effect`'s `run_eocs` looks a
//! name up there and runs it (`npctalk.cpp`). The port keeps the same shape - a singleton holding programs by id - so an effect
//! can name another one and the machine composes the way the data expects.
//!
//! Recursion is bounded on purpose. The data nests programs freely (`EOC_A` runs `EOC_B`), and a loop in that graph would hang a
//! headless run with no window to interrupt, so a program may only run [`DEPTH_LIMIT`] deep. The reference has no such guard;
//! the port needs one because it cannot be Ctrl-C'd out of a turn loop in a test.

use bevy_ecs::prelude::*;
pub use cdda_content::dialogue::Program;
use std::collections::BTreeMap;

use crate::singletons;
use crate::dialogue_effects::Applied;

/// How deep a program may run another program before the port refuses.
pub const DEPTH_LIMIT: usize = 16;

/// Every program, by id.
#[derive(Component, Debug, Clone, Default)]
pub struct Programs {
    by_id: BTreeMap<String, Program>,
    /// The event programs, by the event they answer.
    ///
    /// Reference: `eoc_events::notify` caches `event_EOCs[required_event]` (`effect_on_condition.cpp:526-544`) and runs only that vector for an event, so an event program
    /// is never a candidate for another kind. The port keeps the same index, built when the programs are installed.
    events: BTreeMap<String, Vec<String>>,
}

/// Marks the entity holding the programs.
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct ThePrograms;

/// Every program defined inline inside a list of effects, at any depth the port's containers reach.
///
/// Reference: `load_inline_eoc` (`effect_on_condition.cpp:119-133`): an entry given an `id` beside its `effect` is registered under that id. The walk covers the
/// containers the port has - `if`, `switch`, `run_eocs`, the selector, the weighted list and the inventory eocs.
fn inline_definitions(effects: &[cdda_content::dialogue::Effect]) -> Vec<Program> {
    use cdda_content::dialogue::{Effect, ProgramRef};
    fn from_ref(reference: &ProgramRef, out: &mut Vec<Program>) {
        if let ProgramRef::Inline { program, id, .. } = reference
            && id.is_some()
        {
            out.push((**program).clone());
            out.extend(inline_definitions(&program.effects));
        }
    }
    let mut found = Vec::new();
    for effect in effects {
        match effect {
            Effect::If { then, otherwise, .. } => {
                found.extend(inline_definitions(then));
                found.extend(inline_definitions(otherwise));
            }
            Effect::Switch { cases, .. } => {
                for (_, case) in cases {
                    found.extend(inline_definitions(case));
                }
            }
            Effect::RunEoc(calls) => {
                for call in calls {
                    from_ref(call, &mut found);
                }
            }
            Effect::RunEocSelector { options, .. } => {
                for option in options {
                    from_ref(option, &mut found);
                }
            }
            Effect::WeightedList(weighted) => {
                for (option, _) in weighted {
                    from_ref(option, &mut found);
                }
            }
            Effect::RunInvEocs { programs, .. } => {
                for program in programs {
                    from_ref(program, &mut found);
                }
            }
            _ => {}
        }
    }
    found
}

/// Installs the programs into the store, replacing whatever was there.
pub fn set_programs(world: &mut World, programs: &[Program]) {
    let mut by_id = BTreeMap::new();
    for program in programs {
        by_id.insert(program.id.clone(), program.clone());
    }
    // A program defined **where it is called** is installed under its own id, the way the reference's `load_inline_eoc` (`effect_on_condition.cpp:119-133`) adds it
    // to the factory. It never replaces a program the data defines in its own right: an inline body is a definition of last resort.
    let defined: Vec<Program> = programs
        .iter()
        .flat_map(|program| inline_definitions(&program.effects))
        .collect();
    for body in defined {
        if !body.id.is_empty() {
            by_id.entry(body.id.clone()).or_insert(body);
        }
    }
    let events: BTreeMap<String, Vec<String>> = {
        let mut index: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for program in by_id.values() {
            if let Some(kind) = &program.event {
                index.entry(kind.clone()).or_default().push(program.id.clone());
            }
        }
        index
    };
    let holder = singletons::singleton::<Programs, ThePrograms>(world);
    if let Some(mut existing) = world.get_mut::<Programs>(holder) {
        existing.by_id = by_id;
        existing.events = events;
        return;
    }
    world.entity_mut(holder).insert(Programs { by_id, events });
}

/// A program by id, or nothing when the data does not define it.
#[must_use]
pub fn program_of(world: &World, id: &str) -> Option<Program> {
    let mut query = world.try_query_filtered::<&Programs, With<ThePrograms>>()?;
    query.iter(world).next()?.by_id.get(id).cloned()
}

/// How many programs the store holds.
#[must_use]
pub fn count(world: &mut World) -> usize {
    let holder = singletons::singleton::<Programs, ThePrograms>(world);
    world.get::<Programs>(holder).map_or(0, |programs| programs.by_id.len())
}

/// Runs a program by id, and the programs it names, up to [`DEPTH_LIMIT`].
///
/// Reference: `dialogue::effect` runs a named effect-on-condition with the same dialoguers; the port hands the same speaker and
/// listener down, so a program's `u_val` and `n_val` mean the same thing at every depth.
pub fn run_program(
    world: &mut World,
    speaker: Entity,
    listener: Option<Entity>,
    id: &str,
    depth: usize,
) -> Applied {
    if depth >= DEPTH_LIMIT {
        return Applied::CannotAnswer;
    }
    let Some(program) = program_of(world, id) else {
        return Applied::CannotAnswer;
    };
    if let Some(condition) = &program.condition
        && !crate::conditions::holds_in_store(world, speaker, listener, condition)
    {
        return Applied::Done;
    }
    for effect in &program.effects {
        match effect {
            cdda_content::dialogue::Effect::Switch { on, cases } => {
                // Reference: every case the value reaches replaces the chosen one, so the **last** match runs.
                let mut dice = crate::singletons::take_rolls(world);
                let switched =
                    crate::conditions::value_drawing(world, speaker, listener, on, true, dice.as_mut());
                if let Some(dice) = dice.take() {
                    crate::singletons::put_rolls(world, dice);
                }
                let Some(switched) = switched else {
                    return Applied::CannotAnswer;
                };
                let mut chosen: Option<&Vec<cdda_content::dialogue::Effect>> = None;
                for (case, effects) in cases {
                    let mut dice = crate::singletons::take_rolls(world);
                    let reached =
                        crate::conditions::value_drawing(world, speaker, listener, case, true, dice.as_mut());
                    if let Some(dice) = dice.take() {
                        crate::singletons::put_rolls(world, dice);
                    }
                    let Some(reached) = reached else {
                        return Applied::CannotAnswer;
                    };
                    if switched >= reached {
                        chosen = Some(effects);
                    }
                }
                if let Some(effects) = chosen
                    && crate::dialogue_effects::run_effects(world, speaker, listener, effects)
                        == Applied::CannotAnswer
                {
                    return Applied::CannotAnswer;
                }
            }
            cdda_content::dialogue::Effect::RunNpcEocs {
                programs,
                unique_ids,
                npc_range,
                local,
            } => {
                if run_on_people(world, speaker, programs, unique_ids, *npc_range, *local, depth)
                    == Applied::CannotAnswer
                {
                    return Applied::CannotAnswer;
                }
            }
            cdda_content::dialogue::Effect::RunMonsterEocs {
                programs,
                mtype_ids,
                monster_range,
            } => {
                if run_on_monsters(world, speaker, programs, mtype_ids, *monster_range, depth)
                    == Applied::CannotAnswer
                {
                    return Applied::CannotAnswer;
                }
            }
            cdda_content::dialogue::Effect::RunEoc(programs) => {
                for program in programs {
                    let started = run_ref(world, speaker, listener, program, depth + 1);
                    if started == Applied::CannotAnswer {
                        return Applied::CannotAnswer;
                    }
                }
            }
            other => {
                if crate::dialogue_effects::run_effect(world, speaker, listener, other)
                    == Applied::CannotAnswer
                {
                    return Applied::CannotAnswer;
                }
            }
        }
    }
    Applied::Done
}

/// Runs a program written where it is used.
///
/// Reference: `load_inline_eoc` (`npctalk.cpp:512-541`), which turns an inline object into an effect-on-condition with a derived id and
/// runs it in place. The port runs it without installing it, because an inline program has no id anything else can name.
pub fn run_inline(
    world: &mut World,
    speaker: Entity,
    listener: Option<Entity>,
    program: &Program,
    depth: usize,
) -> Applied {
    if depth >= DEPTH_LIMIT {
        return Applied::CannotAnswer;
    }
    if let Some(condition) = &program.condition
        && !crate::conditions::holds_in_store(world, speaker, listener, condition)
    {
        return Applied::Done;
    }
    for effect in &program.effects {
        match effect {
            cdda_content::dialogue::Effect::RunEoc(programs) => {
                for named in programs {
                    let started = run_ref(world, speaker, listener, named, depth + 1);
                    if started == Applied::CannotAnswer {
                        return Applied::CannotAnswer;
                    }
                }
            }
            other => {
                if crate::dialogue_effects::run_effect(world, speaker, listener, other)
                    == Applied::CannotAnswer
                {
                    return Applied::CannotAnswer;
                }
            }
        }
    }
    Applied::Done
}

/// Runs a list of programs on every person the effect's filters leave.
///
/// Reference: `f_run_npc_eocs` (`npctalk.cpp:6552-6600`) asks `g->get_npcs_if` for the people its filters keep - the `unique_ids` when given, a distance from the
/// actor when `npc_range` is, and the actor's own level otherwise - and runs every program on each of them with that person as the listener.
fn run_on_people(
    world: &mut World,
    speaker: Entity,
    programs: &[cdda_content::dialogue::ProgramRef],
    unique_ids: &[cdda_content::dialogue::StringSource],
    npc_range: Option<i32>,
    local: bool,
    depth: usize,
) -> Applied {
    let Some(at) = world.get::<crate::Position>(speaker).copied() else {
        return Applied::CannotAnswer;
    };
    let _ = local;
    let wanted = unique_ids
        .iter()
        .map(|id| crate::conditions::text_of(world, speaker, None, id))
        .collect::<Option<Vec<_>>>();
    let Some(wanted) = wanted else {
        return Applied::CannotAnswer;
    };
    let people: Vec<Entity> = crate::npcs::npcs(world)
        .into_iter()
        .filter(|(entity, square)| {
            let id_matches = wanted.is_empty()
                || crate::npcs::unique_id_of(world, *entity)
                    .is_some_and(|id| wanted.iter().any(|wanted| wanted == id));
            // Reference: the level always has to match when no z filter is set - the range narrows the square, it does not reach up or down a storey.
            let same_level = square.2 == at.z;
            let near_matches = match npc_range {
                Some(range) => (square.0 - at.x).abs().max((square.1 - at.y).abs()) <= range,
                None => true,
            };
            id_matches && same_level && near_matches
        })
        .map(|(entity, _)| entity)
        .filter(|entity| *entity != speaker)
        .collect();
    for person in people {
        for reference in programs {
            if run_ref(world, speaker, Some(person), reference, depth + 1) == Applied::CannotAnswer {
                return Applied::CannotAnswer;
            }
        }
    }
    Applied::Done
}

/// Runs a list of programs on every monster the effect's filters leave.
///
/// Reference: `f_run_monster_eocs` (`npctalk.cpp:6654-6700`) asks `g->get_creatures_if` for the monsters its filters keep - the `mtype_ids` when given, a
/// distance from the actor when `monster_range` is, and the actor's own level otherwise - and runs every program on each, with the monster as the listener.
fn run_on_monsters(
    world: &mut World,
    speaker: Entity,
    programs: &[cdda_content::dialogue::ProgramRef],
    mtype_ids: &[cdda_content::dialogue::StringSource],
    monster_range: Option<i32>,
    depth: usize,
) -> Applied {
    let Some(at) = world.get::<crate::Position>(speaker).copied() else {
        return Applied::CannotAnswer;
    };
    let wanted = mtype_ids
        .iter()
        .map(|id| crate::conditions::text_of(world, speaker, None, id))
        .collect::<Option<Vec<_>>>();
    let Some(wanted) = wanted else {
        return Applied::CannotAnswer;
    };
    let kinds = crate::singletons::holder::<crate::monster_registry::TheMonsterTypes>(world)
        .and_then(|holder| world.get::<crate::monster_registry::MonsterTypes>(holder));
    let mut found: Vec<Entity> = Vec::new();
    if let Some(mut query) = world.try_query::<(Entity, &crate::creature::CreatureKind, &crate::Position)>() {
        for (entity, kind, square) in query.iter(world) {
            let named = kinds.and_then(|known| known.name_of(kind.0).map(str::to_owned));
            let type_matches = wanted.is_empty() || named.is_some_and(|named| wanted.contains(&named));
            let same_level = square.z == at.z;
            let near_matches = match monster_range {
                Some(range) => (square.x - at.x).abs().max((square.y - at.y).abs()) <= range,
                None => true,
            };
            if type_matches && same_level && near_matches {
                found.push(entity);
            }
        }
    }
    for monster in found {
        for reference in programs {
            if run_ref(world, speaker, Some(monster), reference, depth + 1) == Applied::CannotAnswer {
                return Applied::CannotAnswer;
            }
        }
    }
    Applied::Done
}

/// Runs one `run_eocs` entry, with the context values the call hands it.
///
/// Reference: `f_run_eocs` (`npctalk.cpp:6332-6338`) evaluates each `variables` member into the invocation's context, runs the program,
/// and lets the context go - so a program called with values sees them, and its caller's own values come back afterwards.
pub fn run_ref(
    world: &mut World,
    speaker: Entity,
    listener: Option<Entity>,
    reference: &cdda_content::dialogue::ProgramRef,
    depth: usize,
) -> Applied {
    let (variables, id) = match reference {
        cdda_content::dialogue::ProgramRef::Named { id, variables } => (variables, Some(id.clone())),
        // Reference: `f_run_eocs` evaluates each entry's `str_or_var` when the effects run (`npctalk.cpp:5100-5115`), so a program named by a variable is looked up now
        // rather than when the data was read.
        cdda_content::dialogue::ProgramRef::NamedByVariable {
            scope,
            name,
            variables,
        } => {
            let named = crate::conditions::value_of_variable_text(world, speaker, listener, scope, name);
            (variables, named)
        }
        cdda_content::dialogue::ProgramRef::Inline { variables, .. } => (variables, None),
    };
    let handed: std::collections::BTreeMap<String, String> = variables
        .iter()
        .filter_map(|(name, source)| {
            let value = crate::conditions::text_of(world, speaker, listener, source)?;
            Some((name.clone(), value))
        })
        .collect();
    let previous = if handed.is_empty() {
        None
    } else {
        Some(crate::dialogue_context::put(world, speaker, handed))
    };
    let started = match (reference, id) {
        (cdda_content::dialogue::ProgramRef::Inline { program, .. }, _) => {
            run_inline(world, speaker, listener, program, depth)
        }
        (_, Some(id)) => run_program(world, speaker, listener, &id, depth),
        _ => Applied::CannotAnswer,
    };
    if let Some(previous) = previous {
        crate::dialogue_context::restore(world, speaker, previous);
    }
    started
}

/// The programs that answer an event of a kind, in the order the data defines them.
///
/// Reference: `eoc_events::notify` (`effect_on_condition.cpp:526-544`), which builds the same list once and runs it for every matching event.
#[must_use]
pub fn event_programs(world: &World, kind: &str) -> Vec<String> {
    singletons::holder::<ThePrograms>(world)
        .and_then(|entity| world.get::<Programs>(entity))
        .and_then(|programs| programs.events.get(kind).cloned())
        .unwrap_or_default()
}

/// Whether a program answers events rather than being called by name.
#[must_use]
pub fn is_event(world: &World, id: &str) -> bool {
    program_of(world, id).is_some_and(|program| program.event.is_some())
}
