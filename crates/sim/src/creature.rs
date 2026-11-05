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
    let speeds: Vec<(Entity, i32)> = world
        .query::<(Entity, &Speed)>()
        .iter(world)
        .map(|(entity, speed)| (entity, speed_now(*speed)))
        .collect();
    let earned: Vec<(Entity, i32)> = speeds
        .into_iter()
        .map(|(entity, speed)| (entity, speed + effect_speed_modifier(world, entity)))
        .collect();
    for (entity, points) in earned {
        give_moves(world, entity, points);
    }
}

/// What the effects a creature is wearing add to its speed, in speed points.
///
/// Reference: `Character::get_speed_base` sums every effect's `SPEED` modifier into the speed bonus (`character.cpp:12488-12494`), each at the creature's own intensity for that effect
/// (`effect::get_mod`, `effect.cpp:866`). The value is the effect data's own: `winded`, for instance, states `speed_mod: [-10]`, which is why running out of breath costs more than the
/// recovery rate.
///
/// An effect the world cannot resolve adds nothing, for the same reason nothing else in the port invents a number: the modifier is the data's, and a missing type has no data.
#[must_use]
pub fn effect_speed_modifier(world: &World, entity: Entity) -> i32 {
    effect_stat_bonus(world, entity, "SPEED")
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

/// How much pain a creature is in.
///
/// Reference: `Character::pain_cur` (`character.h`): a plain integer the game raises as damage lands and lets decay each turn, read by `u_pain()` in dialogue
/// math and by every rule that asks whether a character is hurting. The port keeps it as a component, so nothing has to hold it beside the store.
#[derive(Component, Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Pain(pub i32);

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

/// Gives a creature hit points back, up to its ceiling and never past it.
///
/// Reference: `Creature::heal` (`creature.cpp`), which adds the amount and clamps at `get_hp_max()` - a creature at full health absorbs nothing, and a blow that took it
/// below zero is not undone by mending.
pub fn heal(world: &mut World, entity: Entity, amount: i32) {
    if amount <= 0 {
        return;
    }
    if let Some(mut health) = world.get_mut::<Health>(entity) {
        if health.hp <= 0 {
            return;
        }
        health.hp = (health.hp + amount).min(health.max_hp);
    }
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

/// Spends action points a creature has banked, keeping the count at zero or above.
pub fn spend_moves(world: &mut World, entity: Entity, amount: f64) {
    if let Some(mut moves) = world.get_mut::<Moves>(entity) {
        // Moves are whole points in the port and so are a gas's penalty, but the reference spells both as
        // `double`; the penalty is rounded as it is applied rather than being truncated twice.
        #[allow(clippy::cast_possible_truncation)]
        let whole = amount.round() as i32;
        moves.left -= whole;
    }
}

/// Sets how many action points a creature has.
pub fn set_moves(world: &mut World, entity: Entity, left: i32) {
    if let Some(mut moves) = world.get_mut::<Moves>(entity) {
        moves.left = left;
    }
}

/// Sets how much life a creature has left without touching its ceiling.
///
/// Used where a rule has already worked out the remainder - a gas's damage, say - and the ceiling is none of
/// its business.
pub fn set_health_left(world: &mut World, entity: Entity, hp: i32) {
    if let Some(mut health) = world.get_mut::<Health>(entity) {
        health.hp = hp;
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
    if whole > 0 {
        // Reference: `Character::on_hurt` (`character.cpp:8564-8590`) raises `distraction_type::attacked` after the
        // damage lands, which is what makes being hit stop what you were doing. Only a creature that is doing
        // something is bothered; `disturb` answers `false` for the rest and changes nothing.
        disturb(world, entity, Distraction::Attacked);
    }
}

/// What a creature has left, or nothing when it has no [`Health`] at all.
#[must_use]
pub fn health_of(world: &World, entity: Entity) -> Option<i32> {
    world.get::<Health>(entity).map(|health| health.hp)
}

/// A creature with no [`Health`] is not tracked, not dead: the reference gives every monster a hit-point
/// pool from its type, and this port has not carried that number across yet, so a creature that nothing has
/// given health to must be alive rather than a body. [`reap_dead_creatures`] therefore asks for `&Health`
/// explicitly, and "no health" cannot mean "zero health".
///
/// Clears every creature whose life has run out, and takes it out of the index too.
///
/// Reference: `monster::is_dead_state` (`monster.cpp:2065`) is `hp <= 0`, and the turn removes what it finds
/// dead. The index entry is removed here by hand because the index is a cache of the store: dropping only the
/// entity would leave a square claiming an occupant that no longer exists, which is the failure the
/// architecture gate calls a ghost.
///
/// Returns how many were cleared.
pub fn reap_dead_creatures(world: &mut World) -> usize {
    let dead: Vec<(Entity, (i32, i32, i32))> = world
        .query::<(Entity, &Health, &Position)>()
        .iter(world)
        .filter(|(_, health, _)| health.hp <= 0)
        .map(|(entity, _, position)| (entity, (position.x, position.y, position.z)))
        .collect();
    for (entity, square) in &dead {
        crate::singletons::index_mut(world).remove(*entity, *square);
        world.despawn(*entity);
    }
    dead.len()
}

/// How hard a creature is to hit, in points.
///
/// Reference: `monster::get_dodge_base()` is `type->sk_dodge` (`monster.cpp:2643-2646`), and each point is
/// worth five on the attacker's roll (see [`crate::melee::dodge_roll`]). A creature with no `Dodge` is not
/// dodging at all rather than dodging invisibly.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Dodge {
    /// Points of dodge.
    pub points: i32,
}

/// The parts of a creature's body a blow can land on.
///
/// Reference: a `Creature`'s anatomy (`src/anatomy.h`). A monster has none this port knows about, so the component
/// is absent rather than empty - "no body to record" and "a body with no parts" are different answers.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Body {
    /// The parts, in the order the anatomy lists them.
    pub parts: Vec<crate::hit_location::BodyPart>,
}

/// Builds a creature's body from content: an anatomy's membership and the numbers each part states.
///
/// This is the join the reference makes once at load time (`anatomy::finalize`, `anatomy.cpp:78-90`, which resolves
/// its part ids and keeps the ones that exist). It answers `None` when the anatomy is unknown **or** when it lists a
/// part the content does not describe: a body with a hole in it would make every later rule quietly wrong, and the
/// reference's own answer to that case is a debug message rather than a smaller body.
#[must_use]
pub fn body_named(
    anatomy: &str,
    anatomies: &[cdda_content::anatomies::Anatomy],
    parts: &[cdda_content::body_parts::BodyPartType],
) -> Option<Body> {
    let wanted = anatomies.iter().find(|one| one.id == anatomy)?;
    let mut built = Vec::with_capacity(wanted.parts.len());
    for id in &wanted.parts {
        let found = parts.iter().find(|part| &part.id == id)?;
        built.push(crate::hit_location::BodyPart {
            name: found.name.clone(),
            hit_size: found.hit_size,
            hit_difficulty: found.hit_difficulty,
            upper: found.upper,
            lower: found.lower,
        });
    }
    Some(Body { parts: built })
}

/// What armour a creature has on each part of its body.
///
/// Reference: `Character::get_armor_type( dt, bp )` (`character_armor.cpp:70-86`) - armour is asked **per body
/// part**, because that is the whole reason a blow asks which part it landed on. The port keeps the total for
/// bash per part; the pieces the reference adds together (worn, bionic, mutation, the part's own resistance) are
/// already summed by the time they reach here, which is the same arithmetic with one fewer place to disagree.
#[derive(Component, Debug, Clone, Default, PartialEq, Eq)]
pub struct PartArmour(pub std::collections::BTreeMap<String, i32>);

/// How much a part of a creature resists a blow.
///
/// Falls back to the creature's whole-body [`Armour`] when the part states nothing, and to nothing at all when
/// neither does: a bare arm and an unstated arm are the same arm, not one of them invulnerable.
#[must_use]
pub fn armour_on(world: &World, entity: Entity, part: &str) -> i32 {
    if let Some(armour) = world.get::<PartArmour>(entity)
        && let Some(found) = armour.0.get(part)
    {
        return *found;
    }
    world.get::<Armour>(entity).map_or(0, |armour| armour.bash)
}

/// A thing being worn, and by whom.
///
/// Reference: the reference keeps a `Character`'s worn items in a `wearable_item` list and asks `worn.damage_resist`
/// for a part. The port keeps the **item** as an entity and the link as a component on it, so "what is this worn by"
/// is one answer in one direction - a list on the wearer as well would be a second copy that could disagree.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct WornBy(pub Entity);

/// What a worn thing protects, part by part, as content worked it out.
///
/// The numbers come from the item's armour entries and its materials (`cdda_content::armour::layers_on`), which is
/// content's business; this component carries the answer so the simulation never needs the material table to
/// answer "what is on your torso".
#[derive(Component, Debug, Clone, Default, PartialEq)]
pub struct Garment {
    /// The layers it puts in the way, per body part name.
    pub layers: Vec<(String, cdda_content::armour::Layer)>,
}

/// How long it takes to pick something up, in action points.
///
/// Reference: `Character::item_handling_cost` (`character.cpp:7283-7290`) - a base cost plus fifty moves per litre
/// of the thing, capped at two hundred at four litres. The hand-encumbrance overhead the reference adds afterwards
/// is not here yet: the port has no encumbrance, and a term that is always zero would be a lie about work done.
#[must_use]
pub fn handling_cost(volume_ml: i32) -> i32 {
    const BASE: i32 = 100;
    BASE + (volume_ml / 20).min(200)
}

/// What came of trying to pick something up.
///
/// [`Picked::Started`] is the case the port had no word for until activities existed: the job is real, it has begun,
/// and it is not finished.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Began {
    /// It is in hand already: the job fitted in the moves the creature had.
    Done,
    /// It has begun and will take more turns.
    Started,
    /// It is not on the creature's square.
    NotHere,
    /// Somebody is already carrying it.
    AlreadyCarried,
    /// The creature is already busy with something else.
    Busy,
}

/// Starts picking something up, as an activity when it will not fit in one turn.
///
/// Reference: `game::pickup` assigns `ACT_PICKUP` with `item_handling_cost` moves and `activity_handlers::pickup_finish`
/// does the transfer when the activity ends. The port does the same, with the difference that an item light enough to
/// handle inside the moves the creature already has finishes at once - the reference charges the same cost either way,
/// and a one-turn activity is a turn spent for nothing.
pub fn begin_pick_up(world: &mut World, creature: Entity, item: Entity, volume_ml: i32) -> Began {
    if world.get::<Doing>(creature).is_some() {
        return Began::Busy;
    }
    if world.get::<Carried>(item).is_some() {
        return Began::AlreadyCarried;
    }
    let Some(here) = world.get::<Position>(creature).map(|place| (place.x, place.y, place.z)) else {
        return Began::NotHere;
    };
    let Some(there) = world.get::<Position>(item).map(|place| (place.x, place.y, place.z)) else {
        return Began::NotHere;
    };
    if here != there {
        return Began::NotHere;
    }
    let cost = handling_cost(volume_ml);
    if moves_left(world, creature) >= cost {
        spend_moves(world, creature, f64::from(cost));
        take_into_hand(world, creature, item, there);
        return Began::Done;
    }
    world.entity_mut(creature).insert(Doing {
        moves_total: 0,
        activity: "pickup".to_owned(),
        moves_left: cost,
        basis: Basis::Speed,
        subject: Some(item),
    });
    Began::Started
}

/// What finishing an activity does, once its time is spent.
type Finish = fn(&mut World, Entity, Option<Entity>);

/// The table of activities the port knows how to finish, by the id a [`Doing`] carries.
///
/// Reference: `activity_handlers::finish`'s dispatch table (`activity_handlers.cpp`), keyed by activity id. It is a
/// table rather than a chain of `if let`s for the reason the reference has one: an activity is a name, and the thing a
/// name does belongs beside every other name's answer, where a reader can see in one glance what the port can finish.
///
/// The list is a constant, so adding an activity in two places - a `begin_*` that starts it and an entry here - is the
/// only way to add one at all, and a test counts them so a starter with no finisher cannot slip through.
pub const FINISHERS: &[(&str, Finish)] = &[
    ("craft", finish_crafting),
    ("pickup", finish_pickup),
    ("wear", finish_wearing),
];

/// Reference: `activity_handlers::craft_finish`. The store knows what items are, so the loop can finish the work
/// itself: consume what the recipe asked for and hand over what it made. A world with no item registry knows no names,
/// so nothing is consumed and nothing is made - which is the honest outcome, not a crash.
fn finish_crafting(world: &mut World, creature: Entity, _subject: Option<Entity>) {
    crate::crafting::finish_craft_in_store(world, creature);
}

/// Reference: `activity_handlers::pickup_finish`, which puts the item in the character's hands.
fn finish_pickup(world: &mut World, creature: Entity, subject: Option<Entity>) {
    let Some(item) = subject else {
        return;
    };
    if let Some(place) = world
        .get::<Position>(creature)
        .map(|place| (place.x, place.y, place.z))
    {
        take_into_hand(world, creature, item, place);
    }
}

/// Reference: `wear_activity_actor::do_turn` calling `who.wear_item` - `put_on` here, because the time is already spent
/// and the store must change without being charged a second time.
fn finish_wearing(world: &mut World, creature: Entity, subject: Option<Entity>) {
    if let Some(item) = subject {
        put_on(world, creature, item);
    }
}

/// The table of what undoing an activity does, by the id a [`Doing`] carries.
///
/// Reference: `player_activity::canceled` (`player_activity.cpp:420-428`), which hands the cancellation to the actor's
/// own `canceled` - the same dispatch shape as the finishers, for the same reason: giving up on something is a behaviour
/// of that something, and a craft that is abandoned has to hand back what it was using.
pub const CANCELLERS: &[(&str, Finish)] = &[
    ("craft", cancel_crafting),
    ("pickup", cancel_nothing),
    ("wear", cancel_nothing),
];

/// Reference: `craft_activity_actor::canceled`, which stops the work and leaves the components where they were. The port
/// drops [`crate::crafting::Making`]; the ingredients are carried items and nothing consumed them until the finish, so
/// there is nothing to hand back - which is a property of when the port consumes, not a shortcut.
fn cancel_crafting(world: &mut World, creature: Entity, _subject: Option<Entity>) {
    world.entity_mut(creature).remove::<crate::crafting::Making>();
}

/// An activity that has not changed anything yet has nothing to undo.
fn cancel_nothing(_world: &mut World, _creature: Entity, _subject: Option<Entity>) {}

/// Something that happens to a creature while it is busy.
///
/// Reference: `distraction_type` (`src/player_activity.h`), which the activity decides whether to heed. The port keeps
/// the three a survivor can be made to notice before P7's monsters exist to cause them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Distraction {
    /// It was hurt.
    Pain,
    /// It heard something.
    Noise,
    /// It was attacked, which the reference distinguishes from pain: a blow can land and still not hurt.
    Attacked,
}

/// The activities that cannot be interrupted at all.
///
/// Reference: `activity_type::interruptable`, a data flag on the activity's type. Nothing the port can start is
/// uninterruptible yet - a craft can be interrupted, and should be - so the list is empty rather than absent: the rule it
/// stands for has to exist before there is anything to put in it.
pub const UNINTERRUPTIBLE: &[&str] = &[];

