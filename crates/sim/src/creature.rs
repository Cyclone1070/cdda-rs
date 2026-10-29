//! The living things in the world.
//!
//! Reference: `Creature` (`src/creature.h:47`) and its subclasses - `monster`, `npc`, `avatar` - are a
//! class hierarchy with virtual methods, and the simulation keeps them in parallel containers beside
//! the map. The port keeps none of that structure: a creature is an entity like a square or an item,
//! and what a creature *has* becomes components it carries.
//!
//! What it has, and nothing more than the reference's own rules read:
//!
//! * [`Position`] - the square it stands on, shared with everything else that has a place.
//! * [`Creature`] - the name it is known by and how big it is, which the reference reads for damage
//!   scaling and for what it can wear.
//! * [`Moves`] - the action points it has banked. The reference holds these on the creature
//!   (`Creature::moves`); here they are a component, so a system that spends them asks the store.

use crate::CreatureSize;
use crate::Position;
use bevy_ecs::prelude::*;

/// A living thing: what it is called, and how big it is.
///
/// Reference: `Creature::disp_name`, and `Creature::get_size`, which `monster` and `npc` each answer
/// from their own type. The size is a component's field rather than a subclass, so a dog that grows is
/// a change to one value, not a different entity.
#[derive(Component, Debug, Clone, PartialEq, Eq)]
pub struct Creature {
    /// The name a message about it uses.
    pub name: String,
    /// How big it is, which decides how much damage it takes and what fits it.
    pub size: CreatureSize,
}

/// Action points banked this turn.
///
/// Reference: `Creature::moves` (`src/creature.h`), which the reference adds to and spends; a creature
/// with enough moves takes a step, and one without waits. Kept as a component so that "who can act" is
/// a query rather than a loop over a list of creatures.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Moves {
    /// The points left to spend.
    pub left: i32,
}

/// How fast a creature moves, in action points per turn.
///
/// Reference: `Creature::get_speed` is `get_speed_base() + get_speed_bonus()` (`src/creature.cpp`),
/// and a monster's base comes from its type, scaled once at load time by the `MONSTER_SPEED` option.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Speed {
    /// The speed its type gives it.
    pub base: i32,
    /// What effects and enchantments add or take away.
    pub bonus: i32,
}

/// Speed as the reference scales it by the `MONSTER_SPEED` option.
///
/// Reference: `monstergenerator.cpp:346` - `mon.speed *= get_option<int>( "MONSTER_SPEED" ) / 100.0;`
/// on an `int`, so the product is truncated toward zero rather than rounded: a hundred and thirty at
/// seventy-five percent is ninety-seven, not ninety-eight.
#[must_use]
pub fn scaled_speed(speed: i32, monster_speed_percent: i32) -> i32 {
    let percent = f64::from(monster_speed_percent) / 100.0;
    #[allow(clippy::cast_possible_truncation)]
    let scaled = (f64::from(speed) * percent) as i32;
    scaled
}

/// Sets a creature's speed.
pub fn set_speed(world: &mut World, entity: Entity, base: i32, bonus: i32) {
    if let Some(mut speed) = world.get_mut::<Speed>(entity) {
        speed.base = base;
        speed.bonus = bonus;
    }
}

/// What a creature's speed is now.
#[must_use]
pub fn speed_of(world: &World, entity: Entity) -> i32 {
    world.get::<Speed>(entity).map_or(0, |speed| speed_now(*speed))
}

/// The speed a [`Speed`] adds up to: base plus bonus, in one place so the two callers cannot drift.
fn speed_now(speed: Speed) -> i32 {
    speed.base + speed.bonus
}

/// Gives every creature the action points its speed earns for one turn.
///
/// Reference: the turn loop adds each creature's speed to its banked moves and lets it spend them
/// (`game.cpp`, `monster::move`). Here it is one system-shaped function over the store: no list of
/// creatures is kept beside it, because the query is the list.
pub fn bank_a_turn(world: &mut World) {
    let earned: Vec<(Entity, i32)> = world
        .query::<(Entity, &Speed)>()
        .iter(world)
        .map(|(entity, speed)| (entity, speed_now(*speed)))
        .collect();
    for (entity, points) in earned {
        give_moves(world, entity, points);
    }
}

