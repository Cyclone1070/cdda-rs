//! Casting a spell at a creature, as far as the data allows it today.
//!
//! Reference: `spell::cast` (`magic.cpp`) picks the shape, `spell_effect::target_attack` finds what is in the area,
//! and `damage_target` hands each of them a `damage_instance` - which the creature then meets with the armour on the
//! part the blow landed on. The port keeps that shape: a spell's numbers come from content ([`cdda_content::spells`]),
//! the range and damage are worked out at the caster's level, and the damage goes through the same part-armour the
//! melee path uses, so a vest protects you from a taser for the same reason it protects you from a fist.

use bevy_ecs::prelude::*;
use cdda_content::spells::Spell;

use crate::creature::{Body, armour_on, hurt};
use crate::through_armour;

/// What a caster's body does to a spell's price: how much its hands encumber it, and whether an enchantment has made the
/// casting wordless and gestureless.
///
/// Reference: `spell::energy_cost` (`magic.cpp:1178-1192`) reads `avg_encumb_of_limb_type( hand ) - 5`, ignores the first ten
/// points of combined encumbrance, and charges different rates per energy source; `json_flag_SUBTLE_SPELL` skips the term
/// entirely. A caster that floats free of its hands is the same caster the reference calls `no_hands()`.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Somatic {
    /// The caster's average hand encumbrance, in the reference's own units.
    pub hand_encumbrance: i32,
    /// Whether the caster has the subtle-spell flag, which makes every gesture free.
    pub subtle_spell: bool,
    /// Whether the spell itself needs no hands.
    ///
    /// Reference: `spell::no_hands()` is `has_flag( spell_flag::NO_HANDS ) || temp_somatic_difficulty_multiplyer <= 0`
    /// (`magic.cpp:1204-1207`). **The port does not load spell flags yet**, so the caller states it - and with a caster that carries
    /// nothing the whole term is zero whichever way it is stated, which is why no gate could tell the two apart today.
    pub no_hands: bool,
}

/// What a spell costs to cast, in the units of its own energy source.
///
/// Reference: `spell::energy_cost` (`magic.cpp:1163-1195`). The price is not `Scaling::at`'s clamp: the reference branches on
/// where the base sits against the final value and then **rounds** `base + increment * level`, so a spell whose increment
/// runs away from its final value still stops there, and one whose base already equals its final ignores the increment
/// altogether. `None` when the data wrote an expression where a number belongs, because a guessed price is a silent
/// divergence in every cast.
#[must_use]
#[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss, clippy::cast_sign_loss)]
pub fn energy_cost(spell: &Spell, level: i32, somatic: Somatic) -> Option<i32> {
    let base = spell.energy.start?;
    let increment = spell.energy.increment?;
    let end = spell.energy.end?;
    let climbed = base + increment * f64::from(level);
    let mut cost = if base < end {
        end.min(climbed.round())
    } else if base > end {
        end.max(climbed.round())
    } else {
        base
    } as i32;
    // Reference: `if( !no_hands() && !guy.has_flag( json_flag_SUBTLE_SPELL ) )` (`magic.cpp:1178`).
    if !somatic.no_hands && !somatic.subtle_spell {
        let encumbrance = 0.max(somatic.hand_encumbrance - 5);
        cost += match spell.energy_source.as_str() {
            "HP" => encumbrance,
            "STAMINA" => 100 * encumbrance,
            _ => 10 * encumbrance,
        };
    }
    Some(cost.max(0))
}

/// Whether a caster can pay a spell's price.
///
/// Reference: `known_magic::has_enough_energy` (`magic.cpp:2638-2661`), which is one comparison per energy source, and where the comparisons differ on purpose: mana, stamina
/// and stored power are compared with `>=`, so a caster with exactly the price can cast, while blood magic asks whether any part has **more** than the price (`>`), so a part
/// that could pay exactly is not allowed to. A source the port has no store for answers [`Afford::Unknown`] rather than "yes", because a free cast of a spell that should cost
/// something is a silent divergence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Afford {
    /// The caster has enough.
    Yes,
    /// It has not, and this is what it has.
    No {
        /// What the store holds now, in the price's own units.
        available: i64,
    },
    /// The port has no store for this source, so it cannot answer.
    Unknown,
}