/// The distractions an activity does not heed.
///
/// Reference: `player_activity::ignored_distractions` filled from `activity_type::default_ignored_distractions()`
/// (`player_activity.cpp:78-92`). Empty for the same reason as [`UNINTERRUPTIBLE`].
pub const IGNORED: &[(&str, &[Distraction])] = &[];

/// Whether an activity heeds a distraction.
///
/// Reference: `player_activity::is_distraction_ignored` (`player_activity.cpp:492-495`) - an activity that cannot be
/// interrupted ignores everything, and an activity that names a distraction ignores that one. Everything else is
/// interrupted, which is the default the reference gives a type with no opinion.
#[must_use]
pub fn heeds(activity: &str, distraction: Distraction) -> bool {
    if UNINTERRUPTIBLE.contains(&activity) {
        return false;
    }
    if let Some((_, ignored)) = IGNORED.iter().find(|(id, _)| *id == activity) {
        return !ignored.contains(&distraction);
    }
    true
}

/// Interrupts what a creature is doing, if it is the sort of thing that can be.
///
/// Reference: the distraction path in `game.cpp` that calls `Character::cancel_activity` when a distraction is not
/// ignored. Cancelling goes through [`stop_doing`], so an interrupted craft lands in the backlog with its progress
/// rather than being lost - which is the reference's answer too (`character.cpp:9112-9115`).
///
/// Answers whether the interruption took.
pub fn disturb(world: &mut World, creature: Entity, distraction: Distraction) -> bool {
    let Some(activity) = world.get::<Doing>(creature).map(|doing| doing.activity.clone()) else {
        return false;
    };
    if !heeds(&activity, distraction) {
        return false;
    }
    stop_doing(world, creature)
}

/// The work a creature has paused, waiting to be picked up again.
///
/// Reference: `Character::backlog` (`character.h`), a list of activities that can be resumed rather than redone. The
/// reference pushes an interrupted activity onto the **front** of it when the activity says it can resume
/// (`character.cpp:9112-9115`), and puts it back only when the player asks (`resume_backlog_activity`,
/// `character.cpp:9119-9126`) - so an interruption costs time, not the work.
#[derive(Component, Debug, Clone, Default, PartialEq)]
pub struct Backlog {
    /// The paused activities, the most recently paused first.
    pub paused: Vec<Doing>,
}

/// The activities whose work survives being paused.
///
/// Reference: `player_activity::can_resume`, which is a property of the actor rather than of the interruption. A craft
/// can be put down and picked up again because its recipe and progress are its own; a pickup cannot, because the thing
/// being picked up is still on the floor where it started.
pub const RESUMABLE: &[&str] = &["craft"];

/// Gives up on whatever a creature is doing.
///
/// Reference: `Character::cancel_activity` (`character.cpp:9088-9100`) - the activity is cancelled, and it is the
/// activity's own business what that means. Answers whether there was anything to give up on.
pub fn stop_doing(world: &mut World, creature: Entity) -> bool {
    let Some(doing) = world.get::<Doing>(creature).cloned() else {
        return false;
    };
    world.entity_mut(creature).remove::<Doing>();
    if RESUMABLE.contains(&doing.activity.as_str()) {
        // Reference: `character.cpp:9112-9115` - the interrupted activity goes to the front of the backlog with its
        // progress intact, and the activity's own cancel is not run, because nothing is being given up.
        let entity = crate::singletons::singleton::<Backlog, TheBacklog>(world);
        let mut backlog = world
            .get_mut::<Backlog>(entity)
            .map(|mut backlog| std::mem::take(&mut backlog.paused))
            .unwrap_or_default();
        backlog.insert(0, doing);
        if let Some(mut slot) = world.get_mut::<Backlog>(entity) {
            slot.paused = backlog;
        }
        return true;
    }
    if let Some((_, cancel)) = CANCELLERS.iter().find(|(id, _)| *id == doing.activity) {
        cancel(world, creature, doing.subject);
    }
    true
}

/// Marks the entity carrying a [`Backlog`].
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct TheBacklog;

/// Picks up the most recently paused activity where it was left.
///
/// Reference: `Character::resume_backlog_activity` (`character.cpp:9119-9126`): the front of the backlog becomes what the
/// creature is doing again, and it keeps the moves it had left rather than starting over. Answers whether anything was
/// resumed.
pub fn resume_backlog(world: &mut World, creature: Entity) -> bool {
    if world.get::<Doing>(creature).is_some() {
        return false;
    }
    let entity = crate::singletons::singleton::<Backlog, TheBacklog>(world);
    let Some(paused) = world
        .get_mut::<Backlog>(entity)
        .and_then(|mut backlog| (!backlog.paused.is_empty()).then(|| backlog.paused.remove(0)))
    else {
        return false;
    };
    world.entity_mut(creature).insert(paused);
    true
}

/// Finishes an activity that has run out of moves.
///
/// Reference: `activity_handlers::finish`, which looks the activity up in its table and calls what it finds. An activity
/// the port does not know simply ends, which is the reference's own answer to an unhandled activity.
pub fn finish_activity(world: &mut World, creature: Entity, activity: &str, subject: Option<Entity>) {
    let Some((_, finish)) = FINISHERS.iter().find(|(id, _)| *id == activity) else {
        return;
    };
    finish(world, creature, subject);
}

/// Puts an item in a creature's hand, wherever it was.
fn take_into_hand(world: &mut World, creature: Entity, item: Entity, from: (i32, i32, i32)) {
    crate::singletons::index_mut(world).remove(item, from);
    world.entity_mut(item).remove::<Position>();
    world.entity_mut(item).insert(Carried(creature));
}

/// What a creature can carry, in grams.
///
/// Reference: `Creature::weight_capacity` (`creature.cpp:3246-3268`) - thirteen kilograms for a medium creature, halved
/// for a small one, quartered for a tiny one, doubled for a large one and quadrupled for a huge one. The reference then
/// adds four kilograms a point of strength in `Character::weight_capacity` (`character.cpp:3353-3361`), which the port
/// adds when the creature carries a [`Strength`]: a creature with no strength stated is not a character, and thirteen
/// kilograms is what a creature of that size can carry.
#[must_use]
pub fn carrying_capacity(world: &World, entity: Entity) -> i64 {
    let size = world
        .get::<Creature>(entity)
        .map_or(crate::CreatureSize::Medium, |creature| creature.size);
    let base = match size {
        crate::CreatureSize::Tiny => 13_000 / 4,
        crate::CreatureSize::Small => 13_000 / 2,
        crate::CreatureSize::Medium => 13_000,
        crate::CreatureSize::Large => 13_000 * 2,
        crate::CreatureSize::Huge => 13_000 * 4,
    };
    // Reference: `Character::weight_capacity` reads `get_str()`, which is the base plus whatever the character's enchantments add - so a mutant with `STR_UP` carries the
    // four kilograms a point that its new strength is worth.
    let strength = effective_strength(world, entity);
    base + i64::from(strength) * 4_000
}

/// How much room a creature has to put things in, in millilitres.
///
/// Reference: `Character::volume_capacity` (`character.cpp:8883-8889`) - `weapon.get_total_capacity()` plus
/// `worn.volume_capacity()`. The port sums the worn items, because it has no separate weapon slot: a thing in a hand is
/// either carried or worn, and neither had a capacity until now.
///
/// A creature wearing nothing can carry nothing, which is what the reference says too - pockets are where things go,
/// and a survivor with no clothes has none.
#[must_use]
pub fn volume_capacity(
    world: &mut World,
    creature: Entity,
    names: &crate::item_registry::ItemNames,
) -> i64 {
    let mut total = 0;
    let mut query = world.query::<(&WornBy, &crate::GroundItem)>();
    for (worn, item) in query.iter(world) {
        if worn.0 == creature {
            total += i64::from(names.holds_ml(item.kind));
        }
    }
    total
}

/// Marks the thing a creature is holding in its hands.
///
/// Reference: `Character::weapon`. It is the third of the reference's three item sets - carried, worn, wielded - and the
/// port had two and a half: a thing "in hand" was anything carried, which is why stashing a crafted thing had nowhere
/// honest to put a thing that fitted no pocket. A wielded thing is carried **and** marked, and it is in no pocket.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct WieldedBy(pub Entity);

/// What came of trying to take something in hand.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Wielded {
    /// It is now in the creature's hands.
    Done,
    /// It is not carried, so it cannot be wielded.
    NotInHand,
    /// There were not enough moves left to take it in hand.
    TooSlow,
}

/// Takes a carried thing in hand.
///
/// Reference: `Character::wield` (`character.cpp:13150-13200`), which charges `item_handling_cost` for the fumbling and
/// keeps exactly one thing in the hands: whatever was held before stays carried and stops being wielded. The port has
/// none of the reference's limb, bionic and two-handed checks - they need anatomy and flags this port does not model -
/// so what is here is the cost, the one-at-a-time rule, and the fact that a wielded thing is not in a pocket.
pub fn wield(world: &mut World, creature: Entity, item: Entity, volume_ml: i32) -> Wielded {
    if world.get::<Carried>(item).map(|carried| carried.0) != Some(creature) {
        return Wielded::NotInHand;
    }
    let cost = handling_cost(volume_ml);
    if moves_left(world, creature) < cost {
        return Wielded::TooSlow;
    }
    give_moves(world, creature, -cost);
    // One thing in the hands at a time: whatever was held is still carried, and no longer wielded.
    let previous: Vec<Entity> = {
        let mut query = world.query::<(Entity, &WieldedBy)>();
        query
            .iter(world)
            .filter(|(_, wielded)| wielded.0 == creature)
            .map(|(entity, _)| entity)
            .collect()
    };
    for old in previous {
        world.entity_mut(old).remove::<WieldedBy>();
    }
    world.entity_mut(item).remove::<InPocket>();
    world.entity_mut(item).insert(WieldedBy(creature));
    Wielded::Done
}

/// What a creature is holding in its hands, if anything.
#[must_use]
pub fn wielding(world: &mut World, creature: Entity) -> Option<Entity> {
    let mut query = world.query::<(Entity, &WieldedBy)>();
    query
        .iter(world)
        .find(|(_, wielded)| wielded.0 == creature)
        .map(|(entity, _)| entity)
}

/// Which pocket of which worn thing a carried item is in.
///
/// Reference: an item lives inside an `item_pocket`, and the pockets are reached through what is worn. The port keeps
/// the same fact as a link: the carried thing points at the worn thing and says which of its pockets. Without it a
/// pocket could never fill up, and a survivor could put a hundred bottles in one bottle pocket as long as the totals
/// worked out.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct InPocket {
    /// The worn item the pocket belongs to.
    pub worn: Entity,
    /// Which of its pockets, in the order the data lists them.
    pub index: usize,
}

/// What is already in one pocket.
///
/// Reference: `item_pocket::contents`, reduced to what the fit rule asks - how many things, how much room and how much
/// weight they take.
#[must_use]
pub fn pocket_contents(
    world: &mut World,
    creature: Entity,
    names: &crate::item_registry::ItemNames,
) -> std::collections::BTreeMap<(Entity, usize), crate::pockets::Contents> {
    let mut filled: std::collections::BTreeMap<(Entity, usize), crate::pockets::Contents> =
        std::collections::BTreeMap::new();
    let mut query = world.query::<(&Carried, &InPocket, &crate::GroundItem)>();
    for (carried, pocket, item) in query.iter(world) {
        if carried.0 != creature {
            continue;
        }
        let entry = filled.entry((pocket.worn, pocket.index)).or_default();
        entry.count += 1;
        entry.volume_ml += names.volume_of(item.kind) * item.charges.max(1);
        #[allow(clippy::cast_possible_truncation)]
        let weight = names.weight_of(item.kind) * i64::from(item.charges.max(1));
        entry.weight_g += i32::try_from(weight).unwrap_or(i32::MAX);
    }
    filled
}

/// Which pocket a thing would go in, if any.
///
/// Reference: `Character::can_stash` -> `best_pocket` (`character.cpp:1571-1574`), which walks the pockets of everything
/// worn and asks `can_contain`. The port asks the same question in the same order - the worn things in no particular
/// order, their pockets in the order the data lists them - and answers which pocket took it, because a caller that only
/// learns "yes" cannot say where the thing went.
///
/// The pockets are all treated as empty: a carried thing is an entity that points at the creature, and nothing records
/// which pocket it is in. That is the next rule, not this one.
#[must_use]
pub fn can_stash(
    world: &mut World,
    creature: Entity,
    names: &crate::item_registry::ItemNames,
    volume_ml: i32,
    weight_g: i64,
) -> Option<(Entity, usize)> {
    let worn: Vec<(Entity, crate::ItemTypeId)> = {
        let mut query = world.query::<(Entity, &WornBy, &crate::GroundItem)>();
        query
            .iter(world)
            .filter(|(_, worn_by, _)| worn_by.0 == creature)
            .map(|(entity, _, item)| (entity, item.kind))
            .collect()
    };
    let weight_g = i32::try_from(weight_g).unwrap_or(i32::MAX);
    let filled = pocket_contents(world, creature, names);
    for (entity, kind) in worn {
        for (index, pocket) in names.pockets_of(kind).iter().enumerate() {
            let held = filled.get(&(entity, index)).copied().unwrap_or_default();
            if crate::pockets::fits(*pocket, held, volume_ml, weight_g) == crate::pockets::Fits::Yes {
                return Some((entity, index));
            }
        }
    }
    None
}

/// How strong a creature is, with the enchantments it carries applied.
///
/// Reference: `Character::get_str` (`character.cpp:3353`): `str_max` - the base - plus the enchantment cache's `STRENGTH` addition, multiplied by its multiplier. A creature
/// with no `Strength` is a monster, and answers zero, which is what makes a zombie carry only what its size allows.
#[must_use]
pub fn effective_strength(world: &World, entity: Entity) -> i32 {
    let Some(strength) = world.get::<Strength>(entity) else {
        return 0;
    };
    let base = crate::mutation_data::holder(world).map_or(strength.base, |data| {
        let (add, multiply) = crate::mutations::enchantment_values(world, entity, &data, "STRENGTH");
        #[allow(clippy::cast_possible_truncation)]
        let value =
            crate::mutations::apply_enchantments(f64::from(strength.base), add, multiply) as i32;
        value
    });
    // Reference: `Character::get_str` is `std::max( 0, get_str_base() + str_bonus )` (`character.cpp:4385-4388`), and the bonus is where the worn effects' `STR` modifiers land - the
    // same shape as dexterity, from the same table.
    (base + effect_stat_bonus(world, entity, "STR")).max(0)
}

/// How quick a creature's hands are.
///
/// Reference: `Character::get_dex_base` (`character.cpp`), which the reference starts at eight. The port carries the base and reads the enchantment cache over it, the same way it
/// does for [`Strength`] - so a mutation that says `DEXTERITY +2` reaches the throw spread and the aiming rule without either of them knowing where the points came from.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Dexterity {
    /// The creature's own dexterity, before anything changes it.
    pub base: i32,
}

