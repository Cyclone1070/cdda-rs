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

/// **Tiredness prices a step**: the reciprocal of the reference's stamina modifier.
///
/// Reference: `run_cost_effects` multiplies by `1.0 / get_modifier( character_modifier_stamina_move_cost_mod )` (`character.cpp:10527-10528`), and that modifier is
/// `( stamina / stamina_max + 1 ) / 2` (`character_modifier.cpp:295-300`) - half speed with the tank empty, full speed full. So the applied factor is `2 / ( ratio + 1 )`, which is
/// **above one** while any stamina remains: the reference's modifier is a divisor, and a fresh survivor pays a hair more than the base until the arithmetic cancels.
#[must_use]
pub fn stamina_cost_modifier(stamina_ratio: f32) -> f32 {
    2.0 / (stamina_ratio + 1.0)
}

/// **What a step costs**, given the body's factors and the shape of the step.
///
/// Reference: `Character::run_cost` (`character.cpp:10363-10374`) with the effects that bind for a survivor walking on open ground, in the reference's own order: the limb blend
/// (`:10436-10441`), flat ground when the ground was flat **before** that blend (`:10443-10446`), and stamina (`:10527-10528`); the diagonal is divided out before all of it and
/// multiplied back after (`:10366-10372`). The result is floored at one (`:10523`), because a step never costs nothing.
#[must_use]
pub fn run_cost(
    base: f32,
    limb_modifier: f32,
    flat_ground: f32,
    diagonal: bool,
    stamina_ratio: f32,
) -> i32 {
    // **The diagonal is divided out before the effects run and multiplied back after** (`character.cpp:10366-10372`), because the effect arithmetic assumes a hundred-point base. The
    // flat-ground question is asked of the **divided** cost (`:10396`), so a diagonal step can be flat ground when a straight one of the same squares is not.
    let mut movecost = base;
    if diagonal {
        movecost /= std::f32::consts::SQRT_2;
    }
    let flat = movecost < 105.0;
    movecost *= limb_modifier;
    if flat {
        movecost *= flat_ground;
    }
    movecost *= stamina_cost_modifier(stamina_ratio);
    if diagonal {
        movecost *= std::f32::consts::SQRT_2;
    }
    let floored = movecost.max(1.0);
    #[expect(
        clippy::cast_possible_truncation,
        reason = "the reference returns static_cast<int>( movecost ), a truncation"
    )]
    let charge = floored as i32;
    charge
}

/// **What a step takes out of a creature**, in the same units the reference keeps stamina in.
///
/// Reference: `Character::burn_move_stamina` (`character.cpp:6972-7013`): the burn ratio starts at the `PLAYER_BASE_STAMINA_BURN_RATE` option - **15**, "base stamina burn per 100 moves
/// of the walking player" (`data/core/external_options.json:39-43`) - gains a percentage for being overburdened and for swimming, is multiplied by the movement mode's stamina
/// multiplier (one for walking), and the burn itself is `moves * ratio / 100` times `stamina_move_cost_mod`, which is `( ratio + 1 ) / 2` (`character_modifier.cpp:295-300`).
///
/// **Note the two different uses of the same modifier**: the *price* of a step divides by it (`:10527`), and the *burn* multiplies by it. A tired creature pays more and tires more
/// slowly, which is why both are here rather than one being derived from the other.
#[must_use]
pub fn stamina_burn(moves: i32, burn_ratio: i32, stamina_ratio: f32) -> i32 {
    let modifier = stamina_ratio.midpoint(1.0);
    #[expect(
        clippy::cast_precision_loss,
        reason = "a step's movement points are a small count"
    )]
    let spent = moves as f32;
    #[expect(
        clippy::cast_precision_loss,
        reason = "the burn ratio is a small percentage"
    )]
    let ratio = burn_ratio as f32;
    let burn = spent * ratio / 100.0 * modifier;
    #[expect(
        clippy::cast_possible_truncation,
        reason = "the reference burns a whole number of stamina points"
    )]
    let points = burn as i32;
    points
}
