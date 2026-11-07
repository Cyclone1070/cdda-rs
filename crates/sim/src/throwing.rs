//! Throwing something at a square: the spread rules, the landing rule and the move, wired together.
//!
//! Reference: `Character::throw_item` (`src/ranged.cpp:1467-1600`). The reference's version also computes damage, shattering and noise; the port carries the part that
//! decides **where the thing goes**, because that is the part every later rule (a hit, a break, a fire) reads. An item is an entity here, so "throwing" is taking it out of
//! the hand and putting it on the square the rules name.

use bevy_ecs::prelude::*;

use crate::dispersion::{Dispersion, MAX_SKILL, Throw, throwing_dispersion};

/// What came of a throw.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Toss {
    /// It left the hand and stopped here.
    Landed {
        /// The square it stopped on.
        at: (i32, i32, i32),
        /// How badly the throw missed, as a fraction of the target.
        missed_by: f64,
        /// What it hit, and for how much, when something was standing where it landed.
        hit: Option<(Entity, i32)>,
    },
    /// The thrower is not in the world, so there is nowhere to throw from.
    NoBody,
    /// The thing is not in the thrower's hands.
    NotCarried,
    /// The target was the thrower's own square, which the reference refuses outright.
    AtYourself,
}

/// Throws what a creature holds at a square.
///
/// Reference: `Character::throw_item` (`src/ranged.cpp:1467-1600`) composes `throwing_dispersion` with `projectile_attack`, which rolls the spread through
/// [`crate::ballistics`]. The port does the same three steps: the spread from what is thrown and who throws it, a roll of that spread, and the aim rule that turns the roll
/// into a square. **The reference's damage, shattering and noise are not here** - they belong to what happens *on landing*, which is a rule of the square's own.
pub fn throw_at(world: &mut World, thrower: Entity, item: Entity, target: (i32, i32, i32)) -> Toss {
    let Some(source) = world
        .get::<crate::Position>(thrower)
        .map(|place| (place.x, place.y, place.z))
    else {
        return Toss::NoBody;
    };
    if target == source {
        return Toss::AtYourself;
    }
    let held = world.get::<crate::creature::Carried>(item).map(|who| who.0) == Some(thrower)
        || world.get::<crate::creature::WieldedBy>(item).map(|who| who.0) == Some(thrower);
    if !held {
        return Toss::NotCarried;
    }
    let Some(kind) = world.get::<crate::GroundItem>(item).map(|held| held.kind) else {
        return Toss::NotCarried;
    };
    let charges = world.get::<crate::GroundItem>(item).map_or(1, |held| held.charges);
    let names = crate::item_registry::snapshot(world);
    // Reference: `Character::throw_item` (`ranged.cpp:1474-1475`) charges the arm **before** it rolls anything, so a throw that misses costs the same as one that hits.
    crate::creature::spend_moves(world, thrower, f64::from(throw_cost(
        crate::melee::attack_time(i64::from(names.volume_of(kind)), names.weight_of(kind), i64::from(charges)),
        crate::skills_of::level_of(world, thrower, "throw"),
        crate::creature::effective_dexterity(world, thrower),
        stamina_ratio(world, thrower),
    )));
    // Reference: `throw_dispersion_per_dodge` reads the **arm** strength, which is the strength a creature can put behind a throw.
    let arm_strength = crate::creature::effective_strength(world, thrower);
    // Reference: `get_skill_level( skill_throw )`, from the creature's own skill map.
    let skill = f64::from(crate::skills_of::level_of(world, thrower, "throw"));
    // Reference: a throw at a **creature** is dodged and the dodging adds spread (`throw_dispersion_per_dodge`); a throw at open ground is not. The port asks whether a creature stands on
    // the target square, and reads its size here so the aim roll below judges the miss against the thing that is actually there (`occupied_tile_fraction`).
    let target_size = target_size(world, thrower, target);
    // The port has no eyes to encumber and no perception score, so those two terms of the reference's sum are zero - said here rather than left to look accidental.
    let spread = throwing_dispersion(&Throw {
        weight_g: names.weight_of(kind),
        volume_ml: i64::from(names.volume_of(kind)),
        charges,
        arm_strength,
        skill,
        perception: 0,
        eye_encumbrance: 0,
        blinded: false,
        target: None,
    });
    // `rl_dist` in the reference is the **larger** of the two offsets, which is what a diagonal step counts as one square.
    let range = (target.0 - source.0).abs().max((target.1 - source.1).abs());
    let shot = Dispersion {
        linear: vec![f64::from(spread)],
        ..Dispersion::default()
    };
    // The dice are taken for the length of a draw and put back, which is how every other rule in the port draws. A world with no generator has no spread to draw from, so
    // the throw is a perfect aim - the same convention [`crate::craft_roll`] uses, where a missing generator draws zero rather than refusing the action.
    let rolled = match crate::singletons::take_rolls(world) {
        Some(mut dice) => {
            let drawn = shot.roll(&mut dice);
            crate::singletons::put_rolls(world, dice);
            drawn
        }
        None => 0.0,
    };
    let aim = crate::ballistics::aim_of(rolled, f64::from(range), target_size);
    let flight = crate::ballistics::Shot {
        source,
        target,
        range,
        aim,
        // A throw at open ground stops where it was aimed rather than flying on, which is the reference's `no_overshoot` for a passable square with no creature.
        no_overshoot: true,
    };
    let at = match crate::singletons::take_rolls(world) {
        Some(mut dice) => {
            let stopped = crate::ballistics::landing(&flight, &mut dice);
            crate::singletons::put_rolls(world, dice);
            stopped
        }
        None => target,
    };
    // Reference: a projectile heavier than **500 grams** carries `HEAVY_HIT` (`ranged.cpp:1522-1524`), and the landing branch answers it with a thud of volume **8**
    // (`ballistics.cpp:180-186`). A thing too light for that lands in silence, which is the half a test can tell apart from "everything thuds".
    if let Some(volume) = landing_noise(world, at, names.weight_of(kind)) {
        crate::sounds::make_sound(world, at, volume);
    }
    // The move itself: out of the hand, onto the square. The store finds an item by its position, so this is one component's worth of change.
    world
        .entity_mut(item)
        .remove::<crate::creature::Carried>()
        .remove::<crate::creature::WieldedBy>()
        .insert(crate::Position {
            x: at.0,
            y: at.1,
            z: at.2,
        });
    // Reference: `impact.add_damage( damage_bash, std::min( weight / 100_gram, thrown_item_adjusted_damage( thrown ) ) )` (`ranged.cpp:1505-1506`), then
    // `deal_projectile_attack` against whatever the projectile reached. The port resolves the blow through the creature's hit points, which is where a thrown rock's damage
    // goes; **armour, body parts and weakpoints are the P4 half that is not written yet**, and a target with no hit points takes nothing rather than being invented.
    // Reference: `proj.impact = thrown.base_damage_thrown()` (`ranged.cpp:1435`) and then `impact.add_damage( damage_bash, min( weight / 100 g, arm ) )`
    // (`ranged.cpp:1505-1506`) - the thing's own damage **plus** what the arm adds, not the larger of the two.
    let blow = names.thrown_of(kind)
        + damage_of_throw(arm_strength, crate::skills_of::level_of(world, thrower, "throw"), names.weight_of(kind));
    #[allow(clippy::cast_possible_truncation)]
    let blow_whole = blow as i32;
    let hit = strike_whatever_stands_there(world, thrower, at, blow_whole);
    Toss::Landed {
        at,
        missed_by: aim.missed_by,
        hit,
    }
}