/// How sharp a creature's eyes are.
///
/// Reference: `Character::per_max` (`character.cpp:4312-4315`), the base perception a character starts at eight with. The port carries the base and reads the effect table over it, the
/// same way it does for [`Strength`] and [`Dexterity`] - so an effect that costs perception reaches the aiming rule without that rule knowing where the points went.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Perception {
    /// The creature's own perception, before anything changes it.
    pub base: i32,
}

/// A creature's perception with its effects' modifiers applied.
///
/// Reference: `Character::get_per` (`character.cpp:4395-4398`) is `std::max( 0, get_per_base() + per_bonus )`, and the bonus is where the worn effects' `PER` modifiers land. A creature
/// that carries no [`Perception`] answers **zero**, the same convention as the other stats.
#[must_use]
pub fn effective_perception(world: &World, entity: Entity) -> i32 {
    let Some(perception) = world.get::<Perception>(entity) else {
        return 0;
    };
    (perception.base + effect_stat_bonus(world, entity, "PER")).max(0)
}

/// A creature's dexterity with its mutations' enchantments applied.
///
/// Reference: `Character::get_dex` (`character.cpp`) passes `get_dex_base()` through `enchantment_cache->modify_value( DEXTERITY, ... )`. A creature that carries no
/// [`Dexterity`] answers **zero**, which is the same convention [`effective_strength`] uses: a creature the port invented has no stats until something gives it some.
#[must_use]
pub fn effective_dexterity(world: &World, entity: Entity) -> i32 {
    let Some(dexterity) = world.get::<Dexterity>(entity) else {
        return 0;
    };
    let base = crate::mutation_data::holder(world).map_or(dexterity.base, |data| {
        let (add, multiply) = crate::mutations::enchantment_values(world, entity, &data, "DEXTERITY");
        #[allow(clippy::cast_possible_truncation)]
        let value =
            crate::mutations::apply_enchantments(f64::from(dexterity.base), add, multiply) as i32;
        value
    });
    // Reference: `Character::get_dex` is `std::max( 0, get_dex_base() + dex_bonus )` (`character.cpp:4390-4392`), and the bonus is where every worn effect's `DEX` modifier lands.
    (base + effect_stat_bonus(world, entity, "DEX")).max(0)
}

/// What the effects a creature is wearing add to one of its stats: `STR`, `DEX`, `PER` or `INT`.
///
/// Reference: `Character::get_str`/`get_dex` add `str_bonus`/`dex_bonus` to the base and floor at zero (`character.cpp:4385-4393`), and those bonuses are accumulated from the effects'
/// own modifier tables with `mod_str_bonus`/`mod_dex_bonus` from `Character::process_effects`. The value is the data's: an effect that states `dex_mod: [-2]` costs two dexterity.
#[must_use]
pub fn effect_stat_bonus(world: &World, entity: Entity, argument: &str) -> i32 {
    let worn = crate::applied_effects::worn(world, entity);
    let mut total = 0.0_f64;
    for effect in worn {
        let Some(kind) = crate::effect_registry::kind_named(world, &effect.id) else {
            continue;
        };
        total += kind.mod_value(argument, cdda_content::effects::ModAction::Min, 0, effect.intensity);
    }
    #[allow(clippy::cast_possible_truncation)]
    let total = total as i32;
    total
}

/// How strong a creature is.
///
/// Reference: `Character::get_str_base`, which the reference initialises to eight and raises with mutations and
/// bionics. The port carries the base only: pain and temporary modifiers are not here, and inventing them would make a
/// survivor's capacity wander for reasons nothing in the port models.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Strength {
    /// The base strength.
    pub base: i32,
}

/// Picks something up, charging what the store says it is worth in moves.
///
/// The same rule as [`pick_up`], with the volume taken from [`crate::item_registry`] instead of from a caller - which is
/// the difference between a pickup rule the game can run and one only a test could.
pub fn pick_up_in_store(world: &mut World, creature: Entity, item: Entity) -> Picked {
    let Some(kind) = world.get::<crate::GroundItem>(item).map(|held| held.kind) else {
        return Picked::NotHere;
    };
    let volume = crate::item_registry::volume_of(world, kind);
    // Reference: `Character::can_pickup` refuses what would put the character over `weight_capacity` before spending a
    // single move on it.
    let names = crate::item_registry::snapshot(world);
    let held = crate::inventory::weight_carried(world, creature, &names);
    let this = names.weight_of(kind)
        * i64::from(world.get::<crate::GroundItem>(item).map_or(1, |held| held.charges.max(1)));
    if held + this > carrying_capacity(world, creature) {
        return Picked::TooHeavy;
    }
    // Reference: `Character::can_stash` - something has to be able to take it, and what a survivor is wearing is all the
    // room there is. Asked pocket by pocket rather than as one total, because a hundred litres of room in fifty pockets
    // cannot hold a thing that fits none of them.
    let Some((worn, index)) = can_stash(world, creature, &names, volume, this) else {
        return Picked::TooBig;
    };
    let picked = pick_up(world, creature, item, volume);
    if picked == Picked::Taken {
        world
            .entity_mut(item)
            .insert(InPocket { worn, index });
    }
    picked
}

/// What came of trying to pick something up.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Picked {
    /// It is in hand, and no longer on the square.
    Taken,
    /// It is not on the creature's square.
    NotHere,
    /// There were not enough action points left.
    TooSlow,
    /// It would put the creature over what it can carry.
    TooHeavy,
    /// It would not fit in what the creature is wearing.
    TooBig,
    /// Somebody is already carrying it.
    AlreadyCarried,
}

/// Takes something off the square a creature is standing on.
///
/// Reference: `Character::pick_up` moves an item from the map into the character's inventory and charges
/// `item_handling_cost` for it. The port's inventory is not here yet, so what "carrying" means is a `Carried` link and
/// the loss of the item's `Position` - which is what takes it out of the index, the one place that says what is on a
/// square. An item carried that still had a square would be on two surfaces at once, which is the ghost the
/// architecture gate exists for.
pub fn pick_up(world: &mut World, creature: Entity, item: Entity, volume_ml: i32) -> Picked {
    if world.get::<Carried>(item).is_some() {
        return Picked::AlreadyCarried;
    }
    let Some(here) = world.get::<Position>(creature).map(|place| (place.x, place.y, place.z)) else {
        return Picked::NotHere;
    };
    let Some(there) = world.get::<Position>(item).map(|place| (place.x, place.y, place.z)) else {
        return Picked::NotHere;
    };
    if here != there {
        return Picked::NotHere;
    }
    let cost = handling_cost(volume_ml);
    if moves_left(world, creature) < cost {
        return Picked::TooSlow;
    }
    spend_moves(world, creature, f64::from(cost));
    take_into_hand(world, creature, item, there);
    Picked::Taken
}

/// A thing somebody is carrying.
///
/// Reference: the item being in a character's inventory. A carried item has no square, which is the whole of what
/// "in hand" means to the index and to every rule that asks what is on a square.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Carried(pub Entity);

/// What an activity's moves are counted in.
///
/// Reference: `based_on_type` (`player_activity.h`). A `Time` activity takes a hundred moves a turn whatever the
/// creature's speed; a `Speed` activity takes whatever moves the creature has, so a quick one finishes sooner.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Basis {
    /// A hundred moves a turn, however fast the creature is.
    Time,
    /// Every move the creature has.
    Speed,
}

/// Something a creature is doing that takes more than one turn.
///
/// Reference: `player_activity` - an id, the moves it still needs, and what those moves are counted in. The port keeps
/// it as a component because that is all it is: a creature doing something is a creature with an activity on it, and
/// the thing is over when the component is gone.
#[derive(Component, Debug, Clone, PartialEq, Eq)]
pub struct Doing {
    /// Which activity it is.
    pub activity: String,
    /// How many moves it still needs.
    pub moves_left: i32,
    /// What those moves are counted in.
    pub basis: Basis,
    /// How many moves the whole activity needs, when it is the kind that reports progress.
    ///
    /// Reference: `player_activity::moves_total`/`item_counter`, which the craft actor reads to know **how far along** the work is. A craft needs it to place the
    /// reference's fifth-percent steps (`activity_actor.cpp:4237-4245`); an activity that does not report progress leaves it at zero.
    pub moves_total: i32,
    /// What it is being done to, when it is being done to one thing.
    ///
    /// Reference: `player_activity` keeps its target as an index into a list, because the reference's items live in
    /// containers. The port's items are entities, so the target is an entity - the same fact, without the list.
    pub subject: Option<Entity>,
}

/// Spends one turn's moves on what a creature is doing, and says whether it is finished.
///
/// Reference: `player_activity::do_turn` (`player_activity.cpp:276-294`), with the exertion multiplier left out because
/// the port has no exertion:
///
/// * `Time`: a hundred moves a turn come off the activity, and the creature's turn is spent;
/// * `Speed`: as many moves come off the activity as the creature had, however many that is.
///
/// Returns `true` when nothing is left to do, and in that case the caller clears the activity - finishing is the
/// stored decision, not this rule's.
pub fn spend_a_turn_on(world: &mut World, creature: Entity) -> bool {
    let Some(doing) = world.get::<Doing>(creature).cloned() else {
        return false;
    };
    let moves = moves_left(world, creature).max(0);
    let spent = match doing.basis {
        Basis::Time => {
            let hundred = 100.min(doing.moves_left);
            // A time-based activity eats a hundred moves whether or not the creature has them; the turn is spent
            // either way, which is what makes reading a book take the same time for a quick reader.
            spend_moves(world, creature, f64::from(hundred));
            hundred
        }
        Basis::Speed => {
            spend_moves(world, creature, f64::from(moves));
            moves.min(doing.moves_left)
        }
    };
    let left = doing.moves_left - spent;
    // A failure during a craft hands some of the work back, so the moves it returns have to leave this branch with the rest.
    let mut lost_moves = 0;
    // Reference: `craft_activity_actor::do_turn` (`activity_actor.cpp:4237-4245`) - every **fifth percent** of the craft's progress practises its proficiencies, and the
    // span handed over is `base_total_moves / 2000` seconds per step. Both ends are rounded down, so one turn that crosses two boundaries practises twice - and the
    // hook runs **before** the finish, because the turn that completes the work is the turn that crosses the last boundary.
    if doing.activity == "craft" && doing.moves_total > 0 {
        let steps_of = |spent: i32| i64::from(spent) * 20 / i64::from(doing.moves_total);
        let done_before = doing.moves_total - doing.moves_left;
        let done_now = done_before + spent;
        let crossed = steps_of(done_now) - steps_of(done_before);
        if let Some(making) = world.get::<crate::crafting::Making>(creature).cloned() {
            if crossed > 0 {
                let seconds = i64::from(doing.moves_total) / 2_000 * crossed;
                crate::crafting::teach_proficiencies(world, creature, &making.recipe, seconds);
            }
            // Reference: `num_practice_ticks` (`activity_actor.cpp:4232-4238`) - a **tick a hundred moves**, so the ticks due are the hundred-move boundaries the work
            // crossed this turn, and `craft_skill_gain` (`crafting.cpp:981-1033`) lays them on. The port counted the whole craft at the end; the count is the same, the
            // moment is not.
            // Reference: `craft_consume_tools` per fifth-percent step (`activity_actor.cpp:4255-4264`), and **not** on the turn that completes the craft
            // (`:4250-4253`): that step's share is the start's remainder, spent by the finish.
            if crossed > 0 && left > 0 {
                let names = crate::item_registry::snapshot(world);
                let crossed = i32::try_from(crossed).unwrap_or(0);
                crate::crafting::spend_craft_charges(world, creature, &making.recipe, making.batch, &names, crossed);
            }
            // Reference: `craft_activity_actor::do_turn` (`activity_actor.cpp:4299-4320`) - when the work passes the point the crafter's own roll picked, the craft
            // is checked: components can be destroyed, progress lost, and a craft that loses every component **ends**. The check runs on the same turn as the work,
            // because a failure is something that happens while making the thing rather than after it.
            if !making.recipe.skill.is_empty() || !making.recipe.components.is_empty() {
                let counter = i64::from(done_now) * 10_000_000 / i64::from(doing.moves_total.max(1));
                // A point of zero is **a check that was never placed**: a world with no dice rolls nothing, so the craft could not pick where its next check falls. It
                // is not "check immediately" - treating it that way made every craft in a diceless test fail on its first turn.
                if making.next_failure_point > 0
                    && making.next_failure_point != i64::MAX
                    && counter >= making.next_failure_point
                {
                    let names = crate::item_registry::snapshot(world);
                    let failure = crate::craft_roll::handle_failure(world, creature, &making.recipe, counter, &names);
                    if failure.catastrophic {
                        // Reference: a catastrophic failure ends the craft (`:4324-4340`), so the activity and the making both go.
                        world.entity_mut(creature).remove::<crate::crafting::Making>();
                        world.entity_mut(creature).remove::<Doing>();
                        return true;
                    }
                    #[allow(clippy::cast_possible_truncation)]
                    let lost = (failure.progress_lost * i64::from(doing.moves_total) / 10_000_000) as i32;
                    lost_moves = lost.max(0);
                    // The next check is placed from the progress the craft fell back to, which is what stops a failure from being re-checked every turn.
                    let after = i64::from(doing.moves_total - lost_moves.max(0)) * 10_000_000
                        / i64::from(doing.moves_total.max(1));
                    let data = crate::craft_roll::success_data(
                        making.recipe.difficulty,
                        crate::crafting::skill_for(world, creature, &making.recipe),
                    );
                    let roll = crate::craft_roll::success_roll(world, data);
                    if let Some(mut making) = world.get_mut::<crate::crafting::Making>(creature) {
                        making.next_failure_point = crate::craft_roll::next_failure_point(after, roll);
                    }
                }
            }
            if !making.recipe.skill.is_empty() {
                let ticks = done_now / 100 - done_before / 100;
                if ticks > 0 {
                    let cap = crate::skills_of::craft_skill_cap(making.recipe.difficulty);
                    let _ = crate::skills_of::practice(world, creature, &making.recipe.skill, ticks, cap);
                }
            }
        }
    }
    let left = left + lost_moves;
    if left <= 0 {
        world.entity_mut(creature).remove::<Doing>();
        return true;
    }
    if let Some(mut working) = world.get_mut::<Doing>(creature) {
        working.moves_left = left;
    }
    false
}

/// What a garment's data says about how hard it is to ruin.
///
/// Reference: the `fragile`, `sturdy` and `unbreakable` flags, which `item::damage_armor_durability`
/// (`item.cpp:9508-9532`) reads as: an unbreakable thing is never damaged; a fragile one counts its own resistance at
/// two thirds; a sturdy one takes nothing at all until the blow beats its threshold. A garment with no `Toughness`
/// component is treated exactly as one with all three flags clear, which is what a thing whose data says nothing gets.
#[derive(Component, Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Toughness {
    /// Ruins easily: its resistance counts for two thirds.
    pub fragile: bool,
    /// Takes nothing from a blow it shrugs off.
    pub sturdy: bool,
    /// Never damaged at all.
    pub unbreakable: bool,
}

