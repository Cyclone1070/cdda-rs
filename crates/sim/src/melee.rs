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
    /// Stabbing damage.
    ///
    /// The reference's `damage_instance` carries every damage type; the data's melee uses exactly three - bash, cut and stab - and a knife's whole worth is the third, so a port
    /// with two of them cannot describe a knife at all.
    pub stab: i32,
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
    /// Its armour against cut.
    pub cut_resist: f32,
    /// Its armour against stab.
    pub stab_resist: f32,
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
    // Each damage type meets its own armour, and the blow carries all three added together.
    //
    // Reference: `Creature::deal_melee_hit` passes the whole `damage_instance` to `absorb_hit`, which walks its units and calls `get_effective_resist` per unit's **type**
    // (`damage.cpp:724-727`) - so a knife's stabbing never meets a target's bash armour, and a blow that is all cutting is not silently worth nothing.
    let damage = if hit {
        #[allow(clippy::cast_precision_loss)]
        let penetration = rolled.penetration as f32;
        let mut total = 0.0;
        for (amount, resist) in [
            (rolled.bash, defence.resist),
            (rolled.cut, defence.cut_resist),
            (rolled.stab, defence.stab_resist),
        ] {
            #[allow(clippy::cast_precision_loss)]
            let amount = amount as f32;
            total += after_armour(amount, resist, defence.worn_armour, penetration);
        }
        total
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

/// How much a character's dexterity contributes to aiming, in tenths of an accuracy point.
///
/// Reference: `Character::get_hit_base` (`src/character.cpp:5705-5709`) is `get_dex() / 4.0f`. The port keeps accuracy in **tenths** everywhere a melee attack is described, which
/// is the unit `MeleeAttack::accuracy_tenths` already speaks, so this returns `dex * 10 / 4` rather than a float nobody else uses.
#[must_use]
pub fn hit_base_tenths(dexterity: i32) -> i32 {
    dexterity * 10 / 4
}

/// How much a character's skills and weapon contribute to aiming, in tenths of an accuracy point.
///
/// Reference: `Character::get_hit_weapon` (`src/melee.cpp:356-370`) is `skill / 3.0f + melee_skill / 2.0f + weap.get_to_hit()`, where `skill` is the skill of the weapon's own
/// damage type - unarmed for a fist, bashing for a bat. The division is the reference's own and is **truncating** here because the answer is in tenths: three levels of unarmed
/// are exactly one point.
#[must_use]
pub fn hit_weapon_tenths(weapon_skill: i32, melee_skill: i32, to_hit: i32) -> i32 {
    weapon_skill * 10 / 3 + melee_skill * 10 / 2 + to_hit * 10
}

/// The skill a weapon is swung with: the one its biggest damage type trains.
///
/// Reference: `item::melee_skill` (`item.cpp:11071-11090`) walks the **damage types**, keeps the one whose melee damage is highest, and answers that type's `skill`; a type with no
/// skill is skipped, and an item that is not a melee weapon answers the null skill. The three melee types and their skills are the data's own - `bash` trains `bashing`, `cut`
/// trains `cutting`, `stab` trains `stabbing` (`data/json/damage_types.json`) - and they are transcribed here rather than loaded because nothing else in the port asks a damage
/// type what skill it trains. When something does, this table moves into the content crate and the function keeps its shape.
#[must_use]
pub fn weapon_skill(bash: f64, cut: f64, stab: f64) -> Option<&'static str> {
    let mut best: Option<(f64, &'static str)> = None;
    for (amount, skill) in [(bash, "bashing"), (cut, "cutting"), (stab, "stabbing")] {
        // The reference's test is `val > hi` **and** the type carries a skill, starting from zero - so a weapon whose every damage type is zero trains nothing.
        if amount > 0.0 && best.is_none_or(|(highest, _)| amount > highest) {
            best = Some((amount, skill));
        }
    }
    best.map(|(_, skill)| skill)
}

/// What a swing's critical chance is worked out from.
///
/// Reference: `Character::crit_chance( roll_hit, target_dodge, weap )` reads the weapon's `to_hit`, the unarmed skill when nothing is held, dexterity and perception, the weapon's
/// own melee skill and the character's melee skill (`melee.cpp:1116-1193`). The struct names them the way the source does rather than in the order the arithmetic happens to use them.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CritInputs {
    /// The weapon's own accuracy, which is `item::get_to_hit`.
    pub to_hit: i32,
    /// Whether nothing is held, which makes the unarmed skill the weapon's chance.
    pub unarmed: bool,
    /// The unarmed skill.
    pub unarmed_skill: i32,
    /// Dexterity.
    pub dexterity: i32,
    /// Perception.
    pub perception: i32,
    /// The skill the weapon trains: bashing, cutting, stabbing or unarmed.
    pub weapon_skill: i32,
    /// The general melee skill, which adds half again to the weapon's.
    pub melee_skill: i32,
    /// Whatever the martial art in use adds, as a fraction rather than a percent.
    pub martial_arts_bonus: f64,
}

