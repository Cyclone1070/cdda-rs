//! What switching a mutation on does, and what it costs.
//!
//! Reference: `Character::activate_mutation`, `activate_cached_mutation` and `can_power_mutation` (`mutation.cpp:813-880`, `:411-419`). This is the rule behind the
//! `u_activate_trait` effect (`npctalk.cpp:3518-3525`): a mutation that is off, or whose charge has run out, **pays** the data's cost and starts its cooldown; one that is
//! already on merely spends a turn of what is left. A mutation the creature cannot afford never switches on at all, and the player is told so.
//!
//! Kept in its own file because it is the whole of the store's behaviour: one record per creature of what is switched on, and the arithmetic that changes it.

/// What a creature's record says about one mutation: whether it is switched on, and how many turns of charge it has left.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Switched {
    /// Whether it is switched on.
    pub powered: bool,
    /// Turns of charge left, which is what an already-on mutation spends instead of paying again.
    pub charge: i32,
}

/// The record after switching it on, which is the reference's own two branches.
///
/// ```text
/// if ( powered && charge > 0 ) charge -= 1
/// else { if ( cooldown > 0 ) charge = cooldown - 1; powered = true }
/// ```
///
/// `cooldown` is the mutation's own, in turns. A mutation with no cooldown therefore switches on and carries **no** charge at all, which is what the reference does: there is
/// nothing to spend, so every switch-on pays whatever the cost is.
#[must_use]
pub fn after_switching_on(before: Switched, cooldown: i32) -> Switched {
    if before.powered && before.charge > 0 {
        return Switched {
            powered: true,
            charge: before.charge - 1,
        };
    }
    Switched {
        powered: true,
        charge: if cooldown > 0 { cooldown - 1 } else { 0 },
    }
}

/// The reference's sleepiness level at which a mutation that costs sleepiness can no longer be paid for (`character.h:154`).
pub const EXHAUSTED: i32 = 575;

/// Which of the four currencies a mutation is paid in, and how much of it.
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, Copy)]
pub struct Affordability {
    /// Whether its cost is paid in hunger.
    pub hunger: bool,
    /// Whether its cost is paid in thirst.
    pub thirst: bool,
    /// Whether its cost is paid in sleepiness.
    pub sleepiness: bool,
    /// Whether its cost is paid in mana.
    pub mana: bool,
    /// How much of it the mutation costs.
    pub cost: i32,
}

/// Whether a creature can pay for one switch-on, from the figures the caller measured.
///
/// Reference: `can_power_mutation` (`mutation.cpp:411-419`): each currency is checked **only if the mutation is paid in it**, and each check is the reference's own - hunger below half the body's calories,
/// thirst at 260 or more, sleepiness at `EXHAUSTED`, and mana at `available <= cost`, which is the one that reads backwards until you see it means "exactly enough is not enough".
#[must_use]
#[allow(clippy::cast_precision_loss, clippy::cast_possible_truncation)]
pub fn can_power_mutation(
    cost: Affordability,
    kcal_percent: f32,
    thirst: i32,
    sleepiness: i32,
    mana_available: i32,
) -> bool {
    let hungry = cost.hunger && kcal_percent < 0.5;
    let thirsty = cost.thirst && thirst >= 260;
    let sleepy = cost.sleepiness && sleepiness >= EXHAUSTED;
    let out_of_mana = cost.mana && mana_available <= cost.cost;
    !(hungry || thirsty || sleepy || out_of_mana)
}

/// Takes the cost of one switch-on out of the body, in the currencies the mutation is paid in.
///
/// Reference: `activate_cached_mutation` (`mutation.cpp:846-855`), the `else` branch only - an already-on mutation spends a turn of charge and pays nothing, which is
/// [`after_switching_on`]'s other branch. The four currencies are the reference's: stored kilocalories, thirst, sleepiness and mana, and a mutation is charged in **exactly**
/// the ones its data names, so a mutation paid in hunger leaves thirst alone.
///
/// Mana is not paid here: the port has no mana store yet, and a charge taken from somewhere else would be an invented number rather than the reference's.
pub fn pay_for_switching_on(world: &mut bevy_ecs::prelude::World, creature: bevy_ecs::entity::Entity, cost: Affordability) {
    if cost.hunger
        && let Some(mut body) = world.get_mut::<crate::body::Body>(creature)
    {
        body.mod_stored_kcal(-i64::from(cost.cost));
    }
    if cost.thirst
        && let Some(mut body) = world.get_mut::<crate::body::Body>(creature)
    {
        let thirst = body.thirst.saturating_add(cost.cost);
        body.set_thirst(thirst);
    }
    if cost.sleepiness {
        crate::needs::mod_sleepiness(world, creature, cost.cost);
    }
}

