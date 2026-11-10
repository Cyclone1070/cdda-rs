//! A spell that turns a monster into another monster: `targeted_polymorph`.
//!
//! Reference: `spell_effect::targeted_polymorph` (`magic_spell_effect.cpp:742-754`) and `monster::poly` (`monster.cpp:363-386`). Two facts carry the rule and both are
//! gates below: the victim has to be hurt **strictly less** than the spell's damage, and what survives the change is the hit **percentage**, not the hit points - a body at
//! half health becomes a half-healthy body of the new kind, whatever that kind's ceiling is.

#![allow(clippy::panic, clippy::expect_used, clippy::indexing_slicing)]

use bevy_ecs::prelude::*;

/// A world with three monster kinds: the victim's, a hundred-hit-point one at speed 70, and a ten-hit-point one at speed 300.
fn a_world() -> (World, cdda_sim::MonsterKindId) {
    let mut world = World::new();
    cdda_sim::monster_registry::set_monster_types(
        &mut world,
        &[
            cdda_sim::monster_registry::MonsterKind { id: "mon_prey", hp: 60 },
            cdda_sim::monster_registry::MonsterKind { id: "mon_big", hp: 100 },
            cdda_sim::monster_registry::MonsterKind { id: "mon_small", hp: 10 },
        ],
    );
    let big = cdda_sim::monster_registry::id_of(&mut world, "mon_big").expect("mon_big");
    let small = cdda_sim::monster_registry::id_of(&mut world, "mon_small").expect("mon_small");
    cdda_sim::monster_registry::set_speeds(&mut world, &[(big, 70), (small, 300)]);
    let prey = cdda_sim::monster_registry::id_of(&mut world, "mon_prey").expect("mon_prey");
    (world, prey)
}

fn a_spell(kind: &str, damage: f64) -> cdda_content::spells::Spell {
    let scaling = |value: f64| cdda_content::spells::Scaling {
        start: Some(value),
        increment: Some(0.0),
        end: Some(value),
    };
    cdda_content::spells::Spell {
        id: "gate".to_owned(),
        name: "Gate".to_owned(),
        effect_name: "targeted_polymorph".to_owned(),
        effect_str: kind.to_owned(),
        damage_type: String::new(),
        shape: "blast".to_owned(),
        energy_source: "NONE".to_owned(),
        damage: scaling(damage),
        range: cdda_content::spells::Scaling::default(),
        area: cdda_content::spells::Scaling::default(),
        energy: cdda_content::spells::Scaling::default(),
        duration: cdda_content::spells::Scaling::default(),
        casting_time: cdda_content::spells::Scaling::default(),
        difficulty: None,
        max_level: None,
        valid_targets: std::collections::BTreeSet::new(),
        targeted_monster_ids: std::collections::BTreeSet::new(),
        targeted_species_ids: std::collections::BTreeSet::new(),
        ignored_species_ids: std::collections::BTreeSet::new(),
        flags: std::collections::BTreeSet::new(),
    }
}

#[test]
fn a_polymorph_keeps_the_hit_percentage_and_takes_the_new_body_s_speed() {
    let (mut world, prey) = a_world();
    let victim = cdda_sim::spawn_monster(&mut world, (0, 0), prey, 30);
    cdda_sim::creature::set_speed(&mut world, victim, 100, 0);
    let big = cdda_sim::monster_registry::id_of(&mut world, "mon_big").expect("mon_big");
    // The data's own spells state ten million damage, so any living victim is below the threshold; the threshold has its own gate below.
    let spell = a_spell("mon_big", 10_000_000.0);
    assert_eq!(
        cdda_sim::spell_effects::apply(&mut world, victim, &spell, 0, (0, 0, 0)),
        cdda_sim::spell_effects::Effective::Done,
        "the spell changed the monster"
    );
    assert_eq!(
        world.get::<cdda_sim::creature::CreatureKind>(victim).map(|kind| kind.0),
        Some(big),
        "into the kind the spell names"
    );
    let health = world.get::<cdda_sim::creature::Health>(victim).expect("still alive");
    assert_eq!(
        (health.hp, health.max_hp),
        (50, 100),
        "**half of sixty becomes half of a hundred**, because the percentage is what survives"
    );
    assert_eq!(
        cdda_sim::creature::speed_of(&world, victim),
        70,
        "and it moves at the new body's pace"
    );
    assert_eq!(
        world.get::<cdda_sim::creature::Moves>(victim).map(|moves| moves.left),
        Some(0),
        "with its moves spent, which is the reference's `moves = 0`"
    );
}

/// **A whole victim resists**, at the boundary exactly.
///
/// Reference: `magic_spell_effect.cpp:747`: `if( victim->get_hp() < sp.damage( caster ) )` - strictly less, so a victim whose hit points equal the spell's damage is not
/// transformed. The port reads the same comparison, and this gate is what stops `<` from becoming `<=` in a later round.
#[test]
fn a_monster_whose_hit_points_are_not_below_the_spell_s_damage_resists() {
    let (mut world, prey) = a_world();
    let victim = cdda_sim::spawn_monster(&mut world, (0, 0), prey, 60);
    let spell = a_spell("mon_small", 60.0);
    assert_eq!(
        cdda_sim::spell_effects::apply(&mut world, victim, &spell, 0, (0, 0, 0)),
        cdda_sim::spell_effects::Effective::NothingAffected,
        "**equal hit points are not below, so the monster resists**"
    );
    assert_eq!(
        world.get::<cdda_sim::creature::CreatureKind>(victim).map(|kind| kind.0),
        Some(prey),
        "and it is still what it was"
    );
    let weaker = a_spell("mon_small", 61.0);
    assert_eq!(
        cdda_sim::spell_effects::apply(&mut world, victim, &weaker, 0, (0, 0, 0)),
        cdda_sim::spell_effects::Effective::Done,
        "one point more of damage and it changes"
    );
    assert_eq!(
        world.get::<cdda_sim::creature::Health>(victim).map(|health| health.max_hp),
        Some(10),
        "into the ten-hit-point body"
    );
}
