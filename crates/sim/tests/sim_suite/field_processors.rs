//! The two processors that need no subsystem at all.
//!
//! Reference: `field_processor_extra_radiation` and `field_processor_wandering_field` (`src/map_field.cpp:664-690`). Between them they are nine lines of C++ and they
//! were the last two processors in the ledger whose data the port already had - everything else still waiting needs monsters, items or the fungal and radiation grids
//! to exist first. Both are transcribed here rather than paraphrased, because both have a detail that a paraphrase loses:
//!
//! * the radiation processor draws **nothing at all** when the level's maximum is zero, which is what keeps a non-radiating field out of the RNG stream;
//! * the wandering processor **adds to** a target field that is already as strong as the vent, rather than leaving it alone - the `else` branch is `add_field`, not
//!   `continue`.

use bevy_ecs::prelude::*;
use cdda_core_types::field::FieldId;
use cdda_core_types::rng::Rolls;
use cdda_sim::radiation::Radiation;
use cdda_sim::{Field, Position, add_field_strength, process_extra_radiation, process_wandering_field};

const NUKE_GAS: FieldId = FieldId(84);
const SMOKE_VENT: FieldId = FieldId(92);
const SMOKE: FieldId = FieldId(91);

/// How much radiation a square holds, zero when it holds none.
fn radiation(world: &World, square: (i32, i32, i32)) -> i32 {
    world.resource::<Radiation>().get(square)
}

/// The intensity of a field on a square, zero when the square has no field of that kind.
fn intensity(world: &mut World, square: (i32, i32, i32), id: FieldId) -> i32 {
    let mut query = world.query::<(&Position, &Field)>();
    query
        .iter(world)
        .find(|(at, field)| (at.x, at.y, at.z) == square && field.id == id)
        .map_or(0, |(_, field)| field.entry.intensity())
}

#[test]
fn a_radiating_field_adds_what_it_rolls_and_a_field_that_does_not_draws_nothing() {
    // **The `extra_radiation_max > 0` guard.** `fd_nuke_gas` at intensity three rolls `rng(0, 3)`; at a level whose maximum is zero the processor returns before the
    // roll, so a square of ordinary fog costs the RNG stream nothing. A port that rolled first and multiplied by zero afterwards would produce the same radiation and a
    // different world, which is the kind of difference that only shows up when the stream is compared.
    let mut world = World::new();
    world.insert_resource(Radiation::new());
    add_field_strength(&mut world, (0, 0, 0), NUKE_GAS, 3);
    let mut rolls = Rolls::new(41);
    let drawn = rolls.rng(0, 3);
    let mut replay = Rolls::new(41);

    process_extra_radiation(&mut world, (0, 0, 0), 0, 3, &mut replay);
    assert_eq!(
        radiation(&world, (0, 0, 0)),
        drawn,
        "the square holds exactly what the processor rolled"
    );

    let mut quiet = Rolls::new(41);
    process_extra_radiation(&mut world, (1, 0, 0), 0, 0, &mut quiet);
    assert_eq!(radiation(&world, (1, 0, 0)), 0, "a level that radiates nothing adds nothing");
    assert_eq!(
        quiet.rng(0, 3),
        Rolls::new(41).rng(0, 3),
        "and draws nothing: the stream is where it was before the call"
    );
}

#[test]
fn radiation_accumulates_on_the_square_it_radiates() {
    // `adjust_radiation` **adds** to the submap's value (`src/map.cpp:4893-4904`) and the store is a plain `int` array (`submap.h:69`), with no clamp: two turns of
    // radiating are two additions, and the number is the square's total, not the last processor's opinion.
    let mut world = World::new();
    world.insert_resource(Radiation::new());
    let mut rolls = Rolls::new(9);
    for _ in 0..4 {
        process_extra_radiation(&mut world, (2, 3, 0), 0, 3, &mut rolls);
    }
    assert!(
        radiation(&world, (2, 3, 0)) > 0,
        "four rolls of 0 to 3 leave something behind"
    );
    // A neighbouring square is untouched: radiation is per square, not per field.
    assert_eq!(radiation(&world, (2, 4, 0)), 0);
}

