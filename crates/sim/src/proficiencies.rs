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
#[derive(Component, Debug, Clone, Default, PartialEq)]
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

/// What killing a body teaches: every weak-point family on it practises its proficiency for **one minute**.
///
/// Reference: `weakpoint_families::practice_kill` (`weakpoint.cpp:152-155`) is `practice( learner, 1_minutes )`, which walks the body's families and hands each one's proficiency that span
/// (`:133-145`). A family whose proficiency the killer already knows is skipped by [`practice`] itself, exactly as the reference's `proficiency_set::practice` skips it.
///
/// The reference's `practice_proficiency` caps a single call at **1000 seconds** (`weakpoint.cpp:159-164`'s note, which is why `practice_dissect` splits its half hour into two calls). A minute is
/// far under that cap, so the cap cannot be reached by this rule and is not modelled; the dissect half is named in `docs/parity.md`.
///
/// `learn_times` says how long each proficiency takes to learn, because the port has no proficiency table yet. It answers zero for a proficiency it does not know, which means such a proficiency
/// accrues practice and is never learned - the honest answer when the data cannot say.
#[must_use]
pub fn practise_a_kill(
    world: &mut World,
    killer: Entity,
    families: &[cdda_content::weakpoints::Family],
    learn_times: &dyn Fn(&str) -> i64,
) -> Vec<String> {
    let mut taught = Vec::new();
    for family in families {
        let time_to_learn = learn_times(&family.proficiency);
        if practice(world, killer, &family.proficiency, 60, time_to_learn) {
            taught.push(family.proficiency.clone());
        }
    }
    taught
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
#[derive(Component, Debug, Clone, Default, PartialEq)]
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

/// Which proficiencies each kind of weapon may use (`weapon_category::category_proficiencies`).
#[derive(Component, Debug, Clone, Default, PartialEq, Eq)]
pub struct WeaponCategoryProficiencies(pub BTreeMap<String, Vec<String>>);

/// Marks the entity carrying the weapon category table.
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct TheWeaponCategoryProficiencies;

/// Installs which proficiencies each kind of weapon may use.
pub fn set_weapon_categories(world: &mut World, categories: &[(String, Vec<String>)]) {
    let entity = crate::singletons::singleton::<WeaponCategoryProficiencies, TheWeaponCategoryProficiencies>(world);
    let table: BTreeMap<String, Vec<String>> = categories.iter().cloned().collect();
    world.entity_mut(entity).insert(WeaponCategoryProficiencies(table));
}

/// What a weapon's own categories and the creature's own proficiencies multiply a swing's stamina cost by.
///
/// Reference: `get_total_melee_stamina_cost` (`melee.cpp:979-995`): for each category the wielded item belongs to, the sum of the **known** proficiencies of that category's list that carry a
/// `melee_attack`/`stamina` bonus, fed to [`crate::melee::proficiency_stamina_multiplier`].
///
/// **All three tables have to agree**: the item's category, the category's proficiency list, and the proficiency's own bonus. A master of the short sword swinging an axe gets nothing, which
/// is the reference's own behaviour and the case a port that keyed off the proficiency's name would get wrong.
#[must_use]
pub fn stamina_multiplier(world: &mut World, creature: Entity, weapon: crate::ItemTypeId) -> f32 {
    let categories = crate::item_registry::weapon_categories_of(world, weapon);
    if categories.is_empty() {
        return 1.0;
    }
    multiplier_for_categories(world, creature, &categories)
}

/// What a swing multiplies its stamina cost by, given what the swinger holds.
///
/// Reference: `wielded_weapon_categories` (`melee.cpp:570-578`): a character wielding nothing is read as the **`UNARMED`** category, so its proficiencies apply like any other weapon's.
/// [`Self::stamina_multiplier`] answers the wielded case; this answers the question the swing actually asks.
#[must_use]
pub fn swing_multiplier(world: &mut World, creature: Entity, wielded: Option<crate::ItemTypeId>) -> f32 {
    match wielded {
        Some(weapon) => stamina_multiplier(world, creature, weapon),
        None => multiplier_for_categories(world, creature, &["UNARMED".to_owned()]),
    }
}

/// The multiplier a set of weapon categories earns, given the creature's proficiencies.
#[must_use]
pub fn multiplier_for_categories(world: &mut World, creature: Entity, categories: &[String]) -> f32 {
    if categories.is_empty() {
        return 1.0;
    }
    let holder = crate::singletons::singleton::<WeaponCategoryProficiencies, TheWeaponCategoryProficiencies>(world);
    let table = world
        .get::<WeaponCategoryProficiencies>(holder)
        .map(|table| table.0.clone())
        .unwrap_or_default();
    let dump = crate::singletons::singleton::<ProficiencyRules, TheProficiencyRules>(world);
    let bonuses = world.get::<ProficiencyRules>(dump).map(|rules| rules.dump.clone());
    let mut losses = Vec::new();
    for category in categories {
        let Some(proficiencies) = table.get(category) else {
            continue;
        };
        let mut loss = 0.0f32;
        for proficiency in proficiencies {
            if !knows(world, creature, proficiency) {
                continue;
            }
            if let Some(bonus) = bonuses
                .as_ref()
                .and_then(|dump| dump.by_id.get(proficiency))
                .and_then(|prof| prof.bonus_for("melee_attack", "stamina"))
            {
                loss += bonus;
            }
        }
        losses.push(loss);
    }
    crate::melee::proficiency_stamina_multiplier(&losses)
}

/// A stat bonus from the proficiencies a creature knows.
///
/// Reference: `proficiency_set::get_proficiency_bonus` (`proficiency.cpp:529-543`):
///
/// ```text
/// float stat_bonus = 0;
/// for( const proficiency_id &knows : known ) {
///     const std::vector<proficiency_bonus> &prof_bonuses = knows->get_bonuses( category );
///     for( const proficiency_bonus &bonus : prof_bonuses ) if( bonus.type == prof_bonus ) stat_bonus += bonus.value;
/// }
/// ```
///
/// **The sum runs over everything known, not over a weapon's category** - a different shape from [`stamina_multiplier`], which walks the weapon's categories. Both exist because the reference
/// has both.
#[must_use]
pub fn bonus(world: &mut World, creature: Entity, category: &str, kind: &str) -> f32 {
    let holder = crate::singletons::singleton::<ProficiencyRules, TheProficiencyRules>(world);
    let Some(rules) = world.get::<ProficiencyRules>(holder).map(|rules| rules.dump.clone()) else {
        return 0.0;
    };
    let Some(learned) = world.get::<Proficiencies>(creature).map(|set| set.known.clone()) else {
        return 0.0;
    };
    let mut total = 0.0f32;
    for proficiency in &learned {
        if let Some(prof) = rules.by_id.get(proficiency)
            && let Some(value) = prof.bonus_for(category, kind)
        {
            total += value;
        }
    }
    total
}