/// Limits a probability to `[0, 1]`, which the reference does to each of the three chances before multiplying them (`melee.cpp:1111-1114`).
fn limited(probability: f64) -> f64 {
    probability.clamp(0.0, 1.0)
}

/// How likely a swing that lands is a critical hit.
///
/// Reference: `Character::crit_chance` (`melee.cpp:1116-1193`). Three chances - weapon, body, skill - multiply into a critical that happens whatever the rolls; a roll past
/// **one and a half times** the target's dodge adds the chance of a **pair** of them, `0.5 * (wp + ps + ws - 3 * wps)`, which is the sum of the three pairs with the triple
/// counted three times removed. The two halves are therefore mutually exclusive and the reference simply adds them.
#[must_use]
pub fn crit_chance(inputs: &CritInputs, roll_hit: f32, target_dodge: f32) -> f64 {
    // The weapon's chance: half, improved by the unarmed skill or by a positive accuracy, and worsened by a negative one.
    let mut weapon = 0.5;
    if inputs.unarmed {
        weapon = 0.5 + 0.05 * f64::from(inputs.unarmed_skill);
    }
    // An empty hand states no accuracy at all: the reference asks a **null item** for `get_to_hit()`, which answers zero, so neither branch below fires. A caller that leaves a
    // weapon's `to_hit` in the struct while also saying the hand is empty would otherwise have the empty hand's chance cut by a weapon it is not holding.
    let to_hit = if inputs.unarmed { 0 } else { inputs.to_hit };
    if to_hit > 0 {
        weapon = weapon.max(0.5 + 0.1 * f64::from(to_hit));
    } else if to_hit < 0 {
        weapon += 0.1 * f64::from(to_hit);
    }
    let weapon = limited(weapon);

    // The body's: a quarter, plus a hundredth a point of dexterity and a fiftieth a point of perception.
    let body = limited(0.25 + 0.01 * f64::from(inputs.dexterity) + 0.02 * f64::from(inputs.perception));

    // The skill's: a quarter, plus a fortieth a point of the weapon's skill, and half the melee skill on top of that.
    let skill = limited(0.25 + (f64::from(inputs.weapon_skill) + f64::from(inputs.melee_skill) / 2.5) * 0.025);

    let triple = weapon * body * skill;
    let bonus = inputs.martial_arts_bonus;
    if roll_hit > target_dodge * 1.5 {
        let double = 0.5 * (weapon * body + body * skill + weapon * skill - 3.0 * triple);
        return triple + double + bonus;
    }
    triple + bonus
}

/// How long a weapon takes to swing, in the reference's own points.
///
/// Reference: `item::attack_time` (`item.cpp:7595-7602`): `65 + ( volume / 62.5_ml + weight / 60_gram ) / count`. The reference holds the answer as an integer and reaches it by
/// truncating a real, so the port truncates too rather than rounding - a crowbar of 520 ml and 1,456 g takes **97**, not 98.
#[must_use]
pub fn attack_time(volume_ml: i64, weight_g: i64, count: i64) -> i32 {
    if count <= 0 {
        return 65;
    }
    #[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)]
    let time = 65.0 + (volume_ml as f64 / 62.5 + weight_g as f64 / 60.0) / count as f64;
    #[allow(clippy::cast_possible_truncation)]
    let whole = time as i32;
    whole
}