/// How loud a landing is, or nothing at all when the thing is too light to be heard.
///
/// Reference: the landing branch of `projectile_attack` (`src/ballistics.cpp:180-186`): inside the `HEAVY_HIT` test, a square carrying `TFLAG_LIQUID` answers "splash!"
/// at volume **10** and any other square a thud at volume **8**. A thing of 500 grams or less is not heavy (`weight > 500_gram`, `ranged.cpp:1522-1524`), so it says nothing
/// wherever it lands.
#[must_use]
pub fn landing_noise(world: &World, square: (i32, i32, i32), weight_g: i64) -> Option<i32> {
    if weight_g <= 500 {
        return None;
    }
    let wet = crate::square::terrain_at(world, square)
        .is_some_and(|code| crate::terrain_types::has_flag(world, code, "LIQUID"));
    Some(if wet { 10 } else { 8 })
}

/// What the **arm** adds to a thrown thing's blow, in points of damage.
///
/// The thing's own damage is `item::base_damage_thrown` and lives in the item table ([`crate::item_registry::ItemNames::thrown_of`]); this is the half the thrower adds.
///
/// Reference: `Character::thrown_item_adjusted_damage` (`src/ranged.cpp:1411-1428`): `arm_strength / 2` times `(min(10, skill) / 10) * 0.85 + 0.15`, and the whole thing is
/// capped by the thing's own weight at **a point a hundred grams**. The reference's comment states the cap in words - "up to str/2 or weight/100g (lower), so 10 str is 5
/// damage before multipliers" - and this function reproduces both halves, so a strong arm cannot make a pebble hit like a brick.
#[must_use]
pub fn damage_of_throw(arm_strength: i32, skill: i32, weight_g: i64) -> f64 {
    // Reference: `stats_mod *= ( std::min( MAX_SKILL, skill ) / MAX_SKILL ) * 0.85 + 0.15` - a novice still gets a seventh of the arm, which is why an unskilled throw is
    // weak rather than nothing.
    let practised = (f64::from(skill).min(MAX_SKILL) / MAX_SKILL) * 0.85 + 0.15;
    let from_the_arm = f64::from(arm_strength) / 2.0 * practised;
    // Reference: `std::min( weight / 100.0_gram, ... )` - the weight is in grams here and the divisor in hundreds of them.
    #[allow(clippy::cast_precision_loss)]
    let from_the_weight = weight_g as f64 / 100.0;
    from_the_arm.min(from_the_weight)
}

