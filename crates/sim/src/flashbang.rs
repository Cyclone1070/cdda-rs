//! A flashbang: a bang everyone hears and a flash anyone looking at it sees.
//!
//! Reference: `explosion_handler::flashbang` (`explosion.cpp:657-707`). It is two different rules wearing one name:
//!
//! * **the bang** reaches every creature within eight squares through the **HEARS** flag - forty turns of deafness less four a square, so the bang is loudest at its centre;
//! * **the flash** only blinds what is looking at it: within four squares a creature is stunned, and one with the **SEES** flag and a clear line to the centre is blinded for
//!   eighteen turns less one a square.
//!
//! A **robot is not deafened**, and the reference says so by skipping every monster of the `ROBOT` species (`:689-691`) - a machine has no ears to hurt.
//!
//! **Named, not invented**: the avatar's own effects depend on things the port does not have in this data - the `PER_SLIME` and `PER_SLIME_OK` traits, the `rm13` armour, and
//! worn `BLIND` and `FLASH_PROTECTION` flags all shorten or skip the blindness. The port applies the trait and worn-flag rules it *can* read and leaves the armour item
//! unportable until the armour data is loaded, which `docs/parity.md` records. The reference's own `TODO: Blind/deafen NPC` is left as the reference left it: NPCs get neither.

use bevy_ecs::prelude::*;

/// The square the flash is at, and how far its two rules reach.
///
/// Reference: `:660-663` and `:696-705` - the bang's eight squares, the stun's four, and the sound's own twelve.
const BANG_RANGE: i32 = 8;
/// Squares within which a creature is stunned rather than merely blinded.
const STUN_RANGE: i32 = 4;
/// How loud the bang is, in the sound system's own units (`:705`).
const LOUDNESS: i32 = 12;

/// What a flashbang did, so a caller can tell a bang in an empty field from one in a crowd.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Flashbang {
    /// Every creature the bang deafened.
    pub deafened: Vec<Entity>,
    /// Every creature the flash blinded.
    pub blinded: Vec<Entity>,
    /// Every creature the flash stunned.
    pub stunned: Vec<Entity>,
}

/// Sets off a flashbang at a square.
///
/// Reference: `explosion_handler::flashbang( p, player_immune )`. `immune` is the reference's `player_immune`, which the spell sets when its caster is the avatar and the
/// spell does not count the caster as a valid target - a caster who can hit itself does not get the immunity.
pub fn flashbang(world: &mut World, centre: (i32, i32, i32), immune: bool) -> Flashbang {
    let mut outcome = Flashbang::default();
    let creatures: Vec<Entity> = {
        let mut query = world.query::<(Entity, &crate::Position)>();
        query
            .iter(world)
            .filter(|(_, at)| at.z == centre.2)
            .filter(|(_, at)| (at.x - centre.0).abs().max((at.y - centre.1).abs()) <= BANG_RANGE)
            .map(|(entity, _)| entity)
            .collect()
    };
    for creature in creatures {
        let Some(at) = world
            .get::<crate::Position>(creature)
            .map(|held| (held.x, held.y))
        else {
            continue;
        };
        let distance = (at.0 - centre.0).abs().max((at.1 - centre.1).abs());
        let person =
            world.get::<crate::Player>(creature).is_some() || crate::npcs::is_npc(world, creature);
        if !person && crate::creature_flags::has_flag(world, creature, "ROBOT") {
            // Reference: `if( critter.type->in_species( species_ROBOT ) ) { continue; }` - a robot has no ears to hurt.
            continue;
        }
        // The bang: `add_effect( deaf, 40 - dist * 4 )`, which every creature with ears within eight squares takes.
        let ears = person || crate::creature_flags::has_flag(world, creature, "HEARS");
        if ears && !(person && immune) {
            let turns = i64::from(40 - distance * 4).max(0);
            if turns > 0 && put_effect(world, creature, "deaf", turns) {
                outcome.deafened.push(creature);
            }
        }
        // The flash: only what is looking at it.
        let eyes = person || crate::creature_flags::has_flag(world, creature, "SEES");
        if !eyes || !crate::sees(world, at, (centre.0, centre.1), BANG_RANGE) {
            continue;
        }
        if distance <= STUN_RANGE && !person {
            let turns = i64::from(10 - distance).max(0);
            if turns > 0 && put_effect(world, creature, "stunned", turns) {
                outcome.stunned.push(creature);
            }
        }
        // Reference: `18 - dist` for a monster, and the avatar's own figure is `( 12 - flash_mod - dist ) / 2` turns of blindness with a `10 - dist` cap - which the port
        // applies with the reductions it can read.
        let turns = if person {
            let flash_mod = flash_reduction(world, creature);
            i64::from((12 - flash_mod - distance) / 2)
                .max(0)
                .min(i64::from(10 - distance).max(0))
        } else {
            i64::from(18 - distance).max(0)
        };
        if turns > 0 && put_effect(world, creature, "blind", turns) {
            outcome.blinded.push(creature);
        }
    }
    crate::sounds::make_a_sound_of_kind(
        world,
        centre,
        LOUDNESS,
        "a huge boom!",
        crate::sounds::SoundCategory::Combat,
    );
    outcome
}

/// Puts a timed effect on a creature, answering whether the content defines one by that name.
///
/// Reference: `critter.add_effect( effect_stunned, time_duration::from_turns( n ) )`. The port can only name an effect the world's registry resolves, which is the same guard
/// every other effect in the port uses: a component may only carry what the content defined.
fn put_effect(world: &mut World, creature: Entity, name: &str, turns: i64) -> bool {
    let Some(kind) = crate::effect_registry::kind_of(world, name) else {
        return false;
    };
    crate::applied_effects::add(
        world,
        creature,
        crate::effects::ActiveEffect::new(&kind, turns, 1),
    );
    true
}

/// How much of the flash a **person** shrugs off, which is the reference's `flash_mod`.
///
/// Reference: `explosion.cpp:672-684`: a slime-person has a chance of not looking at all (`3`, or `8` for one that can grow fresh eyes), glare resistance or the `rm13` armour
/// gives `6`, and worn `BLIND` or `FLASH_PROTECTION` gives `3`. The armour item is not ported - it is an `itype` the port's item data has, but the "worn and active" test the
/// reference uses is a bionic rule the port has not reached - so what is here are the trait and flag halves.
fn flash_reduction(world: &World, creature: Entity) -> i32 {
    if crate::traits::has_trait(world, creature, "PER_SLIME_OK") {
        return 8;
    }
    if crate::creature_flags::has_flag(world, creature, "GLARE_RESIST") {
        return 6;
    }
    if crate::worn_flags::worn_with_flag(world, creature, "BLIND")
        || crate::worn_flags::worn_with_flag(world, creature, "FLASH_PROTECTION")
    {
        return 3;
    }
    0
}