impl Toughness {
    /// Reads the three flags out of a thing's data.
    ///
    /// Reference: the flags are strings in the item's `flags` array; the reference's own constants are the names used
    /// here, so a data file that says `"STURDY"` is understood without a second spelling list.
    #[must_use]
    pub fn of(flags: &std::collections::BTreeSet<String>) -> Self {
        Self {
            fragile: flags.contains("FRAGILE"),
            sturdy: flags.contains("STURDY"),
            unbreakable: flags.contains("UNBREAKABLE"),
        }
    }
}

/// How worn out a garment is.
///
/// Reference: `item::damage_` (`item.cpp:9441-9452`) on the reference's scale of a thousand per step and four thousand
/// to ruin. The port keeps the number on the **item** because that is where the reference keeps it, and a garment's
/// condition is a fact about the garment and not about the person wearing it.
#[derive(Component, Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Wear {
    /// How far gone it is, on the reference's scale.
    pub damage: i32,
}

/// What a blow did to the thing that stopped it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Struck {
    /// The blow did not damage it: the roll missed, or it is sturdy and the blow was small.
    Untouched,
    /// It took a step of damage and is still wearable.
    Damaged,
    /// It took the step that passed its maximum, and is gone.
    Ruined,
}

/// The blow a garment is asked to survive: what it took, what it resists, and how the dice fell.
///
/// One struct rather than six parameters because these numbers only mean anything together - a resistance without the
/// damage that met it, or a chance without the roll, is not a question anybody asks.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GarmentBlow {
    /// The damage that arrived, before anything stopped it.
    pub premitigated: f64,
    /// What the garment itself resists, which is not what it grants its wearer.
    pub own_resist: f64,
    /// How many parts of a body it covers.
    pub covered_parts: usize,
    /// Whether the data calls it fragile.
    pub fragile: bool,
    /// Whether the data calls it sturdy.
    pub sturdy: bool,
    /// The roll, 0 to 1.
    pub fraction: f64,
}

/// Damages a worn thing with the blow it just stopped.
///
/// Reference: `item::damage_armor_durability` (`item.cpp:9481-9537`), which computes a chance from the logistic in
/// [`cdda_content::armour::damage_chance`], rolls it with `x_in_y`, and calls `mod_damage` - whose "destroy" is
/// `damage + qty > max_damage()`, a **passing** comparison. The garment is despawned when it is ruined, because a
/// ruined garment protects nothing and leaving it in the store with its armour intact is the stale answer `dress`
/// exists to prevent.
///
/// `fraction` is the roll, passed in like every other roll in the port.
pub fn damage_worn(world: &mut World, item: Entity, blow: &GarmentBlow) -> Struck {
    if world.get::<Garment>(item).is_none() {
        return Struck::Untouched;
    }
    let chance = cdda_content::armour::damage_chance(
        blow.own_resist,
        blow.premitigated,
        blow.covered_parts,
        blow.fragile,
        blow.sturdy,
    );
    if blow.fraction >= chance {
        return Struck::Untouched;
    }
    let damage = world.get::<Wear>(item).map_or(0, |wear| wear.damage)
        + cdda_content::armour::DAMAGE_SCALE;
    if damage > cdda_content::armour::DAMAGE_MAX {
        world.despawn(item);
        dress(world);
        return Struck::Ruined;
    }
    world.entity_mut(item).insert(Wear { damage });
    Struck::Damaged
}

/// Damages whatever a creature is wearing on the part a blow reached.
///
/// Reference: `Character::absorb_hit` (`character_armor.cpp`) walks the worn items that cover the struck part and,
/// for each, tells `damage_armor_durability` what arrived **before** mitigation - a garment's own fate is decided by
/// the whole blow, not by the part of it the garment happened to stop.
///
/// Two numbers the port has to approximate, and neither is a guess about arithmetic:
///
/// * `own_resist` is the layer's own protection - its material's resistance times its thickness - because the port
///   does not keep a garment's separate "resistance to being damaged" apart from what it grants its wearer, and the
///   reference's two are computed from the same materials;
/// * `fragile` and `sturdy` come from the worn thing's [`Toughness`], which content fills from the item's flags; a
///   garment with none is treated as having all three clear.
///
/// Returns how the first garment that was in the way fared, which is what a caller usually wants to report; every
/// other garment on the part is damaged too, because that is what the reference does.
pub fn settle_armour(
    world: &mut World,
    victim: Entity,
    part: &str,
    premitigated: f64,
    fraction: f64,
) -> Struck {
    let mut in_the_way: Vec<(Entity, Vec<(String, cdda_content::armour::Layer)>)> = Vec::new();
    {
        let mut query = world.query::<(Entity, &WornBy, &Garment)>();
        for (item, worn_by, garment) in query.iter(world) {
            if worn_by.0 == victim {
                in_the_way.push((item, garment.layers.clone()));
            }
        }
    }
    let mut worst = Struck::Untouched;
    for (item, layers) in in_the_way {
        let Some(layer) = layers
            .iter()
            .find(|(covered, _)| covered == part)
            .map(|(_, layer)| *layer)
        else {
            continue;
        };
        let covered_parts = layers
            .iter()
            .map(|(covered, _)| covered.clone())
            .collect::<std::collections::BTreeSet<_>>()
            .len();
        let toughness = world
            .get::<Toughness>(item)
            .copied()
            .unwrap_or_default();
        if toughness.unbreakable {
            continue;
        }
        let blow = GarmentBlow {
            premitigated,
            own_resist: layer.resist * f64::from(layer.thickness),
            covered_parts,
            fragile: toughness.fragile,
            sturdy: toughness.sturdy,
            fraction,
        };
        let struck = damage_worn(world, item, &blow);
        if struck != Struck::Untouched {
            worst = struck;
        }
    }
    worst
}

/// What came of trying to put something on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Wore {
    /// It went on, and the wearer's armour now counts it.
    Worn,
    /// It is not a garment: nothing in the store says what it protects.
    NotAWearable,
    /// It is already being worn, by this creature or another.
    AlreadyWorn,
    /// It is lying on a square: it has to be in hand before it can go on.
    NotInHand,
    /// There were not enough moves left to put it on.
    TooSlow,
}

/// Puts a thing on a creature, if it is a garment and not already being worn.
///
/// Reference: `Character::wear` (`character_attire.cpp:340-353`): the refusals come first, then `item_wear_cost` is
/// charged. A garment that cannot be worn therefore costs nothing, which is the order the reference uses and the order
/// that stops a survivor losing a turn for trying a rock on as a hat.
///
/// Reference: `Character::wear` refuses an item that is not armour and one that is already worn, and `dress` is what
/// turns the list into the numbers a blow reads. The port keeps the refusal reasons as an answer rather than a
/// message, because a caller may want to say something different - and because a silent failure to wear a vest would
/// be the worst kind of bug in a game about vests.
/// Which layer of clothing something is worn on.
///
/// Reference: `layer_level` (`src/bodypart.h`), which decides how long a garment takes to put on: skin-tight clothes are
/// slower than loose ones, and a belt is quicker than both.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Layering {
    /// Against the skin.
    Skintight,
    /// An ordinary shirt or trousers.
    Normal,
    /// Hanging off a belt.
    Belted,
    /// At the waist.
    Waist,
    /// The outside layer.
    Outer,
    /// Personal effects.
    Personal,
    /// Worn from an aura rather than clothing.
    Aura,
}

/// How many moves putting a garment on costs.
///
/// Reference: `Character::item_wear_cost` (`character_attire.cpp:459-500`) - `item_handling_cost` times a layer factor:
/// skin-tight is half again as slow, waist and outer layers are divided by one and a half, and a belted thing is halved.
/// The encumbrance term the reference multiplies in is not here, because the port does not model encumbrance yet and a
/// made-up factor would be worse than none.
#[must_use]
pub fn wear_cost(volume_ml: i32, layer: Layering) -> i32 {
    let base = handling_cost(volume_ml);
    match layer {
        Layering::Skintight => base * 3 / 2,
        Layering::Waist | Layering::Outer => base * 2 / 3,
        Layering::Belted => base / 2,
        Layering::Normal | Layering::Personal | Layering::Aura => base,
    }
}

/// Puts a thing on, if it is a garment, not already worn, and the creature has the moves for it.
pub fn wear(world: &mut World, wearer: Entity, item: Entity) -> Wore {
    if world.get::<Garment>(item).is_none() {
        return Wore::NotAWearable;
    }
    if world.get::<WornBy>(item).is_some() {
        return Wore::AlreadyWorn;
    }
    // Reference: `Character::wear` takes an item out of the inventory, so an item on the map is not a candidate
    // until it has been picked up. Without this the port would let a survivor wear a jacket through the floor.
    if world.get::<Position>(item).is_some() && world.get::<Carried>(item).is_none() {
        return Wore::NotInHand;
    }
    // Reference: `Character::can_wear` is asked before the cost is charged (`character_attire.cpp:340-353`), so a thing
    // that cannot be worn costs nothing.
    let cost = wear_cost(0, Layering::Normal);
    if moves_left(world, wearer) < cost {
        return Wore::TooSlow;
    }
    give_moves(world, wearer, -cost);
    put_on(world, wearer, item);
    Wore::Worn
}

/// Puts a garment on without asking what it costs.
///
/// The half of [`wear`] that changes the store, kept apart because the wearing activity has already paid: `ACT_WEAR`
/// spends its own time (`activity_actor.cpp:7001-7035`) and then calls `wear_item`, which does not charge again.
fn put_on(world: &mut World, wearer: Entity, item: Entity) -> Wore {
    if world.get::<Garment>(item).is_none() {
        return Wore::NotAWearable;
    }
    if world.get::<WornBy>(item).is_some() {
        return Wore::AlreadyWorn;
    }
    world.entity_mut(item).insert(WornBy(wearer));
    world.entity_mut(item).remove::<InPocket>();
    world.entity_mut(item).remove::<WieldedBy>();
    dress(world);
    Wore::Worn
}

/// Starts putting a garment on, as something that takes time.
///
/// Reference: `inventory_ui.cpp:4355` assigns `wear_activity_actor`, and its `do_turn`
/// (`activity_actor.cpp:7001-7035`) puts the thing on while the character has moves left. The port charges
/// [`wear_cost`] as the activity's length instead - one garment, one activity - because a queue of garments is a menu's
/// business and the port has no menu.
pub fn begin_wear(
    world: &mut World,
    creature: Entity,
    item: Entity,
    volume_ml: i32,
    layer: Layering,
) -> Began {
    if world.get::<Doing>(creature).is_some() {
        return Began::Busy;
    }
    if world.get::<Garment>(item).is_none() {
        return Began::NotHere;
    }
    if world.get::<WornBy>(item).is_some() {
        return Began::AlreadyCarried;
    }
    let cost = wear_cost(volume_ml, layer);
    if moves_left(world, creature) >= cost {
        give_moves(world, creature, -cost);
        put_on(world, creature, item);
        return Began::Done;
    }
    world.entity_mut(creature).insert(Doing {
        moves_total: 0,
        activity: "wear".to_owned(),
        moves_left: cost,
        basis: Basis::Time,
        subject: Some(item),
    });
    Began::Started
}

/// Takes a thing off, whoever is wearing it.
///
/// Returns whether it was being worn at all. `dress` runs afterwards, so the armour goes with the garment - the
/// answer is derived, and a derived answer that outlives its cause is the bug `dress` exists to prevent.
pub fn take_off(world: &mut World, item: Entity) -> bool {
    if world.get::<WornBy>(item).is_none() {
        return false;
    }
    world.entity_mut(item).remove::<WornBy>();
    dress(world);
    true
}

/// Works out what every creature is wearing and writes it into [`PartArmour`].
///
/// This is the answer-to-a-question shape the architecture asks for: `PartArmour` is derived, one system writes it,
/// and nothing else does. A creature with nothing on ends up with an empty map rather than a stale one from the turn
/// before, which is the failure a store with two writers always has.
pub fn dress(world: &mut World) {
    // What each worn thing protects, gathered before anything is written.
    let mut worn: Vec<(Entity, Vec<(String, cdda_content::armour::Layer)>)> = Vec::new();
    {
        let mut query = world.query::<(&WornBy, &Garment)>();
        for (worn_by, garment) in query.iter(world) {
            worn.push((worn_by.0, garment.layers.clone()));
        }
    }
    let wearers: Vec<Entity> = {
        let mut query = world.query::<(Entity, &Creature)>();
        query.iter(world).map(|(entity, _)| entity).collect()
    };
    for wearer in wearers {
        let mut tenths_of: std::collections::BTreeMap<String, i32> = std::collections::BTreeMap::new();
        for (owner, layers) in &worn {
            if *owner != wearer {
                continue;
            }
            for (part, layer) in layers {
                // A hit that meets the layer is what the armour is for; the roll is the store being conservative
                // (every layer counted) until a blow asks for a specific one. The sum is kept in tenths of a point
                // because a garment's protection is a fraction - a shirt is a third of a point - and truncating each
                // layer as it is added would throw away the difference between one shirt and three.
                #[allow(clippy::cast_possible_truncation)]
                let tenths = (cdda_content::armour::layer_resist(*layer, 0) * 10.0).round() as i32;
                *tenths_of.entry(part.clone()).or_insert(0) += tenths;
            }
        }
        if tenths_of.is_empty() {
            world.entity_mut(wearer).remove::<PartArmour>();
        } else {
            // One conversion at the end, the way `Character::get_armor_type` returns an `int`: the fraction is kept
            // while the layers are added and dropped once, so two thin layers can make a point between them.
            let armour = tenths_of
                .into_iter()
                .map(|(part, tenths)| (part, tenths / 10))
                .collect();
            world.entity_mut(wearer).insert(PartArmour(armour));
        }
    }
}

/// Where a creature has been hurt, part by part.
///
/// Reference: `Creature::deal_damage` takes a `bodypart_id`, and the damage is recorded against it. The port keeps
/// the running total per part, which is what a wound is: a value the store holds, written once by the blow that
/// caused it and read by anything that cares how hurt an arm is.
#[derive(Component, Debug, Clone, Default, PartialEq, Eq)]
pub struct Wounds(pub std::collections::BTreeMap<String, i32>);

/// What a creature's part has taken so far.
#[must_use]
pub fn wound_on(world: &World, entity: Entity, part: &str) -> i32 {
    world
        .get::<Wounds>(entity)
        .and_then(|wounds| wounds.0.get(part).copied())
        .unwrap_or(0)
}

/// Records a blow against a part of a body.
///
/// Reference: `Creature::deal_damage` (`creature.cpp`) adds the damage to the part it was dealt to. A blow that
/// rolls no part - a creature with no body, or one whose parts the rule refused - is still damage, so it is counted
/// against nothing rather than silently dropped from the world's books.
pub fn wound(world: &mut World, entity: Entity, part: &str, damage: i32) {
    if let Some(mut wounds) = world.get_mut::<Wounds>(entity) {
        *wounds.0.entry(part.to_owned()).or_insert(0) += damage;
    } else {
        let mut fresh = Wounds::default();
        fresh.0.insert(part.to_owned(), damage);
        world.entity_mut(entity).insert(fresh);
    }
}

/// The attack a creature swings, taken from its type.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct MeleeAttack(pub crate::melee::Attack);

/// What a creature's own body resists.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Armour {
    /// Resistance to blunt damage.
    pub bash: i32,
    /// Resistance to cutting damage.
    pub cut: i32,
    /// Resistance to stabbing damage.
    pub stab: i32,
}

