//! What a body part is worth once it is hurt or carrying something.
//!
//! Reference: the three adjustments `bodypart::get_limb_score` applies to a part's own figure before the creature sums them (`src/bodypart.cpp:967-1018`). They are kept as three small
//! functions rather than one because they are asked separately as well as together, and because each has a threshold that is a rule in its own right.

/// What a part's figure is worth once it is hurt.
///
/// Reference: `bodypart::wound_adjusted_limb_value` (`bodypart.cpp:967-976`) - above **three quarters** of its health the part is worth its whole figure; below, the figure is scaled by
/// `health / 0.75`, so a part at a quarter of its health is worth a third of what it was. The reference keeps the arithmetic in `double` and answers a `float`, which is why a part at half
/// health answers two thirds rather than something that rounds to it.
#[must_use]
#[allow(clippy::cast_precision_loss, clippy::cast_possible_truncation)]
pub fn wound_adjusted_limb_value(value: f32, health: i32, health_max: i32) -> f32 {
    if health_max <= 0 {
        return value;
    }
    let percent = f64::from(health) / f64::from(health_max);
    if percent > 0.75 {
        return value;
    }
    let scaled = f64::from(value) * (percent / 0.75);
    scaled as f32
}

/// What a part's figure is worth once something is in the way of it.
///
/// Reference: `bodypart::encumb_adjusted_limb_value` (`bodypart.cpp:978-990`): **zero** for a limb over its limit, otherwise `value * 19 / ( 19 + load / 3 )` past the part's threshold. The curve
/// starts at one, is a **twentieth** down at three points over, and **never reaches zero** - which is exactly why the reference needs the over-limit flag beside it.
#[must_use]
#[allow(clippy::cast_precision_loss, clippy::cast_possible_truncation)]
pub fn encumb_adjusted_limb_value(value: f32, load: i32, threshold: i32, over_the_limit: bool) -> f32 {
    if over_the_limit {
        return 0.0;
    }
    let past = (load - threshold).max(0);
    let scaled = f64::from(value) * 19.0 / (19.0 + f64::from(past) / 3.0);
    scaled as f32
}

/// What a part's figure is worth once the creature is skilled with it.
///
/// Reference: `bodypart::skill_adjusted_limb_value` (`bodypart.cpp:992-996`) - `min( value, value * ( 1 + skill / 10 ) )`. **The minimum is inert**: the second figure is larger than the first for
/// every non-negative skill, so the reference answers its own input. That is what this port copies, because the reference's behaviour is the behaviour - a port that made skill help here would
/// answer differently from the game it ported.
#[must_use]
pub fn skill_adjusted_limb_value(value: f32, skill: i32) -> f32 {
    #[allow(clippy::cast_precision_loss)]
    let mitigated = value * (1.0 + skill as f32 / 10.0);
    value.min(mitigated)
}

/// What one part contributes to a limb score.
///
/// Reference: `body_part_type::get_limb_score` (`bodypart.cpp:584-588`) - the part's own figure for that score, and **zero for a score it does not state**, which is a fact about the part rather
/// than a missing number.
#[must_use]
pub fn part_limb_score(scores: &[(String, f32, f32)], score: &str) -> f32 {
    scores
        .iter()
        .find(|(stated, _, _)| stated == score)
        .map_or(0.0, |(_, value, _)| *value)
}

/// A creature's score, added up from its parts.
///
/// Reference: `Character::get_limb_score` (`character_modifier.cpp:222-266`) - each part's adjusted figure is added, and a part of the limb type being asked for counts **only by its weight in
/// that type** (`* id.first->limbtypes.at( bp )`). The total is floored at zero (`:266`), so a body hurt past nothing still has no negative score.
///
/// The adjustments are the caller's: [`wound_adjusted_limb_value`], [`encumb_adjusted_limb_value`] and [`skill_adjusted_limb_value`] are applied to each part's figure **before** it is handed
/// here, which is the order the reference applies them in.
#[must_use]
pub fn limb_score_total(parts: &[(f32, f32)]) -> f32 {
    let total: f32 = parts
        .iter()
        .map(|(figure, weight)| figure * weight)
        .sum();
    total.max(0.0)
}

/// A creature's reaction, from its parts and their condition.
///
/// Reference: `Character::get_dodge` multiplies the dodge by `get_limb_score( limb_score_reaction )` (`melee.cpp:1229-1231`), and that score is each part's `reaction` figure **after** the
/// wound adjustment (`bodypart.cpp:967-976`) added up (`:266`). A whole human's reaction is exactly **one**, so the rule leaves a healthy survivor's dodge alone and moves it only when a part
/// that reacts is hurt - which is what makes it a consequence rather than a constant.
#[must_use]
pub fn reaction_score(parts: &[(f32, i32, i32)]) -> f32 {
    let total: f32 = parts
        .iter()
        .map(|(score, health_max, health)| wound_adjusted_limb_value(*score, *health, *health_max))
        .sum();
    total.max(0.0)
}

/// What a creature's balance, legs and feet multiply its dodge by.
///
/// Reference: `Character::get_dodge` multiplies by `get_modifier( character_modifier_limb_dodge_mod )` (`melee.cpp:1233-1235`), whose data is
/// `{ "limb_score": [["balance", 1.5], "move_speed", "footing"], "limb_score_op": "x", "min": 0.0 }` (`data/json/character_modifiers.json`) and whose arithmetic is `character_modifier::modifier`
/// (`character_modifier.cpp:338-386`): the scores are multiplied, the weight is an **exponent**, and the data's `min` is a floor. A whole human multiplies by exactly one, so the rule only
/// moves a body that has lost balance, a leg or a foot.
#[must_use]
pub fn limb_dodge_modifier(balance: f64, move_speed: f64, footing: f64) -> f64 {
    (balance.powf(1.5) * move_speed * footing).max(0.0)
}
