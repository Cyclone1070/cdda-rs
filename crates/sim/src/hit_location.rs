//! Which part of a body a blow lands on.
//!
//! Reference: `anatomy::select_body_part` (`src/anatomy.cpp:208-256`) builds a weighted list from each part's
//! `hit_size`, filters and rescales it, and then lets `weighted_float_list::pick` choose. The port keeps the
//! same shape: the caller supplies the parts and the roll, so the rule can be tested without a body and without
//! dice, and a monster - which has no parts at all - simply never asks.

/// One part of a body, as the rule needs it.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct BodyPart {
    /// What it is called, which is all a message needs.
    pub name: String,
    /// How big a target it is (`hit_size`).
    pub hit_size: f32,
    /// How much a better attack roll favours it (`hit_difficulty`).
    pub hit_difficulty: f32,
    /// Whether it is high up, and so out of reach of a short attacker (the `LIMB_UPPER` flag).
    pub upper: bool,
    /// Whether it is low down, and so easier for a short attacker to reach (the `LIMB_LOWER` flag).
    pub lower: bool,
    /// What the part contributes to each limb score, as `( score, value, max )` - the data's `limb_scores`. The hit-location rule does not read it; the limb-score rules do, and carrying it
    /// here is what lets a **body** answer for its reaction without a second list keyed by part name.
    pub limb_scores: Vec<(String, f32, f32)>,
    /// How much health the part has (`base_hp`), which a wound is measured against.
    pub base_hp: i32,
    /// The limb kind it mostly is (`primary_limb_type`), which is what a move's `limb_req` names.
    ///
    /// The reference caches this from the heaviest of a part's `limb_types` (`bodypart.cpp:359-370`); the port keeps the cached answer rather than the weights, because the only rules that ask are
    /// asking **which kind** a limb is, never how much of each.
    pub primary_limb_type: Option<String>,
    /// The hit points **below which** the part stops counting towards a move's needs (`health_limit`, `bodypart.h:272`), which the data states and which loads as one when it says nothing
    /// (`bodypart.cpp:414`).
    pub health_limit: i32,
}

/// Picks one entry of a weighted list, given a draw.
///
/// Reference: `weighted_float_list::pick_ent` (`src/weighted_list.h`) - the draw is scaled to the total weight and
/// the **first** entry whose running total reaches it wins, with `>=` rather than `>`. A list whose weights are
/// all zero picks nothing, which the reference reports as a null pointer and a debug message.
#[must_use]
pub fn pick_by_weight(weights: &[f32], fraction: f64) -> Option<usize> {
    let total: f64 = weights.iter().map(|weight| f64::from(*weight)).sum();
    if total <= 0.0 {
        return None;
    }
    let picked = fraction * total;
    let mut accumulated = 0.0;
    for (index, weight) in weights.iter().enumerate() {
        accumulated += f64::from(*weight);
        if accumulated >= picked {
            return Some(index);
        }
    }
    // Only reachable through floating-point drift past the end of the list; the reference would run off the
    // array here, and returning the last entry is the nearest thing to what it meant.
    weights.len().checked_sub(1)
}

/// The weights `select_body_part` builds, in order.
///
/// Reference: `anatomy::select_body_part` (`anatomy.cpp:212-241`): a part is dropped when it is smaller than
/// `min_hit` or larger than `max_hit` (when that is not -1), an attacker that cannot reach high loses the upper
/// parts and triples the lower ones, and a positive hit roll raises each weight by `hit_roll ^ hit_difficulty`.
#[must_use]
pub fn hit_weights(
    parts: &[BodyPart],
    min_hit: i32,
    max_hit: i32,
    can_attack_high: bool,
    hit_roll: i32,
) -> Vec<f32> {
    let mut weights = Vec::with_capacity(parts.len());
    #[allow(clippy::cast_precision_loss)]
    let smallest = min_hit as f32;
    #[allow(clippy::cast_precision_loss)]
    let largest = max_hit as f32;
    for part in parts {
        let mut weight = part.hit_size;
        if weight < smallest || (max_hit > -1 && weight > largest) {
            continue;
        }
        if !can_attack_high {
            if part.upper {
                continue;
            }
            if part.lower {
                weight *= 3.0;
            }
        }
        if hit_roll > 0 {
            #[allow(clippy::cast_precision_loss)]
            let roll = hit_roll as f32;
            weight *= roll.powf(part.hit_difficulty);
        }
        weights.push(weight);
    }
    weights
}

/// Which part a blow lands on, or nothing when no part of the body is reachable.
///
/// The parts that survive [`hit_weights`] are the ones this indexes, so the answer is one of them and never a part
/// the filter removed.
#[must_use]
pub fn select_body_part(
    parts: &[BodyPart],
    min_hit: i32,
    max_hit: i32,
    can_attack_high: bool,
    hit_roll: i32,
    fraction: f64,
) -> Option<&BodyPart> {
    let weights = hit_weights(parts, min_hit, max_hit, can_attack_high, hit_roll);
    let survivors: Vec<&BodyPart> = parts
        .iter()
        .filter(|part| {
            let weight = part.hit_size;
            #[allow(clippy::cast_precision_loss)]
            let smallest = min_hit as f32;
            #[allow(clippy::cast_precision_loss)]
            let largest = max_hit as f32;
            !(weight < smallest || (max_hit > -1 && weight > largest) || (!can_attack_high && part.upper))
        })
        .collect();
    pick_by_weight(&weights, fraction).and_then(|index| survivors.get(index).copied())
}

/// The part a blow landed on, given the margin it landed by.
///
/// Reference: `Creature::deal_melee_hit` (`creature.cpp:959`) - `select_body_part( -1, -1, source->can_attack_high(),
/// hit_spread )`, so a blow is offered every part it can reach and the margin it landed by is what tilts the choice
/// towards the small ones.
#[must_use]
pub fn struck_part(
    parts: &[BodyPart],
    spread: i32,
    can_attack_high: bool,
    fraction: f64,
) -> Option<&BodyPart> {
    select_body_part(parts, -1, -1, can_attack_high, spread, fraction)
}