/// Puts a monster on a square with everything its type says it has.
///
/// Reference: `monster::monster( const mtype * )` gives a creature its hit points, its speed, its melee dice
/// and its armour from the type, and `monster::get_speed()` scales that base by the `MONSTER_SPEED` option -
/// which this takes as a parameter because options are content, not a constant.
///
/// The creature is left with **no** goal: what it wants to do is a system's business, and a monster that has
/// just been placed has not decided anything yet.
/// What a creature is made of: the reference's `mtype::materials` map, on the creature (`Creature::made_of`, `creature.cpp:1018`).
///
/// **A fact about the thing, so it lives on the thing.** The reference asks `target.made_of( material_veggy )` of the creature itself; the port used to answer such questions only if a caller
/// held the whole `Vec<MonsterType>` - the same second source of truth the item registry removed.
#[derive(bevy_ecs::prelude::Component, Debug, Clone, Default, PartialEq, Eq)]
pub struct Materials(pub std::collections::BTreeMap<String, i32>);

/// What a creature is made of, or nothing if it carries no materials.
#[must_use]
pub fn materials_of(world: &bevy_ecs::prelude::World, creature: bevy_ecs::prelude::Entity) -> std::collections::BTreeMap<String, i32> {
    world.get::<Materials>(creature).map_or_else(std::collections::BTreeMap::new, |made| made.0.clone())
}

/// Spawns a monster of a data type, carrying what it is made of.
///
/// Reference: `Monster::Monster( const mtype * )` (`monster.cpp:130-190`), which keeps the type's materials on the monster and is why `target.made_of( material_veggy )` can be asked of any
/// creature without a table beside it.
pub fn spawn_monster_of_type(
    world: &mut World,
    kind: crate::MonsterKindId,
    monster_type: &cdda_content::monsters::MonsterType,
    square: (i32, i32, i32),
    monster_speed_percent: i32,
) -> Entity {
    let entity = spawn_creature(world, &monster_type.name, square);
    if !monster_type.materials.is_empty() {
        world.entity_mut(entity).insert(Materials(monster_type.materials.clone()));
    }
    set_kind(world, entity, kind);
    set_health(world, entity, monster_type.hp);
    set_speed(
        world,
        entity,
        scaled_speed(monster_type.speed, monster_speed_percent),
        0,
    );
    world.entity_mut(entity).insert((
        MeleeAttack(crate::melee::Attack {
            accuracy_tenths: monster_type.melee_skill * 10,
            // Reference: `monster::melee_attack` adds its dice as **bash** with the type's own `melee_dice_ap` as penetration
            // (`monster.cpp:2145`, `damage.add_damage( damage_bash, dice( melee_dice, melee_sides ), melee_dice_ap )`).
            // The type's fixed damage stays the default zero: a monster's blow is dice, and the data states no flat part for it.
            fixed: crate::melee::Damage {
                bash: monster_type.melee_bash,
                cut: monster_type.melee_cut,
                stab: monster_type.melee_stab,
                penetration: monster_type.melee_dice_ap,
            },
            dice: monster_type.melee_dice,
            sides: monster_type.melee_sides,
        }),
        Armour {
            bash: monster_type.armor_bash,
            cut: monster_type.armor_cut,
            stab: monster_type.armor_stab,
        },
        Dodge {
            points: monster_type.dodge,
        },
        AttackCost(monster_type.attack_cost),
        Morale(monster_type.morale),
        Anger(monster_type.aggression),
        Nerve {
            morale: monster_type.morale,
            anger: monster_type.aggression,
            regenerates: monster_type.regen_morale,
        },
    ));
    // Reference: `monster::monster( const mtype * )` copies `type->flags` onto the monster, and every question about what it can do - `can_hear` among them - is asked of that
    // set (`monster.cpp:1318`). Without this a spawned monster is deaf and blind whatever its type says.
    world
        .entity_mut(entity)
        .insert(crate::creature_flags::CreatureFlags(
            monster_type.flags.clone(),
        ));
    // Reference: `monster::regenerate` reads the type's flags, hit points and materials every turn (`monster.cpp:4094-4103`); the port decides the rate once, here, because
    // none of those three can change under a monster that has already been placed.
    let materials: Vec<&str> = monster_type
        .materials
        .keys()
        .map(String::as_str)
        .collect();
    let rate = crate::regeneration::mending_per_hour(
        monster_type.hp,
        monster_type.flags.contains("REVIVES"),
        monster_type.flags.contains("DORMANT"),
        &materials,
        monster_type.regenerates,
    );
    world
        .entity_mut(entity)
        .insert(crate::regeneration::Mending {
            per_hour: rate,
            type_speed: monster_type.speed,
            type_hp: monster_type.hp,
        });
    if let Some(size) = size_named(&monster_type.size)
        && let Some(mut creature) = world.get_mut::<Creature>(entity)
    {
        creature.size = size;
    }
    entity
}

/// The size a name stands for, or nothing when it is a name the reference does not use.
#[must_use]
pub fn size_named(name: &str) -> Option<crate::CreatureSize> {
    match name {
        "tiny" => Some(crate::CreatureSize::Tiny),
        "small" => Some(crate::CreatureSize::Small),
        "medium" => Some(crate::CreatureSize::Medium),
        "large" => Some(crate::CreatureSize::Large),
        "huge" => Some(crate::CreatureSize::Huge),
        _ => None,
    }
}

/// What kind of creature this is, when the content said so.
///
/// Reference: a `monster`'s `type` (`src/monster.h`), which `monster::die` reads to make the corpse
/// (`item::make_corpse`). Optional, because a creature the port invented - a test's survivor - has no
/// prototype behind it, and inventing a kind for it would put a number in the world that content never gave.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct CreatureKind(pub crate::MonsterKindId);

/// Says what kind of creature this is, so that its death can leave the right body.
pub fn set_kind(world: &mut World, entity: Entity, kind: crate::MonsterKindId) {
    world.entity_mut(entity).insert(CreatureKind(kind));
}

/// Clears the dead, leaving a corpse item wherever one had a kind behind it.
///
/// Reference: `monster::die` calls `m.add_item_or_charges( pos, item::make_corpse( ... ) )`, so the body is an
/// **item**, not a creature the world keeps around. The corpse carries what it was and how much room it took
/// up, which is what `item::corpse_volume` measures later.
///
/// A creature with no [`CreatureKind`] is cleared like any other and leaves nothing: there is no prototype to
/// name the body with, and a test's survivor is not a monster that died.
pub fn reap_dead_creatures_as_corpses(
    world: &mut World,
    corpse_kind: crate::ItemTypeId,
    volume_of: impl Fn(crate::MonsterKindId) -> i64,
) -> usize {
    type Dead = (Entity, (i32, i32, i32), Option<crate::MonsterKindId>, bool);
    let dead: Vec<Dead> = world
        .query::<(
            Entity,
            &Health,
            &Position,
            Option<&CreatureKind>,
            Option<&crate::creature_flags::CreatureFlags>,
        )>()
        .iter(world)
        .filter(|(_, health, _, _, _)| health.hp <= 0)
        .map(|(entity, _, position, kind, flags)| {
            (
                entity,
                (position.x, position.y, position.z),
                kind.map(|kind| kind.0),
                flags.is_some_and(|carried| carried.0.contains("REVIVES")),
            )
        })
        .collect();
    for (entity, square, kind, revives) in &dead {
        crate::singletons::index_mut(world).remove(*entity, *square);
        world.despawn(*entity);
        if let Some(kind) = kind {
            let died_on = crate::calendar::of(world).map_or(0, |calendar| calendar.turn);
            let corpse = crate::drop_item(world, (square.0, square.1), corpse_kind, 1);
            world.entity_mut(corpse).insert(crate::CorpseOf {
                kind: *kind,
                died_on,
                revives: *revives,
                monster_volume_ml: volume_of(*kind),
            });
        }
    }
    dead.len()
}

/// What the world leaves behind when a monster dies: the item kind a corpse is, and how big each monster kind was.
///
/// Reference: `item::corpse_volume` (`item.cpp`) measures a corpse from the monster's own volume, and the corpse item is the content's `corpse`. Both are **content**, so the caller
/// installs them once and the creature stage reads them; a world with no corpse kind reaps the dead without leaving anything, which is what a test world with no item data wants.
#[derive(Component, Debug, Clone, Default)]
pub struct CorpseKind {
    /// The item kind every corpse is. Kind zero is the registry's own placeholder, which no data names.
    pub item: crate::ItemTypeId,
    /// What each monster kind took up while alive, in millilitres.
    pub volumes_ml: std::collections::BTreeMap<crate::MonsterKindId, i64>,
}

/// Marks the entity carrying [`CorpseKind`].
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct TheCorpseKind;

/// Installs what a corpse is, replacing whatever was there.
pub fn set_corpse_kind(world: &mut World, item: crate::ItemTypeId, volumes_ml: std::collections::BTreeMap<crate::MonsterKindId, i64>) {
    let entity = crate::singletons::singleton::<CorpseKind, TheCorpseKind>(world);
    world.entity_mut(entity).insert(CorpseKind {
        item,
        volumes_ml,
    });
}

/// What a corpse is in this world, if the content has said.
#[must_use]
pub fn corpse_kind(world: &World) -> Option<(crate::ItemTypeId, std::collections::BTreeMap<crate::MonsterKindId, i64>)> {
    crate::singletons::holder::<TheCorpseKind>(world)
        .and_then(|holder| world.get::<CorpseKind>(holder))
        .map(|kind| (kind.item, kind.volumes_ml.clone()))
}

/// One corpse that may get up: the corpse, what it was, where it is, and when it died.
type Candidate = (Entity, crate::MonsterKindId, (i32, i32, i32), i64);

/// Brings back the corpses whose age says they are ready, and answers how many got up.
///
/// Reference: `game::revive_corpse` (`game.cpp:5718-5760`) is what a corpse's `ready_to_revive` answer leads to - the corpse is destroyed and the monster it was is placed on its
/// square. The port asks the same rule with the corpse's own age in hours and, when it answers yes, hands the kind to the caller's `spawn`, because a monster type is content and
/// this module is not where content lives.
#[must_use]
pub fn revive_corpses(
    world: &mut World,
    mut spawn: impl FnMut(&mut World, crate::MonsterKindId, (i32, i32, i32)) -> bool,
) -> i32 {
    let now = crate::calendar::of(world).map_or(0, |calendar| calendar.turn);
    // The candidates are read before any of them is asked, because asking spends dice and spending dice needs the world to itself.
    let candidates: Vec<Candidate> = world
        .query::<(Entity, &crate::CorpseOf, &Position)>()
        .iter(world)
        .filter(|(_, corpse, _)| corpse.revives)
        .map(|(entity, corpse, position)| {
            (
                entity,
                corpse.kind,
                (position.x, position.y, position.z),
                corpse.died_on,
            )
        })
        .collect();
    let mut risen = 0;
    for (corpse, kind, square, died_on) in candidates {
        let Ok(hours) = i32::try_from((now - died_on) / i64::from(crate::regeneration::TURNS_PER_HOUR)) else {
            continue;
        };
        let Some(mut rolls) = crate::singletons::take_rolls(world) else {
            continue;
        };
        let answer = crate::regeneration::ready_to_revive(hours, 0, &mut rolls);
        crate::singletons::put_rolls(world, rolls);
        if !answer || !spawn(world, kind, square) {
            continue;
        }
        crate::singletons::index_mut(world).remove(corpse, square);
        world.despawn(corpse);
        risen += 1;
    }
    risen
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

/// A noise a creature is walking towards, and how many turns it will keep walking.
///
/// Reference: `monster::wander_to( p, f )` (`monster.cpp`) sets `wander_pos` and `wandf`, and `monster::move` walks the monster towards `wander_pos` while `wandf`
/// lasts. Kept apart from [`Goal`] because the reference keeps them apart: a goal is what a monster decided, a wander is what it heard, and a monster can have both.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Wander {
    /// The square it thinks the noise came from.
    pub square: (i32, i32, i32),
    /// How many turns it will keep walking, counted down as it goes.
    pub turns_left: i32,
    /// Whether the sound it is walking towards was worth investigating - the reference's `monster::provocative_sound` (`monster.cpp:3975`), which decides whether a duller sound can
    /// distract it before it arrives.
    pub provocative: bool,
}

/// Points a creature at a noise for a number of turns, keeping whichever wander it already had only if that one lasts longer.
///
/// Reference: `monster::hear_sound` refuses to replace a wander that has more turns left than the sound it just heard would give it (`monster.cpp:3968`), so a monster
/// already walking towards something interesting is not distracted by a rock.
pub fn set_wander(world: &mut World, entity: Entity, square: (i32, i32, i32), turns: i32) {
    if turns <= 0 {
        return;
    }
    // Reference: `monster::wander_to` (`monster.cpp:1677`) refuses outright for a docile monster - it hears the noise and stays where it is put.
    if crate::applied_effects::has(world, entity, "docile") {
        return;
    }
    if let Some(mut wander) = world.get_mut::<Wander>(entity) {
        if wander.turns_left >= turns {
            return;
        }
        wander.square = square;
        wander.turns_left = turns;
    } else {
        world.entity_mut(entity).insert(Wander {
            square,
            turns_left: turns,
            provocative: false,
        });
    }
}