/// What entering a square costs, in moves.
///
/// Reference: `map::combined_movecost` (`src/map.cpp:2466-2488`). The two squares are charged for
/// together, the caller's modifier is added, and the sum is scaled by `{ 0, 50, 71, 100 }` - one entry
/// per number of differing axes - and halved. Integer arithmetic throughout, so a diagonal floor step
/// is 142 moves where a straight one is 100: 71 is the reference's stand-in for a hundred over the
/// square root of two, which is why `Character::run_cost` can then apply its own effects and cancel the
/// root exactly.
#[must_use]
pub fn combined_move_cost(from: i32, to: i32, modifier: i32, differing_axes: usize) -> i32 {
    const MULTIPLIERS: [i32; 4] = [0, 50, 71, 100];
    let multiplier = MULTIPLIERS
        .get(differing_axes.min(3))
        .copied()
        .unwrap_or_default();
    (from + to + modifier) * multiplier / 2
}

/// Whether a creature moved.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepOutcome {
    /// It took the step and paid for it.
    Moved,
    /// It had fewer points than the step costs, so it stayed where it was and paid nothing.
    TooSlow,
}

/// What a step costs, after the effects a diagonal applies.
///
/// Reference: `Character::run_cost` (`src/character.cpp:10363-10374`). The cost is divided by the
/// square root of two when the step is diagonal, effect modifiers are applied between the two
/// multiplications, and the result is truncated to an integer. The port has no run-cost effects yet, so
/// the divide and multiply cancel and the answer is the square's own cost - which is the behaviour,
/// not a shortcut: the reference's own comment says the division exists so effect arithmetic can assume
/// a hundred-point base.
#[must_use]
pub fn run_cost(base_cost: i32, diagonal: bool) -> i32 {
    let mut move_cost = f64::from(base_cost);
    if diagonal {
        move_cost /= std::f64::consts::SQRT_2;
    }
    // Where `run_cost_effects` will go: additive and multiplicative modifiers, in the reference's order.
    if diagonal {
        move_cost *= std::f64::consts::SQRT_2;
    }
    #[allow(clippy::cast_possible_truncation)]
    let cost = move_cost as i32;
    cost
}

/// Moves a creature one square, spending what the step costs.
///
/// The cost is the caller's, because what a square costs to enter is the map's answer
/// (`map::move_cost`), and the creature only decides whether it can pay. On success the entity's
/// [`Position`] and the index are both updated: the index is derived, and leaving it behind would make
/// it a second source of truth.
pub fn try_step(world: &mut World, entity: Entity, to: (i32, i32, i32), cost: i32) -> StepOutcome {
    let Some(moves) = world.get::<Moves>(entity).copied() else {
        return StepOutcome::TooSlow;
    };
    if moves.left < cost {
        return StepOutcome::TooSlow;
    }
    let from = {
        let Some(mut position) = world.get_mut::<Position>(entity) else {
            return StepOutcome::TooSlow;
        };
        let from = (position.x, position.y, position.z);
        position.x = to.0;
        position.y = to.1;
        position.z = to.2;
        from
    };
    if let Some(mut moves) = world.get_mut::<Moves>(entity) {
        moves.left -= cost;
    }
    let mut index = crate::singletons::index_mut(world);
    index.remove(entity, (from.0, from.1, from.2));
    index.insert(entity, to);
    StepOutcome::Moved
}

/// How much life a creature has left.
///
/// Reference: `Creature::hp` (`src/creature.h`), and `monster::is_dead_state` is simply `hp <= 0`
/// (`monster.cpp:2065`). Kept as a component so "who is dead" is a query and death needs no list.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Health {
    /// What it has left.
    pub hp: i32,
    /// What it started with, which the reference keeps for the health bar.
    pub max_hp: i32,
}

/// Sets a creature's health, and its ceiling.
pub fn set_health(world: &mut World, entity: Entity, hp: i32) {
    if let Some(mut health) = world.get_mut::<Health>(entity) {
        health.hp = hp;
        health.max_hp = hp;
    } else {
        world
            .entity_mut(entity)
            .insert(Health { hp, max_hp: hp });
    }
}