/// How big the thing on the target square is, as a fraction of its tile.
///
/// Reference: `projectile_attack`'s `target_critter->ranged_target_size()` (`ballistics.cpp:255-264`). An empty square - or the thrower's own - is a whole tile, which is what the reference's
/// `map::ranged_target_size` answers for terrain; **a creature is the fraction its size fills**.
pub(crate) fn target_size(world: &World, thrower: Entity, target: (i32, i32, i32)) -> f64 {
    crate::creature::creature_at(world, target)
        .into_iter()
        .find(|entity| *entity != thrower)
        .and_then(|entity| world.get::<crate::creature::Creature>(entity).map(|creature| creature.size))
        .map_or(1.0, crate::ballistics::occupied_tile_fraction)
}

/// What a throw costs the arm that makes it, in action points.
///
/// Reference: `throw_cost` (`src/ranged.cpp:1247-1273`). It is the melee swing with three deliberate differences the reference's own comment names: **the base is halved**, **the skill
/// divides by twenty rather than fifteen** (so mastery takes the cost down by a twentieth a level instead of a fifteenth), and **dexterity comes off whole rather than halved**. The stamina
/// penalty multiplies the base only, never the skill term. The floor is twenty-five moves - a throw is never free - which is the number the reference returns last.
///
/// The two melee modifier multipliers (`melee_thrown_move_lift_mod` and `..._balance_mod`) are **one point zero** in the port because no effect it carries states either, and the
/// reference's `ATTACK_SPEED` enchantment is not applied for the same reason. Said here rather than left to look accidental.
///
/// Reference: `item::attack_time( c )` is the port's [`crate::melee::attack_time`], and it already takes the held thing's volume and weight.
#[must_use]
pub fn throw_cost(base_move_cost: i32, throw_skill: i32, dexterity: i32, stamina_ratio: f64) -> i32 {
    let base = f64::from(base_move_cost) / 2.0;
    let skill = base * f64::from(20 - throw_skill.min(10)) / 20.0;
    #[allow(clippy::cast_possible_truncation)]
    let skill_cost = skill as i32;
    let stamina_penalty = 1.0 + ((0.25 - stamina_ratio) * 4.0).max(0.0);
    #[allow(clippy::cast_possible_truncation)]
    let cost = (base * stamina_penalty) as i32 + skill_cost - dexterity;
    cost.max(25)
}

/// What share of its wind a creature has left, from zero to one: `get_stamina() / get_stamina_max()`.
///
/// A creature that carries no [`crate::creature::Stamina`] is treated as **full**, which is the convention the port's default stamina follows - a thrower the port invented has not been
/// running.
fn stamina_ratio(world: &bevy_ecs::prelude::World, thrower: bevy_ecs::prelude::Entity) -> f64 {
    world.get::<crate::creature::Stamina>(thrower).map_or(1.0, |stamina| stamina.cur / stamina.max.max(1.0))
}