/// Whether a caster can pay for a spell at this level.
#[must_use]
pub fn can_afford(world: &World, caster: Entity, spell: &Spell, level: i32, somatic: Somatic) -> Afford {
    let Some(cost) = energy_cost(spell, level, somatic) else {
        return Afford::Unknown;
    };
    match spell.energy_source.as_str() {
        "MANA" => match world.get::<crate::creature::Mana>(caster) {
            Some(mana) => match i64::from(mana.cur).cmp(&i64::from(cost)) {
                std::cmp::Ordering::Less => Afford::No {
                    available: i64::from(mana.cur),
                },
                _ => Afford::Yes,
            },
            None => Afford::Unknown,
        },
        "STAMINA" => match world.get::<crate::creature::Stamina>(caster) {
            Some(stamina) => {
                #[allow(clippy::cast_possible_truncation)]
                let available = stamina.cur as i64;
                if available >= i64::from(cost) {
                    Afford::Yes
                } else {
                    Afford::No { available }
                }
            }
            None => Afford::Unknown,
        },
        "BIONIC" => match world.get::<crate::creature::BionicPower>(caster) {
            Some(power) => {
                // Reference: `guy.get_power_level() >= units::from_kilojoule( cost )`, a thousand joules a kilojoule.
                let needed = i64::from(cost) * 1_000;
                if power.joules >= needed {
                    Afford::Yes
                } else {
                    Afford::No {
                        available: power.joules / 1_000,
                    }
                }
            }
            None => Afford::Unknown,
        },
        "NONE" => Afford::Yes,
        // Blood magic asks the caster to choose a part to draw from, and its own rule is a menu (`handle_action.cpp:1946-1949`, `magic.cpp:1867-1887`). The port has body
        // parts and their wounds but no way to choose one, so it declines to answer rather than draining the wrong limb.
        _ => Afford::Unknown,
    }
}

/// Takes a spell's price from whatever pays it.
///
/// Reference: `spell::consume_spell_cost` (`magic.cpp:1889-1912`). The failure discount is the caller's business here (`cast_success` is a parameter, as in the reference), and
/// blood magic arrives as [`Afford::Unknown`] from [`can_afford`] and is not spent at all.
#[must_use]
pub fn spend(world: &mut World, caster: Entity, spell: &Spell, level: i32, somatic: Somatic, cast_success: bool, failure_percent: f64) -> bool {
    let Some(cost) = energy_cost(spell, level, somatic) else {
        return false;
    };
    #[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)]
    let cost = if cast_success {
        cost
    } else {
        (f64::from(cost) * failure_percent) as i32
    };
    match spell.energy_source.as_str() {
        "MANA" => {
            let Some(mut mana) = world.get_mut::<crate::creature::Mana>(caster) else {
                return false;
            };
            mana.cur -= cost;
            true
        }
        "STAMINA" => {
            if world.get::<crate::creature::Stamina>(caster).is_none() {
                return false;
            }
            crate::creature::spend_stamina(world, caster, -cost);
            true
        }
        "BIONIC" => {
            let Some(mut power) = world.get_mut::<crate::creature::BionicPower>(caster) else {
                return false;
            };
            power.joules -= i64::from(cost) * 1_000;
            true
        }
        "NONE" => true,
        _ => false,
    }
}

/// What came of casting.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Cast {
    /// It landed: which part it hit and how much got through.
    Hit {
        /// The body part the blow landed on.
        part: String,
        /// What the target actually lost, after armour.
        dealt: i32,
    },
    /// The spell's effect is not one this rule runs. Only `attack` is ported so far.
    NotAnAttack,
    /// The target is further away than the spell reaches at this level.
    OutOfRange,
    /// The target has no body, or the caster and target are the same thing.
    NothingToHit,
}