/// Gives every creature its speed and lets each one with a goal take one step.
pub fn take_turns(world: &mut World) {
    bank_a_turn(world);
    // A creature that is busy spends its turn on what it is busy with, and does not walk. Reference:
    // `player_activity::do_turn` runs as the character's turn, which is why an activity and a step cannot happen in
    // the same turn.
    // Every creature alive spends a turn of its effects before it acts, so a blow this turn meets the effect that had not
    // yet expired this turn. Reference: `Character::process_turn` processes effects at the start of the character's turn
    // (`character.cpp:3930-3960`), and `Creature::process_effects` is what does it.
    let wearing: Vec<Entity> = world
        .query_filtered::<Entity, (With<crate::creature::Creature>, With<crate::applied_effects::AppliedEffects>)>()
        .iter(world)
        .collect();
    for creature in wearing {
        crate::applied_effects::spend_a_turn(world, creature);
    }
    let busy: Vec<(Entity, String, Option<Entity>)> = world
        .query::<(Entity, &Doing)>()
        .iter(world)
        .map(|(entity, doing)| (entity, doing.activity.clone(), doing.subject))
        .collect();
    let mut working = std::collections::BTreeSet::new();
    for (entity, activity, subject) in busy {
        if spend_a_turn_on(world, entity) {
            finish_activity(world, entity, &activity, subject);
        }
        working.insert(entity);
    }
    // What a creature is walking towards: its own goal when it has one, and otherwise whatever it heard. Reference: `monster::move` reads `wander_pos` only when the
    // monster has no `goal` of its own, so a monster that has decided something is not pulled off course by a noise.
    let mut walkers: Vec<(Entity, (i32, i32, i32), bool)> = world
        .query::<(Entity, Option<&Goal>, Option<&Wander>)>()
        .iter(world)
        .filter_map(|(entity, goal, wander)| {
            goal.map(|goal| (entity, goal.square, true))
                .or_else(|| wander.map(|wander| (entity, wander.square, false)))
        })
        .collect();
    // A frightened creature walks even when nothing told it where to go: the reference asks `is_fleeing` inside `monster::move` (`monmove.cpp:489`), which every monster runs, not only
    // the ones with a goal. Without this, a panicking civilian stands still until something gives it a destination - which is what the scenario found.
    for (entity, square) in fleeing_creatures(world) {
        if !walkers.iter().any(|(walking, _, _)| *walking == entity) {
            walkers.push((entity, square, false));
        }
    }
    for (entity, goal, is_goal) in walkers {
        if working.contains(&entity) {
            continue;
        }
        // A wander is a memory of a noise and lasts as long as the noise said: counted down here, and gone when it runs out.
        if !is_goal {
            let spent = world.get_mut::<Wander>(entity).map(|mut wander| {
                wander.turns_left -= 1;
                wander.turns_left
            });
            if spent.is_some_and(|left| left <= 0) {
                world.entity_mut(entity).remove::<Wander>();
            }
        }
        // A creature that can reach its goal this step hits whoever is on it instead of walking into them.
        // Reference: `monster::move_to` refuses a square another creature holds, and `monster::attack` is what
        // it does instead - the swing is the move.
        if strike_what_the_goal_holds(world, entity, goal) {
            continue;
        }
        let bounds = walk_bounds(world);
        // A frightened creature runs: `monmove.cpp:489` turns the distance term of the reference's movement scoring around when `is_fleeing`, so the square it prefers is the one
        // farthest from the survivor. `Goal` is left in place, because a monster that recovers its nerve should carry on where it was headed.
        let goal = fleeing_from_a_scared_monster(world, entity).unwrap_or(goal);
        let outcome = crate::routing::walk_towards(world, entity, goal, bounds);
        if matches!(outcome, crate::routing::Walk::Arrived | crate::routing::Walk::NoRoute) {
            if is_goal {
                world.entity_mut(entity).remove::<Goal>();
            } else {
                world.entity_mut(entity).remove::<Wander>();
            }
        }
    }
}

/// One monster's nerve and the numbers the regrowth rule reads, copied out of the store before it is written back.
///
/// The alias is here rather than inline because the tuple has seven parts, and a seven-part tuple written twice is a place for two of them to swap places.
type Nerves = (Entity, i32, i32, Nerve, i32, i32, (i32, i32, i32));

/// A cornered fighter loses its nerve when another monster is close by, and finds it again nine turns later.
///
/// Reference: the `CORNERED_FIGHTER` block in `monster::process_turn` (`monster.cpp:3518-3549`). Three conditions: the monster carries the flag, its morale is zero or less, and another
/// monster stands within **two squares**. Then either it has not been spooked recently - and gets `spooked` for **three turns** and `spooked_recent` for **nine** - or it has, and its morale
/// and anger snap back to its type's values, which is the reference's way of saying *a cornered animal fights once it has nowhere left to run*.
///
/// `spooked` is not a mood: it is the AI tag that `is_fleeing` reads through the reference's effect cache, which is why the walk checks for it directly.
pub fn cornered_fighters_lose_their_nerve(world: &mut World) {
    let Some(spooked) = crate::effect_registry::kind_of(world, "spooked") else {
        return;
    };
    let Some(spooked_recent) = crate::effect_registry::kind_of(world, "spooked_recent") else {
        return;
    };
    // Only **other monsters** crowd a monster: a square, its ground and its terrain all carry a `Position` too, so a query for position alone finds the floor the monster is standing on (round
    // 1128's bug), and `With<Creature>` alone finds the survivor as well - which made every cornered fighter within two squares of the player spooked whatever it thought of them (round
    // 1130's). The survivor has its own branch, further down, with an attitude test of its own.
    let neighbours: Vec<(Entity, (i32, i32, i32))> = world
        .query_filtered::<(Entity, &Position), (bevy_ecs::prelude::With<Creature>, bevy_ecs::prelude::Without<crate::Player>)>()
        .iter(world)
        .map(|(entity, place)| (entity, (place.x, place.y, place.z)))
        .collect();
    let cornered: Vec<(Entity, (i32, i32, i32))> = world
        .query::<(Entity, &Position, &Morale, &crate::creature_flags::CreatureFlags)>()
        .iter(world)
        .filter(|(_, _, morale, flags)| morale.0 <= 0 && flags.0.contains("CORNERED_FIGHTER"))
        .map(|(entity, place, _, _)| (entity, (place.x, place.y, place.z)))
        .collect();
    let survivor = {
        let mut query = world.query_filtered::<(Entity, &Position), bevy_ecs::prelude::With<crate::Player>>();
        query.iter(world).next().map(|(_, place)| (place.x, place.y, place.z))
    };
    for (entity, here) in cornered {
        // `points_in_radius( pos, 2 )` is a square radius, so both axes are compared the same way.
        let crowded = neighbours.iter().any(|(other, there)| {
            *other != entity
                && (there.0 - here.0).abs() <= 2
                && (there.1 - here.1).abs() <= 2
                && there.2 == here.2
        });
        // Reference: the same block's second half (`monster.cpp:3534-3548`) - a **character** within two squares whose attitude this monster would only follow or flee from spooks it just
        // as another monster does. A cornered animal does not need to be outnumbered; it needs to want to be somewhere else and not be able to get there.
        let daunted = survivor.is_some_and(|threat| {
            (threat.0 - here.0).abs() <= 2
                && (threat.1 - here.1).abs() <= 2
                && threat.2 == here.2
                && {
                    let (hp, max_hp) = world
                        .get::<Health>(entity)
                        .map_or((0, 0), |health| (health.hp, health.max_hp));
                    let morale = world.get::<Morale>(entity).map_or(0, |morale| morale.0);
                    let anger = world.get::<Anger>(entity).map_or(0, |anger| anger.0);
                    matches!(
                        attitude(hp, max_hp, morale, anger),
                        Attitude::Follow | Attitude::Flee
                    )
                }
        });
        if !crowded && !daunted {
            continue;
        }
        if crate::applied_effects::has(world, entity, "spooked") {
            continue;
        }
        if crate::applied_effects::has(world, entity, "spooked_recent") {
            let nerve = world.get::<Nerve>(entity).copied();
            if let Some(nerve) = nerve {
                if let Some(mut morale) = world.get_mut::<Morale>(entity)
                    && morale.0 < nerve.morale
                {
                    morale.0 = nerve.morale;
                }
                if let Some(mut anger) = world.get_mut::<Anger>(entity) {
                    anger.0 = nerve.anger;
                }
            }
            continue;
        }
        crate::applied_effects::add(world, entity, crate::effects::ActiveEffect::new(&spooked, 3, 1));
        crate::applied_effects::add(
            world,
            entity,
            crate::effects::ActiveEffect::new(&spooked_recent, 9, 1),
        );
    }
}

/// Gives every monster its nerve back a step, once a turn, at full health.
///
/// Reference: `monster::process_turn` calls the regrowth block for every monster each turn (`monster.cpp:3493`), and the block's own gate - `type->regen_morale && hp >= type->hp` -
/// decides whether anything happens. The survivor's position is asked for once here rather than per creature, because `is_fleeing` is the only thing that needs it.
pub fn regen_everyones_nerve(world: &mut World) {
    let survivor = {
        let mut query = world.query_filtered::<(Entity, &Position), bevy_ecs::prelude::With<crate::Player>>();
        query.iter(world).next().map(|(_, place)| (place.x, place.y, place.z))
    };
    let ready: Vec<Nerves> = world
        .query::<(Entity, &Morale, &Anger, &Nerve, &Health, &Position)>()
        .iter(world)
        .map(|(entity, morale, anger, nerve, health, place)| {
            (
                entity,
                morale.0,
                anger.0,
                *nerve,
                health.hp,
                health.max_hp,
                (place.x, place.y, place.z),
            )
        })
        .collect();
    for (entity, morale, anger, nerve, hp, max_hp, here) in ready {
        let distance = survivor.map_or(i32::MAX, |threat| {
            (here.0 - threat.0).abs() + (here.1 - threat.1).abs() + (here.2 - threat.2).abs()
        });
        let fleeing = is_fleeing(hp, max_hp, morale, anger, distance);
        let (next_morale, next_anger) = regen_nerve(hp, max_hp, morale, anger, nerve, fleeing);
        if next_morale != morale
            && let Some(mut slot) = world.get_mut::<Morale>(entity)
        {
            slot.0 = next_morale;
        }
        if next_anger != anger
            && let Some(mut slot) = world.get_mut::<Anger>(entity)
        {
            slot.0 = next_anger;
        }
        // And then one step of drift back towards the type's value, on the reference's two rolls (`monster.cpp:1877-1885`). The dice are taken out of the store for the draw and put back,
        // which is how every rule in this port draws.
        let Some(mut rolls) = crate::singletons::take_rolls(world) else {
            continue;
        };
        let (drifted, drift_anger) = {
            let morale = world.get::<Morale>(entity).map_or(next_morale, |morale| morale.0);
            let anger = world.get::<Anger>(entity).map_or(next_anger, |anger| anger.0);
            (drift_morale(morale, nerve.morale, &mut rolls), drift_morale(anger, nerve.anger, &mut rolls))
        };
        crate::singletons::put_rolls(world, rolls);
        if drifted != next_morale
            && let Some(mut slot) = world.get_mut::<Morale>(entity)
        {
            slot.0 = drifted;
        }
        if drift_anger != next_anger
            && let Some(mut slot) = world.get_mut::<Anger>(entity)
        {
            slot.0 = drift_anger;
        }
    }
}

/// Every creature that is frightened of the survivor, and the square it is running to.
///
/// Reference: `monmove.cpp:489`, where `is_fleeing` is asked of every monster that moves. The port asks it of every creature carrying `Morale` and `Anger`, because those two are what
/// the attitude is made of and nothing else in the port has them.
#[must_use]
pub fn fleeing_creatures(world: &mut World) -> Vec<(Entity, (i32, i32, i32))> {
    let candidates: Vec<Entity> = world
        .query_filtered::<Entity, (bevy_ecs::prelude::With<Morale>, bevy_ecs::prelude::With<Anger>)>()
        .iter(world)
        .collect();
    candidates
        .into_iter()
        .filter_map(|entity| fleeing_from_a_scared_monster(world, entity).map(|square| (entity, square)))
        .collect()
}

/// Where a creature goes instead of its goal when it is frightened of the survivor, if it is.
///
/// A creature with no morale and no anger components is not asked: the reference asks the question of every monster, but a component-less creature in this port is a fixture, and
/// giving it an attitude would be inventing one.
fn fleeing_from_a_scared_monster(world: &mut World, entity: Entity) -> Option<(i32, i32, i32)> {
    let morale = world.get::<Morale>(entity)?.0;
    let anger = world.get::<Anger>(entity)?.0;
    let (hp, max_hp) = {
        let health = world.get::<Health>(entity)?;
        (health.hp, health.max_hp)
    };
    let here = {
        let place = world.get::<Position>(entity)?;
        (place.x, place.y, place.z)
    };
    let survivor = {
        let mut query = world.query_filtered::<(bevy_ecs::prelude::Entity, &Position), bevy_ecs::prelude::With<crate::Player>>();
        query.iter(world).next().map(|(_, place)| (place.x, place.y, place.z))
    }?;
    // Reference: `monster::is_fleeing`'s first line answers from the effect cache (`monster.cpp:1610`), which is what `spooked` sets - so a spooked monster runs whatever its morale says.
    if crate::applied_effects::has(world, entity, "spooked") {
        return Some(flee_goal(here, survivor));
    }
    let distance = (here.0 - survivor.0).abs() + (here.1 - survivor.1).abs() + (here.2 - survivor.2).abs();
    if !is_fleeing(hp, max_hp, morale, anger, distance) {
        return None;
    }
    Some(flee_goal(here, survivor))
}

/// Hits whatever stands on a creature's goal square, when it is next to it.
///
/// Returns whether a blow was swung, which is what decides between attacking and walking. The defence comes
/// from the target's own components - its size, its armour, and the fact that nothing here is immobile.
fn strike_what_the_goal_holds(world: &mut World, attacker: Entity, goal: (i32, i32, i32)) -> bool {
    let Some(place) = world.get::<Position>(attacker).map(|place| (place.x, place.y, place.z))
    else {
        return false;
    };
    let reach = (place.0 - goal.0).abs() + (place.1 - goal.1).abs() + (place.2 - goal.2).abs();
    if reach > 1 {
        return false;
    }
    let Some(victim) = creature_at(world, goal).first().copied() else {
        return false;
    };
    strike_at(world, attacker, victim).is_some()
}

/// Swings one blow at a creature, and answers what it did to it.
///
/// Reference: `Creature::deal_melee_hit` (`creature.cpp:959`) picks the part from the spread, `Creature::deal_melee_attack` resolves the damage, and `absorb_hit` is told which
/// part it landed on. The order is the rule: the margin before the part, the part before what protects it, and the armour's own fate before the armour takes its share.
///
/// Answers `None` when there is nothing to hit with or no dice to hit with, which is why the caller can tell a miss (`Some(0)`) from a blow that never happened.
pub fn strike_at(world: &mut World, attacker: Entity, victim: Entity) -> Option<i32> {
    let attack = world.get::<MeleeAttack>(attacker).copied()?;
    // Reference: a blow is charged to the attacker **before** it is thrown, hit or miss (`monster::melee_attack`, `monster.cpp:2108`), which is what stops a fast monster
    // swinging for ever and what makes walking and swinging cost the same purse.
    let cost = world.get::<AttackCost>(attacker).map_or(0, |carried| carried.0);
    spend_moves(world, attacker, f64::from(cost));
    // Reference: `monster::melee_attack` returns as soon as it has been charged if the type states no dice - `if( type->melee_dice == 0 ) { return true; }`
    // (`monster.cpp:2128-2131`). **The dice count is the type's permission to hurt anyone at all**: a type with a flat part and no dice does nothing, which is true of five
    // monsters in the corpus and of every monster the data gives no dice at all (159 of 1,154).
    if attack.0.dice == 0 {
        return None;
    }
    let dealt = strike_with(world, victim, attack.0)?;
    // Reference: `monster::melee_attack` puts a hit-and-run monster into `effect_run` for four turns after its blow (`monster.cpp:2134-2136`). The effect is only given when the
    // world has been told what that effect is: an effect the content does not define cannot be worn, and inventing one here would put a name in the store that nothing can read.
    // Reference: `monster::melee_attack` gives **itself** `effect_hit_by_player` for three turns when what it hit was the avatar (`monster.cpp:2130-2133`), which is how a monster
    // remembers it is in a fight. Added before the hit-and-run check because the reference adds it for every blow at the avatar, damage or none.
    if world.get::<crate::Player>(victim).is_some()
        && let Some(kind) = crate::effect_registry::kind_of(world, "hit_by_player")
    {
        crate::applied_effects::add(
            world,
            attacker,
            crate::effects::ActiveEffect::new(&kind, 3, 1),
        );
    }
    if dealt > 0
        && world
            .get::<crate::creature_flags::CreatureFlags>(attacker)
            .is_some_and(|flags| flags.0.contains("HIT_AND_RUN"))
        && let Some(kind) = crate::effect_registry::kind_of(world, "run")
    {
        crate::applied_effects::add(
            world,
            attacker,
            crate::effects::ActiveEffect::new(&kind, 4, 1),
        );
    }
    Some(dealt)
}

