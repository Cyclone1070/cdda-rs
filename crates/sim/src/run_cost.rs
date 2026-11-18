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
//! The last layer is **flat ground**, and the recording named it: `run_cost_effects` returns its own chain, and the second entry is `Flat Ground Mut.` at **0.85**. That is the
//! `FLEET` mutation - a starting trait, "15% speed bonus on sure footing" (`data/json/mutations/mutations.json:331`), whose enchantment is `MOVECOST_FLATGROUND_MOD multiply -0.15`.
//! Flat ground is decided **before** the limb multiplier runs (`flatground = movecost < 105`, `character.cpp:10396`), and the multiplication happens after it, which is why a loaded
//! survivor and an empty one pay the same for the same step.

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

/// A summed enchantment `multiply` as the multiplier the cache answers: one plus the sum.
///
/// Reference: `enchantment_cache->modify_value( MOVECOST_FLATGROUND_MOD, 1 )` (`character.cpp:10444-10445`) - the cache starts from the argument and every contributing
/// enchantment adds its `multiply`. `FLEET` contributes `-0.15`, which is the 0.85 the recording shows.
#[must_use]
pub fn flat_ground_multiplier(multiply_sum: f32) -> f32 {
    1.0 + multiply_sum
}

/// **What a step costs**, given the limb blend and the flat-ground multiplier.
///
/// Reference: `Character::run_cost` (`character.cpp:10363-10374`) with the two effects that bind for a survivor on open ground: "Encum./Wounds" multiplies by the limb blend
/// (`:10436-10441`) and, **when the ground was flat before that multiply**, "Flat Ground Mut." multiplies again (`:10443-10446`). The result is truncated toward zero and floored at
/// one (`:10523`), because a step never costs nothing.
///
/// `flat` is decided from the **base**, not from the multiplied cost: `const bool flatground = movecost < 105` sits above the whole chain (`:10396`), so whether the ground is flat is
/// a fact about the square, not about the walker.
#[must_use]
pub fn run_cost(base: f32, limb_modifier: f32, flat_ground: f32) -> i32 {
    let flat = base < 105.0;
    let mut movecost = base * limb_modifier;
    if flat {
        movecost *= flat_ground;
    }
    let floored = movecost.max(1.0);
    #[expect(
        clippy::cast_possible_truncation,
        reason = "the reference returns static_cast<int>( movecost ), a truncation"
    )]
    let charge = floored as i32;
    charge
}