/// What one swing costs a character in moves.
///
/// Reference: `Character::attack_speed` (`melee.cpp:2668-2705`). Half the weapon's own time, plus `base * (15 - melee_skill) / 15`, less half the dexterity, all scaled by a stamina
/// penalty that grows from **one at a quarter stamina to two at none**. The reference also applies modifiers, enchantments and martial arts; none of the three exists in this port yet,
/// and each is named here so its absence is a fact rather than a silence.
#[must_use]
pub fn attack_speed(base_move_cost: i32, melee_skill: i32, dexterity: i32, stamina_ratio: f64) -> i32 {
    let base = f64::from(base_move_cost);
    #[allow(clippy::cast_possible_truncation)]
    let skill_cost = (base * f64::from(15 - melee_skill) / 15.0) as i32;
    let dex_bonus = dexterity / 2;
    let penalty = 1.0 + ((0.25 - stamina_ratio) * 4.0).max(0.0);
    #[allow(clippy::cast_possible_truncation)]
    let cost = (base * penalty) as i32 + skill_cost - dex_bonus;
    cost.max(0)
}

/// How a swinger is standing, which is what a low swing charges extra for.
///
/// Reference: the chain in `get_total_melee_stamina_cost` (`melee.cpp:975-977`). **A struct rather than four bools**, and named for the body rather than the penalty: a caller answers
/// what it knows about the creature, and this rule answers what that costs.
#[derive(Debug, Clone, Copy, Default)]
pub struct Stance {
    /// The swinger is on the floor (`is_on_ground`): downed, prone, or without working legs.
    pub on_ground: bool,
    /// The swinger is crouched (`is_crouching`).
    pub crouching: bool,
    /// The swinger is holding a weapon (`!unarmed_attack()`), which is what makes a crouch cost anything.
    pub armed: bool,
    /// The swinger is in a natural stance (`effect_natural_stance`), which is a stance that does not care.
    pub natural_stance: bool,
    /// The swinger grips with pseudopods (`PSEUDOPOD_GRASP`), which cancels **both** maluses.
    pub pseudopod_grasp: bool,
}

/// What the stance charges a swing, in the reference's stamina units.
///
/// Reference: `get_total_melee_stamina_cost` (`melee.cpp:975-977`):
///
/// ```text
/// const int stance_malus = ( is_on_ground() && !has_flag( PSEUDOPOD_GRASP ) ) ? 50
///     : ( !has_flag( PSEUDOPOD_GRASP ) && !has_effect( natural_stance ) && !unarmed_attack() && is_crouching() ) ? 20
///     : 0;
/// ```
///
/// **A chain, not a sum**: standing on the floor is fifty **whatever else is true**, and the twenty is only asked when the first test failed - so a crouched, armed, grounded, natural-stance
/// creature still pays the fifty. **`PSEUDOPOD_GRASP` cancels both**, in its own words because a thing on pseudopods is not inconvenienced by being low.
#[must_use]
pub fn stance_malus(stance: Stance) -> i32 {
    if stance.on_ground && !stance.pseudopod_grasp {
        return 50;
    }
    if !stance.pseudopod_grasp && !stance.natural_stance && stance.armed && stance.crouching {
        return 20;
    }
    0
}

/// What a set of proficiencies multiplies a swing's stamina cost by.
///
/// Reference: `get_total_melee_stamina_cost` (`melee.cpp:979-995`):
///
/// ```text
/// float proficiency_multiplier = 1.f;
/// for( const weapon_category_id &cat : wielded_weapon_categories( *this ) ) {
///     float loss = 0.f;
///     for( const proficiency_id &prof : cat->category_proficiencies() ) { ... loss += bonus.value(); }
///     proficiency_multiplier = std::clamp( 1.f - loss, 0.f, proficiency_multiplier );
/// }
/// ```
///
/// `losses` is **one number per weapon category**: the sum of that category's known proficiencies' `melee_attack`/`stamina` bonuses, which is what the inner loop adds.
///
/// **The upper bound of the clamp is the running multiplier, not one**, so a category that loses nothing leaves the multiplier where it was instead of resetting it. The floor at zero
/// stops a body of bonuses at **free** rather than turning into a rebate - the reference's own guard, because a negative multiplier would pay the character stamina for swinging.
#[must_use]
pub fn proficiency_stamina_multiplier(losses: &[f32]) -> f32 {
    let mut multiplier = 1.0f32;
    for loss in losses {
        multiplier = (1.0 - loss).clamp(0.0, multiplier);
    }
    multiplier
}