/// Takes damage off a creature, never below zero.
///
/// The reference's own arithmetic is `hp -= damage` on an int with the damage rounded down
/// (`Creature::deal_damage`), and `is_dead_state` reads `hp <= 0`.
pub fn hurt(world: &mut World, entity: Entity, damage: f32) {
    #[allow(clippy::cast_possible_truncation)]
    let whole = damage as i32;
    if let Some(mut health) = world.get_mut::<Health>(entity) {
        health.hp -= whole;
    }
}

/// What a creature has left, or nothing when it has no [`Health`] at all.
#[must_use]
pub fn health_of(world: &World, entity: Entity) -> Option<i32> {
    world.get::<Health>(entity).map(|health| health.hp)
}

/// Where a creature is trying to get to.
///
/// Reference: a monster's `goal` (`src/monster.h`), which `monster::move` walks towards and drops when it
/// arrives. Kept as its own component so a creature with no goal has nothing to do and needs no flag.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Goal {
    /// The square it is heading for.
    pub square: (i32, i32, i32),
}

/// Sets the square a creature is heading for.
pub fn set_goal(world: &mut World, entity: Entity, square: (i32, i32, i32)) {
    if let Some(mut goal) = world.get_mut::<Goal>(entity) {
        goal.square = square;
    } else {
        world.entity_mut(entity).insert(Goal { square });
    }
}

/// Gives every creature its speed and lets each one with a goal take one step.
pub fn take_turns(world: &mut World) {
    bank_a_turn(world);
    let walkers: Vec<(Entity, (i32, i32, i32))> = world
        .query::<(Entity, &Goal)>()
        .iter(world)
        .map(|(entity, goal)| (entity, goal.square))
        .collect();
    for (entity, goal) in walkers {
        let bounds = walk_bounds(world);
        let outcome = crate::routing::walk_towards(world, entity, goal, bounds);
        if matches!(outcome, crate::routing::Walk::Arrived | crate::routing::Walk::NoRoute) {
            world.entity_mut(entity).remove::<Goal>();
        }
    }
}

/// The rectangle a walk is allowed to search: far enough for any level the store holds.
fn walk_bounds(world: &World) -> (i32, i32) {
    let mut widest = (0, 0);
    for square in crate::square::squares(world) {
        if let Some(position) = world.get::<Position>(square) {
            widest = (widest.0.max(position.x + 1), widest.1.max(position.y + 1));
        }
    }
    widest
}

/// Puts a creature in the world, standing on `square`.
///
/// The square is written twice - as the entity's [`Position`] and into the index - because the index is
/// derived: [`crate::PositionIndex::rebuild`] would produce the same thing from the store, and the
/// architecture test checks exactly that.
pub fn spawn_creature(world: &mut World, name: &str, square: (i32, i32, i32)) -> Entity {
    let entity = world
        .spawn((
            Position {
                x: square.0,
                y: square.1,
                z: square.2,
            },
            Creature {
                name: name.to_owned(),
                size: CreatureSize::Medium,
            },
            Moves::default(),
            Speed::default(),
            Health {
                hp: 0,
                max_hp: 0,
            },
        ))
        .id();
    crate::singletons::index_mut(world).insert(entity, square);
    entity
}

/// Every creature standing on `square`, in the order the world holds them.
///
/// A square can hold more than one: two dogs in a doorway is an ordinary state, and the reference's
/// `creature_at` returns a list for that reason.
#[must_use]
pub fn creature_at(world: &World, square: (i32, i32, i32)) -> Vec<Entity> {
    let Some(index) = crate::singletons::index(world) else {
        return Vec::new();
    };
    index
        .at_level(square.0, square.1, square.2)
        .iter()
        .copied()
        .filter(|entity| world.get::<Creature>(*entity).is_some())
        .collect()
}

/// How many action points a creature has left.
#[must_use]
pub fn moves_left(world: &World, entity: Entity) -> i32 {
    world.get::<Moves>(entity).map_or(0, |moves| moves.left)
}

/// Adds action points to a creature.
pub fn give_moves(world: &mut World, entity: Entity, points: i32) {
    if let Some(mut moves) = world.get_mut::<Moves>(entity) {
        moves.left += points;
    }
}
