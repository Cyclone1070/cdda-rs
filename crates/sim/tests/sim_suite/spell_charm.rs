//! A spell that wins a monster over: `charm_monster`.
//!
//! Reference: `spell_effect::charm_monster` (`magic_spell_effect.cpp:1530-1549`). Two facts carry it: a monster is only a candidate when its hit points are **at or below**
//! the spell's damage, and the charm is an **addition** to how long it stays friendly - the spell's duration over a hundred - so charming twice lasts twice as long.

#![allow(clippy::panic, clippy::expect_used, clippy::indexing_slicing)]

use bevy_ecs::prelude::*;

fn a_spell(damage: f64, seconds: f64, area: f64) -> cdda_content::spells::Spell {
    let scaling = |value: f64| cdda_content::spells::Scaling {
        start: Some(value),
        increment: Some(0.0),
        end: Some(value),
    };
    cdda_content::spells::Spell {
        id: "gate".to_owned(),
        name: "Gate".to_owned(),
        effect_name: "charm_monster".to_owned(),
        effect_str: String::new(),
        damage_type: String::new(),
        shape: "blast".to_owned(),
        energy_source: "NONE".to_owned(),
        damage: scaling(damage),
        range: cdda_content::spells::Scaling::default(),
        area: scaling(area),
        energy: cdda_content::spells::Scaling::default(),
        duration: scaling(seconds),
        casting_time: cdda_content::spells::Scaling::default(),
        difficulty: None,
        max_level: None,
        valid_targets: ["hostile".to_owned()].into_iter().collect(),
        targeted_monster_ids: std::collections::BTreeSet::new(),
        targeted_species_ids: std::collections::BTreeSet::new(),
        ignored_species_ids: std::collections::BTreeSet::new(),
        flags: std::collections::BTreeSet::new(),
    }
}

#[test]
fn a_charm_adds_to_how_long_a_wounded_monster_stays_friendly() {
    let mut world = World::new();
    let caster = cdda_sim::spawn_player(&mut world, (0, 0), 80, 0);
    // A monster with thirty hit points against a spell that promises a hundred: it is hurt enough to be charmed.
    let monster = cdda_sim::spawn_monster(&mut world, (1, 0), cdda_sim::MonsterKindId(0), 30);
    cdda_sim::PositionIndex::rebuild(&mut world);
    // `AEA_ENTRANCE` is the content's spell, with a duration of thirty thousand seconds.
    let spell = a_spell(100.0, 30_000.0, 1.0);
    assert_eq!(
        cdda_sim::spell_effects::apply(&mut world, caster, &spell, 0, (1, 0, 0)),
        cdda_sim::spell_effects::Effective::Done,
        "the monster was charmed"
    );
    assert_eq!(
        cdda_sim::friendly::turns_of(&world, monster),
        300,
        "**thirty thousand seconds is three hundred turns**, which is the duration over the reference's hundred"
    );
    assert!(
        cdda_sim::friendly::is_friendly(&world, monster),
        "and it is on our side"
    );
}

