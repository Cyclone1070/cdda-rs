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
