//! A summoned creature's lease on existence.
//!
//! Reference: `Creature::set_summon_time` and `Creature::decrement_summon_timer` (`creature.cpp:2249-2274`), called at the top of `Creature::process_turn` (`:372-378`).
//!
//! The rule is a **deadline**, not a countdown: `set_summon_time( length )` stores `calendar::turn + length`, and every turn compares the stored deadline against the clock and
//! kills the creature when it has passed. A port that stored "turns left" and decremented it would behave the same only while every turn is one unit - and the reference's own
//! deadline arithmetic is what `spell` duration is expressed in.
//!
//! **A summon with no deadline never expires**, which is what the spell flag `PERMANENT` means: the reference simply never calls `set_summon_time`, so the creature has nothing to
//! check. The port says the same thing by not adding the component.

use bevy_ecs::prelude::*;

/// What turn the world is on, or zero when it has no calendar - the same "no clock, no deadline" answer a world without a clock gives everywhere else.
fn now(world: &World) -> i64 {
    crate::calendar::of(world).map_or(0, |calendar| calendar.turn)
}

/// The turn a summoned creature stops existing on.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Summoned {
    /// The absolute turn its lease runs out at, which is `lifespan_end` in the reference.
    pub ends_on: i64,
}

/// Gives a creature a deadline `turns` from now.
///
/// Reference: `set_summon_time( time_duration::from_turns( n ) )` - an **absolute** turn, so the deadline does not move when the creature is idle.
pub fn set_summon_time(world: &mut World, creature: Entity, turns: i64) {
    let now = now(world);
    world.entity_mut(creature).insert(Summoned {
        ends_on: now.saturating_add(turns),
    });
}

/// Kills every summon whose deadline has passed, answering how many went.
///
/// Reference: `decrement_summon_timer` is called once per creature at the top of its turn, and `die( &get_map(), nullptr )` is what a passed deadline means - a death with no
/// killer, which is why the port kills through [`crate::creature::hurt`] rather than through a killing blow.
pub fn expire(world: &mut World) -> usize {
    let now = now(world);
    let expired: Vec<Entity> = world
        .query::<(Entity, &Summoned)>()
        .iter(world)
        .filter(|(_, summons)| summons.ends_on <= now)
        .map(|(entity, _)| entity)
        .collect();
    let mut gone = 0;
    for creature in expired {
        let Some(health) = crate::creature::health_of(world, creature) else {
            world.entity_mut(creature).remove::<Summoned>();
            continue;
        };
        // Enough to take the body to zero whatever it had left: the reference does not damage the creature, it ends it.
        #[allow(clippy::cast_precision_loss)]
        crate::creature::hurt(world, creature, health.max(0) as f32 + 1.0);
        gone += 1;
    }
    gone
}