/// **A monster with more hit points than the spell's damage shrugs it off**, and a friendly one is not re-charmed.
///
/// Reference: `:1543-1546`: the charm needs `get_hp() <= sp.damage( caster )`, and `mon->friendly == 0` unless the spell carries `RECHARM`. The content's own
/// `AEA_ENTRANCE` states a damage of **zero**, so it cannot charm anything still standing - which is the data's arithmetic and not a port decision.
#[test]
fn a_healthy_monster_resists_and_a_friendly_one_is_left_alone() {
    let mut world = World::new();
    let caster = cdda_sim::spawn_player(&mut world, (0, 0), 80, 0);
    let healthy = cdda_sim::spawn_monster(&mut world, (1, 0), cdda_sim::MonsterKindId(0), 30);
    cdda_sim::PositionIndex::rebuild(&mut world);
    let weak = a_spell(20.0, 10_000.0, 1.0);
    assert_eq!(
        cdda_sim::spell_effects::apply(&mut world, caster, &weak, 0, (1, 0, 0)),
        cdda_sim::spell_effects::Effective::NothingAffected,
        "**thirty hit points against twenty damage is not below the threshold**"
    );
    assert!(
        !cdda_sim::friendly::is_friendly(&world, healthy),
        "and it is still hostile"
    );

    // The content's own spell: a damage of zero against a monster with hit points left.
    let content = a_spell(0.0, 30_000.0, 8.0);
    assert_eq!(
        cdda_sim::spell_effects::apply(&mut world, caster, &content, 0, (1, 0, 0)),
        cdda_sim::spell_effects::Effective::NothingAffected,
        "**`AEA_ENTRANCE`'s zero damage charms nothing that is still alive**"
    );

    let strong = a_spell(100.0, 10_000.0, 1.0);
    assert_eq!(
        cdda_sim::spell_effects::apply(&mut world, caster, &strong, 0, (1, 0, 0)),
        cdda_sim::spell_effects::Effective::Done,
        "a strong enough charm takes it"
    );
    assert_eq!(
        cdda_sim::friendly::turns_of(&world, healthy),
        100,
        "for a hundred turns"
    );
    assert_eq!(
        cdda_sim::spell_effects::apply(&mut world, caster, &strong, 0, (1, 0, 0)),
        cdda_sim::spell_effects::Effective::NothingAffected,
        "**and a second charm leaves it alone**, because the port does not load the `RECHARM` flag that would let it stack"
    );
    assert_eq!(
        cdda_sim::friendly::turns_of(&world, healthy),
        100,
        "so the count has not moved"
    );
}

/// **A charm runs out, one turn at a time, and a friendship for good never does.** Reference: `monster::move` (`monmove.cpp:946-948`) decrements a positive `friendly`
/// before the monster walks; the `-1` form is not a count and is never touched. At zero the creature is nobody's ally - which `is_friendly` already reads as "the count is
/// zero" - so the monster that was charmed two turns ago attacks again.
#[test]
fn a_charm_spends_a_turn_of_itself_and_a_friendship_for_good_does_not() {
    let mut world = bevy_ecs::prelude::World::new();
    let told = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
    cdda_sim::missions::set_avatar(&mut world, told);
    let charmed = world
        .spawn((
            cdda_sim::Position { x: 5, y: 5, z: 0 },
            cdda_sim::creature::Creature {
                name: "the dog".to_owned(),
                size: cdda_sim::CreatureSize::Medium,
            },
            cdda_sim::creature::Moves::default(),
        ))
        .id();
    cdda_sim::creature::set_goal(&mut world, charmed, (9, 9, 0));
    cdda_sim::friendly::make_friendly(&mut world, charmed, 2);
    let loyal = world
        .spawn((
            cdda_sim::Position { x: 8, y: 8, z: 0 },
            cdda_sim::creature::Creature {
                name: "the pet".to_owned(),
                size: cdda_sim::CreatureSize::Medium,
            },
            cdda_sim::creature::Moves::default(),
        ))
        .id();
    cdda_sim::creature::set_goal(&mut world, loyal, (12, 12, 0));
    cdda_sim::friendly::make_friendly(&mut world, loyal, cdda_sim::friendly::FOR_EVER);
    cdda_sim::creature::take_turns(&mut world);
    assert_eq!(
        cdda_sim::friendly::turns_of(&world, charmed),
        1,
        "**one turn of the charm is spent by one turn of walking**"
    );
    assert!(
        cdda_sim::friendly::is_friendly(&world, charmed),
        "and it is still on the player's side"
    );
    cdda_sim::creature::take_turns(&mut world);
    assert_eq!(
        cdda_sim::friendly::turns_of(&world, charmed),
        0,
        "**two turns spend it all**"
    );
    assert!(
        !cdda_sim::friendly::is_friendly(&world, charmed),
        "**and the dog is nobody's ally again**"
    );
    assert_eq!(
        cdda_sim::friendly::turns_of(&world, loyal),
        cdda_sim::friendly::FOR_EVER,
        "**a friendship for good does not count down**"
    );
}