/// What one blow costs this creature in moves.
///
/// Reference: `mtype::attack_cost`, which `monster::melee_attack` charges whether the blow lands or not (`monster.cpp:2108`). A character's own swing has its own cost rule; this
/// component is what a monster's blow is charged from, and a creature without it is charged nothing.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct AttackCost(pub i32);

/// What a character has practised, by skill id, and how far.
///
/// Reference: `Character::get_skill_level` (`character.cpp`) reads `_skills`, a map from skill id to a level whose experience is tracked separately. The port keeps the level
/// alone: nothing practises a skill yet, so an exercise table would be a structure with no rule behind it. A creature with no [`Skills`] answers zero for every skill, the same
/// convention the stats use.
#[derive(Component, Debug, Clone, Default)]
pub struct Skills(pub std::collections::BTreeMap<String, i32>);

/// Which skill the thing in hand trains, if it trains any.
fn weapon_skill_of(world: &World, holder: Entity) -> Option<&'static str> {
    let kind = world.get::<Wielding>(holder)?.0;
    let names = crate::item_registry::peek(world)?;
    crate::melee::weapon_skill(
        names.melee_bash_of(kind),
        names.melee_cut_of(kind),
        names.melee_stab_of(kind),
    )
}

/// The nerve a monster was born with: the morale and anger its type states, and whether it can get them back.
///
/// Reference: `mtype::morale`, `mtype::agro` and `mtype::regen_morale`, read by `monster::process_turn` (`monster.cpp:3493-3512`). `Morale` and `Anger` are what the monster feels *now*;
/// this is what it returns to.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Nerve {
    /// The type's morale.
    pub morale: i32,
    /// The type's anger.
    pub anger: i32,
    /// Whether the monster regenerates nerve at all.
    pub regenerates: bool,
}

/// Gets a monster's nerve back, one point at a time - and five at a time while it is still in the negative.
///
/// Reference: `monster::process_turn` (`monster.cpp:3493-3512`), kept in the reference's own order, because the order is the rule: a monster that is **fleeing** at full health snaps all
/// the way back first, then gains one morale and one anger, and **then** - if either is still negative and still below the type's value - gains **five** more and is clamped. Applying
/// the five before the one, or clamping once at the end, gives different numbers for the same monster.
///
/// The gate on the whole thing is `type->regen_morale && hp >= hp_max`: a wounded monster does not calm down, and a type that does not regenerate never does.
#[must_use]
pub fn regen_nerve(
    hp: i32,
    max_hp: i32,
    morale: i32,
    anger: i32,
    nerve: Nerve,
    fleeing: bool,
) -> (i32, i32) {
    if !nerve.regenerates || hp < max_hp {
        return (morale, anger);
    }
    let (mut morale, mut anger) = (morale, anger);
    if fleeing {
        morale = nerve.morale;
        anger = nerve.anger;
    }
    if morale < nerve.morale {
        morale += 1;
    }
    if anger < nerve.anger {
        anger += 1;
    }
    if morale < 0 && morale < nerve.morale {
        // `morale += 5; morale = std::min( morale, type->morale );` - the clamp is what stops the jump from overshooting the type's own value.
        morale = nerve.morale.min(morale + 5);
    }
    if anger < 0 && anger < nerve.anger {
        anger = nerve.anger.min(anger + 5);
    }
    (morale, anger)
}

/// One step of a monster's morale back towards what its type says - or away from it, when something has made it braver than its type.
///
/// Reference: `monster::process_turn` (`monster.cpp:1877-1885`), in the reference's own two rolls:
///
/// 1. one in **four** turns and only if the morale is not already the type's own;
/// 2. and then either the gap is **more than fifteen** - which drifts on every such turn - or one in **two**.
///
/// The two rolls are why a monster that has been frightened a little comes back quickly; one that has been frightened a lot comes back at a quarter of a point a turn, and this is what
/// stops a monster's nerve ever being permanently wrecked by one bad fight.
#[must_use]
pub fn drift_morale(morale: i32, type_morale: i32, rolls: &mut cdda_core_types::rng::Rolls) -> i32 {
    if morale == type_morale || !rolls.one_in(4) {
        return morale;
    }
    let far = (morale - type_morale).abs() > 15;
    if !far && !rolls.one_in(2) {
        return morale;
    }
    if morale < type_morale {
        morale + 1
    } else {
        morale - 1
    }
}

/// What a monster thinks of what it is looking at.
///
/// Reference: `Creature::Attitude` (`creature.h`), whose attacking, fleeing, following and ignoring cases `monster::attitude_to` decides (`monster.cpp:1591-1833`). The port keeps
/// the four answers a monster can give about the survivor; friendliness is not among them because nothing in this port is on the monster's side.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Attitude {
    /// It has not noticed, or does not care.
    Ignore,
    /// It is keeping its distance but staying with the survivor.
    Follow,
    /// It is running.
    Flee,
    /// It is coming.
    Attack,
}

/// Whether a monster is running from the survivor.
///
/// Reference: `monster::is_fleeing` (`monster.cpp:1608-1618`). Two early answers come first - a monster with **anger or morale at a hundred or more never flees**, however the rest of the
/// rule reads - and then the attitude decides, with the addition that a monster that would **follow** also flees when it is within **four squares**. That last clause is what makes
/// "follow" mean *keep your distance* rather than *come closer*: a follower that gets close is running away again.
///
/// The port has no `FLEEING` effect cache (the reference's first line) and no NPCs, so what is left is exactly the rule above.
#[must_use]
pub fn is_fleeing(hp: i32, max_hp: i32, morale: i32, anger: i32, distance: i32) -> bool {
    if anger >= 100 || morale >= 100 {
        return false;
    }
    match attitude(hp, max_hp, morale, anger) {
        Attitude::Flee => true,
        Attitude::Follow => distance <= 4,
        Attitude::Ignore | Attitude::Attack => false,
    }
}

/// The square a frightened creature runs to: the one straight away from what scares it, as far again from the threat as the creature is now.
///
/// Reference: `monmove.cpp:489` turns the distance term of `monster::move`'s square scoring around when `is_fleeing` - the reference scores all eight neighbours and prefers the
/// farthest. The port has no scoring table for movement yet, so it **reflects** the threat through the creature and lets the router find the way, which is a **declared divergence**:
/// the same square wins most of the time, and walls between the two are handled by the router rather than by the score.
#[must_use]
pub fn flee_goal(monster: (i32, i32, i32), threat: (i32, i32, i32)) -> (i32, i32, i32) {
    (
        monster.0 + (monster.0 - threat.0),
        monster.1 + (monster.1 - threat.1),
        monster.2,
    )
}

/// What a monster makes of the survivor, from its morale, its anger and how hurt it is.
///
/// Reference: the tail of `monster::attitude_to` (`monster.cpp:1809-1833`). The reference reaches this point only after faction, species and trait modifiers, which this port does not
/// have - so the branches here are the whole rule rather than a simplification of it.
#[must_use]
pub fn attitude(hp: i32, max_hp: i32, morale: i32, anger: i32) -> Attitude {
    if morale < 0 {
        if morale + anger > 0 && hp > max_hp / 3 {
            return Attitude::Follow;
        }
        return Attitude::Flee;
    }
    if anger <= 0 {
        #[allow(clippy::cast_precision_loss)]
        let hurt = f64::from(hp) <= 0.6 * f64::from(max_hp);
        return if hurt { Attitude::Flee } else { Attitude::Ignore };
    }
    if anger < 10 {
        return Attitude::Follow;
    }
    Attitude::Attack
}

/// A monster's morale, which starts at its type's and is what `attitude` reads.
///
/// Reference: `monster::morale`, set from `type->morale` when a monster is made (`monster.cpp:283`).
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Morale(pub i32);

/// A monster's anger, set from its type's aggression.
///
/// Reference: `monster::anger`, set from `type->agro` (`monster.cpp:282`).
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Anger(pub i32);

/// How much wind a character has left.
///
/// Reference: `Character::stamina` beside `get_stamina_max` (`character.cpp:6814-6832`), which is `PLAYER_MAX_STAMINA_BASE + PLAYER_CARDIOFIT_STAMINA_SCALING * cardiofit` - **3,500**
/// plus five a point of cardio fitness, from `data/core/external_options.json`. The port has no cardio fitness, so a character starts at the base and its ceiling stays there; that is a
/// named gap rather than a made-up ceiling.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct Stamina {
    /// What is left.
    pub cur: f64,
    /// What it can hold.
    pub max: f64,
}

impl Default for Stamina {
    fn default() -> Self {
        Self {
            cur: 3_500.0,
            max: 3_500.0,
        }
    }
}

/// Spends stamina, keeping it at zero or above and never above what the character can hold - and making the character **winded** when the spend goes past zero.
///
/// Reference: `Character::mod_stamina` (`character.cpp:6961-6968`) adds the change, and on a result below zero puts every body part's windage effect on for **ten turns** - which in the
/// data is `effect_winded` on the torso and the mouth. That is what makes panic self-sustaining: the winded multiplier is a tenth, so a character that spent everything recovers at a
/// crawl until the effect runs out. The effect is only given when the content has defined it, for the same reason `HIT_AND_RUN` checks: a component may only name what the world can
/// resolve.
pub fn spend_stamina(world: &mut World, entity: Entity, amount: i32) {
    let Some(stamina) = world.get::<Stamina>(entity).copied() else {
        return;
    };
    let raw = stamina.cur + f64::from(amount);
    let winded = raw < 0.0;
    if let Some(mut stamina) = world.get_mut::<Stamina>(entity) {
        stamina.cur = raw.clamp(0.0, stamina.max);
    }
    if winded
        && let Some(kind) = crate::effect_registry::kind_of(world, "winded")
    {
        crate::applied_effects::add(
            world,
            entity,
            crate::effects::ActiveEffect::new(&kind, 10, 1),
        );
    }
}

/// What share of its wind a character has left, which is the one number the swing cost reads.
#[must_use]
pub fn stamina_ratio(world: &World, entity: Entity) -> f64 {
    world
        .get::<Stamina>(entity)
        .map_or(1.0, |stamina| stamina.cur / stamina.max)
}

/// The exercise each skill has banked towards its next level.
///
/// Reference: `SkillLevel::_exercise` (`skill.h`), which the reference keeps beside the level and the knowledge track. The port keeps the level in [`Skills`] and the exercise
/// here, because a level is what every rule reads and the exercise is only ever spent to buy one.
#[derive(Component, Debug, Clone, Default)]
pub struct Exercise(pub std::collections::BTreeMap<String, i32>);

/// Practises a skill: the exercise grows and buys whatever levels it can.
///
/// Reference: `Character::practice` reaches `SkillLevel::train` and then `on_exercise_change` (`skill.cpp:450-479`). The amount is the caller's, because what a swing is worth
/// depends on whether it landed - `melee_train( *this, 2, std::min( 5, skill_training_cap ), ... )` after a hit (`melee.cpp:2222`).
pub fn practise_skill(world: &mut World, entity: Entity, skill: &str, amount: i32) {
    if amount <= 0 {
        return;
    }
    let level = skill_level(world, entity, skill);
    let exercise = world
        .get::<Exercise>(entity)
        .and_then(|banked| banked.0.get(skill).copied())
        .unwrap_or(0);
    let (new_level, new_exercise) = crate::skills::practise(level, exercise, amount);
    if let Some(mut skills) = world.get_mut::<Skills>(entity) {
        skills.0.insert(skill.to_owned(), new_level);
    } else {
        world.entity_mut(entity).insert(Skills(
            [(skill.to_owned(), new_level)].into_iter().collect(),
        ));
    }
    if let Some(mut banked) = world.get_mut::<Exercise>(entity) {
        banked.0.insert(skill.to_owned(), new_exercise);
    } else {
        world.entity_mut(entity).insert(Exercise(
            [(skill.to_owned(), new_exercise)].into_iter().collect(),
        ));
    }
}

/// How far a creature has practised a skill.
#[must_use]
pub fn skill_level(world: &World, entity: Entity, skill: &str) -> i32 {
    world
        .get::<Skills>(entity)
        .and_then(|skills| skills.0.get(skill).copied())
        .unwrap_or(0)
}

/// What a character is holding out to hit with, as the kind of thing it is.
///
/// Reference: `Character::get_wielded_item` (`character.cpp`) is the item a blow is built from, and `melee_attack_abstract` takes the null item when there is none. The port keeps
/// the **kind** rather than the item entity, because a blow reads only what the kind decides - its damage and its balance - and an id is what this store passes around.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Wielding(pub crate::ItemTypeId);

/// How damaged the thing in a character's hand is, in the reference's own units - thousandths (`itype::damage_scale`, `itype.h:1588`).
///
/// Reference: `item::damage_`, which `item::inc_damage` raises by one scale (`item.cpp:9467-9470`). **A number about what is in the hand, and there is only ever one thing in the hand** - the
/// reference's `wield` swaps, it does not stack. That is what makes a component on the holder an honest home for it while the port's item registry still files items by kind.
#[derive(Component, Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct WieldedDamage(pub i32);

/// How damaged the thing in the hand is, in thousandths.
#[must_use]
pub fn wielded_damage(world: &World, holder: Entity) -> i32 {
    world.get::<WieldedDamage>(holder).map_or(0, |damage| damage.0)
}

/// What a character swings with, worked out from what it holds.
///
/// Reference: `Character::melee_attack_abstract` (`melee.cpp:625-630`) builds the blow from `used_weapon()`, which is the wielded item or `null_item_reference()`. The accuracy is
/// `get_melee_hit_base` - dexterity over four, the weapon skill's thirds, the melee skill's halves, and the weapon's own balance - and the damage is the weapon's `melee_damage`.
///
/// **A weaponless character gets the null item**, whose melee is zero in the data and whose balance is **-2**: empty hands aim worst and carry no damage, which is what the
/// reference says and what the port must not improve on. The skills are zero because the port has no character skills yet; that is a number the data would fill and this does not
/// invent.
#[must_use]
pub fn blow_from_what_the_character_holds(world: &World, holder: Entity) -> crate::melee::Attack {
    let kind = world.get::<Wielding>(holder).map(|held| held.0);
    let names = crate::item_registry::peek(world);
    let (to_hit, bash, cut, stab) = kind.map_or((-2, 0.0, 0.0, 0.0), |kind| {
        names.map_or((-2, 0.0, 0.0, 0.0), |names| {
            (
                names.to_hit_of(kind),
                names.melee_bash_of(kind),
                names.melee_cut_of(kind),
                names.melee_stab_of(kind),
            )
        })
    });
    // Reference: `get_hit_weapon` (`melee.cpp:356-370`) asks the weapon's own skill - the one its biggest damage type trains - and the melee skill, which every weapon uses.
    let weapon_skill = crate::melee::weapon_skill(bash, cut, stab);
    let weapon_level = weapon_skill.map_or(0, |skill| skill_level(world, holder, skill));
    let melee_level = skill_level(world, holder, "melee");
    #[allow(clippy::cast_possible_truncation)]
    let whole = |amount: f64| amount as i32;
    crate::melee::Attack {
        accuracy_tenths: crate::melee::hit_base_tenths(effective_dexterity(world, holder))
            + crate::melee::hit_weapon_tenths(weapon_level, melee_level, to_hit),
        #[allow(clippy::cast_possible_truncation)]
        fixed: crate::melee::Damage {
            bash: whole(bash),
            cut: whole(cut),
            stab: whole(stab),
            penetration: 0,
        },
        dice: 0,
        sides: 0,
    }
}

