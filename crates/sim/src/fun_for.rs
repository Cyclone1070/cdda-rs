//! How much fun a character gets from eating something, and how much they could get from it.
//!
//! Reference: `Character::fun_for` (`src/consumption.cpp:483-...`), transcribed upstream as `tests/food_fun_for_test.cpp`. The function is **pure** -
//! it reads the food, the eater and the eater's recent meals and returns a pair of integers - which is why it is a free function here rather than a
//! method on anything: the systems that will feed it have not been written yet, and a rule this branchy is worth gating before they are.
//!
//! The two numbers are not the same question:
//!
//! * the **first** is what the bite is worth right now;
//! * the **second** is the ceiling - `fun * 3` for food that tastes good and `fun * 6` for food that tastes bad, because the reference's morale
//!   modifier scales a positive fun by three and a negative one by six.
//!
//! It is all `float` arithmetic in the reference and is `f32` here: the reference divides by three for a cold, halves half of a melted ice cream and
//! multiplies a gourmand's fun by `3 / 2`, and none of that survives being done in integers.

use std::collections::BTreeSet;

/// The seven foods aimed at one species, and the threshold trait that likes them (`src/consumption.cpp:543-551`).
pub const THRESHOLD_FOODS: [(&str, &str); 7] = [
    ("LUPINE", "THRESH_LUPINE"),
    ("CATTLE", "THRESH_CATTLE"),
    ("RABBIT", "THRESH_RABBIT"),
    ("MOUSE", "THRESH_MOUSE"),
    ("RAT", "THRESH_RAT"),
    ("BIRD", "THRESH_BIRD"),
    ("FELINE", "THRESH_FELINE"),
];

/// Two days, in turns: `time_duration::from_turns` counts a turn as a second (`src/calendar.h`), which is what the monotony window compares against.
pub const TWO_DAYS: i64 = 2 * 24 * 60 * 60;

/// A meal the eater has already had, which is what makes the fifth toast-em less fun than the first.
///
/// Reference: `consumption_event` (`src/consumption.h`), kept as a list on the character. Two meals count as the same food when both the **type** and the
/// **component hash** match, and only meals from the last two days count at all (`src/consumption.cpp:512-521`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConsumptionEvent {
    /// When it was eaten, in turns since the cataclysm.
    pub when: i64,
    /// What was eaten.
    pub type_id: String,
    /// Which components went into it: a toast-em made with a different jam is a different meal.
    pub component_hash: u64,
}

/// The food, as the rule sees it.
#[derive(Debug, Clone, Copy)]
pub struct Food<'a> {
    /// What it is, for matching meals in the history.
    pub type_id: &'a str,
    /// Its stated fun, before anything here touches it.
    pub fun: i32,
    /// How much fun one more plate of it costs (the comestible's `monotony_penalty`).
    pub monotony_penalty: i32,
    /// Its item flags, which is where `EATEN_COLD`, `MELTS` and the threshold flags live.
    pub flags: &'a BTreeSet<String>,
    /// How far through its rot it is: above `1.0` it is rotten.
    pub relative_rot: f32,
    /// Which components went into it, for matching meals in the history.
    pub component_hash: u64,
    /// Whether it is **food** rather than a drink, which the gross-food bionic asks about (`item::is_food`).
    pub is_food: bool,
}

/// The eater, as the rule sees them.
#[derive(Debug, Clone, Copy)]
pub struct Eater<'a> {
    /// The effects on them, by id: `common_cold` and `flu` are the two that matter here.
    pub effects: &'a BTreeSet<String>,
    /// Their traits, by id.
    pub traits: &'a BTreeSet<String>,
    /// Their json flags, by id: `BLOODFEEDER` and `HEMOVORE`.
    pub flags: &'a BTreeSet<String>,
    /// Their **active** bionics, by id.
    pub active_bionics: &'a BTreeSet<String>,
    /// Their **installed** bionics, by id.
    pub bionics: &'a BTreeSet<String>,
    /// Their power level, in kilojoules: the taste blocker spends one kilojoule per unit of bad taste it hides.
    pub power_kj: i64,
    /// Every meal they have eaten.
    pub history: &'a [ConsumptionEvent],
    /// The current turn, for the two-day window.
    pub now: i64,
}

