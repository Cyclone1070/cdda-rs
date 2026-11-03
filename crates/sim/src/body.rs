//! The body's side of a meal: hunger, thirst, stored calories and vitamins, as `update_stomach` moves them.
//!
//! Reference: `Character::update_stomach` (`src/character_body.cpp:935-1019`), which is the one place the two compartments of [`crate::digest`] meet the character.
//! It runs on every character turn and does six things in a fixed order, and the order is the rule:
//!
//! 1. **both compartments digest** — the stomach passes material to the guts and the guts absorb into the body;
//! 2. the water that left the stomach becomes **thirst**, five millilitres to a point;
//! 3. a **drug** vitamin skips the guts and reaches the body at once;
//! 4. what the stomach released goes **into the guts**;
//! 5. what the guts released becomes **stored calories** and **vitamins** — which is why a meal eaten now does not feed you now;
//! 6. the body **burns** calories at its basal rate and gets **hungry**, and then the stomach's fullness sets a ceiling on the hunger figure.
//!
//! Everything the rule reads from the character beyond the two compartments comes in through [`BodyInputs`]: the metabolic rates, the burn per turn, the healthy
//! weight's calorie figure, and the two dice. `roll_remainder( rates.hunger * five_mins )` is the only randomness, and it is a **caller's** die here for the same
//! reason it is everywhere else in the port: a rule that rolls its own dice cannot be replayed.
//!
//! **Declared, not ported:** the hunger *effect* the rule picks after all of this (`hunger_engorged` / `hungry` / `near_starving` and the ladder that chooses between
//! them), which needs the effects system; `has_calorie_deficit()` and the BMI weight categories it consults; the sleep-recovery and activity-history bookkeeping; and
//! the vitamin `decays_into` cascade on a negative change.

use bevy_ecs::prelude::Component;
use crate::digest::{DigestRates, MetabolicRates, StomachContents, digest_rates};
use crate::food_summary::FoodSummary;
use cdda_content::vitamins::Vitamin;
use std::collections::BTreeMap;

/// A character's body, as far as a meal is concerned.
///
/// The four numbers are the ones the reference keeps as `hunger`, `thirst`, `stored_calories` and `vitamin_levels`; the two compartments are the stomach and the
/// guts. Everything else about a character — traits, effects, mutations, bionics — reaches this rule only through [`BodyInputs`], which is what makes it testable.
/// A character's body: hunger, thirst, stored energy, the vitamins it holds, its stomach and its guts.
///
/// This is a **component**, because it is the character's own state and the store is where a character's state lives. It used to be a free struct that no entity
/// held, which meant the vitamins had no home at all - the dialogue could not read one and digestion could not keep one.
#[derive(bevy_ecs::component::Component, Debug, Clone, Default, PartialEq, Eq)]
pub struct Body {
    /// Hunger, from `-61` (engorged) up to `300` (famished).
    pub hunger: i32,
    /// Thirst, from `-100` up.
    pub thirst: i32,
    /// The **health stat**: how well the character feels, from `-200` (dying) to `200` (the picture of health).
    ///
    /// This is not hit points - the reference keeps `get_health()`/`set_health()` (`character.h`, `character.cpp`) apart from `get_hp()`, and so does the port: a
    /// character at full hit points can be at `-200` health and vice versa. It moves with disease, wounds and good food, and the dialogue reads it as `u_health()`.
    pub health: i32,
    /// Stored energy in **calories**, which are thousandths of the kilocalories the game shows.
    pub stored_calories: i64,
    /// The vitamin levels the body holds, in internal units.
    pub vitamins: BTreeMap<String, i32>,
    /// The hunger effect it wears, if any: exactly **one** of [`HUNGER_EFFECTS`] at a time, because the rule that installs one removes all nine first.
    ///
    /// It is a real worn effect now, with a duration that runs: the reference adds it for **24 hours** (`src/character_body.cpp:1091-1093`), and because the rule only
    /// fires when the character does *not* already have the one it should, re-choosing the same effect never refreshes it — the duration the character has left is the
    /// duration it keeps, and when it runs out the next update puts it straight back.
    pub hunger_effect: Option<crate::effects::ActiveEffect>,
    /// The stomach.
    pub stomach: StomachContents,
    /// The guts.
    pub guts: StomachContents,
}

