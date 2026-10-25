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

use crate::digest::{DigestRates, MetabolicRates, StomachContents, digest_rates};
use crate::food_summary::FoodSummary;
use cdda_content::vitamins::Vitamin;
use std::collections::BTreeMap;

/// A character's body, as far as a meal is concerned.
///
/// The four numbers are the ones the reference keeps as `hunger`, `thirst`, `stored_calories` and `vitamin_levels`; the two compartments are the stomach and the
/// guts. Everything else about a character — traits, effects, mutations, bionics — reaches this rule only through [`BodyInputs`], which is what makes it testable.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Body {
    /// Hunger, from `-61` (engorged) up to `300` (famished).
    pub hunger: i32,
    /// Thirst, from `-100` up.
    pub thirst: i32,
    /// Stored energy in **calories**, which are thousandths of the kilocalories the game shows.
    pub stored_calories: i64,
    /// The vitamin levels the body holds, in internal units.
    pub vitamins: BTreeMap<String, i32>,
    /// The stomach.
    pub stomach: StomachContents,
    /// The guts.
    pub guts: StomachContents,
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
}

/// Ten minutes in turns, the line between "just ate" and "settled".
pub const TEN_MINUTES: i64 = 10 * 60;

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
        self.hunger = hunger.min(300);
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
    #[allow(clippy::cast_precision_loss, clippy::cast_possible_truncation)]
    pub fn update_stomach(
        &mut self,
        rates: &MetabolicRates,
        inputs: &BodyInputs,
        vitamins: &BTreeMap<String, Vitamin>,
    ) {
        let stomach_capacity = self.stomach.capacity_ml();
        if inputs.five_mins > 0 {
            let stomach_rates: DigestRates =
                digest_rates(rates, true, self.stomach.capacity_ml());
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