/// Casts an `attack` spell at one creature.
///
/// Reference: `spell_effect::target_attack` -> `damage_target`, which deals the spell's damage type at
/// `spell::get_damage()` (the level-scaled value) to the part the blow lands on, and lets the creature's armour meet
/// it.
///
/// `fraction` is where in the hit-location roll the blow lands: 0.0 is the first part a body offers. It is a
/// parameter rather than a draw inside this function for the same reason every other roll in the port is - a rule
/// that reached for its own randomness could not be tested at all.
#[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)]
pub fn cast_attack(
    world: &mut World,
    caster: (i32, i32, i32),
    spell: &Spell,
    level: i32,
    target: Entity,
    fraction: f64,
) -> Cast {
    if spell.effect_name != "attack" {
        return Cast::NotAnAttack;
    }
    let Some(place) = world.get::<crate::Position>(target) else {
        return Cast::NothingToHit;
    };
    // Reference: `spell::range()` is the level-scaled reach and `rl_dist` is the distance it is measured against -
    // the larger of the two axes, not the diagonal.
    let away = (place.x - caster.0).abs().max((place.y - caster.1).abs());
    let reach = spell.range.at(level).unwrap_or(f64::from(away));
    if f64::from(away) > reach {
        return Cast::OutOfRange;
    }
    damage_one(world, spell, level, target, fraction)
}

/// The damage a spell does at a level, as the reference's integer.
///
/// Reference: `spell::damage` (`magic.cpp:816-840`) returns `min_leveled_damage( caster )`, which is
/// `static_cast<int>( min_damage.evaluate( d ) + damage_increment * level )` - a **truncation toward zero**, not a rounding. That difference is invisible for the whole numbers most
/// of the data uses and decides the answer for the halves: 2.5 damage is 2 to the reference.
///
/// **Named gap**: the reference's `RANDOM_DAMAGE` branch draws between the minimum and the maximum, and no dump carries which spells have it, so the port answers the minimum
/// branch for every spell rather than drawing with a generator the data did not ask for.
#[must_use]
pub fn damage_of(spell: &Spell, level: i32) -> Option<i32> {
    let damage = spell.damage.at(level)?;
    #[allow(clippy::cast_possible_truncation)]
    let damage = damage as i32;
    Some(damage)
}

/// How loud a spell is when it is cast the ordinary way.
///
/// Reference: `spell::sound_volume` (`magic.cpp:1630-1640`): nothing at all when the spell is `SILENT`; otherwise `abs( damage ) / 3`, with `LOUD` adding `1 + damage / 3`, and the
/// whole thing multiplied by a temporary sound multiplier (which nothing in the port sets, so it is the identity) and clamped to at least zero.
///
/// **`SILENT` is checked here and not by every effect.** `spell_effect::noise` calls `make_sound( target, damage )` directly, so a silent spell that *is* a noise still makes one;
/// what the flag silences is the ordinary cast's own sound.
#[must_use]
pub fn sound_volume(spell: &Spell, level: i32) -> i32 {
    if spell.has_flag("SILENT") {
        return 0;
    }
    let Some(damage) = damage_of(spell, level) else {
        return 0;
    };
    let mut loudness = damage.abs() / 3;
    if spell.has_flag("LOUD") {
        loudness += 1 + damage / 3;
    }
    loudness.max(0)
}