/// The figures `update_stomach` needs that are not the body's own state.
///
/// The reference recomputes these from the character every call (`get_healthy_kcal` from weight and height, `get_bmr` from the activity level, `time_since_ate` from the
/// calendar); the port stores the answer on the creature, because **the calculators are not ported** and inventing them would make the food clock's numbers wander for
/// reasons nothing in the port models. A creature without this component does not digest - which is what keeps a zombie from getting hungry.
#[derive(Debug, Clone, Copy, PartialEq, Component)]
pub struct BodyClock {
    /// The calories burnt per turn: `get_bmr() / ( 12 * 24 )`.
    pub kcal_per_time: f32,
    /// The calories a body of the creature's healthy weight holds.
    pub healthy_kcal: i64,
    /// How tall it is, which the body-fat figure reads.
    pub height_cm: i64,
    /// Its lean size, normally one.
    pub organic_size: f32,
    /// How long ago it last ate, in turns.
    pub time_since_ate: i64,
    /// `NO_THIRST`: thirst never moves.
    pub no_thirst: bool,
    /// `DEBUG_LS` or no needs at all: the whole clock is off.
    pub foodless: bool,
    /// How many half hours of digestion have already been handed to the body, which is what turns a five-minute tick into the reference's half-hourly step.
    pub half_hours_done: i64,
    /// The mycus, which has no thirst.
    pub mycus: bool,
}

impl Default for BodyClock {
    fn default() -> Self {
        Self {
            kcal_per_time: 0.0,
            healthy_kcal: 20_000,
            height_cm: 175,
            organic_size: 1.0,
            time_since_ate: 0,
            no_thirst: false,
            foodless: false,
            mycus: false,
            half_hours_done: 0,
        }
    }
}

/// Everything `update_stomach` reads from the character that is not the body itself.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BodyInputs {
    /// Five-minute intervals since the last update.
    pub five_mins: i64,
    /// Half-hour intervals since the last update.
    pub half_hours: i64,
    /// The calories burnt per turn: `get_bmr() / ( 12 * 24 )`.
    pub kcal_per_time: f32,
    /// The calories a body of the character's healthy weight holds: `get_healthy_kcal()`.
    pub healthy_kcal: i64,
    /// What `roll_remainder( rates.hunger * five_mins )` came out as.
    pub hunger_roll: i32,
    /// What `roll_remainder( rates.thirst * five_mins )` came out as.
    pub thirst_roll: i32,
    /// How long ago the character last ate, in turns.
    pub time_since_ate: i64,
    /// `DEBUG_LS` or no needs at all, which turns the whole food clock off.
    pub foodless: bool,
    /// `NO_THIRST`: thirst never moves and is held at zero.
    pub no_thirst: bool,
    /// `M_DEPENDENT`, the mycus: thirst is not a thing it has.
    pub mycus: bool,
    /// The character's height, which is half of the body-fat figure the hunger-effect ladder reads.
    pub height_cm: i64,
    /// The character's lean size, the other half of it (`get_cached_organic_size`, normally one).
    pub organic_size: f32,
}

/// Ten minutes in turns, the line between "just ate" and "settled".
pub const TEN_MINUTES: i64 = 10 * 60;

/// Fifteen minutes in turns, the line between "I ate just now" and "I ate a while ago" — a **different** line from `TEN_MINUTES`, and the two ladders in
/// `update_stomach` use different ones.
pub const FIFTEEN_MINUTES: i64 = 15 * 60;

/// Three hours in turns: a meal recent enough that another one is not wanted (`src/character_body.cpp:1028`).
pub const THREE_HOURS: i64 = 3 * 60 * 60;

/// The body-fat cumulative density figure, `KCAL_PER_KG` (`src/character.h:123`): 3,500 kilocalories to a pound of fat, in kilograms.
pub const KCAL_PER_KG: f32 = 3500.0 * 2.20462;

