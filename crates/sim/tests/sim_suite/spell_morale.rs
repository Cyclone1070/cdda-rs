//! A `morale` spell: what it makes people feel, and who it can reach.
//!
//! Reference: `spell_effect::morale` (`magic_spell_effect.cpp:1503-1527`). Three facts are load-bearing and each has its own gate below: only **characters** feel it (a
//! monster has no morale), the type has to be one the data defines or the spell is refused whole, and the bonus, the duration and the fade are the spell's own three numbers.

#![allow(clippy::panic, clippy::expect_used, clippy::indexing_slicing)]

use bevy_ecs::prelude::*;

fn a_spell(kind: &str, bonus: f64, seconds: f64, area: f64) -> cdda_content::spells::Spell {
    let scaling = |value: f64| cdda_content::spells::Scaling {
        start: Some(value),
        increment: Some(0.0),
        end: Some(value),
    };
    cdda_content::spells::Spell {
        id: "gate".to_owned(),
        name: "Gate".to_owned(),
        effect_name: "morale".to_owned(),
        effect_str: kind.to_owned(),
        damage_type: String::new(),
        shape: "blast".to_owned(),
        energy_source: "NONE".to_owned(),
        damage: scaling(bonus),
        range: cdda_content::spells::Scaling::default(),
        area: scaling(area),
        energy: cdda_content::spells::Scaling::default(),
        duration: scaling(seconds),
        casting_time: cdda_content::spells::Scaling::default(),
        difficulty: None,
        max_level: None,
    }
}

/// A person and a monster standing side by side, with the data's morale types loaded.
fn a_world() -> (World, Entity, Entity) {
    let mut world = World::new();
    cdda_sim::morale::set_types(&mut world, &["morale_feeling_good".to_owned()]);
    let person = cdda_sim::spawn_player(&mut world, (0, 0), 80, 0);
    let monster = world.spawn(cdda_sim::Position { x: 0, y: 0, z: 0 }).id();
    world.entity_mut(monster).insert(cdda_sim::creature::Creature {
        name: "a zombie".to_owned(),
        size: cdda_sim::CreatureSize::Medium,
    });
    cdda_sim::PositionIndex::rebuild(&mut world);
    (world, person, monster)
}

#[test]
fn a_morale_spell_makes_a_person_feel_its_bonus_for_its_own_duration() {
    let (mut world, person, _) = a_world();
    // `AEA_FUN` is the data's own: `morale_feeling_good`, twenty points, thirty thousand seconds.
    let spell = a_spell("morale_feeling_good", 20.0, 30_000.0, 0.0);
    assert_eq!(
        cdda_sim::spell_effects::apply(&mut world, person, &spell, 0, (0, 0, 0)),
        cdda_sim::spell_effects::Effective::Done,
        "the spell was felt"
    );
    let felt = world
        .get::<cdda_sim::morale::Morale>(person)
        .expect("the point was added");
    assert_eq!(felt.0.len(), 1, "one point");
    let point = &felt.0[0];
    assert_eq!(point.kind, "morale_feeling_good", "of the type the spell names");
    assert_eq!(point.bonus, 20, "worth the spell's damage");
    assert_eq!(point.duration, 30_000, "for the spell's duration in turns");
    assert_eq!(
        point.decay_start, 3_000,
        "**and it starts to fade at a tenth of that**, which is the reference's own divisor (`:1524`)"
    );
}

/// **A monster has no morale, and a type the data does not define is not felt by anyone.**
///
/// Reference: `magic_spell_effect.cpp:1511-1522`: the type is checked against the morale registry and the spell returns without doing anything when it is unknown, and the
/// area walk only takes `Character`s.
#[test]
fn only_people_feel_it_and_only_a_type_the_data_defines() {
    let (mut world, person, monster) = a_world();
    let spell = a_spell("morale_feeling_good", 20.0, 30_000.0, 1.0);
    assert_eq!(
        cdda_sim::spell_effects::apply(&mut world, person, &spell, 0, (0, 0, 0)),
        cdda_sim::spell_effects::Effective::Done,
        "the blast covers both of them and the person feels it"
    );
    assert!(
        world.get::<cdda_sim::morale::Morale>(monster).is_none(),
        "**and the monster standing on the same square has no morale to move**"
    );

    let unknown = a_spell("morale_nobody_defined", 20.0, 30_000.0, 0.0);
    assert_eq!(
        cdda_sim::spell_effects::apply(&mut world, person, &unknown, 0, (0, 0, 0)),
        cdda_sim::spell_effects::Effective::NothingAffected,
        "a type the data does not define is refused whole"
    );
    let felt = world.get::<cdda_sim::morale::Morale>(person).expect("the first point");
    assert_eq!(
        felt.0.len(),
        1,
        "**and nothing was added for it**, so the person still feels exactly one thing"
    );
}
