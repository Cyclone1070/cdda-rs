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

/// The accuracy roll an attack makes, from a creature's accuracy.
///
/// Reference: `melee::melee_hit_range` (`melee.cpp:2912`) is `normal_roll( accuracy * 5, 25 )` - five
/// points of roll per point of accuracy, with a deviation of twenty-five. That wide deviation is why a
/// good attacker still misses: two thirds of its rolls land within twenty-five points of its mean.
pub fn melee_hit_range(rolls: &mut cdda_core_types::rng::Rolls, accuracy: f32) -> f32 {
    // The reference keeps the roll as a `float` and the generator as a `double`, so the narrowing is
    // the reference's own; twenty-five is its deviation, not a number chosen here.
    #[allow(clippy::cast_possible_truncation)]
    let value = rolls.normal_roll(f64::from(accuracy) * 5.0, 25.0) as f32;
    value
}

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
/// Reference: `monster::melee_attack` deals damage on `hitspread >= 0` (`monster.cpp:2151`). Note what
/// that means at exactly zero - see [`dodged`].
#[must_use]
pub const fn is_a_hit(spread: i32) -> bool {
    spread >= 0
}

/// Whether the target is told it dodged.
///
/// Reference: `Creature::deal_melee_attack` calls the target's dodge event on `hit_spread <= 0`
/// (`creature.cpp:924`). At exactly zero the reference says **both**: the target dodged and the attack
/// landed. The two comparisons are one line apart in the source and disagree at one point, which is the
/// reference's behaviour rather than a reading of it.
#[must_use]
pub const fn dodged(spread: i32) -> bool {
    spread <= 0
}

/// Damage in the reference's four kinds, with the penetration the attack carries.
///
/// Reference: `damage_instance` (`src/damage.h`), a map from damage type to amount plus an armour
/// penetration value. Bash and cut are the two a monster's claws use; the others are carried so that a
/// caller cannot lose them.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Damage {
    /// Blunt damage.
    pub bash: i32,
    /// Cutting damage.
    pub cut: i32,
    /// Points of armour this attack ignores.
    pub penetration: i32,
}

/// What armour resists, after the attack's own penetration.
///
/// Reference: `resistances::get_effective_resist` (`damage.cpp:724-727`) is
/// `std::max( type_resist( du.type ) - du.res_pen, 0 )`. The clamp is the reference's: penetration eats
/// resistance down to nothing and never turns armour into a weakness.
#[must_use]
pub fn effective_resist(resist: f32, penetration: f32) -> f32 {
    (resist - penetration).max(0.0)
}

/// What is left of a blow after armour.
///
/// Reference: `monster::absorb_hit` (`monster.cpp:2085`) subtracts
/// `min( effective_resist + worn_armour, amount )` - so armour can stop a blow entirely but cannot push it
/// below zero and heal the target. The `min` is on the *resistance*, not on the result; the two are the
/// same arithmetic, but only one of them says what the reference meant.
#[must_use]
pub fn after_armour(amount: f32, resist: f32, worn_armour: f32, penetration: f32) -> f32 {
    let resisted = effective_resist(resist, penetration) + worn_armour;
    amount - resisted.min(amount)
}

/// The sum of `number` dice with `sides` faces.
///
/// Reference: `dice` (`rng.cpp:211-218`) - `rng( 1, sides )` drawn `number` times and added. Each die is
/// a separate draw, which matters: a single `rng( 1, number * sides )` gives the same range and a
/// different distribution.
pub fn dice_total(rolls: &mut cdda_core_types::rng::Rolls, number: i32, sides: i32) -> i32 {
    rolls.dice(number, sides)
}

/// Everything an attack brings to a target.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Attack {
    /// The attacker's accuracy, which sets the mean of its roll.
    pub accuracy_tenths: i32,
    /// Its type's fixed damage.
    pub fixed: Damage,
    /// How many dice its type rolls, and with how many faces.
    pub dice: i32,
    /// The faces on those dice.
    pub sides: i32,
}

/// Everything a target brings to being hit.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Defence {
    /// Its dodge, in points; each is worth five on the roll.
    pub dodge: i32,
    /// How big it is.
    pub size: crate::CreatureSize,
    /// Its armour against bash.
    pub resist: f32,
    /// What it is wearing that also resists.
    pub worn_armour: f32,
    /// Whether it cannot move at all, which makes it all but unmissable.
    pub immobile: bool,
}

/// What came of one swing.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Strike {
    /// The margin the roll landed by.
    pub spread: i32,
    /// Whether damage was dealt.
    pub hit: bool,
    /// Whether the target was told it dodged - which is true at exactly zero, as well as on a miss.
    pub dodged: bool,
    /// What got past the armour.
    pub damage: f32,
}

/// One swing, from accuracy roll to what got through.
///
/// The pieces are each ported and tested separately; this composes them in the order the reference does -
/// `monster::melee_attack` rolls, asks the target for its spread (`deal_melee_attack`), rolls damage and
/// lets the target absorb it (`absorb_hit`).
pub fn strike(rolls: &mut cdda_core_types::rng::Rolls, attack: Attack, defence: Defence) -> Strike {
    #[allow(clippy::cast_precision_loss)]
    let accuracy = attack.accuracy_tenths as f32 / 10.0;
    let hitroll = melee_hit_range(rolls, accuracy);
    // The dice are rolled before the rules see them, so that a miss still spends its roll - the reference
    // computes damage before asking whether it connected (`monster.cpp:2143-2151`).
    let rolled = monster_melee_damage(rolls, attack.fixed, attack.dice, attack.sides);
    resolve(hitroll, rolled, defence)
}

/// The same swing, with both rolls given rather than drawn.
///
/// This is the rule on its own: a hit roll and a damage roll in, what happened out. Splitting it from the
/// dice is what lets a test say "a roll of thirty against two dodge is twenty - a hit" without pretending to
/// know what the engine would have rolled, and it keeps [`strike`] to one line of dice.
#[must_use]
pub fn resolve(hitroll: f32, rolled: Damage, defence: Defence) -> Strike {
    #[allow(clippy::cast_possible_truncation)]
    let spread = hit_spread(
        hitroll as i32,
        dodge_roll(defence.dodge),
        defence.size,
        defence.immobile,
    );
    let hit = is_a_hit(spread);
    let damage = if hit {
        #[allow(clippy::cast_precision_loss)]
        let bash = rolled.bash as f32;
        #[allow(clippy::cast_precision_loss)]
        let penetration = rolled.penetration as f32;
        after_armour(bash, defence.resist, defence.worn_armour, penetration)
    } else {
        0.0
    };
    Strike {
        spread,
        hit,
        dodged: dodged(spread),
        damage,
    }
}

/// What a monster's melee does before armour: its type's own damage plus its dice.
///
/// Reference: `monster::melee_attack` (`monster.cpp:2143-2146`) starts from the type's `melee_damage`,
/// then - only when the type has dice - adds `dice( melee_dice, melee_sides )` of bash carrying
/// `melee_dice_ap`. A type with no dice adds nothing, and the penetration still comes from the type.
pub fn monster_melee_damage(
    rolls: &mut cdda_core_types::rng::Rolls,
    fixed: Damage,
    dice: i32,
    sides: i32,
) -> Damage {
    let mut damage = fixed;
    if dice > 0 {
        damage.bash += dice_total(rolls, dice, sides);
    }
    damage
}