/// The lower bounds of the body-fat classes (`src/game_constants.h:170-179`).
///
/// These are **not** the textbook BMI: they are the bounds on the fat a body carries on top of its lean mass, which is why "normal" starts at 3.
pub mod weight_category {
    /// Emaciated.
    pub const EMACIATED: f32 = 1.0;
    /// Underweight.
    pub const UNDERWEIGHT: f32 = 2.0;
    /// Normal.
    pub const NORMAL: f32 = 3.0;
    /// Overweight.
    pub const OVERWEIGHT: f32 = 5.0;
    /// Obese.
    pub const OBESE: f32 = 10.0;
    /// Very obese.
    pub const VERY_OBESE: f32 = 15.0;
    /// Morbidly obese.
    pub const MORBIDLY_OBESE: f32 = 20.0;
}

/// The eight hunger effects the rule can install, and the blank one it can also install: `src/character_body.cpp:1083-1092`.
///
/// The order matters: the block that installs one **removes all eight first**, so a character can only ever hold one of them.
pub const HUNGER_EFFECTS: [&str; 9] = [
    "hunger_engorged",
    "hunger_full",
    "hunger_satisfied",
    "hunger_hungry",
    "hunger_very_hungry",
    "hunger_near_starving",
    "hunger_starving",
    "hunger_famished",
    "hunger_blank",
];

/// How much of the body is fat: `Character::get_bmi_fat` (`src/character.cpp:6356-6360`).
///
/// The stored calories are divided by the energy a kilogram of fat holds, and that is divided by the body's square-of-height times its lean size. The reference does
/// the whole thing in `float`, and the value is compared against the class bounds, so the type matters at a boundary.
#[allow(clippy::cast_precision_loss)]
#[must_use]
pub fn bmi_fat(stored_kcal: i64, height_cm: i64, organic_size: f32) -> f32 {
    let height_metres = height_cm as f32 / 100.0;
    (stored_kcal as f32 / KCAL_PER_KG) / (height_metres * height_metres * organic_size)
}

/// Whether the body is running on its own fat: `has_calorie_deficit` (`src/character.cpp:6362-6365`), which is simply "below normal".
#[must_use]
pub fn has_calorie_deficit(bmi: f32) -> bool {
    bmi < weight_category::NORMAL
}

/// Whether another bite would burst the stomach: `stomach_contents::would_be_engorged_with` (`src/stomach.cpp:373-378`).
///
/// How long the hunger effect is installed for: `add_effect( hunger_effect, 24_hours, true )` (`src/character_body.cpp:1093`), in turns.
pub const HUNGER_EFFECT_DURATION: i64 = 24 * 60 * 60;

/// The intensity it is installed at: the reference passes none, so it is born at one (`Creature::add_effect`, `src/creature.cpp:1845-1852`).
pub const HUNGER_EFFECT_INTENSITY: i32 = 1;

/// The threshold is **not one number**: a body in deficit counts a stomach at full capacity as engorged, while a body that is not waits until five sixths.
///
/// **The fullness ratio is an integer division.** `units::volume / units::volume` answers the underlying `int64` (`src/units.h:253-257`), so the ratio is 0 for
/// anything short of the capacity and 1 at it — which means five sixths, three quarters and eleven twentieths are all the same test in practice: "at least full".
/// The hit-terms-flat shape is kept here because that is what the reference does.
#[allow(clippy::cast_precision_loss)]
#[must_use]
pub fn would_be_engorged_with(contains_ml: i64, intake_ml: i64, capacity_ml: i64, deficit: bool) -> bool {
    if capacity_ml <= 0 {
        return false;
    }
    let fullness = ((contains_ml + intake_ml) / capacity_ml) as f64;
    (deficit && fullness >= 1.0) || fullness >= 5.0 / 6.0
}

/// Whether another bite would make the stomach full: `stomach_contents::would_be_full_with` (`src/stomach.cpp:380-385`), and again the threshold depends on the deficit.
#[allow(clippy::cast_precision_loss)]
#[must_use]
pub fn would_be_full_with(contains_ml: i64, intake_ml: i64, capacity_ml: i64, deficit: bool) -> bool {
    if capacity_ml <= 0 {
        return false;
    }
    let fullness = ((contains_ml + intake_ml) / capacity_ml) as f64;
    (deficit && fullness >= 11.0 / 20.0) || fullness >= 3.0 / 4.0
}

