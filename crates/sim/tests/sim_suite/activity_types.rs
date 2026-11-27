//! What an activity's moves are counted in, and what a neither-based one does to a turn.
//!
//! Reference: `based_on_type` (`activity_type.h:23-46`) and `player_activity::do_turn` (`player_activity.cpp:276-294`).

#![allow(clippy::expect_used)]

use bevy_ecs::prelude::World;

use cdda_sim::creature::Basis;

fn an_activity(
    id: &str,
    based_on: &str,
    can_resume: bool,
) -> cdda_content::activity_types::ActivityType {
    cdda_content::activity_types::ActivityType {
        id: id.to_owned(),
        verb: "waiting".to_owned(),
        based_on: based_on.to_owned(),
        can_resume,
        multi_activity: false,
        rooted: false,
    }
}

/// **The three answers, and no fourth.** `speed`, `time` and `neither` are the data's only values; an activity the data does not define has **no** answer rather than a default,
/// because a `Doing` naming something unknown is a broken save or a broken rule and counting its moves either way would hide it.
#[test]
fn an_activity_is_counted_in_what_its_data_says() {
    let mut world = World::new();
    cdda_sim::activity_types::set_activity_types(
        &mut world,
        vec![
            an_activity("ACT_SPEED", "speed", true),
            an_activity("ACT_TIME", "time", false),
            an_activity("ACT_NEITHER", "neither", false),
        ],
    );
    assert_eq!(
        cdda_sim::activity_types::basis_of(&world, "ACT_SPEED"),
        Some(Basis::Speed),
        "**speed is the creature's own moves**"
    );
    assert_eq!(
        cdda_sim::activity_types::basis_of(&world, "ACT_TIME"),
        Some(Basis::Time),
        "**time is a hundred a turn**"
    );
    assert_eq!(
        cdda_sim::activity_types::basis_of(&world, "ACT_NEITHER"),
        Some(Basis::Neither),
        "**neither is the actor's own spending**"
    );
    assert_eq!(
        cdda_sim::activity_types::basis_of(&world, "ACT_UNHEARD_OF"),
        None,
        "**an activity the data never defined has no answer**"
    );
}

/// **A neither-based activity is not finished by spending.** A multiple activity starts with nothing left to do - it looks for work - so a rule that took the creature's whole
/// turn and cleared the activity at zero would end every `ACT_MULTIPLE_*` on its first turn, before it found anything.
///
/// The test gives the creature a hundred moves and an activity of zero moves, spends a turn, and asserts all three: nothing came off the creature, the answer is "not finished",
/// and the activity is still there.
#[test]
fn a_neither_activity_is_left_to_its_actor() {
    use cdda_sim::creature::Doing;
    let mut world = World::new();
    let worker = cdda_sim::creature::spawn_creature(&mut world, "Vlad", (0, 0, 0));
    cdda_sim::creature::set_moves(&mut world, worker, 100);
    world.entity_mut(worker).insert(Doing {
        activity: "ACT_MULTIPLE_BUTCHER".to_owned(),
        moves_left: 0,
        basis: Basis::Neither,
        moves_total: 0,
        subject: None,
    });
    let finished = cdda_sim::creature::spend_a_turn_on(&mut world, worker);
    assert!(
        !finished,
        "**the actor ends a neither activity, not the counter**"
    );
    assert_eq!(
        cdda_sim::creature::moves_left(&world, worker),
        100,
        "**and none of the creature's moves were spent**"
    );
    assert!(
        world.get::<Doing>(worker).is_some(),
        "**the activity is still on the creature**"
    );
    // **The contrast that makes the rule a rule**: the same zero-move activity under `speed` finishes at once, because speed's rule is "spend what you have and count down".
    world.entity_mut(worker).insert(Doing {
        activity: "ACT_SPEED".to_owned(),
        moves_left: 0,
        basis: Basis::Speed,
        moves_total: 0,
        subject: None,
    });
    assert!(
        cdda_sim::creature::spend_a_turn_on(&mut world, worker),
        "**a speed activity at zero moves is over**"
    );
}