/// What a swing costs a character in stamina.
///
/// Reference: `Character::get_total_melee_stamina_cost` (`melee.cpp:973-997`): the standard cost is `- ( weight / 16_gram + 50 )` (`character.cpp:3107-3120`), and the total adds
/// the melee skill as a rebate and subtracts any stance penalty, then keeps at least **-50** - `std::min( -50, ... )`. The result is **negative**, because the reference spends it by
/// adding it: a swing returns the stamina to take.
#[must_use]
pub fn stamina_cost(weight_g: i64, melee_skill: i32, stance_malus: i32, proficiency_multiplier: f32) -> i32 {
    let standard = -(weight_g / 16 + 50);
    #[allow(clippy::cast_possible_truncation)]
    let total = i32::try_from(standard).unwrap_or(i32::MIN / 2) + melee_skill - stance_malus;
    // Reference: `std::min<int>( -50, proficiency_multiplier * ( mod_sta + melee - stance_malus ) )` - the multiplier scales the **total**, and the floor is applied **after** it
    // (`melee.cpp:995`), so a big enough discount can lift a swing to the floor but never past it.
    #[allow(clippy::cast_precision_loss, clippy::cast_possible_truncation)]
    let scaled = (proficiency_multiplier * total as f32) as i32;
    scaled.min(-50)
}

/// How likely a weapon is to be damaged by a blow: `Character::handle_melee_wear`'s arithmetic (`melee.cpp:214-289`).
///
/// Reference:
///
/// ```text
/// const float stat_factor = dex_cur / 2.0f + get_skill_level( skill_melee ) + ( 64.0f / std::max( get_arm_str(), 4 ) );
/// int damage_chance = static_cast<int>( ( stat_factor * material_factor / ( wear_multiplier * enchant_multiplier ) ) );
/// if( shield->has_flag( DURABLE_MELEE ) ) damage_chance *= 4;
/// ```
///
/// **Four things a reader gets wrong here.** The **strength term is inverted** - a stronger arm *divides* the chance, because `64 / arm_str` shrinks as the arm grows, and the reference's own
/// annotation calls it `NEGATIVE`. **Dexterity halves** and **melee skill counts whole**. `DURABLE_MELEE` **multiplies the chance by four**, which looks backwards until one remembers the
/// chance is a `one_in` denominator: **a bigger number is a tougher weapon**. And the result is **truncated to an int**, so a chance of 0.9 becomes zero - which the reference then reads as
/// "damage it every time", the quirk the caller's guard carries.
#[must_use]
pub fn melee_wear_chance(
    dexterity: f64,
    melee_skill: i32,
    arm_strength: f64,
    chip_resistance: f64,
    wear_multiplier: f64,
    enchant_multiplier: f64,
    durable_melee: bool,
) -> i32 {
    if wear_multiplier <= 0.0 || enchant_multiplier <= 0.0 {
        return 0;
    }
    let stat_factor = dexterity / 2.0 + f64::from(melee_skill) + (64.0 / arm_strength.max(4.0));
    #[allow(clippy::cast_possible_truncation)]
    let mut chance = (stat_factor * chip_resistance / (wear_multiplier * enchant_multiplier)) as i32;
    if durable_melee {
        chance *= 4;
    }
    chance
}

/// Whether a blow damages the weapon that made it, given the chance [`melee_wear_chance`] returned.
///
/// Reference: `if( damage_chance > 0 && !one_in( damage_chance ) ) return false;` (`melee.cpp:291-293`) - **the guard is inverted from the way it reads**: a chance of **zero or below** does
/// not save the weapon, it **damages it every time**, because the reference only bails out when there *is* a chance and the die misses it. The same is true of a chance of one, since
/// `one_in( 1 )` always succeeds. **A weapon whose arithmetic said "indestructible" therefore breaks fastest**, which is the reference's behaviour and not a bug in the port.
#[must_use]
pub fn wear_damages(chance: i32, dice: &mut cdda_core_types::rng::Rolls) -> bool {
    // `chance <= 0 || dice.one_in( chance )`, written the way the reference's guard reads.
    chance <= 0 || dice.one_in(chance)
}