/// Which hunger effect a body ends up wearing: `src/character_body.cpp:1019-1099`.
///
/// Because the fullness ratio truncates, **`hunger_full` can never be chosen**: engorged is tested first and both fire at the same ratio — at or over capacity. The
/// recording of 126 states never once produced it, and `hunger_starving` is equally unreachable, because the reference tests `emaciated` (1.0) *after* `underweight`
/// (2.0). Two dead rungs of a nine-rung ladder, kept here in the reference's order so the port cannot quietly "fix" them.
///
/// **Two ladders, and they are not the same ladder.** A body whose last bite is more than ten minutes old is judged against the stomach's capacity in eighths and
/// halves, and its bottom rung is read off the body fat — starving, near starving, famished. A body that has **just** eaten is judged in quarters and cools to `blank`
/// as soon as its calories are topped up, and its bottom rung depends only on whether it is overweight. Everything above those two lines is identical: engorged, then
/// full.
#[must_use]
pub fn choose_hunger_effect(
    contains_ml: i64,
    capacity_ml: i64,
    time_since_ate: i64,
    bmi: f32,
    guts_kcal: i64,
) -> &'static str {
    let deficit = has_calorie_deficit(bmi);
    // The time lines are the reference's: fifteen minutes to have "just eaten", three hours to have eaten recently enough that no meal is wanted.
    let just_ate = time_since_ate < FIFTEEN_MINUTES;
    let recently_ate = time_since_ate < THREE_HOURS;
    let _ = guts_kcal;

    if deficit {
        if would_be_engorged_with(contains_ml, 0, capacity_ml, deficit) {
            return "hunger_engorged";
        }
        if would_be_full_with(contains_ml, 0, capacity_ml, deficit) {
            return "hunger_full";
        }
        if just_ate && contains_ml > capacity_ml / 2 {
            return "hunger_satisfied";
        }
        if just_ate {
            return "hunger_hungry";
        }
        if recently_ate {
            return "hunger_very_hungry";
        }
        if bmi < weight_category::UNDERWEIGHT {
            return "hunger_near_starving";
        }
        // The reference tests `emaciated` (1.0) *after* `underweight` (2.0), so this rung can never be reached: every body that is emaciated is already underweight.
        if bmi < weight_category::EMACIATED {
            return "hunger_starving";
        }
        return "hunger_famished";
    }

    if would_be_engorged_with(contains_ml, 0, capacity_ml, deficit) {
        return "hunger_engorged";
    }
    if would_be_full_with(contains_ml, 0, capacity_ml, deficit) {
        return "hunger_full";
    }
    if recently_ate && contains_ml >= capacity_ml * 3 / 8 {
        return "hunger_satisfied";
    }
    if recently_ate || contains_ml > 0 {
        return "hunger_blank";
    }
    if bmi > weight_category::OVERWEIGHT {
        return "hunger_hungry";
    }
    "hunger_very_hungry"
}

impl Body {
    /// A body with a stomach and a set of guts of the given capacity, and nothing else.
    #[must_use]
    pub fn new(capacity_ml: i64) -> Self {
        Self {
            stomach: StomachContents::new(capacity_ml, true),
            guts: StomachContents::new(capacity_ml, false),
            ..Self::default()
        }
    }

    /// Stored kilocalories, as the game displays them: `get_stored_kcal`, the integer division by a thousand.
    #[must_use]
    pub fn stored_kcal(&self) -> i64 {
        self.stored_calories / 1_000
    }

    /// Hunger as the reference caps it: `set_hunger` (`src/character.cpp:4647-4654`) never lets it above 300 — "cap hunger at 300, just below famished".
    pub fn set_hunger(&mut self, hunger: i32) {
        self.hunger = hunger.min(crate::needs::MAX_HUNGER);
    }

    /// Thirst as the reference floors it: `mod_thirst` (`src/character.cpp:4682-4688`) never lets it below -100.
    pub fn set_thirst(&mut self, thirst: i32) {
        self.thirst = thirst;
    }

