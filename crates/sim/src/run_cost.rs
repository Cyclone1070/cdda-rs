//! **What a step costs a character**, as far as the port can currently say it.
//!
//! Reference: `Charaction::run_cost` does not exist; `Character::run_cost` (`character.cpp:10363-10374`) starts from a base - a hundred for a square - and multiplies it by the
//! character's own state. Most of that state is limbs: `run_cost_effects` (`:10436-10441`) multiplies by `get_modifier(character_modifier_limb_run_cost_mod)`, which
//! `character_modifier.cpp:302-306` defines as `(footing_mod + move_speed_mod * 2) / 3`, and each of those two is a **limb score turned into a multiplier by its data entry**
//! (`data/json/character_modifiers.json`): `{"limb_score": "move_speed", "max": "max_move_cost", "nominator": 1.0}`, which is the score's reciprocal.
//!
//! The port has the limb-score **arithmetic** ([`crate::limbs`]) but no body parts to feed it, so this module states the two transformations that sit between a score and a step
//! price, and nothing else. `corpus/run_cost.json` is `--dump-run-cost`: a survivor's scores, both multipliers and the blended one, unloaded and loaded past capacity.
//!
//! **Named, not guessed:** the recording's final charge is **163** for a base of a hundred, and the blend above gives **1.917674**, which is 191. So one more factor of about
//! **0.85** sits between the blend and the charge and the port has not found it - it is not the road, the terrain, the skates or the diagonal, all of which were checked. Until it
//! is found, the port states the two layers it can prove and does **not** claim to compute `run_cost` itself.

/// A limb score as a multiplier: the reciprocal.
///
/// Reference: the `limb_score` variant of `character_modifier`'s value (`character_modifier.cpp`), which divides its `nominator` (1.0 for both movement multipliers) by the score.
/// A score of zero would divide by zero, and the reference's own clamp at `max_move_cost` is what stops a broken limb from costing an unbounded number of moves; **the port does not
/// carry that clamp yet** - the recording's own footing multiplier is 2.14, above the usual cap of 2, so the cap's data was not reached and is named rather than invented.
#[must_use]
pub fn limb_cost_modifier(score: f32) -> f32 {
    if score.abs() < f32::EPSILON {
        return f32::INFINITY;
    }
    1.0 / score
}

/// The blend `run_cost_effects` multiplies a step by: footing once, movement speed twice.
///
/// Reference: `limb_run_cost_modifier` (`character_modifier.cpp:302-306`) - `( footing + move_speed * 2 ) / 3.0f`. Footing is weighted less than speed, which is why a character
/// with good legs and bad balance still walks nearly as fast.
#[must_use]
pub fn limb_run_cost_modifier(footing_mod: f32, move_speed_mod: f32) -> f32 {
    (footing_mod + move_speed_mod * 2.0) / 3.0
}