/// Who the thrown thing struck and how much it took, or nothing when the square was empty.
///
/// Reference: `projectile_attack`'s hit branch hands the impact to `deal_projectile_attack`, which calls the target's `absorb_hit` (`monster.cpp:2085`) - that subtracts the resistance
/// **capped at the blow**, so armour can stop a rock entirely and can never heal what it hits. A thrown thing lands on a **weak point** like any other projectile (`weakpoint.cpp:588-631`,
/// with `Character::throw_weakpoint_skill`'s `throw`-twice, vision-twice figures), and a creature with no hit points takes nothing rather than being invented.
fn strike_whatever_stands_there(
    world: &mut World,
    thrower: Entity,
    at: (i32, i32, i32),
    blow: i32,
) -> Option<(Entity, i32)> {
    crate::creature::creature_at(world, at)
        .into_iter()
        .find(|standing| *standing != thrower && crate::creature::health_of(world, *standing).is_some())
        .and_then(|struck| {
            let before = crate::creature::health_of(world, struck)?;
            // Reference: `Character::throw_weakpoint_skill` (`weakpoint.cpp:104-107`) - `throw` twice and **vision twice** - and the attack type is `PROJECTILE`, because a thrown rock is
            // not a melee blow however it lands.
            let skill = {
                let level = f64::from(crate::creature::skill_level(world, thrower, "throw"));
                crate::weakpoints::skill_with_families(
                    crate::weakpoints::attacker_skill(
                        level,
                        level,
                        crate::creature::effective_dexterity(world, thrower),
                        crate::creature::effective_perception(world, thrower),
                        crate::creature::limb_score(world, thrower, "vision"),
                        crate::creature::limb_score(world, thrower, "vision"),
                    ),
                    // Reference: the **target's** families are the last term of the attacker's skill (`weakpoint.cpp:401-409`).
                    world
                        .get::<crate::weakpoints::WeakPoints>(struck)
                        .map_or(&[][..], |weak| weak.families.as_slice()),
                    &|proficiency| crate::proficiencies::knows(world, thrower, proficiency),
                )
            };
            let found = match crate::singletons::take_rolls(world) {
                Some(mut dice) => {
                    let found = crate::weakpoints::stored_weak_point(
                        world,
                        struck,
                        crate::weakpoints::AttackType::Projectile,
                        skill,
                        &mut dice,
                    );
                    crate::singletons::put_rolls(world, dice);
                    found
                }
                None => None,
            };
            #[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)]
            let (blow, resisted) = match found.as_ref() {
                Some(weak) => {
                    // A thrown thing's blow is **bash**, and its armour is the body's bash figure - so the weak point's two maps are read for that one type.
                    let (bash, _, _) = crate::weakpoints::apply_to_blow(
                        &weak.factors,
                        (f64::from(blow), 0.0, 0.0),
                        false,
                    );
                    let armour = world.get::<crate::creature::Armour>(struck).map_or(0, |armour| armour.bash);
                    let (resist, _, _) = crate::weakpoints::apply_to_armour_triple(
                        &weak.factors,
                        (f64::from(armour), 0.0, 0.0),
                    );
                    (bash as i32, resist as f32)
                }
                None => (
                    blow,
                    world.get::<crate::creature::Armour>(struck).map_or(0.0, |armour| armour.bash as f32),
                ),
            };
            #[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)]
            let hurt = crate::melee::after_armour(blow as f32, resisted, 0.0, 0.0).floor() as i32;
            let fatal = crate::damage_monster(world, struck, hurt);
            // Reference: `monster::deal_damage` practises the struck body's families for two seconds on any character's hit (`monster.cpp:3888-3892`).
            {
                let families = world
                    .get::<crate::weakpoints::WeakPoints>(struck)
                    .map(|weak| weak.families.clone())
                    .unwrap_or_default();
                crate::weakpoints::practise_a_hit(world, thrower, &families);
            }
            // Reference: `wkpt->apply_effects` runs after the damage lands (`creature.cpp:1453-1455`), for a thrown thing exactly as for a shot.
            if let Some(weak) = found.as_ref() {
                #[allow(clippy::cast_precision_loss)]
                let dealt = hurt as f32;
                crate::creature::triggered_effects(world, struck, weak, dealt);
            }
            // A killed creature has no hit points left to read, so the blow it took is the damage itself.
            let after = if fatal {
                before - hurt
            } else {
                crate::creature::health_of(world, struck).unwrap_or(before)
            };
            Some((struck, (before - after).max(0)))
        })
}