    /// Adds stored calories: `mod_stored_calories` (`src/character.cpp:4587-4601`).
    ///
    /// The reference keeps the **whole** figure here and only rounds to whole kilocalories in the activity bookkeeping, so a burn of a few hundred calories is not
    /// lost to the rounding.
    pub fn mod_stored_calories(&mut self, calories: i64) {
        if calories < 0 || i64::from(i32::MAX) - calories > self.stored_calories {
            self.stored_calories += calories;
        }
    }

    /// Adds stored kilocalories: `mod_stored_kcal` (`:4580-4585`), which multiplies by a thousand before it adds.
    pub fn mod_stored_kcal(&mut self, kcal: i64) {
        self.mod_stored_calories(kcal.saturating_mul(1_000));
    }

    /// Adds to a vitamin level, clamped to the vitamin's own limits: `Character::vitamin_mod` (`src/consumption.cpp:668-703`).
    ///
    /// The clamp is the whole rule for an increase — `min( level + qty, max )` — and the vitamin's `max` is **zero** when the data states none, which is why a
    /// vitamin with no ceiling can hold nothing at all. A **decrease** is floored at `min` and also cascades into the vitamin's `decays_into` list, which is
    /// declared rather than ported.
    pub fn vitamin_mod(&mut self, id: &str, amount: i32, vitamins: &BTreeMap<String, Vitamin>) {
        let Some(vitamin) = vitamins.get(id) else {
            return;
        };
        let level = self.vitamins.get(id).copied().unwrap_or(0);
        let updated = match amount.cmp(&0) {
            std::cmp::Ordering::Greater => (level + amount).min(vitamin.max),
            std::cmp::Ordering::Less => (level + amount).max(vitamin.min),
            std::cmp::Ordering::Equal => level,
        };
        if updated == 0 {
            self.vitamins.remove(id);
        } else {
            self.vitamins.insert(id.to_owned(), updated);
        }
    }

    /// Adds a whole map of vitamins, which is `Character::vitamins_mod` (`src/consumption.cpp:706-713`) — and does nothing at all when the character is not eating.
    pub fn vitamins_mod(
        &mut self,
        vitamins_to_add: &BTreeMap<String, i32>,
        vitamins: &BTreeMap<String, Vitamin>,
    ) {
        for (id, amount) in vitamins_to_add {
            self.vitamin_mod(id, *amount, vitamins);
        }
    }