/// Splits a broken `FRAGILE_MELEE` weapon into the parts that survived and the ones the blow destroyed.
///
/// Reference: `Character::handle_melee_wear`'s fragile branch (`melee.cpp:301-330`):
///
/// ```text
/// for( item &comp : tvp.second ) {
///     int break_chance = comp.typeId() == weak_comp ? 2 : 8;
///     if( one_in( break_chance ) ) { "The %s is destroyed!"; continue; }
///     ... wield the biggest one, drop the rest ...
/// }
/// ```
///
/// **The weakest part is rolled against `one_in( 2 )` and every other part against `one_in( 8 )`** - four times as likely to be lost. The reference picks `weak_comp` as the component with the
/// **lowest chip resistance** and `big_comp` as the one with the largest volume; the caller decides which is which, because that needs the component list this port does not keep.
///
/// Returns `(kept, destroyed)`, both in the data's own order.
#[must_use]
pub fn break_apart(
    parts: &[(String, bool)],
    dice: &mut cdda_core_types::rng::Rolls,
) -> (Vec<String>, Vec<String>) {
    let mut kept = Vec::new();
    let mut destroyed = Vec::new();
    for (part, is_weakest) in parts {
        let chance = if *is_weakest { 2 } else { 8 };
        if dice.one_in(chance) {
            destroyed.push(part.clone());
        } else {
            kept.push(part.clone());
        }
    }
    (kept, destroyed)
}

/// What one part of a thing is, as far as choosing the weak and the big part goes.
#[derive(Debug, Clone, Copy)]
pub struct PartFacts<'a> {
    /// The kind of part it is.
    pub kind: &'a str,
    /// How hard it is to chip, from the kind's facts.
    pub chip_resistance: i32,
    /// How much space it takes.
    pub volume_ml: i32,
}

/// The parts that "should have no bearing on durability" (`melee.cpp:259`).
pub const FRAGILE_BLACKLIST: [&str; 3] = ["sheet_cotton", "leather", "fur"];

/// Picks the part a `FRAGILE_MELEE` weapon wears at first, and the part a breakdown offers to wield.
///
/// Reference: `handle_melee_wear`'s prologue (`melee.cpp:255-270`):
///
/// ```text
/// int weak_chip = INT_MAX; units::volume big_vol = 0_ml;
/// for( comp ) { if( blacklist.count( tvp.first ) ) continue;
///   if( weak_chip > comp.chip_resistance() ) { weak_chip = ...; weak_comp = ...; }
///   if( comp.volume() > big_vol )            { big_vol  = ...; big_comp  = ...; } }
/// ```
///
/// **Three rules in four lines, and each one is easy to lose:** the blacklist is checked **per type**, so cloth never becomes either part however weak or thick it is; both comparisons are
/// **strict**, so among equal candidates **the first one wins**; and a part of **zero volume is never the big one**, because `big_vol` starts at zero and nothing is greater than it.
///
/// Returns the chosen kinds, or `None` for a part that nothing qualifies as.
#[must_use]
pub fn weak_and_big_parts<'a>(parts: &'a [PartFacts<'a>]) -> (Option<&'a str>, Option<&'a str>) {
    let (weak, big) = weak_and_big(parts);
    (weak.map(|part| part.kind), big.map(|part| part.kind))
}

/// The same choice, keeping the parts themselves: a caller that wants the chosen part's numbers must not have to find it again by kind, which would pick the wrong one when two parts share a kind.
fn weak_and_big<'a>(parts: &'a [PartFacts<'a>]) -> (Option<&'a PartFacts<'a>>, Option<&'a PartFacts<'a>>) {
    let mut weak: Option<&PartFacts<'a>> = None;
    let mut big: Option<&PartFacts<'a>> = None;
    for part in parts {
        if FRAGILE_BLACKLIST.contains(&part.kind) {
            continue;
        }
        if weak.is_none_or(|chosen| chosen.chip_resistance > part.chip_resistance) {
            weak = Some(part);
        }
        if big.is_none_or(|chosen| part.volume_ml > chosen.volume_ml) {
            big = Some(part);
        }
    }
    // A zero-volume part can win the loop only by being the first; the reference's `big_vol` staying at zero means it never wins at all.
    (weak, big.filter(|part| part.volume_ml > 0))
}

