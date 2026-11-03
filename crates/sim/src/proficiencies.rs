//! What a survivor has learned to do well, and how a craft teaches it.
//!
//! Reference: `proficiency_set` (`proficiency.cpp:152-345`), the reference's four numbers about a proficiency: `has_learned`, the **practised** time toward it,
//! its `time_to_learn`, and whether the prerequisites are known. A proficiency is learned by **practising** it - `practice` (`proficiency.cpp:306-345`) adds the
//! time, and when the practised time reaches the time it takes to learn, the proficiency is **learned** and the progress is forgotten.
//!
//! The port keeps the two facts that matter as one component: the set of what is known, and the time practised toward what is not. Nothing else about a survivor's
//! hands is stored, because nothing else has a rule that reads it yet.

use bevy_ecs::prelude::*;
use std::collections::{BTreeMap, BTreeSet};

/// What a creature has learned, and how far along it is with what it has not.
#[derive(Component, Debug, Clone, Default, PartialEq, Eq)]
pub struct Proficiencies {
    /// The proficiencies it knows.
    pub known: BTreeSet<String>,
    /// The ones it is learning, with the time practised toward each, in seconds.
    pub learning: BTreeMap<String, i64>,
}

/// Whether a creature knows a proficiency.
#[must_use]
pub fn knows(world: &World, creature: Entity, proficiency: &str) -> bool {
    world
        .get::<Proficiencies>(creature)
        .is_some_and(|set| set.known.contains(proficiency))
}

/// How long a creature has practised toward a proficiency it does not know yet, in seconds.
#[must_use]
pub fn practiced_seconds(world: &World, creature: Entity, proficiency: &str) -> i64 {
    world
        .get::<Proficiencies>(creature)
        .and_then(|set| set.learning.get(proficiency).copied())
        .unwrap_or(0)
}

/// Teaches a creature a proficiency outright, which is what the data's `prof_*` grants do.
pub fn learn(world: &mut World, creature: Entity, proficiency: &str) {
    let mut set = world.get::<Proficiencies>(creature).cloned().unwrap_or_default();
    set.known.insert(proficiency.to_owned());
    set.learning.remove(proficiency);
    world.entity_mut(creature).insert(set);
}

/// Practises a proficiency for a span of time, learning it once the span is long enough.
///
/// Reference: `proficiency_set::practice` (`proficiency.cpp:306-345`): nothing happens to a proficiency already known, and once the practised time reaches
/// `time_to_learn` the proficiency is learned and **its progress is removed** - which is the reference's own erase, and the reason a learned proficiency reports no
/// practised time.
///
/// Returns whether this practice is what learned it.
pub fn practice(
    world: &mut World,
    creature: Entity,
    proficiency: &str,
    seconds: i64,
    time_to_learn: i64,
) -> bool {
    if knows(world, creature, proficiency) {
        return false;
    }
    let mut set = world.get::<Proficiencies>(creature).cloned().unwrap_or_default();
    let practiced = set.learning.entry(proficiency.to_owned()).or_insert(0);
    *practiced = practiced.saturating_add(seconds.max(0));
    let learned = time_to_learn > 0 && *practiced >= time_to_learn;
    if learned {
        set.known.insert(proficiency.to_owned());
        set.learning.remove(proficiency);
    }
    world.entity_mut(creature).insert(set);
    learned
}

/// One proficiency a craft teaches, as the recipe states it.
#[derive(Debug, Clone, Copy)]
pub struct Taught<'a> {
    /// Which proficiency.
    pub id: &'a str,
    /// How much faster the craft goes for someone who knows it.
    pub time_multiplier: f64,
    /// How long learning it by this craft takes, as a multiple of the proficiency's own time.
    pub learning_time_mult: f64,
    /// How long the proficiency takes to learn at all, in seconds.
    pub time_to_learn: i64,
}