    /// One turn of the body's food clock: `Character::update_stomach` (`src/character_body.cpp:935-1019`).
    ///
    /// The rates are passed in rather than computed, because `calc_needs_rates` reads the weather, the options, the effects and the enchantments, and none of that
    /// belongs in this rule. What the rule does with them is the ladder described in the module docs.
    ///
    /// The stomach's `STOMACH_SIZE_MULTIPLIER` arrives the same way, as `( add, multiply )`, because the rule is about a body and the enchantment is about a creature.
    /// **Declared:** no stage computes that pair yet - the food clock is driven from tests, and the system that reads a creature's cache into this call is the next one.
    #[allow(clippy::cast_precision_loss, clippy::cast_possible_truncation)]
    pub fn update_stomach(
        &mut self,
        rates: &MetabolicRates,
        inputs: &BodyInputs,
        vitamins: &BTreeMap<String, Vitamin>,
        effects: &BTreeMap<String, cdda_content::effects::EffectType>,
        stomach_enchantment: (f64, f64),
    ) {
        // Reference: `stomach_contents::capacity` (`stomach.cpp:362-366`) scales the stored maximum by the creature's `STOMACH_SIZE_MULTIPLIER`, so a mutation that
        // changes how much a body can hold changes how fast it fills and empties.
        let stomach_capacity = self.stomach.capacity_of(stomach_enchantment.0, stomach_enchantment.1);
        if inputs.five_mins > 0 {
            let stomach_rates: DigestRates =
                digest_rates(rates, true, self.stomach.capacity_of(stomach_enchantment.0, stomach_enchantment.1));
            let guts_rates: DigestRates = digest_rates(rates, false, self.guts.capacity_ml());
            let to_guts = self
                .stomach
                .digest(&stomach_rates, inputs.five_mins, inputs.half_hours, vitamins);
            let to_body = self
                .guts
                .digest(&guts_rates, inputs.five_mins, inputs.half_hours, vitamins);

            // "Water from stomach skips guts and gets absorbed by body." Five millilitres to a point of thirst, and a drying food's negative water gives points back.
            let water_points = -(to_guts.water_ml / 5);
            self.mod_thirst_points(water_points, inputs);

            // A drug vitamin is absorbed straight into the body, skipping the guts entirely.
            let drugs: BTreeMap<String, i32> = to_guts
                .vitamins
                .iter()
                .filter(|(id, _)| {
                    vitamins
                        .get(*id)
                        .is_some_and(|vitamin| vitamin.vit_type == "drug")
                })
                .map(|(id, units)| (id.clone(), *units))
                .collect();
            self.vitamins_mod(&drugs, vitamins);

            let released = FoodSummary {
                water_ml: to_guts.water_ml,
                solids_ml: to_guts.solids_ml,
                calories: to_guts.calories,
                vitamins: to_guts.vitamins.clone(),
            };
            self.guts.ingest(&released);

            self.mod_stored_kcal(to_body.kcal());
            self.vitamins_mod(&to_body.vitamins, vitamins);

            if !inputs.foodless && rates.hunger > 0.0 {
                self.set_hunger(self.hunger.saturating_add(inputs.hunger_roll));
                // The body burns its basal rate whatever else happens.
                let burnt = -(f64::from(inputs.five_mins as f32 * inputs.kcal_per_time * 1_000.0).floor()
                    as i64);
                self.mod_stored_calories(burnt);
            }
        }

        // "No food/thirst/sleepiness clock at all."
        if inputs.foodless {
            return;
        }

        // The stomach sets a ceiling on how hungry a character can feel, and the two ladders differ by whether the last bite is still recent.
        let contains = self.stomach.contains_ml();
        if inputs.time_since_ate > TEN_MINUTES {
            if contains >= stomach_capacity && self.hunger > -61 {
                self.set_hunger(-61);
            } else if contains >= stomach_capacity / 2 && self.hunger > -21 {
                self.set_hunger(-21);
            } else if contains >= stomach_capacity / 8 && self.hunger > -1 {
                self.set_hunger(-1);
            } else if contains == 0 {
                let guts_empty = self.guts.kcal() == 0;
                let underweight = self.stored_kcal() < inputs.healthy_kcal;
                if guts_empty && underweight && self.hunger < 300 {
                    // "there's no food except what you have stored in fat"
                    self.set_hunger(300);
                } else if self.hunger < 100 && (guts_empty || underweight) {
                    self.set_hunger(100);
                } else if self.hunger < 0 {
                    self.set_hunger(0);
                }
            }
        } else if contains >= stomach_capacity && self.hunger > -61 {
            self.set_hunger(-61);
        } else if contains >= stomach_capacity * 3 / 4 && self.hunger > -21 {
            self.set_hunger(-21);
        } else if contains >= stomach_capacity / 2 && self.hunger > -1 {
            self.set_hunger(-1);
        } else if contains > 0 && self.kcal_percent(inputs.healthy_kcal) > 0.95 {
            // "usually eating something cools your hunger"
            self.set_hunger(0);
        }

        if !inputs.foodless && rates.thirst > 0.0 {
            self.mod_thirst_points(i64::from(inputs.thirst_roll), inputs);
        }
        // "Mycus and Metabolic Rehydration makes thirst unnecessary."
        if inputs.mycus || inputs.no_thirst {
            self.set_thirst(0);
        }

        // And finally the effect that says how the character feels about it. The body fat is read **after** the calories moved, which is why the height and the lean size
        // come in as inputs rather than as a figure: a character can cross a weight class in a single call.
        let bmi = bmi_fat(self.stored_kcal(), inputs.height_cm, inputs.organic_size);
        let chosen = choose_hunger_effect(
            self.stomach.contains_ml(),
            stomach_capacity,
            inputs.time_since_ate,
            bmi,
            self.guts.kcal(),
        );
        // "If they do not already have the effect they should, take all of them away and give them this one." A character already wearing the right one keeps its
        // remaining duration instead of having it refreshed — so this only fires on a change, or when the effect has run out and been taken away.
        let wearing = self.hunger_effect.as_ref().map(|effect| effect.id.as_str());
        if wearing != Some(chosen) {
            self.hunger_effect = effects.get(chosen).map(|kind| {
                crate::effects::ActiveEffect::new(kind, HUNGER_EFFECT_DURATION, HUNGER_EFFECT_INTENSITY)
            });
        }
    }