/// How hard a `FRAGILE_MELEE` weapon is to damage: its weakest counted part, divided by six.
///
/// Reference: `handle_melee_wear` (`melee.cpp:272-273`):
///
/// ```text
/// const float fragile_factor = 6.0f;
/// material_factor = ( weak_chip < INT_MAX ? weak_chip : shield->chip_resistance() ) / fragile_factor;
/// ```
///
/// **The division is what the flag means.** A weapon whose weakest part is resistance 12 wears as if it were resistance 2, so **one cheap bolt makes a good blade fall apart** - the reference's own words
/// are *"fragile items that fall apart easily when used as a weapon due to poor construction quality"* (`:254-255`).
///
/// The **fallback** is reached exactly when nothing counts - no parts, or every part blacklisted - because `weak_chip` starts at `INT_MAX` and any counted part replaces it. It is **not** a
/// tie-breaker and not an average: the weapon then answers for itself with its own chip resistance.
#[must_use]
pub fn fragile_material_factor(parts: &[PartFacts<'_>], own_chip_resistance: f64) -> f64 {
    const FRAGILE_FACTOR: f64 = 6.0;
    let (weak, _) = weak_and_big(parts);
    weak.map_or(own_chip_resistance, |part| f64::from(part.chip_resistance) / FRAGILE_FACTOR)
}

/// What a thing is worth as a block, from the techniques it carries.
///
/// Reference: `melee::blocking_ability` (`melee.cpp:1950-1965`):
///
/// ```text
/// if( shield.has_technique( WBLOCK_3 ) )      block_bonus = 10;
/// else if( shield.has_technique( WBLOCK_2 ) ) block_bonus = 6;
/// else if( shield.has_technique( WBLOCK_1 ) ) block_bonus = 4;
/// else if( shield.has_flag( flag_BLOCK_WHILE_WORN ) ) block_bonus = 2;
/// ```
///
/// **The chain is the rule**: a thing carrying several techniques is worth the **best** of them, and the flag is only reached when there is no technique at all. **The two is worth knowing about**:
/// `best_shield` (`:1967-1976`) throws a weapon's two away - `best_value = best_value == 2 ? 0 : best_value` - because the worn bonus only counts on something actually worn.
#[must_use]
pub fn blocking_ability(techniques: &[&str], block_while_worn: bool) -> i32 {
    if techniques.contains(&"WBLOCK_3") {
        return 10;
    }
    if techniques.contains(&"WBLOCK_2") {
        return 6;
    }
    if techniques.contains(&"WBLOCK_1") {
        return 4;
    }
    if block_while_worn { 2 } else { 0 }
}

/// What a character blocks with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shield {
    /// Nothing worth blocking with.
    None,
    /// The thing in the hand.
    Weapon,
    /// Something worn: the reference's `BLOCK_WHILE_WORN` gear.
    Worn,
}

/// Which thing `Character::best_shield` would pick, given the weapon's value and the best worn one's.
///
/// Reference: `melee.cpp:1967-1976`:
///
/// ```text
/// int best_value = blocking_ability( weapon );
/// best_value = best_value == 2 ? 0 : best_value;
/// item_location best = best_value > 0 ? get_wielded_item() : item_location();
/// if( best_worn && blocking_ability( *best_worn ) >= best_value ) best = item_location( *this, best_worn );
/// ```
///
/// **Two comparisons decide it and both are easy to get wrong.** The `== 2` throws away a weapon's worn bonus, because `BLOCK_WHILE_WORN` only pays when the thing is **worn**; and the worn test is
/// `>=`, so **a worn shield that merely ties the weapon wins the tie**.
#[must_use]
pub fn best_shield(weapon_value: i32, best_worn_value: Option<i32>) -> Shield {
    // A worn bonus in the hand counts for nothing, so a weapon worth exactly two is worth none.
    let mut best_value = weapon_value;
    if best_value == 2 {
        best_value = 0;
    }
    let mut choice = if best_value > 0 { Shield::Weapon } else { Shield::None };
    if let Some(worn) = best_worn_value
        && worn >= best_value
    {
        choice = Shield::Worn;
    }
    choice
}