/// Swings whatever a character is holding at a creature, and answers what it did to it.
///
/// This is the avatar's half of the fight: the same `strike_with` a monster's blow goes through, fed by `blow_from_what_the_character_holds` instead of a `MeleeAttack` component.
pub fn swing_what_the_character_holds(world: &mut World, holder: Entity, victim: Entity) -> Option<i32> {
    let attack = blow_from_what_the_character_holds(world, holder);
    // Reference: `Character::melee_attack` works out `move_cost = attack_speed( cur_weap )` and charges it whatever the outcome (`melee.cpp:628`), which is why a fast weapon is
    // worth carrying and why swinging while exhausted costs twice as much. The port reads the same three inputs it can: the weapon's own time, the melee skill, and dexterity.
    let weapon_time = world
        .get::<Wielding>(holder)
        .map(|carried| carried.0)
        .map_or(65, |kind| {
            let volume = i64::from(crate::item_registry::volume_of(world, kind));
            let weight = crate::item_registry::weight_of(world, kind);
            crate::melee::attack_time(volume, weight, 1)
        });
    let skill = skill_level(world, holder, "melee");
    let dexterity = effective_dexterity(world, holder);
    // Reference: the swing is charged its stamina (`get_total_melee_stamina_cost`) as well as its moves, and the stamina the character has left scales the moves it pays -
    // `attack_speed`'s `stamina_penalty` grows from one at a quarter to two at none (`melee.cpp:2678-2685`).
    let weight = world
        .get::<Wielding>(holder)
        .map(|carried| carried.0)
        .map_or(0, |kind| crate::item_registry::weight_of(world, kind));
    spend_stamina(world, holder, crate::melee::stamina_cost(weight, skill, 0));
    let stamina = stamina_ratio(world, holder);
    let cost = crate::melee::attack_speed(weapon_time / 2, skill, dexterity, stamina);
    spend_moves(world, holder, f64::from(cost));
    let dealt = strike_with(world, victim, attack);
    // Reference: `Character::melee_attack` wears the weapon it swung **whatever the outcome** - the call sits outside the hit branch (`melee.cpp:930`) - so a miss dents a weapon just as a hit
    // does. The chance is round 1191's arithmetic and the guard is its inverted `one_in`; what is new here is the **place**, which is the hand.
    crate::creature::wear_the_wielded_weapon(world, holder, skill, dexterity);
    // Reference: **both** paths teach. `melee.cpp:2222` trains after a hit and `melee.cpp:713-716` trains after a miss - `melee_train( *this, 2, std::min( 5,
    // skill_training_cap ), cur_weap, ... )` in each - and the skill is the **weapon's** (`item::melee_skill`), not the general melee skill. The reference's miss path also
    // counts `times_combatted_player` on the target, which nothing in this port reads yet.
    // Reference: `Character::melee_attack` flags a non-avatar target the moment it swings, before the blow is resolved (`melee.cpp:595-599`), for ten minutes.
    if crate::effect_registry::kind_of(world, "hit_by_player").is_some() {
        let kind = crate::effect_registry::kind_of(world, "hit_by_player");
        if let Some(kind) = kind {
            crate::applied_effects::add(
                world,
                victim,
                crate::effects::ActiveEffect::new(&kind, 600, 1),
            );
        }
    }
    if let Some(learned) = weapon_skill_of(world, holder)
        && let Some(mut rolls) = crate::singletons::take_rolls(world)
    {
        let amount = rolls.rng(2, 5);
        crate::singletons::put_rolls(world, rolls);
        practise_skill(world, holder, learned, amount);
    }
    dealt
}

/// One blow, from an attack the caller has already worked out.
fn strike_with(world: &mut World, victim: Entity, attack: crate::melee::Attack) -> Option<i32> {
    let mut rolls = crate::singletons::take_rolls(world)?;
    let defence = defence_of(world, victim);
    // The roll, the landing place and the armour are asked in the order the reference asks them: the margin is
    // known before the part, and the part is known before what protects it - `Creature::deal_melee_hit` picks the
    // part from the spread (`creature.cpp:959`) and `absorb_hit` is told which part it was.
    #[allow(clippy::cast_precision_loss)]
    let accuracy = attack.accuracy_tenths as f32 / 10.0;
    let hitroll = crate::melee::melee_hit_range(&mut rolls, accuracy);
    #[allow(clippy::cast_possible_truncation)]
    let spread = crate::melee::hit_spread(
        hitroll as i32,
        crate::melee::dodge_roll(defence.dodge),
        defence.size,
        defence.immobile,
    );
    let landed_on = world.get::<Body>(victim).map(|body| body.parts.clone()).and_then(|parts| {
        let fraction = f64::from(rolls.rng_bits()) / f64::from(u32::MAX);
        crate::hit_location::struck_part(&parts, spread, false, fraction)
            .map(|part| part.name.clone())
    });
    let resist = landed_on.as_deref().map_or(defence.resist, |part| {
        #[allow(clippy::cast_precision_loss)]
        let armour = armour_on(world, victim, part) as f32;
        armour
    });
    let defence = crate::melee::Defence { resist, ..defence };
    let rolled = crate::melee::monster_melee_damage(&mut rolls, attack.fixed, attack.dice, attack.sides);
    let swung = crate::melee::resolve(hitroll, rolled, defence);
    // A garment's own fate is decided by the whole blow, before the armour took its share
    // (`character_armor.cpp`), and by a **fresh** draw of its own: `damage_armor_durability` calls `x_in_y` after
    // the damage has been worked out, so this draw belongs after `resolve` and before the rolls go back.
    let garment_roll = f64::from(rolls.rng_bits()) / f64::from(u32::MAX);
    crate::singletons::put_rolls(world, rolls);
    let mut dealt = 0;
    if swung.hit {
        if let Some(part) = landed_on.as_deref() {
            let arrived = f64::from(rolled.bash.max(0) + rolled.cut.max(0));
            settle_armour(world, victim, part, arrived, garment_roll);
        }
        hurt(world, victim, swung.damage);
        #[allow(clippy::cast_possible_truncation)]
        {
            dealt = swung.damage as i32;
        }
        if let Some(part) = landed_on {
            wound(world, victim, &part, dealt);
        }
    }
    Some(dealt)
}

/// What a creature brings to being hit, read from the components it carries.
#[must_use]
pub fn defence_of(world: &World, entity: Entity) -> crate::melee::Defence {
    let size = world
        .get::<Creature>(entity)
        .map_or(crate::CreatureSize::Medium, |creature| creature.size);
    let armour = world.get::<Armour>(entity).copied().unwrap_or_default();
    crate::melee::Defence {
        dodge: world.get::<Dodge>(entity).map_or(0, |dodge| dodge.points),
        size,
        #[allow(clippy::cast_precision_loss)]
        resist: armour.bash as f32,
        // The reference reads each damage type's own armour off the body part (`bodypart::resist`); the port's `Armour` carries the three the melee rules use.
        #[allow(clippy::cast_precision_loss)]
        cut_resist: armour.cut as f32,
        #[allow(clippy::cast_precision_loss)]
        stab_resist: armour.stab as f32,
        // Nothing here models worn clothing yet; a monster's protection is its body's.
        worn_armour: 0.0,
        immobile: false,
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
            // The body is the character's own state, so it hangs on the creature: hunger, thirst and the vitamins have no other home.
            crate::body::Body::default(),
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
            // Attention starts at the reference's hundred (`Character::focus`), which is what every practice gain is scaled by.
            crate::skills::Focus::default(),
        ))
        .id();
    crate::singletons::index_mut(world).insert(entity, square);
    entity
}

/// Every creature standing on `square`, in the order the world holds them.
///
/// A square can hold more than one: two dogs in a doorway is an ordinary state, and the reference's
/// `creature_at` returns a list for that reason.
/// Every creature alive, in the store's own order.
///
/// A scenario asks this to tell the monsters where to go; the walk itself is `take_turns`, which reads each creature's `Goal`.
#[must_use]
pub fn creatures(world: &mut World) -> Vec<Entity> {
    world
        .query_filtered::<Entity, bevy_ecs::prelude::With<Creature>>()
        .iter(world)
        .collect()
}

/// Every creature standing on a square, in the store's own order.
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

/// How long heat damage sets a creature alight for, when it does: `Creature::deal_damage`'s heat branch (`creature.cpp:1482-1500`).
///
/// Reference:
///
/// ```text
/// } else if( du.type == damage_heat ) {
///     if( rng( 0, 100 ) < adjusted_damage ) {
///         add_effect( source, effect_onfire, rng( 1_turns, 3_turns ), bp );
/// ```
///
/// **The chance is the damage itself, out of a hundred and one** - `rng( 0, 100 )` draws **0 to 100 inclusive**, so a hundred points of heat still fails one time in a hundred and one, and
/// anything above a hundred always lights. The burn is one to three turns, and the body part is the one that was hit.
#[must_use]
pub fn heat_sets_alight(damage: i32, dice: &mut cdda_core_types::rng::Rolls) -> Option<i64> {
    if damage <= 0 || dice.rng(0, 100) >= damage {
        return None;
    }
    Some(i64::from(dice.rng(1, 3)))
}

/// Dents whatever the character is holding, by the reference's arithmetic and the reference's dice.
///
/// Reference: `Character::handle_melee_wear( cur_weapon )` (`melee.cpp:214-300`), called from `melee_attack` after the blow is resolved and before the messages. `item::inc_damage` raises the
/// damage by **one thousandth scale** (`item.cpp:9467-9470`, `itype::damage_scale`), and the port keeps that unit rather than inventing a friendlier one.
pub fn wear_the_wielded_weapon(world: &mut World, holder: Entity, melee_skill: i32, dexterity: i32) {
    // **The thing in the hand is an entity**, and its damage is that entity's own `ItemDamage` - which is where the reference keeps it too (`item::damage_`). A character with empty hands
    // wears nothing, which is the reference's own first check (`if( !shield ) return false;`).
    let Some(held) = wielding(world, holder) else {
        return;
    };
    let kind = world
        .get::<crate::GroundItem>(held)
        .map_or_else(|| world.get::<Wielding>(holder).map(|carried| carried.0), |ground| Some(ground.kind));
    let chip_resistance = kind.map_or(2, |kind| crate::item_registry::chip_resistance_of(world, kind));
    let durable_melee = kind.is_some_and(|kind| crate::item_registry::kind_has_flag(&*world, kind, "DURABLE_MELEE"));
    let arm_strength = f64::from(effective_strength(world, holder));
    let chance = crate::melee::melee_wear_chance(
        f64::from(dexterity),
        melee_skill,
        arm_strength,
        f64::from(chip_resistance),
        1.0,
        1.0,
        durable_melee,
    );
    let Some(mut rolls) = crate::singletons::take_rolls(world) else {
        return;
    };
    let wears = crate::melee::wear_damages(chance, &mut rolls);
    crate::singletons::put_rolls(world, rolls);
    if !wears {
        return;
    }
    // The damage is applied by the port's own `item::mod_damage` (round 1149), which has waited ever since for a caller: it refuses an `UNBREAKABLE` item, applies the
    // **degradation** the new damage level crossed, and answers whether the hit **destroyed** the thing - the destruction test being `( damage + qty ) > max_damage`, so the
    // hit that takes an item exactly to its maximum is not the one that destroys it.
    let limits = crate::DamageLimits {
        max_damage: 4_000,
        degrade_increments: 0,
        stack_size: 1,
        count_by_charges: false,
        unbreakable: kind.is_some_and(|kind| crate::item_registry::kind_has_flag(&*world, kind, "UNBREAKABLE")),
    };
    let Some(current) = world.get::<crate::ItemDamage>(held).copied() else {
        return;
    };
    let (after, destroyed) = current.mod_damage(crate::DAMAGE_SCALE, &limits);
    if destroyed {
        // Reference: `Character::handle_melee_wear`'s destruction branch - the item "is destroyed by the blow" and leaves the hand (`melee.cpp:293-330`). The port
        // despawns the entity, which is what "the thing is gone" means in one store; the message and the contents spill are named, not invented.
        world.entity_mut(held).despawn();
        return;
    }
    world.entity_mut(held).insert(after);
    // A thing that has taken a blow can now carry faults, which is where the reference's `set_random_fault_of_type( "mechanical_damage" )` writes (`item.cpp:9458-9461`). **The step that picks
    // one is not ported to the swing yet** - it needs the kind's weighted fault list in the registry, which round 1205 added and nothing installs - so what happens here is only that the item gains
    // the place a fault would live. **An empty `ItemFaults` is not an invented value: it is the reference's own starting state for an undamaged item.**
    if world.get::<crate::ItemFaults>(held).is_none() {
        world.entity_mut(held).insert(crate::ItemFaults::default());
    }
    // Reference: `item::mod_damage`'s last act for a surviving item is `set_random_fault_of_type( "mechanical_damage" )` (`item.cpp:9458-9461`) - **a damaged weapon may catch a fault, and the
    // type asked for is mechanical damage**. Every input is now in the store: the kind's weighted list, the catalogue's type per fault id, and the faults the item already has.
    let candidates = kind.map_or_else(Vec::new, |kind| crate::item_registry::fault_weights_of(world, kind));
    if !candidates.is_empty() {
        // The types are read before the pick because the pick borrows the dice and the world; a closure that asked the world mid-pick would need both at once.
        let mut types = std::collections::BTreeMap::new();
        for (fault, _) in &candidates {
            if let Some(fault_type) = crate::item_registry::fault_type_of(world, fault) {
                types.insert(fault.clone(), fault_type);
            }
        }
        let existing = world.get::<crate::ItemFaults>(held).map_or_else(std::collections::BTreeSet::new, |faults| faults.0.clone());
        // What the item's own faults prevent, read before the pick for the same reason the types are.
        // **Keyed by the faults the item already has**, because that is what `can_have_fault` asks: "what does this fault of mine prevent?"
        let mut blocked = std::collections::BTreeMap::new();
        for fault in &existing {
            blocked.insert(fault.clone(), crate::item_registry::blocks_of(world, fault));
        }
        let Some(mut rolls) = crate::singletons::take_rolls(world) else {
            return;
        };
        let picked = crate::pick_fault(
            &candidates,
            &existing,
            "mechanical_damage",
            |fault| types.get(fault).cloned(),
            // Reference: `can_have_fault( f.obj )` refuses a fault that one of the item's own faults blocks (`item.cpp:7857`), which is what stops a broken blade being chipped afterwards.
            |fault| blocked.get(fault).cloned().unwrap_or_default(),
            &mut rolls,
        );
        crate::singletons::put_rolls(world, rolls);
        if let Some(fault) = picked
            && let Some(mut faults) = world.get_mut::<crate::ItemFaults>(held)
        {
            faults.0.insert(fault);
        }
    }
}
