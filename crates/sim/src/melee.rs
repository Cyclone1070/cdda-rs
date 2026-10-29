//! How hard something is to hit.
//!
//! Reference: `Creature::deal_melee_attack` (`creature.cpp:903-929`), which turns one accuracy roll into
//! a spread, and the two numbers it subtracts: the target's dodge roll and its size penalty. Damage
//! comes later and elsewhere - this module answers only "did it connect", because that is the part whose
//! numbers can be checked without a body, an armour value or a hit location.
//!
//! Nothing here draws from the engine. `melee::melee_hit_range` (`melee.cpp:2912`) does - it is
//! `normal_roll(accuracy * 5, 25)` - and the caller supplies that roll, which keeps this a rule rather
//! than a dice game.

use crate::CreatureSize;

/// What a creature's dodge is worth against an attack.
///
/// Reference: `monster::dodge_roll` (`monster.cpp:2717`): `get_dodge() * 5`. Five, because the attacker's
/// roll is also worth five per point of accuracy (`melee_hit_range`).
#[must_use]
pub fn dodge_roll(dodge: i32) -> f32 {
    // Five per point, as a float because the reference keeps the roll as one.
    #[allow(clippy::cast_precision_loss)]
    let roll = (dodge * 5) as f32;
    roll
}

/// How much a target's size moves the spread, in the target's favour when it is small.
///
/// Reference: `Creature::size_melee_penalty` (`creature.cpp:836-852`): thirty for a cat down to minus
/// twenty for something you need a ladder to reach. The sign is the reference's: a penalty on the
/// attacker, so a small target subtracts from the spread.
#[must_use]
pub const fn size_melee_penalty(size: CreatureSize) -> i32 {
    match size {
        CreatureSize::Tiny => 30,
        CreatureSize::Small => 15,
        CreatureSize::Medium => 0,
        CreatureSize::Large => -10,
        CreatureSize::Huge => -20,
    }
}

/// The margin an attack landed by: the roll, less what the target avoided.
///
/// Reference: `creature.cpp:909` - `hit_spread = hitroll - dodge - size_melee_penalty()` - plus the
/// forty added at `:916-921` for a target that cannot move at all, because "even a clumsy person would
/// not miss a turret".
#[must_use]
pub fn hit_spread(hitroll: i32, dodge: f32, size: CreatureSize, immobile: bool) -> i32 {
    #[allow(clippy::cast_possible_truncation)]
    let spread = hitroll - (dodge as i32) - size_melee_penalty(size);
    if immobile { spread + 40 } else { spread }
}

/// Whether a spread connects.
///
/// Reference: the reference treats `hit_spread <= 0` as a miss - that is the branch that calls the
/// target's dodge event (`creature.cpp:924`) - so zero is a miss and one is a hit.
#[must_use]
pub const fn is_a_hit(spread: i32) -> bool {
    spread > 0
}