/// How much the eater would enjoy this bite, and how much they could enjoy it at best.
///
/// Reference: `Character::fun_for` (`src/consumption.cpp:483`). `ignore_already_ate` is the second argument there, which skips the monotony penalty; it is
/// `false` here because nothing has needed the other yet.
#[must_use]
// The reference keeps the whole calculation in `float` and truncates once at the end; the casts are that arithmetic, not a shortcut.
#[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)]
pub fn fun_for(food: &Food<'_>, eater: &Eater<'_>) -> (i32, i32) {
    // As `float` to avoid rounding too many times, which is what the reference says at the top of the function and does.
    let mut fun = food.fun as f32;

    // **Food doesn't taste as good when you're sick** - a third as good, and only while it was worth something to begin with (`:489-495`).
    let sick = eater.effects.contains("common_cold") || eater.effects.contains("flu");
    if sick && fun > 0.0 {
        fun /= 3.0;
    }

    // **Rotten food should be pretty disgusting** (`:496-509`). Three things move at once as the rot climbs: a penalty from -2 to -20, a bonus that drops from
    // 90% to nothing for food that tasted good, and a penalty that grows from 110% to 200% for food that tasted bad. The `clamp` is what makes even the *first*
    // point of rot past 1.0 cost something.
    if food.relative_rot > 1.0 && !eater.traits.contains("SAPROPHAGE") && !eater.traits.contains("SAPROVORE") {
        let rottedness = (2.0 * food.relative_rot - 2.0).clamp(0.1, 1.0);
        fun -= rottedness * 10.0;
        if fun > 0.0 {
            fun *= 1.0 - rottedness;
        } else {
            fun *= 1.0 + rottedness;
        }
    }

    // **Food is less enjoyable when eaten too often** (`:511-522`). Each meal of the same food, made the same way, within the last two days costs the comestible's
    // `monotony_penalty` - but the penalty may never push a food into tasting bad: the loop stops at zero unless the food is marked `NEGATIVE_MONOTONY_OK`, which
    // is what makes sour food stay sour. Food that already tastes bad is not penalised at all.
    if fun > 0.0 || food.flags.contains("NEGATIVE_MONOTONY_OK") {
        for event in eater.history {
            if event.when > eater.now - TWO_DAYS
                && event.type_id == food.type_id
                && event.component_hash == food.component_hash
            {
                fun -= food.monotony_penalty as f32;
                if fun <= 0.0 && !food.flags.contains("NEGATIVE_MONOTONY_OK") {
                    fun = 0.0;
                    break;
                }
            }
        }
    }

    // **The ceiling**, which the reference works out before the temperature and the traits move the bite itself (`:523`): three times the fun for food that
    // tastes good, six times for food that tastes bad, because a positive fun is scaled by three when it becomes morale and a negative one by six.
    let mut fun_max = if fun < 0.0 { fun * 6.0 } else { fun * 3.0 };

    // **Cold food that wants to be eaten cold** (`:524-531`). Doubled when it was already worth something; when it was not, it is rescued to a flat 1 with a
    // ceiling of 5 rather than doubled into being worse.
    if food.flags.contains("EATEN_COLD") && food.flags.contains("COLD") {
        if fun > 0.0 {
            fun *= 2.0;
        } else {
            fun = 1.0;
            fun_max = 5.0;
        }
    }

    // **Melted food** (`:533-541`). A melted ice cream is half as good; a melted food that already tasted bad is 25% worse. Note the ceiling is **not** moved:
    // only the bite is.
    if food.flags.contains("MELTS") && !food.flags.contains("FROZEN") {
        if fun > 0.0 {
            fun *= 0.5;
        } else {
            fun *= 1.25;
        }
    }

    // **Food aimed at a species** (`:543-558`). Seven flags pair with seven thresholds - dog food and a lupine, cat food and a feline, and the same for cattle,
    // rabbits, mice, rats and birds. Unpleasant becomes pleasant, halved; a food worth nothing becomes worth 2. Note this happens **after** the ceiling, so the
    // ceiling keeps the unhappy number the bite used to have.
    if THRESHOLD_FOODS
        .iter()
        .any(|(food_flag, threshold)| food.flags.contains(*food_flag) && eater.traits.contains(*threshold))
    {
        if fun < 0.0 {
            fun = -fun;
            fun /= 2.0;
        }
        if fun == 0.0 {
            fun = 2.0;
        }
    }

    // **A gourmand tastes everything harder** (`:585-592`). Food they dislike - below -1, so a -1 is left alone - becomes half as bad with the **ceiling** taking the
    // old bite; good food becomes `fun * 3 / 2` with three times the ceiling. Note `fun / 2` is applied to a negative number, so it moves *toward* zero.
    if eater.traits.contains("GOURMAND") {
        if fun < -1.0 {
            fun_max = fun;
            fun /= 2.0;
        } else if fun > 0.0 {
            fun_max *= 3.0;
            fun = fun * 3.0 / 2.0;
        }
    }

    // **Blood, and the two ways to be a blood eater** (`:560-578`). Blood food is pleasant to something with `BLOODFEEDER` - 25 on anything not worth something,
    // 20% on anything that was - and a little less so to a plain `HEMOVORE`: 13 and 10%. Both happen **after** the ceiling, and neither touches it.
    if food.flags.contains("HEMOVORE_FUN") {
        if eater.flags.contains("BLOODFEEDER") {
            if fun <= 0.0 {
                fun += 25.0;
            } else {
                fun *= 1.2;
            }
        } else if eater.flags.contains("HEMOVORE") {
            if fun <= 0.0 {
                fun += 13.0;
            } else {
                fun *= 1.1;
            }
        }
    }

    // "This cherry soda's just not the same": **good food that is not blood** is half as interesting to a bloodfeeder (`:580-583`).
    if eater.flags.contains("BLOODFEEDER") && !food.flags.contains("HEMOVORE_FUN") && fun > 0.0 {
        fun *= 0.5;
    }

    // **A faulty gross-food bionic makes everything a little worse** (`:594-597`): 13 off, and only food - the reference asks `is_food()`, so a drink is untouched.
    if eater.bionics.contains("bio_faulty_grossfood") && food.is_food {
        fun -= 13.0;
    }

    // **The taste blocker** (`:599-604`). It hides bad taste **entirely** or not at all: the condition is `power > |the food's stated fun|`, in kilojoules, and the
    // comparison is against the *stated* fun rather than the bite the eater would have had - so a rotten garlic still costs only its own 15 to hide. Exactly
    // enough power is not enough.
    let stated = i64::from(food.fun).abs();
    if fun < 0.0 && eater.active_bionics.contains("bio_taste_blocker") && eater.power_kj > stated {
        fun = 0.0;
    }

    (fun as i32, fun_max as i32)
}