/// Applies an `attack` spell to one creature, once the cast has decided it is in reach.
///
/// Reference: `damage_targets` (`magic_spell_effect.cpp:546-563`), which is what every attack does to each thing it reaches:
/// the effect first, then a body part, then the damage that gets through. Split out of [`cast_attack`] because a spell whose
/// area covers several creatures applies this to each of them without asking about reach again - the area is what decided,
/// not the range.
pub fn damage_one(
    world: &mut World,
    spell: &Spell,
    level: i32,
    target: Entity,
    fraction: f64,
) -> Cast {
    // Reference: `damage_targets` applies the spell's effect **before** it deals damage, and only when the spell names one
    // (`magic_spell_effect.cpp:553-563`). The duration is the spell's own, scaled and clamped by the effect type's caps
    // exactly as `Creature::add_effect` would.
    if !spell.effect_str.is_empty()
        && let Some(turns) = spell.duration.at(level)
        && let Some(kind) = crate::effect_registry::kind_of(world, &spell.effect_str)
    {
        #[allow(clippy::cast_possible_truncation)]
        let turns = turns.round() as i64;
        crate::applied_effects::add(
            world,
            target,
            crate::effects::ActiveEffect::new(&kind, turns, 1),
        );
    }
    let Some(body) = world.get::<Body>(target) else {
        return Cast::NothingToHit;
    };
    let Some(part) = lands_on(&body.parts, fraction) else {
        return Cast::NothingToHit;
    };
    let Some(damage) = spell.damage.at(level) else {
        // An expression the loader would not read as a number: the port cannot say what the spell does, and a guess
        // here would be a silent divergence in every fight that involves it.
        return Cast::NothingToHit;
    };
    let armour = armour_on(world, target, &part);
    #[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)]
    let arrived = damage.round() as i32;
    // The garment that stopped the blow may not survive stopping it, and it is asked with what arrived rather than
    // what got through (`character_armor.cpp`).
    crate::creature::settle_armour(world, target, &part, damage, fraction);
    let dealt = through_armour(arrived, armour);
    #[allow(clippy::cast_precision_loss)]
    let as_float = dealt as f32;
    hurt(world, target, as_float);
    Cast::Hit { part, dealt }
}

/// Casts an `attack` spell at a creature, drawing where it lands from the store's own randomness.
///
/// Reference: `spell_effect::target_attack` asks `Creature::get_random_body_part()`, which is
/// `anatomy::random_body_part` (`anatomy.cpp:203-206`) - a draw over the sum of the parts' `hit_size`, picking the
/// first part whose running total reaches it. Nothing about that draw is the melee rule: a struck body part is
/// tilted by how well the blow landed, while a spell simply lands somewhere on the body.
///
/// This is [`cast_attack`] with the fraction taken from `Rolls` instead of from a caller, so a seed replays the
/// fight exactly - which is the whole reason the port passes rolls around instead of reaching for a global generator.
pub fn cast_at(
    world: &mut World,
    caster: (i32, i32, i32),
    spell: &Spell,
    level: i32,
    target: Entity,
) -> Cast {
    let Some(mut rolls) = crate::singletons::take_rolls(world) else {
        // No generator in the store: the port refuses to invent one, because a cast that used a different
        // generator than the turn would make a replay disagree with itself.
        return Cast::NothingToHit;
    };
    // The reference draws `rng_float( 0, size_sum )`; a unit draw scaled by the total weight is the same thing,
    // because `pick_by_weight` scales it back up. `rng_bits` over `u32::MAX` is the port's idiom for that draw and
    // is what makes a recording replay it.
    let fraction = f64::from(rolls.rng_bits()) / f64::from(u32::MAX);
    crate::singletons::put_rolls(world, rolls);
    let landed = cast_attack(world, caster, spell, level, target, fraction);
    match landed {
        Cast::Hit { .. } => landed,
        other => other,
    }
}

/// The body part a spell lands on, given a draw over the sum of the parts' hit sizes.
///
/// Reference: `anatomy::random_body_part` - weighted by `hit_size`, not by how well anything landed.
#[must_use]
pub fn lands_on(parts: &[crate::hit_location::BodyPart], fraction: f64) -> Option<String> {
    let weights: Vec<f32> = parts.iter().map(|part| part.hit_size).collect();
    let index = crate::hit_location::pick_by_weight(&weights, fraction)?;
    parts.get(index).map(|part| part.name.clone())
}