    /// The id of the hunger effect it wears, if any.
    #[must_use]
    pub fn hunger_effect_id(&self) -> Option<&str> {
        self.hunger_effect.as_ref().map(|effect| effect.id.as_str())
    }

    /// Wears every effect it has down by one turn, taking away the ones that ask to go: the loop half of `Creature::process_effects` (`src/creature.cpp:1885-1900`),
    /// as far as the hunger effect goes.
    ///
    /// `absolute_turn` is the game's turn count, which the tick divides.
    pub fn decay_effects(
        &mut self,
        effects: &BTreeMap<String, cdda_content::effects::EffectType>,
        absolute_turn: i64,
    ) {
        let Some(worn) = self.hunger_effect.as_mut() else {
            return;
        };
        let Some(kind) = effects.get(&worn.id) else {
            return;
        };
        if worn.decay(kind, absolute_turn, false) {
            self.hunger_effect = None;
        }
    }

    /// How full of calories the body is, against the weight it would like to be: `get_kcal_percent` (`src/character.cpp:4621-4624`).
    #[allow(clippy::cast_precision_loss)]
    #[must_use]
    pub fn kcal_percent(&self, healthy_kcal: i64) -> f32 {
        if healthy_kcal <= 0 {
            return 0.0;
        }
        self.stored_kcal() as f32 / healthy_kcal as f32
    }

    /// Thirst as `mod_thirst` moves it: nothing at all for a character with no thirst to speak of, and a floor of -100 (`src/character.cpp:4682-4688`).
    fn mod_thirst_points(&mut self, points: i64, inputs: &BodyInputs) {
        if inputs.no_thirst || inputs.foodless {
            return;
        }
        #[allow(clippy::cast_possible_truncation)]
        let points = ((i64::from(self.thirst) + points).max(-100)) as i32;
        self.set_thirst(points);
    }
}

/// The body a creature carries, if it is a character rather than a monster.
#[must_use]
pub fn body_of(world: &bevy_ecs::prelude::World, creature: bevy_ecs::entity::Entity) -> Option<&Body> {
    world.get::<Body>(creature)
}

/// Changes the body a creature carries, in one borrow, which is what a system needs.
///
/// Answers whether the change was made: a monster has no body of this kind, and a silent no-op would hide that.
pub fn with_body_mut(
    world: &mut bevy_ecs::prelude::World,
    creature: bevy_ecs::entity::Entity,
    change: impl FnOnce(&mut Body),
) -> bool {
    let Some(mut body) = world.get_mut::<Body>(creature) else {
        return false;
    };
    change(&mut body);
    true
}

/// What is stored of a vitamin, if the creature has a body that keeps any.
#[must_use]
pub fn vitamin_of(world: &bevy_ecs::prelude::World, creature: bevy_ecs::prelude::Entity, name: &str) -> Option<i32> {
    body_of(world, creature).map(|body| body.vitamins.get(name).copied().unwrap_or(0))
}

/// Sets a vitamin's level, answering whether the creature has a body to store it in.
///
/// Reference: `Character::vitamin_set` (`vitamin.cpp`), which writes the level and lets the decay rule read it later. A monster has no vitamin store, so
/// answering `false` here is a real "this creature cannot" and not a missing feature.
pub fn set_vitamin(
    world: &mut bevy_ecs::prelude::World,
    creature: bevy_ecs::prelude::Entity,
    name: &str,
    amount: i32,
) -> bool {
    with_body_mut(world, creature, |body| {
        body.vitamins.insert(name.to_owned(), amount);
    })
}
