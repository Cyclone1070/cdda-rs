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

/// What a swing costs a character in stamina.
///
/// Reference: `Character::get_total_melee_stamina_cost` (`melee.cpp:973-997`): the standard cost is `- ( weight / 16_gram + 50 )` (`character.cpp:3107-3120`), and the total adds
/// the melee skill as a rebate and subtracts any stance penalty, then keeps at least **-50** - `std::min( -50, ... )`. The result is **negative**, because the reference spends it by
/// adding it: a swing returns the stamina to take.
#[must_use]
pub fn stamina_cost(weight_g: i64, melee_skill: i32, stance_malus: i32) -> i32 {
    let standard = -(weight_g / 16 + 50);
    #[allow(clippy::cast_possible_truncation)]
    let total = i32::try_from(standard).unwrap_or(i32::MIN / 2) + melee_skill - stance_malus;
    total.min(-50)
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