#[test]
fn a_vent_wanders_its_gas_into_every_square_within_its_reach() {
    // `points_in_radius( p, intensity - 1 )` (`src/map_field.cpp:680`): a vent at intensity two covers the nine squares around and including itself, and each of them
    // gets the wandering field at the vent's intensity. Radius zero - intensity one - is the vent's own square alone.
    let mut world = World::new();
    add_field_strength(&mut world, (10, 10, 0), SMOKE_VENT, 2);
    process_wandering_field(&mut world, (10, 10, 0), 2, SMOKE);

    let mut covered = 0;
    for y in 9..=11 {
        for x in 9..=11 {
            assert_eq!(
                intensity(&mut world, (x, y, 0), SMOKE),
                2,
                "smoke at the vent's intensity on ({x}, {y})"
            );
            covered += 1;
        }
    }
    assert_eq!(covered, 9, "nine squares were checked");
    // One square further out is untouched, which is what makes the radius the intensity's own.
    assert_eq!(intensity(&mut world, (12, 10, 0), SMOKE), 0);
    assert_eq!(
        intensity(&mut world, (10, 10, 0), SMOKE_VENT),
        2,
        "the vent itself is still a vent"
    );
}

#[test]
fn a_weaker_wandering_field_is_strengthened_and_a_stronger_one_is_added_to() {
    // **The `else` branch is not `continue`.** A square holding smoke weaker than the vent's intensity gets one level (`mod_field_intensity`); a square holding smoke
    // *at least* as strong as the vent falls to `add_field`, which **sums** the strengths (`src/field.cpp:180-186`) and lets `set_field_intensity` clamp the total to the
    // type's own level count (`src/field.cpp:58-63`). That reads like a bug and is the reference's behaviour; a port that skipped the strong square would leave a vent
    // unable to thicken the fog it stands in.
    let mut world = World::new();
    add_field_strength(&mut world, (20, 20, 0), SMOKE_VENT, 2);
    add_field_strength(&mut world, (19, 20, 0), SMOKE, 1);
    add_field_strength(&mut world, (21, 20, 0), SMOKE, 2);

    process_wandering_field(&mut world, (20, 20, 0), 2, SMOKE);

    assert_eq!(
        intensity(&mut world, (19, 20, 0), SMOKE),
        2,
        "one level weak, so it gains one"
    );
    assert_eq!(
        intensity(&mut world, (21, 20, 0), SMOKE),
        3,
        "already as strong, so the vent's two are added - 2 + 2 - and the setter clamps the sum to the type's three levels"
    );
}

#[test]
fn a_vent_at_the_first_intensity_only_fills_its_own_square() {
    // The radius is `intensity - 1`, so the weakest vent reaches nowhere. This is the case that makes the subtraction necessary rather than decorative.
    let mut world = World::new();
    add_field_strength(&mut world, (30, 30, 0), SMOKE_VENT, 1);
    process_wandering_field(&mut world, (30, 30, 0), 1, SMOKE);
    assert_eq!(intensity(&mut world, (30, 30, 0), SMOKE), 1);
    assert_eq!(intensity(&mut world, (31, 30, 0), SMOKE), 0);
}

#[test]
fn a_turn_runs_both_processors_where_the_reference_puts_them() {
    // **The wiring, not just the functions.** The reference builds a type's processor list once (`processors_for_type`, `src/map_field.cpp:2292-2330`) and pushes
    // radiation before the wandering field; a turn then runs the list. A port whose processors are correct but never called would pass every test above and simulate
    // nothing.
    let mut world = World::new();
    world.insert_resource(Radiation::new());
    add_field_strength(&mut world, (5, 5, 0), SMOKE_VENT, 2);
    add_field_strength(&mut world, (40, 40, 0), NUKE_GAS, 3);
    let context = cdda_sim::TurnContext {
        now: 100,
        windpower: 0,
        wind_angle: 0,
        bubble: cdda_sim::Bubble {
            center: (0, 0),
            radius: 66,
        },
    };
    let mut rolls = Rolls::new(3);
    let summary = cdda_sim::process_field_turn(
        &mut world,
        &context,
        |id| cdda_sim::FieldTurnData {
            wandering_field: (id == SMOKE_VENT).then_some(SMOKE),
            // A floor of one so the gate does not depend on whether this seed's die came up zero: what is being tested is that the
            // processor runs inside the turn, not what the die said.
            extra_radiation: if id == NUKE_GAS { (1, 3) } else { (0, 0) },
            ..cdda_sim::FieldTurnData::default()
        },
        |_x, _y| cdda_sim::CellFacts::open(),
        &mut rolls,
    );
    assert!(summary.processed >= 2, "both fields were processed: {summary:?}");
    assert_eq!(
        intensity(&mut world, (6, 6, 0), SMOKE),
        2,
        "the vent wandered into the diagonal square"
    );
    assert!(
        radiation(&world, (40, 40, 0)) > 0,
        "the radiating field added radiation on its own square"
    );
}