/// What a block attempt came to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockAttempt {
    /// The character never tried: asleep, winded, out of blocks. **No die was rolled.**
    NotAttempted,
    /// The attempt was made and failed. **A die was rolled and a block was not spent.**
    RollFailed,
    /// The attempt landed. The caller spends the block.
    Blocked,
}

/// Whether a character gets to block, and whether the attempt lands.
///
/// Reference: `Character::block_hit` (`melee.cpp:1983-2005`), whose first steps decide before any block exists:
///
/// ```text
/// if( in_sleep_state() || has_effect( effect_narcosis ) || has_effect( effect_winded ) || ... ) return false;
/// if( blocks_left < 1 ) return false;
/// if( !x_in_y( melee_skill * 20.0 * get_limb_score( limb_score_reaction ), 100 ) ) return false;
/// blocks_left--;
/// ```
///
/// **The order is the rule, and two parts of it are visible to a player:** someone asleep or winded **rolls nothing and spends nothing**, and `blocks_left--` sits **after** the roll - so a failed
/// attempt **spends a die but not a block**, and only a landed one costs.
///
/// The caller owns `blocks_left`: this answers what happened, and the decrement is the caller's because it is state, not a decision.
#[must_use]
pub fn block_attempt(
    can_react: bool,
    blocks_left: i32,
    melee_skill: f64,
    reaction_score: f64,
    dice: &mut cdda_core_types::rng::Rolls,
) -> BlockAttempt {
    if !can_react || blocks_left < 1 {
        return BlockAttempt::NotAttempted;
    }
    if dice.x_in_y(melee_skill * 20.0 * reaction_score, 100.0) {
        BlockAttempt::Blocked
    } else {
        BlockAttempt::RollFailed
    }
}

/// The reference's logistic curve, cut off at four and normalised to `[0, 1]`.
///
/// Reference: `logarithmic( t ) = 1 / ( 1 + exp( -t ) )` and `logarithmic_range( min, max, pos )` (`cata_utility.cpp:161-193`). The curve is cut at `±LOGI_CUTOFF = 4` so its ends are usable, the
/// position is scaled onto that cut, and the result is normalised by the cut's own range.
///
/// **The clamps and the caller together fix the direction.** `block_hit` asks `logarithmic_range( 0, 40, block_score )` and uses the answer as the **multiplier the damage keeps**, so at the
/// minimum (`pos <= min`) it returns `1.0` and **nothing is stopped**, and at the maximum (`pos >= max`) it returns `0.0` and **everything is**. The reference's own comment gives the feel: a beginner
/// around 10 blocks about 15%, a proficient character around 20 about 45%, an expert in the high twenties about 80%.
#[must_use]
pub fn logarithmic_range(min: i32, max: i32, pos: i32) -> f64 {
    const LOGI_CUTOFF: f64 = 4.0;
    /// `1 / ( 1 + exp( -t ) )`, the raw logistic.
    fn logarithmic(t: f64) -> f64 {
        1.0 / (1.0 + (-t).exp())
    }
    let logi_min = logarithmic(-LOGI_CUTOFF);
    let logi_max = logarithmic(LOGI_CUTOFF);
    let logi_range = logi_max - logi_min;
    if min >= max {
        // The reference reports a debug message and answers zero; the port answers zero for the same reason.
        return 0.0;
    }
    if pos <= min {
        return 1.0;
    }
    if pos >= max {
        return 0.0;
    }
    let range = f64::from(max - min);
    let unit_pos = f64::from(pos - min) / range;
    let scaled_pos = LOGI_CUTOFF - 2.0 * LOGI_CUTOFF * unit_pos;
    (logarithmic(scaled_pos) - logi_min) / logi_range
}

/// One unit of incoming damage, as far as a block is concerned.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DamageUnit {
    /// Whether the damage type is physical.
    pub physical: bool,
    /// Whether it comes from a melee blow (`damage_type::melee_only`).
    pub melee_only: bool,
    /// How much of it there is.
    pub amount: f64,
}

