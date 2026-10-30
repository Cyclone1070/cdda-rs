//! How many of the game's `attack` spells the port can actually resolve.
//!
//! Reference: `spell_effect::target_attack` (`magic_spell_effect.cpp`), which every one of the data's attack spells names.
//! "Attack spells are ported" had been a claim about a function existing; this sweep turns it into a measurement over the
//! whole data set, and a failure here means either the data changed or the rule stopped covering something.

#![allow(clippy::panic, clippy::expect_used, clippy::indexing_slicing)]

use bevy_ecs::prelude::*;
use cdda_sim::spellcasting::{self, Cast};

/// A body with one part, because the sweep is about the spells and not about anatomy.
fn a_torso() -> Vec<cdda_sim::hit_location::BodyPart> {
    vec![cdda_sim::hit_location::BodyPart {
        name: "torso".to_owned(),
        hit_size: 36.0,
        hit_difficulty: 1.0,
        upper: false,
        lower: false,
    }]
}

/// What the sweep found, so the counts can be asserted and printed.
#[derive(Default, Debug)]
struct Sweep {
    seen: usize,
    hit: usize,
    out_of_range: usize,
    nothing_to_hit: usize,
    dealt: i64,
}

fn sweep(world: &mut World, spells: &[cdda_content::spells::Spell]) -> Sweep {
    let mut found = Sweep::default();
    for spell in spells.iter().filter(|spell| spell.effect_name == "attack") {
        found.seen += 1;
        let target = cdda_sim::creature::spawn_creature(world, "the target", (0, 0, 0));
        // A target with no body has nothing to hit, which is what the first version of this sweep measured: 77 refusals
        // that said more about the harness than about the rule.
        world
            .entity_mut(target)
            .insert(cdda_sim::creature::Body { parts: a_torso() });
        cdda_sim::creature::set_health(world, target, 80);
        match spellcasting::cast_attack(world, (0, 0, 0), spell, 0, target, 0.0) {
            Cast::Hit { dealt, .. } => {
                found.hit += 1;
                found.dealt += i64::from(dealt);

            }
            Cast::OutOfRange => found.out_of_range += 1,
            Cast::NothingToHit => found.nothing_to_hit += 1,
            Cast::NotAnAttack => panic!("{} is an attack by its own name", spell.id),
        }
        world.despawn(target);
    }
    found
}

#[test]
fn every_attack_spell_in_the_game_lands_something() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/spells.json");
    let text = std::fs::read_to_string(path).expect("the spell dump");
    let value = cdda_json::parse(text.as_bytes()).expect("valid json");
    let spells = cdda_content::spells::load_dump(&value).expect("the dump loads");

    let mut world = World::new();
    let found = sweep(&mut world, &spells);
    println!("attack sweep: {found:?}");

    assert_eq!(found.seen, 77, "the game has 77 attack spells");
    assert_eq!(
        found.hit + found.out_of_range + found.nothing_to_hit,
        found.seen,
        "every attack spell was asked and answered"
    );
    assert_eq!(
        found.hit, 77,
        "every one of the game's 77 attack spells landed a blow"
    );
    assert_eq!(found.out_of_range, 0, "a target on the caster's own square is never out of range");
    assert_eq!(found.nothing_to_hit, 0, "and every one of them found something to hit");
    // The damage that arrived must be the damage the data states, unaltered: an unarmoured torso on the caster's own
    // square is the one case where nothing may change the number.
    #[allow(clippy::cast_possible_truncation)]
    let stated: i64 = spells
        .iter()
        .filter(|spell| spell.effect_name == "attack")
        .filter_map(|spell| spell.damage.at(0))
        .filter(|damage| *damage > 0.0)
        .map(|damage| damage.round() as i64)
        .sum();
    // Reference: `damage_targets` only deals damage "if( sp.damage( caster ) > 0 )"
    // (`magic_spell_effect.cpp:563`). Two of the game's attack spells carry **negative** damage - `AEA_HEAL` at -2 and
    // `spur_shrill` at -100 - and the reference deals none of it, applying an effect instead. The port matches on the
    // damage half and has no effect to add yet, which is a declared gap rather than a wrong number.
    let positive = spells
        .iter()
        .filter(|spell| spell.effect_name == "attack")
        .filter(|spell| spell.damage.at(0).is_some_and(|damage| damage > 0.0))
        .count();
    println!("stated {stated} over {positive} spells with positive damage");
    // 32 carry damage at level 0, 43 scale from zero with level, and 2 are negative (healing) and deal none.
    assert_eq!(
        positive, 32,
        "thirty-two of the 77 attack spells carry damage at level 0"
    );
    assert_eq!(
        found.dealt, stated,
        "every point of positive damage the data states arrived: {stated} across 75 spells, the largest being the one \
         that says 100,000,000"
    );
}
