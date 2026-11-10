//! What an explosion does to the things standing in it.
//!
//! Reference: `explosion_handler::explosion` and the second half of `do_blast` (`explosion.cpp:192-465`). The terrain half - the wavefront, the falloff, the shaped charge - is
//! [`crate::blast`], which was written first and is gated by a recording. This module is the **creature half**: the squares the wavefront reached, and what the force arriving
//! there does to whoever is standing on one.
//!
//! Two creatures, two rules, and the reference tells them apart by what they are:
//!
//! * a **monster** takes `max( force - armour / 2, 0 )` and then a blow between twice and three times that (`:415-421`);
//! * a **character** takes six blows, one per blastable part, each between twice and three times the force and each meeting `armour * 0.5` (torso, head) or `armour * 0.4`
//!   (the four limbs) - which is why a limb is hurt harder and yet is not much more deadly (`:437-462`).
//!
//! **Named, not invented**: the fire a blast starts, the items it smashes, the fragments and shrapnel, the vehicles it damages, and the bash it applies to the terrain are all
//! in the reference's same loop and none of them are ported yet; [`crate::blast`] records that list too. What is here is the damage, which is the part a spell's `explosion`
//! effect exists for.

use bevy_ecs::prelude::*;

/// The body parts the reference's blast spreads a character's damage over, with the multipliers it uses.
///
/// Reference: `blast_parts` (`explosion.cpp:433-445`), in the reference's own order: `low_mul`, `high_mul` and `armor_mul`.
const BLASTABLE_PARTS: [(&str, f32, f32, f32); 6] = [
    ("torso", 2.0, 3.0, 0.5),
    ("head", 2.0, 3.0, 0.5),
    ("leg_l", 2.0, 3.5, 0.4),
    ("leg_r", 2.0, 3.5, 0.4),
    ("arm_l", 2.0, 3.5, 0.4),
    ("arm_r", 2.0, 3.5, 0.4),
];

/// What an explosion did.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Explosion {
    /// The squares the wavefront reached, which is the damage pass's own list.
    pub reached: usize,
    /// How many creatures took a blow, and how much they took.
    pub struck: Vec<(Entity, i32)>,
}

/// The world, as far as a blast needs to know it.
struct WorldGrid<'a> {
    world: &'a World,
}

impl crate::blast::BlastGrid for WorldGrid<'_> {
    fn solid(&self, x: i32, y: i32, z: i32) -> bool {
        crate::routing::square_cost(self.world, (x, y, z)).is_none()
    }

    fn in_bounds(&self, _x: i32, _y: i32) -> bool {
        // The port's world has no edge to fall off: a square it has never loaded is solid, and a blast stops at solid ground of its own accord.
        true
    }
}

/// Explodes at a square, damaging every creature the wavefront reaches.
///
/// Reference: `explosion_handler::explosion( source, p, power, factor, fire )` (`explosion.cpp:1060-1075`) - the entry point `spell_effect::explosion` calls. `power` is the
/// blast's force at its centre and `factor` is what it is multiplied by for every square of distance; the spell states them as its damage and its area over ten.
///
/// The jitter the wavefront draws (`rng_float( 1.0f, 1.2f )`, one per square it pops) comes from the store's own generator, so a seed replays an explosion exactly.
pub fn explode(
    world: &mut World,
    source: Option<Entity>,
    centre: (i32, i32, i32),
    power: f32,
    factor: f32,
    fire: bool,
) -> Explosion {
    let Some(mut rolls) = crate::singletons::take_rolls(world) else {
        return Explosion::default();
    };
    // The wavefront draws one jitter per square it pops; the damage pass then draws one blow per creature. **Both come from the same generator**, in that order, because the
    // reference draws them in exactly that order too.
    let outcome = {
        let grid = WorldGrid { world };
        #[allow(clippy::cast_possible_truncation)]
        let mut jitter = || rolls.rng_float(1.0, 1.2) as f32;
        crate::blast::blast(&grid, power, factor, fire, &mut jitter)
    };

    let mut explosion = Explosion {
        reached: outcome.reached.len(),
        struck: Vec::new(),
    };
    for hit in outcome.reached {
        let square = (centre.0 + hit.x, centre.1 + hit.y, centre.2);
        if hit.force < 1.0 {
            continue;
        }
        for creature in crate::creature::creature_at(world, square) {
            let taken = blast_damages(world, &mut rolls, creature, hit.force);
            if taken > 0 {
                // Reference: `critter->apply_damage( mutable_source, torso, actual_dmg )` - the blow lands and the striker is named.
                #[allow(clippy::cast_precision_loss)]
                let damage = taken as f32;
                crate::creature::hurt_by(world, creature, damage, source);
                explosion.struck.push((creature, taken));
            }
        }
    }
    crate::singletons::put_rolls(world, rolls);
    explosion
}

/// The damage one creature's body takes from a force, before the reference's own dice.
///
/// Reference: `explosion.cpp:413-421` for a monster and `:437-462` for a character. The monster's answer is a single number; the character's is the six parts' blows, which the
/// port records in [`crate::creature::Wounds`] the way every other blow is recorded, and returns as their total.
fn blast_damages(
    world: &mut World,
    rolls: &mut cdda_core_types::rng::Rolls,
    creature: Entity,
    force: f32,
) -> i32 {
    let armour = world
        .get::<crate::creature::Armour>(creature)
        .map_or(0, |held| held.bash);
    let person = world.get::<crate::Player>(creature).is_some() || crate::npcs::is_npc(world, creature);
    if !person {
        // Reference: `dmg = max( force - armour / 2.0, 0.0 )`, then `rng_float( dmg * 2, dmg * 3 )` (`explosion.cpp:415-419`).
        let reduced = (f64::from(force) - f64::from(armour) / 2.0).max(0.0);
        #[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)]
        let drawn = rolls.rng_float(reduced * 2.0, reduced * 3.0);
        #[allow(clippy::cast_possible_truncation)]
        return drawn as i32;
    }
    let mut total = 0;
    for (part, low, high, armour_mul) in BLASTABLE_PARTS {
        // Reference: `rng( force * low_mul, force * high_mul )` (`explosion.cpp:447`), a **whole** number between the two multiples of the force.
        #[allow(clippy::cast_possible_truncation)]
        let blows = (force * high) as i32;
        #[allow(clippy::cast_possible_truncation)]
        let floor = (force * low) as i32;
        let raw = rolls.rng(floor, blows.max(floor));
        // The armour multiplier is the part of a `damage_instance` that scales the **armour** rather than the blow (`:451`), so a limb at 0.4 lets more through than a torso at 0.5.
        #[allow(clippy::cast_precision_loss, clippy::cast_possible_truncation)]
        let resistance = (armour as f32 * armour_mul) as i32;
        let dealt = (raw - resistance).max(0);
        if dealt > 0 {
            crate::creature::wound(world, creature, part, dealt);
            total += dealt;
        }
    }
    total
}