/// Every mutation a creature has switched on, and how much charge each has left.
///
/// Reference: `Character::cached_mutations` (`mutation.cpp:813-819`), the per-character half of a mutation's state - the data says what a mutation is, and this says what this body is doing with it.
#[derive(Debug, Clone, Default, bevy_ecs::component::Component)]
pub struct SwitchedMutations(pub std::collections::BTreeMap<String, Switched>);

/// The record for one mutation, as it stands.
#[must_use]
pub fn state_of(world: &bevy_ecs::prelude::World, creature: bevy_ecs::entity::Entity, id: &str) -> Switched {
    world
        .get::<SwitchedMutations>(creature)
        .and_then(|held| held.0.get(id).copied())
        .unwrap_or_default()
}

/// Switches a mutation on: pays for it if it is off or spent, or spends a turn of its charge if it is already on, and refuses - with the reference's own warning - when the
/// creature cannot afford it.
///
/// Reference: `Character::activate_mutation` (`mutation.cpp:813-819`) over `activate_cached_mutation` (`:821-880`). Returns whether the mutation ended up switched on, which a
/// caller needs because a refusal is not a failure: the reference tells the player and returns.
///
/// The mutation's own `activated_eocs` are **not** run here: they are programs, and the port runs programs through the effect system rather than from inside a rule.
#[must_use]
pub fn switch_on(world: &mut bevy_ecs::prelude::World, creature: bevy_ecs::entity::Entity, id: &str) -> bool {
    let Some(data) = crate::mutation_data::holder(world) else {
        return false;
    };
    let Some(mutation) = data.get(id) else {
        return false;
    };
    let cost = Affordability {
        hunger: mutation.hunger,
        thirst: mutation.thirst,
        sleepiness: mutation.sleepiness,
        mana: mutation.mana,
        cost: mutation.cost,
    };
    let body = world.get::<crate::body::Body>(creature);
    // The body's own healthy weight, from the creature's clock when it has one; twenty thousand kilocalories when it does not, which is the figure the port already uses for a
    // creature nothing has told otherwise.
    let healthy = world
        .get::<crate::body::BodyClock>(creature)
        .map_or(20_000, |clock| clock.healthy_kcal);
    let kcal_percent = body.map_or(1.0, |body| body.kcal_percent(healthy));
    let thirst = body.map_or(0, |body| body.thirst);
    let sleepiness = crate::needs::sleepiness_of(world, creature);
    // The port has no mana store, so nothing is measured for it: a mutation paid in mana is affordable, which is the reference's answer for a creature with mana to spare.
    let mana = i32::MAX;
    if !can_power_mutation(cost, kcal_percent, thirst, sleepiness, mana) {
        // `You feel like using your %s would kill you!` (`mutation.cpp:830-835`), and **nothing changes**: no charge, no cost, no switch.
        crate::messages::push_kind(
            world,
            &format!("You feel like using your {id} would kill you!"),
            crate::messages::MessageKind::Warning,
        );
        return false;
    }
    let before = state_of(world, creature, id);
    let already_on = before.powered && before.charge > 0;
    if !already_on {
        pay_for_switching_on(world, creature, cost);
    }
    let after = after_switching_on(before, mutation.cooldown_turns);
    let mut entity = world.entity_mut(creature);
    let mut held = entity.get_mut::<SwitchedMutations>();
    if let Some(record) = held.as_mut() {
        record.0.insert(id.to_owned(), after);
    } else {
        let mut record = SwitchedMutations::default();
        record.0.insert(id.to_owned(), after);
        entity.insert(record);
    }
    // Reference: `activate_cached_mutation` (`mutation.cpp:867-880`) runs each `activated_eocs` entry, and the block sits **outside** the branch that spends the charge,
    // so every switch-on runs them. The reference refuses a non-activation program with a debug message and turns the mutation off; the port has no program types, so it
    // runs whatever the data names and says nothing.
    for program in &mutation.activated_eocs {
        crate::eoc_registry::run_program(world, creature, None, program, 0);
    }
    true
}