/// Applies a block to a set of damage units and answers how much was stopped.
///
/// Reference: `Character::block_hit`'s damage loop (`melee.cpp:2117-2145`):
///
/// ```text
/// for( damage_unit &elem : dam.damage_units ) {
///     if( elem.type->physical && elem.type->melee_only ) {
///         float block_amount = std::min( total_phys_block, elem.amount );
///         total_phys_block -= block_amount; elem.amount -= block_amount; damage_blocked += block_amount;
///         if( elem.amount <= epsilon ) continue;
///         float previous = elem.amount; elem.amount *= physical_block_multiplier;
///         damage_blocked += previous - elem.amount;
///     }
/// }
/// ```
///
/// **Two orderings live in those lines, and both are visible:** the **flat bonus is spent before the multiplier**, so a bonus big enough to swallow a blow stops it whole and the curve never sees it;
/// and the flat bonus is spent **unit by unit in order**, so the first unit that needs it takes it rather than every unit getting a share. **Damage that is not both physical and melee-only is never
/// touched** - a block does nothing against fire or against a bullet.
pub fn block_damage(units: &mut [DamageUnit], flat_block: f64, multiplier: f64) -> f64 {
    let mut flat_left = flat_block;
    let mut blocked = 0.0;
    for unit in units.iter_mut() {
        if !(unit.physical && unit.melee_only) {
            continue;
        }
        let taken = flat_left.min(unit.amount);
        flat_left -= taken;
        unit.amount -= taken;
        blocked += taken;
        if unit.amount <= f64::EPSILON {
            continue;
        }
        let before = unit.amount;
        unit.amount *= multiplier;
        blocked += before - unit.amount;
    }
    blocked
}

/// How much of a blow a block stopped, in the words the game shows the player.
///
/// Reference: `block_hit`'s ladder (`melee.cpp:2149-2178`), keyed on `blocked_ratio = ( total_damage - damage_blocked ) / total_damage` - **which is the fraction that got through, despite the
/// name**. So every rung is read backwards: **under a fifth through is "nearly all" blocked**, and `< epsilon` is **"all"**. The last two rungs are the awkward pair: anything above epsilon is
/// "a little", and the `else` is "none", which is reached only when the ratio is **not** above epsilon - a blow that did no damage at all lands on "all" through the first rung, so "none" is
/// effectively unreachable from this function and exists for the `if` chain's completeness. The port keeps it for the same reason.
#[must_use]
pub fn blocked_description(blocked_ratio: f64) -> &'static str {
    if blocked_ratio < f64::EPSILON {
        "all"
    } else if blocked_ratio < 0.2 {
        "nearly all"
    } else if blocked_ratio < 0.4 {
        "most"
    } else if blocked_ratio < 0.6 {
        "a lot"
    } else if blocked_ratio < 0.8 {
        "some"
    } else if blocked_ratio > f64::EPSILON {
        "a little"
    } else {
        "none"
    }
}

/// Why a counter-attack did or did not happen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CounterAttack {
    /// The character is under a third of their stamina.
    TooExhausted,
    /// The weapon is more glass than the fifth the reference allows.
    TooFragile,
    /// It happens.
    Yes,
}

/// Whether a block leads to a counter-attack.
///
/// Reference: `Character::block_hit`'s tail (`melee.cpp:2190-2198`):
///
/// ```text
/// int twenty_percent = std::round( ( 20 * weapon.type->mat_portion_total ) / 100.0f );
/// if( get_stamina() < get_stamina_max() / 3 ) { "too exhausted!" }
/// else if( weapon.made_of( material_glass ) > twenty_percent ) { "too fragile to counterattack with!" }
/// else { melee_attack( *source, false, tec ); }
/// ```
///
/// **Both comparisons are strict, and both are easy to get wrong in a way nobody notices:** a character on **exactly a third** of their stamina still counters (`<`, and the third is integer division),
/// and a weapon **exactly at** the glass threshold is still sound (`>` against a rounded allowance). The allowance is rounded rather than floored, so a weapon with a total portion of 108 allows
/// `round( 21.6 ) = 22` glass, not 21.
#[must_use]
pub fn counter_attack(stamina: i32, stamina_max: i32, glass_ml: f64, material_total_ml: f64) -> CounterAttack {
    // Integer division, as the reference's `/ 3` on integers is.
    if stamina < stamina_max / 3 {
        return CounterAttack::TooExhausted;
    }
    let allowance = (20.0 * material_total_ml / 100.0).round();
    if glass_ml > allowance {
        CounterAttack::TooFragile
    } else {
        CounterAttack::Yes
    }
}