/// Teaches what one stretch of a craft has to teach, and answers whether any of it was learned.
///
/// Reference: `Character::craft_proficiency_gain` (`crafting.cpp:1036-1090`): only proficiencies the crafter has not learned are practised, the craft's time is
/// **divided between them**, and each gets `time * ( learning_time_mult / time_multiplier )`. A helper who already knows it would **double** the pace; the port has
/// one crafter for now, so the helper bonus has no caller and is not guessed at.
///
/// `wants` says which proficiencies the crafter can learn at all - the reference's `can_learn` and prerequisite check, which the port answers from the data.
pub fn gain_from_craft(
    world: &mut World,
    crafter: Entity,
    teaches: &[Taught<'_>],
    seconds: i64,
    wants: &dyn Fn(&str) -> bool,
) -> bool {
    let subjects: Vec<&Taught<'_>> = teaches
        .iter()
        .filter(|subject| !knows(world, crafter, subject.id) && wants(subject.id))
        .collect();
    if subjects.is_empty() {
        return false;
    }
    #[allow(clippy::cast_precision_loss)]
    let share = seconds.max(0) as f64 / subjects.len() as f64;
    let mut learned_any = false;
    for subject in subjects {
        let pace = if subject.time_multiplier > 0.0 {
            subject.learning_time_mult / subject.time_multiplier
        } else {
            subject.learning_time_mult
        };
        #[allow(clippy::cast_possible_truncation)]
        let practised = (share * pace) as i64;
        learned_any |= practice(world, crafter, subject.id, practised, subject.time_to_learn);
    }
    learned_any
}

/// Installs the data's proficiency table, so a real world can learn a trade.
///
/// Reference: `proficiency::load_proficiencies` (`proficiency.cpp:40-120`) reads the same three facts. The port takes them from the dump rather than from the
/// reference's JSON, and installs them here because **the table is data, not a rule**: the rules that read it are `time_to_learn`, `gain_from_craft` and `practice`.
pub fn install(world: &mut World, dump: &cdda_content::proficiencies::Proficiencies) {
    let seconds = dump
        .by_id
        .iter()
        .map(|(id, prof)| (id.clone(), prof.seconds))
        .collect();
    set_times(world, seconds);
    let holder = crate::singletons::singleton::<ProficiencyRules, TheProficiencyRules>(world);
    world.entity_mut(holder).insert(ProficiencyRules {
        dump: dump.clone(),
    });
}

/// The data's proficiency table, kept where the rules can read it.
#[derive(Component, Debug, Clone, Default, PartialEq, Eq)]
pub struct ProficiencyRules {
    /// Every proficiency the data defines.
    pub dump: cdda_content::proficiencies::Proficiencies,
}

/// Marks the stored proficiency table.
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct TheProficiencyRules;

/// Whether a creature could learn a proficiency now: the data allows it, and the prerequisites are known.
///
/// Reference: `proficiency_set::practice`'s guard (`proficiency.cpp:307`): `has_learned( ... ) || !practicing->can_learn() || !has_prereqs( ... )` refuses.
#[must_use]
pub fn can_learn(world: &World, creature: Entity, proficiency: &str) -> bool {
    let Some(holder) = crate::singletons::holder::<TheProficiencyRules>(world) else {
        return false;
    };
    let Some(rules) = world.get::<ProficiencyRules>(holder) else {
        return false;
    };
    rules
        .dump
        .learnable_by(proficiency, &|need| knows(world, creature, need))
}

/// How long each proficiency takes to learn, in seconds, as the data states it.
///
/// Reference: `proficiency::time_to_learn()` (`proficiency.h`), filled from the data's `"time_to_learn": "8 h"`. It is the **proficiency's** number rather than the
/// recipe's, so it lives beside the proficiencies rather than on a recipe, and a rule that teaches one asks here.
#[derive(Component, Debug, Clone, Default)]
pub struct ProficiencyTimes {
    seconds: BTreeMap<String, i64>,
}

/// Marks the stored proficiency times.
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct TheProficiencyTimes;

/// Sets how long every proficiency takes to learn.
pub fn set_times(world: &mut World, seconds: BTreeMap<String, i64>) {
    let holder = crate::singletons::singleton::<ProficiencyTimes, TheProficiencyTimes>(world);
    world.entity_mut(holder).insert(ProficiencyTimes { seconds });
}

/// How long one proficiency takes to learn, when the data says.
#[must_use]
pub fn time_to_learn(world: &World, proficiency: &str) -> Option<i64> {
    crate::singletons::holder::<TheProficiencyTimes>(world)
        .and_then(|holder| world.get::<ProficiencyTimes>(holder))
        .and_then(|times| times.seconds.get(proficiency).copied())
}
