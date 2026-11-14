//! What switching a mutation on does to the creature that wears it.
//!
//! Reference: `Character::activate_mutation` and `activate_cached_mutation` (`mutation.cpp:813-880`). A mutation that is already on does not pay again: it spends a
//! turn of its **charge**, and only an off or spent mutation pays the cost and starts a fresh cooldown.

#![allow(clippy::panic, clippy::expect_used, clippy::indexing_slicing)]

use bevy_ecs::prelude::*;
use cdda_sim::body::Body;
use cdda_sim::mutation_activation::{
    Affordability, Switched, after_switching_on, can_power_mutation,
};

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
    let first = after_switching_on(
        Switched {
            powered: false,
            charge: 0,
        },
        10,
    );
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
    let free = after_switching_on(
        Switched {
            powered: false,
            charge: 0,
        },
        0,
    );
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
/// Reference: `can_power_mutation` (`mutation.cpp:411-419`). A mutation that costs nothing in a currency is not checked in it at all, which is why a creature starving
/// to death can still use a mutation that is paid for in mana.
#[test]
fn a_mutation_the_creature_cannot_afford_does_not_switch_on() {
    let hunger_only = Affordability {
        hunger: true,
        thirst: false,
        sleepiness: false,
        mana: false,
        cost: 400,
    };
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

/// **Switching a mutation on takes the cost out of the body.**
///
/// Reference: `activate_cached_mutation` (`mutation.cpp:846-855`): when the mutation is off or spent, the cost is subtracted from whichever of the four the data names - `mod_stored_kcal( -cost )` for
/// hunger, `mod_thirst( cost )` for thirst, `mod_sleepiness( cost )` for sleepiness, `magic->mod_mana( -cost )` for mana. This gate covers the two the port has stores for, and asserts the
/// currency it is **not** paid in is left alone.
#[test]
fn switching_a_mutation_on_pays_its_cost_out_of_the_body() {
    fn world_with(body: Body) -> (World, Entity) {
        let mut world = World::new();
        let creature = world.spawn(body).id();
        (world, creature)
    }
    let mut hungry = Body::new(2_500);
    hungry.stored_calories = 40_000_000;
    hungry.set_thirst(100);
    let (mut world, creature) = world_with(hungry);
    let before = cdda_sim::body::body_of(&world, creature)
        .expect("the body")
        .stored_kcal();
    cdda_sim::mutation_activation::pay_for_switching_on(
        &mut world,
        creature,
        Affordability {
            hunger: true,
            thirst: false,
            sleepiness: false,
            mana: false,
            cost: 400,
        },
    );
    let after = cdda_sim::body::body_of(&world, creature).expect("the body");
    assert_eq!(
        after.stored_kcal(),
        before - 400,
        "**four hundred kilocalories, which is `mod_stored_kcal( -cost )`**"
    );
    assert_eq!(
        after.thirst, 100,
        "and the thirst it is not paid in does not move"
    );

    let mut thirsty = Body::new(2_500);
    thirsty.stored_calories = 40_000_000;
    thirsty.set_thirst(100);
    let (mut world, creature) = world_with(thirsty);
    cdda_sim::mutation_activation::pay_for_switching_on(
        &mut world,
        creature,
        Affordability {
            hunger: false,
            thirst: true,
            sleepiness: false,
            mana: false,
            cost: 30,
        },
    );
    let after = cdda_sim::body::body_of(&world, creature).expect("the body");
    assert_eq!(after.thirst, 130, "**thirst goes up by the cost**");
    assert_eq!(
        after.stored_kcal(),
        40_000,
        "and the calories it is not paid in do not move"
    );
}

/// **The whole switch, through the world: what it costs, what it remembers, and when it refuses.**
///
/// Reference: `Character::activate_mutation` (`mutation.cpp:813-819`) over `activate_cached_mutation` (`:821-880`). Three things are being pinned at once, and they are the three the
/// reference does in one call: the cost leaves the body the **first** time only, the record remembers the charge, and a creature that cannot pay is told so and left exactly as it was.
#[test]
fn switching_on_pays_once_remembers_the_charge_and_refuses_a_creature_that_cannot_afford_it() {
    fn world_with_body(stored_calories: i64) -> (World, Entity) {
        let mut costing = cdda_content::mutations::Mutation {
            cost: 400,
            hunger: true,
            cooldown_turns: 10,
            ..cdda_content::mutations::Mutation::default()
        };
        costing.valid = true;
        let mutations = cdda_content::mutations::Mutations::from_entries([(
            "FLAMING_EYES".to_owned(),
            costing,
        )]);
        let mut world = World::new();
        cdda_sim::mutation_data::install(
            &mut world,
            mutations,
            cdda_content::mutations::Categories::default(),
        );
        let mut body = Body::new(2_500);
        body.stored_calories = stored_calories;
        let creature = world.spawn(body).id();
        (world, creature)
    }

    let (mut world, creature) = world_with_body(40_000_000);
    assert!(
        cdda_sim::mutation_activation::switch_on(&mut world, creature, "FLAMING_EYES"),
        "a creature with calories to spare switches it on"
    );
    assert_eq!(
        cdda_sim::body::body_of(&world, creature)
            .expect("the body")
            .stored_kcal(),
        40_000 - 400,
        "**and pays for it once**"
    );
    assert_eq!(
        cdda_sim::mutation_activation::state_of(&world, creature, "FLAMING_EYES"),
        Switched {
            powered: true,
            charge: 9
        },
        "with nine turns of a ten-turn cooldown left"
    );
    assert!(
        cdda_sim::mutation_activation::switch_on(&mut world, creature, "FLAMING_EYES"),
        "switching it on again while it is already on still succeeds"
    );
    assert_eq!(
        cdda_sim::body::body_of(&world, creature)
            .expect("the body")
            .stored_kcal(),
        40_000 - 400,
        "**but pays nothing more**: the charge is what an already-on mutation spends"
    );
    assert_eq!(
        cdda_sim::mutation_activation::state_of(&world, creature, "FLAMING_EYES"),
        Switched {
            powered: true,
            charge: 8
        },
        "it is eight turns left now"
    );

    // Half the body's healthy weight in calories is the reference's own limit, and a creature under it is refused.
    let (mut starved, creature) = world_with_body(9_000_000);
    assert!(
        !cdda_sim::mutation_activation::switch_on(&mut starved, creature, "FLAMING_EYES"),
        "**a creature under half its healthy calories cannot switch it on**"
    );
    assert_eq!(
        cdda_sim::body::body_of(&starved, creature)
            .expect("the body")
            .stored_kcal(),
        9_000,
        "and nothing is taken from it"
    );
    assert_eq!(
        cdda_sim::mutation_activation::state_of(&starved, creature, "FLAMING_EYES"),
        Switched::default(),
        "nor is anything recorded"
    );
    let said = cdda_sim::messages::lines(&starved);
    assert!(
        said.iter().any(|line| line.contains("would kill you")),
        "and the reference's own warning is said: {said:?}"
    );
}

/// **`u_activate_trait` runs the switch, on the creature the effect names.**
///
/// Reference: `f_activate_trait` (`npctalk.cpp:3518-3525`) reads a `str_or_var` and calls `d.actor( is_npc )->activate_mutation(...)`. The effect is used by
/// `EOC_wing_insect_activate` and its family, which is why the census was refusing a program the data actually runs.
#[test]
fn the_activate_trait_effect_switches_the_mutation_on() {
    let mut costing = cdda_content::mutations::Mutation {
        cost: 400,
        hunger: true,
        cooldown_turns: 10,
        ..cdda_content::mutations::Mutation::default()
    };
    costing.valid = true;
    let mutations =
        cdda_content::mutations::Mutations::from_entries([("WINGS_INSECT".to_owned(), costing)]);
    let mut world = World::new();
    cdda_sim::mutation_data::install(
        &mut world,
        mutations,
        cdda_content::mutations::Categories::default(),
    );
    let mut body = Body::new(2_500);
    body.stored_calories = 40_000_000;
    let creature = world.spawn(body).id();

    let effect = cdda_content::dialogue::Effect::ActivateTrait {
        trait_name: cdda_content::dialogue::StringSource::Literal("WINGS_INSECT".to_owned()),
        listener: false,
    };
    let applied = cdda_sim::dialogue_effects::run_effect(&mut world, creature, None, &effect);
    assert_eq!(
        applied,
        cdda_sim::dialogue_effects::Applied::Done,
        "the effect is answered"
    );
    assert_eq!(
        cdda_sim::mutation_activation::state_of(&world, creature, "WINGS_INSECT"),
        Switched {
            powered: true,
            charge: 9
        },
        "**and the mutation is switched on, through the effect**"
    );
    assert_eq!(
        cdda_sim::body::body_of(&world, creature)
            .expect("the body")
            .stored_kcal(),
        40_000 - 400,
        "paying for it out of the body the acting creature wears"
    );
}

/// **A mutation that names programs runs them when it is switched on, every time it is switched on.**
///
/// Reference: `activate_cached_mutation` (`mutation.cpp:867-880`) runs each `activated_eocs` entry after the charge is settled - and the block sits **outside** the
/// already-on branch, so every switch-on runs them, not only the one that paid.
#[test]
fn a_mutation_runs_its_own_programs_when_it_is_switched_on() {
    use cdda_content::dialogue::{Effect, StringSource};
    use cdda_sim::eoc_registry::Program;

    let mut costing = cdda_content::mutations::Mutation {
        cost: 400,
        hunger: true,
        cooldown_turns: 10,
        activated_eocs: vec!["EOC_TEST_ACTIVATED".to_owned()],
        ..cdda_content::mutations::Mutation::default()
    };
    costing.valid = true;
    let mutations =
        cdda_content::mutations::Mutations::from_entries([("FLAMING_EYES".to_owned(), costing)]);
    let mut world = World::new();
    cdda_sim::mutation_data::install(
        &mut world,
        mutations,
        cdda_content::mutations::Categories::default(),
    );
    let program = Program {
        id: "EOC_TEST_ACTIVATED".to_owned(),
        event: None,
        run_for_npcs: false,
        condition: None,
        effects: vec![Effect::Message {
            text: StringSource::Literal("Your eyes catch fire.".to_owned()),
            fallback: None,
        }],
    };
    cdda_sim::eoc_registry::set_programs(&mut world, &[program]);
    let mut body = Body::new(2_500);
    body.stored_calories = 40_000_000;
    let creature = world.spawn(body).id();

    assert!(
        cdda_sim::mutation_activation::switch_on(&mut world, creature, "FLAMING_EYES"),
        "it switches on"
    );
    assert_eq!(
        cdda_sim::messages::lines(&world),
        vec!["Your eyes catch fire.".to_owned()],
        "**and the program it names has run, once**"
    );
    assert!(cdda_sim::mutation_activation::switch_on(
        &mut world,
        creature,
        "FLAMING_EYES"
    ));
    assert_eq!(
        cdda_sim::messages::lines(&world),
        vec![
            "Your eyes catch fire.".to_owned(),
            "Your eyes catch fire.".to_owned()
        ],
        "**and again on the second switch-on**, because the reference's program block sits outside the branch that spends the charge"
    );
}

/// **`npc_emit` puts a field on the square a variable names, not on the speaker.**
///
/// Reference: `f_emit` (`npctalk.cpp:7740-7749`): the target is `d.actor( is_npc )->pos_abs()` unless a `target_var` names one, and the emission's own chance is scaled by
/// `chance_mult` before `here.emit_field` runs. The data uses exactly this shape to put a fog plume where a monster died (`EOC_YUGG_DEATH`).
#[test]
fn the_emit_effect_puts_a_field_where_the_variable_says() {
    use cdda_content::dialogue::{AssignmentTarget, Effect, StringSource};

    let mut world = World::new();
    // An emission is asked for a die per attempt, so the world needs one before anything can be poured.
    cdda_sim::singletons::put_rolls(&mut world, cdda_core_types::rng::Rolls::new(1));
    cdda_sim::field_registry::set_field_names(&mut world, &[("fd_smoke", 1, 1)]);
    cdda_sim::emit_registry::set_emits(
        &mut world,
        &[(
            "emit_fog_plume".to_owned(),
            vec![cdda_content::emit::Emission {
                field: "fd_smoke".to_owned(),
                intensity: 1,
                chance: 100,
                qty: 1,
            }],
        )],
    );
    // `context_val` is held by the speaker itself (`holder_for_scope`), which is how a program carries a square from one entry to the next.
    let actor = world.spawn(cdda_sim::Position { x: 0, y: 0, z: 0 }).id();
    cdda_sim::dialogue_values::set(
        &mut world,
        actor,
        "death_loc",
        cdda_sim::dialogue_values::Value::Position { x: 5, y: 7, z: 0 },
    );

    let effect = Effect::Emit {
        emit: StringSource::Literal("emit_fog_plume".to_owned()),
        chance_mult: None,
        target: Some(AssignmentTarget::Variable {
            scope: "context_val".to_owned(),
            name: "death_loc".to_owned(),
        }),
        listener: false,
    };
    let applied = cdda_sim::dialogue_effects::run_effect(&mut world, actor, None, &effect);
    assert_eq!(
        applied,
        cdda_sim::dialogue_effects::Applied::Done,
        "the effect is answered"
    );
    let smoke = cdda_sim::field_registry::identity_of(&world, "fd_smoke").expect("the field");
    assert!(
        cdda_sim::field_strength_at(&world, (5, 7, 0), smoke) > 0,
        "**the field is at the square the variable named**"
    );
    assert_eq!(
        cdda_sim::field_strength_at(&world, (0, 0, 0), smoke),
        0,
        "and not where the actor stands"
    );
}
