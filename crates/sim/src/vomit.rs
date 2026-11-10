//! Throwing up.
//!
//! Reference: `Character::vomit` (`character.cpp:7520-7560`). The shape is: **empty the stomach**, leave bile on the ground if there was anything in it, make the character
//! nauseous for a while, spend a hundred moves, and take the edge off whatever the character was poisoned or drunk by.
//!
//! The reference's own quirk is worth keeping rather than tidying: the nausea duration is computed from `stomach.contains()` **after** the stomach has been emptied, so it is
//! always the effect's `int_dur_factor` - the floor - and never the larger `max_duration * ml / 21` it looks like it should be. A port that read the volume before emptying would
//! give a full stomach a longer nausea than the reference does.

use bevy_ecs::prelude::*;

/// What one retch did, so a caller can tell a real vomit from a dry heave.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Vomit {
    /// How much the stomach held before it was emptied, in millilitres.
    pub emptied_ml: i64,
    /// Whether the character was woken by it, which only happens when there was something to bring up.
    pub woke: bool,
    /// Whether the nausea effect was added, which the reference skips while the character is already nauseous.
    pub nauseated: bool,
}

/// Makes a character throw up.
///
/// Reference: `Character::vomit()`. `None` means the creature has no stomach to empty, which is every creature that does not eat.
pub fn vomit(world: &mut World, character: Entity) -> Option<Vomit> {
    let before = {
        let body = crate::body::body_of(world, character)?;
        body.stomach.contains_ml()
    };
    let mut outcome = Vomit {
        emptied_ml: before,
        ..Vomit::default()
    };
    if let Some(mut body) = world.get_mut::<crate::body::Body>(character) {
        body.stomach.empty();
    }
    // Reference: `if( stomach_contents_before_vomit != 0_ml )` - bile on the ground, a message, and the character wakes up.
    if before != 0 {
        let at = world
        .get::<crate::Position>(character)
        .map(|held| (held.x, held.y, held.z));
    if let Some(at) = at
            && let Some(spot) = adjacent_tile(world, at)
            && let Some(bile) = crate::field_registry::id_of(world, "fd_bile")
        {
            crate::add_field_strength(world, spot, bile, 1);
        }
        // Reference: `wake_up()`, which clears the sleep state. A creature that was not asleep was not woken, so the answer is whether the marker was there.
        outcome.woke = world.get::<crate::needs::Asleep>(character).is_some();
        world.entity_mut(character).remove::<crate::needs::Asleep>();
    }
    // Reference: the nausea, only when the character is not already nauseous - the reference's own guard against a retch loop. The duration is the effect's
    // `int_dur_factor`, because the volume it multiplies has just been emptied to zero.
    if !crate::applied_effects::has(world, character, "nausea")
        && let Some(kind) = crate::effect_registry::kind_of(world, "nausea")
    {
        let duration = kind.int_dur_factor.max(0);
        crate::applied_effects::add(
            world,
            character,
            crate::effects::ActiveEffect::new(&kind, duration, 1),
        );
        outcome.nauseated = true;
    }
    // Reference: `mod_moves( -100 )`.
    crate::creature::spend_moves(world, character, 100.0);
    // Reference: thirty minutes off any food poisoning, and ten to fifty minutes off being drunk.
    let mut taken = crate::singletons::take_rolls(world);
    let drunk_loss = taken.as_mut().map(|dice| dice.rng(600, 3000));
    if let Some(rolls) = taken {
        crate::singletons::put_rolls(world, rolls);
    }
    shorten(world, character, "foodpoison", 1_800);
    if let Some(loss) = drunk_loss {
        shorten(world, character, "drunk", i64::from(loss));
    }
    // Reference: `remove_effect` for the five painkiller effects, all of them removed whatever they were doing.
    for pkill in [
        "pkill1_generic",
        "pkill1_acetaminophen",
        "pkill1_nsaid",
        "pkill2",
        "pkill3",
    ] {
        crate::applied_effects::remove(world, character, pkill);
    }
    Some(outcome)
}

/// Takes time off an effect the character wears, if it wears one.
fn shorten(world: &mut World, creature: Entity, id: &str, loss: i64) {
    let Some(mut worn) = world.get_mut::<crate::applied_effects::AppliedEffects>(creature) else {
        return;
    };
    let Some(effect) = worn.0.iter_mut().find(|effect| effect.id == id) else {
        return;
    };
    effect.duration = (effect.duration - loss).max(0);
}

/// A safe square beside one, which is where the bile lands.
///
/// Reference: `Character::adjacent_tile()` (`character.cpp:7566-7600`) walks the eight neighbours and skips the occupied, the impassable, and any square with a visible
/// dangerous trap or field - answering the character's own square when nothing qualifies. The port takes the **first neighbour no creature stands on and nothing solid blocks**,
/// which is the same rule with the trap and field checks declared rather than guessed: the port knows about traps now, but not which of them a character can see.
fn adjacent_tile(world: &World, at: (i32, i32, i32)) -> Option<(i32, i32, i32)> {
    for dy in -1..=1 {
        for dx in -1..=1 {
            if dx == 0 && dy == 0 {
                continue;
            }
            let spot = (at.0 + dx, at.1 + dy, at.2);
            if !crate::creature::creature_at(world, spot).is_empty() {
                continue;
            }
            if crate::routing::square_cost(world, spot).is_none_or(|cost| cost == 0) {
                continue;
            }
            return Some(spot);
        }
    }
    None
}
