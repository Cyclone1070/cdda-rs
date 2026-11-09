//! What switching a mutation on does to the creature that wears it.
//!
//! Reference: `Character::activate_mutation` and `activate_cached_mutation` (`mutation.cpp:813-880`). A mutation that is already on does not pay again: it spends a
//! turn of its **charge**, and only an off or spent mutation pays the cost and starts a fresh cooldown.

#![allow(clippy::panic, clippy::expect_used)]

use cdda_sim::mutation_activation::{Switched, after_switching_on};

/// **Switching on costs a cooldown's charge, and switching on again spends a turn of it instead.**
///
/// Reference: `activate_cached_mutation` (`mutation.cpp:838-861`):
///
/// ```text
/// if ( tdata.powered && tdata.charge > 0 ) { tdata.charge -= 1; }        // already on: just burn a turn
/// else {
///     if ( mdata.cooldown > 0 ) { tdata.charge = mdata.cooldown - 1; }   // off or spent: pay, and start the clock
///     ...
///     tdata.powered = true;
/// }
/// ```
///
/// So a mutation with a ten-turn cooldown that is switched on for the first time ends **on with nine turns of charge**, and switching it on again ends **on with eight** - the power cost is not
/// paid twice while the charge lasts.
#[test]
fn switching_a_mutation_on_starts_its_charge_and_switching_it_on_again_spends_a_turn() {
    let first = after_switching_on(Switched { powered: false, charge: 0 }, 10);
    assert_eq!(
        first,
        Switched {
            powered: true,
            charge: 9
        },
        "**the first switch-on pays and starts the clock at one turn less than the cooldown**"
    );
    let again = after_switching_on(first, 10);
    assert_eq!(
        again,
        Switched {
            powered: true,
            charge: 8
        },
        "and switching it on again spends a turn of what is left rather than paying again"
    );
    let free = after_switching_on(Switched { powered: false, charge: 0 }, 0);
    assert_eq!(
        free,
        Switched {
            powered: true,
            charge: 0
        },
        "a mutation with no cooldown carries no charge at all, and is simply on"
    );
}

/// **A mutation the creature cannot afford never switches on, and the four currencies are checked in the reference's own order.**
///
/// Reference: `can_power_mutation` (`mutation.cpp:411-419`):
///
/// ```text
/// hunger     = mutation.hunger     && kcal percent < 0.5
/// thirst     = mutation.thirst     && thirst >= 260
/// sleepiness = mutation.sleepiness && sleepiness >= EXHAUSTED
/// mana       = mutation.mana       && available mana <= mutation.cost
/// cannot power = hunger || sleepiness || thirst || mana
/// ```
///
/// A mutation that costs nothing in a currency is not checked in it at all, which is why a creature starving to death can still use a mutation that is paid for in mana. The caller answers with
/// what it is measuring - the reference reads the body's own figures, and the port reads them the same way, so the rule takes the numbers rather than the creature.
#[test]
fn a_mutation_the_creature_cannot_afford_does_not_switch_on() {
    use cdda_sim::mutation_activation::{Affordability, can_power_mutation};

    let hunger_only = Affordability {
        hunger: true,
        thirst: false,
        sleepiness: false,
        mana: false,
        cost: 400,
    };
    // Half the body's calories left is the boundary: `get_kcal_percent() < 0.5f` is a **strict** test, so exactly half is still affordable.
    assert!(
        can_power_mutation(hunger_only, 0.5, 0, 0, 0),
        "**half the calories is enough**, so hunger does not block it"
    );
    assert!(
        !can_power_mutation(hunger_only, 0.49, 0, 0, 0),
        "and a hair under half blocks it - the test is the reference's, not the port's"
    );

    let thirst_only = Affordability {
        hunger: false,
        thirst: true,
        sleepiness: false,
        mana: false,
        cost: 0,
    };
    assert!(
        !can_power_mutation(thirst_only, 1.0, 260, 0, 0),
        "**260 thirst blocks it**, which is the reference's own limit"
    );
    assert!(
        can_power_mutation(thirst_only, 1.0, 259, 0, 0),
        "and one under it does not"
    );

    let sleepy = Affordability {
        hunger: false,
        thirst: false,
        sleepiness: true,
        mana: false,
        cost: 0,
    };
    assert!(
        can_power_mutation(sleepy, 1.0, 0, 574, 0),
        "**exhaustion is 575** (`character.h:154`), so 574 is still allowed"
    );
    assert!(
        !can_power_mutation(sleepy, 1.0, 0, 575, 0),
        "and 575 blocks it"
    );

    let mana_only = Affordability {
        hunger: false,
        thirst: false,
        sleepiness: false,
        mana: true,
        cost: 400,
    };
    assert!(
        !can_power_mutation(mana_only, 0.0, 1_000, 0, 400),
        "**exactly enough mana is not enough**: the reference blocks at `available <= cost`"
    );
    assert!(
        can_power_mutation(mana_only, 0.0, 1_000, 0, 401),
        "one over it is"
    );

    let nothing = Affordability {
        hunger: false,
        thirst: false,
        sleepiness: false,
        mana: false,
        cost: 0,
    };
    assert!(
        can_power_mutation(nothing, 0.0, 1_000, 1_000, 0),
        "and a mutation that is paid for in nothing is never blocked, however starved or exhausted the creature is"
    );
}
