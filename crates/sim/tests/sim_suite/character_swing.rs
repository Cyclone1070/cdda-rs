#![allow(clippy::expect_used, clippy::float_cmp, clippy::panic)]

//! The avatar's own blow: what a character holding nothing or holding a knife does to a monster.
//!
//! Reference: `Character::melee_attack_abstract` (`melee.cpp:625-630`) builds the blow from `used_weapon()`, which is the wielded item or `null_item_reference()`; the accuracy is
//! `Character::get_melee_hit_base` and the damage is the weapon's `melee_damage`. The port keeps the weapon as a **kind** - an id - on a `Wielding` component, so the blow is the
//! same rule a monster's is once the attack is worked out.

use bevy_ecs::prelude::*;
use cdda_sim::item_registry::{Kind, set_item_types};

/// A world with three kinds in it: empty hands' `null`, a rock, and a combat knife.
fn world_with_kinds() -> (World, cdda_sim::ItemTypeId, cdda_sim::ItemTypeId, cdda_sim::ItemTypeId) {
    let mut world = World::new();
    cdda_sim::singletons::put_rolls(&mut world, cdda_core_types::rng::Rolls::new(5));
    let kinds = [
        Kind {
            id: "null",
            volume_ml: 0,
            weight_g: 0,
            pockets: &[],
            thrown_damage: 0.0,
            melee_bash: 0.0,
            melee_cut: 0.0,
            melee_stab: 0.0,
            to_hit: -2,
        },
        Kind {
            id: "rock",
            volume_ml: 50,
            weight_g: 657,
            pockets: &[],
            thrown_damage: 7.0,
            melee_bash: 7.0,
            melee_cut: 0.0,
            melee_stab: 0.0,
            to_hit: -1,
        },
        Kind {
            id: "knife_combat",
            volume_ml: 250,
            weight_g: 500,
            pockets: &[],
            thrown_damage: 0.0,
            melee_bash: 0.0,
            melee_cut: 0.0,
            melee_stab: 21.0,
            to_hit: -1,
        },
    ];
    set_item_types(&mut world, &kinds);
    let null = cdda_sim::item_registry::id_of(&mut world, "null").expect("the null kind");
    let rock = cdda_sim::item_registry::id_of(&mut world, "rock").expect("the rock kind");
    let knife = cdda_sim::item_registry::id_of(&mut world, "knife_combat").expect("the knife kind");
    (world, null, rock, knife)
}

/// A character with nothing in its hands swings with the null item: it aims worst and carries no damage.
///
/// The reference's own dump says `null` has melee bash 0 and `to_hit` -2, so a punch that hurt would be the port's invention rather than its port. Dexterity eight still gives the
/// character two points of accuracy, which is `get_hit_base`'s whole contribution.
#[test]
fn empty_hands_swing_for_nothing_and_aim_worst() {
    let (mut world, _, _, _) = world_with_kinds();
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
    world
        .entity_mut(survivor)
        .insert(cdda_sim::creature::Dexterity { base: 8 });

    let blow = cdda_sim::creature::blow_from_what_the_character_holds(&world, survivor);
    assert_eq!(blow.accuracy_tenths, 20 - 20, "two points of dexterity and the null item's -2 balance");
    assert_eq!(blow.fixed.bash, 0, "empty hands carry no bash");
    assert_eq!(blow.fixed.stab, 0, "and no stabbing");
    assert_eq!(blow.dice, 0, "and no dice at all");
}

/// A knife in the hand carries its own damage and its own balance.
#[test]
fn a_knife_is_worth_its_own_stabbing_and_its_own_balance() {
    let (mut world, _, _, knife) = world_with_kinds();
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
    world
        .entity_mut(survivor)
        .insert(cdda_sim::creature::Dexterity { base: 8 })
        .insert(cdda_sim::creature::Wielding(knife));

    let blow = cdda_sim::creature::blow_from_what_the_character_holds(&world, survivor);
    assert_eq!(blow.fixed.bash, 0, "a combat knife has no bash");
    assert_eq!(blow.fixed.stab, 21, "and twenty-one points of stabbing");
    assert_eq!(
        blow.accuracy_tenths, 10,
        "two points of dexterity less the knife's own -1 balance"
    );
}

/// And the blow lands on a monster: a knife takes hit points off a zombie, and empty hands take none.
#[test]
fn a_knife_draws_blood_and_empty_hands_do_not() {
    let (mut world, _, _, knife) = world_with_kinds();
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
    world
        .entity_mut(survivor)
        .insert(cdda_sim::creature::Dexterity { base: 8 });
    let zombie = cdda_sim::creature::spawn_creature(&mut world, "the zombie", (1, 0, 0));
    cdda_sim::creature::set_health(&mut world, zombie, 80);
    world
        .entity_mut(zombie)
        .insert(cdda_sim::creature::Armour { bash: 0, cut: 0, stab: 0 });

    let bare = cdda_sim::creature::swing_what_the_character_holds(&mut world, survivor, zombie);
    assert_eq!(bare, Some(0), "empty hands swing and do nothing");
    assert_eq!(
        cdda_sim::creature::health_of(&world, zombie),
        Some(80),
        "the zombie is untouched"
    );

    world
        .entity_mut(survivor)
        .insert(cdda_sim::creature::Wielding(knife));
    let cut = cdda_sim::creature::swing_what_the_character_holds(&mut world, survivor, zombie);
    assert!(
        cut.is_some_and(|dealt| dealt > 0),
        "a knife in the hand draws blood: {cut:?}"
    );
    assert!(
        cdda_sim::creature::health_of(&world, zombie).is_some_and(|hp| hp < 80),
        "and the zombie is hurt"
    );
}

/// A practised hand aims better, by the weapon's own skill and by melee.
///
/// Reference: `Character::get_hit_weapon` (`melee.cpp:356-370`) is `skill / 3 + melee_skill / 2 + to_hit`, where `skill` is the one the weapon's **biggest damage type** trains
/// (`item::melee_skill`, `item.cpp:11071-11090`). A crowbar's damage is bash 19 and cut 1, so its skill is bashing; a combat knife's is stabbing. The melee skill is worth half a
/// point a level to every weapon, which is why a survivor who has practised anything at all is better with a rock than one who has not.
#[test]
fn a_practised_hand_aims_better_by_the_weapons_own_skill() {
    use std::collections::BTreeMap;

    let (mut world, _null, rock, knife) = world_with_kinds();
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
    world
        .entity_mut(survivor)
        .insert(cdda_sim::creature::Dexterity { base: 8 });

    // Nothing practised: dexterity's two points and the rock's own -1 balance.
    world
        .entity_mut(survivor)
        .insert(cdda_sim::creature::Wielding(rock));
    let bare = cdda_sim::creature::blow_from_what_the_character_holds(&world, survivor);
    assert_eq!(
        bare.accuracy_tenths, 10,
        "two points of dexterity less one of balance, and no skill at all"
    );

    // Bashing three and melee four: three tenths a level of bashing and five a level of melee.
    world.entity_mut(survivor).insert(cdda_sim::creature::Skills(
        BTreeMap::from([("bashing".to_owned(), 3), ("melee".to_owned(), 4)]),
    ));
    assert_eq!(
        cdda_sim::creature::blow_from_what_the_character_holds(&world, survivor).accuracy_tenths,
        10 + 10 + 20,
        "the rock trains bashing, so its skill counts"
    );

    // A knife trains stabbing, which the survivor has not practised: only the melee skill helps it.
    world
        .entity_mut(survivor)
        .insert(cdda_sim::creature::Wielding(knife));
    assert_eq!(
        cdda_sim::creature::blow_from_what_the_character_holds(&world, survivor).accuracy_tenths,
        10 + 20,
        "practising bashing does nothing for a blade"
    );
}

/// The weapon's skill is the one its **biggest** damage type trains, and a weapon with no damage trains nothing.
#[test]
fn a_weapons_skill_is_the_one_its_biggest_damage_trains() {
    use cdda_sim::melee::weapon_skill;
    assert_eq!(weapon_skill(19.0, 1.0, 0.0), Some("bashing"), "a crowbar is a club");
    assert_eq!(weapon_skill(0.0, 0.0, 21.0), Some("stabbing"), "a knife is a point");
    assert_eq!(weapon_skill(0.0, 12.0, 3.0), Some("cutting"), "a blade is an edge");
    assert_eq!(weapon_skill(0.0, 3.0, 12.0), Some("stabbing"), "and the point wins when it does more");
    assert_eq!(weapon_skill(0.0, 0.0, 0.0), None, "a thing that does no damage trains nothing");
}

/// A survivor's swing costs moves, taken from the weapon it holds.
///
/// Reference: `Character::melee_attack` charges `attack_speed( cur_weap )` whether the blow lands or not (`melee.cpp:628`), and `attack_speed` starts from the weapon's own
/// `attack_time` (`item.cpp:7595-7602`). The gate reads the **store**, not the arithmetic: the fixture's combat knife is 250 ml and 500 g, so its attack time is
/// `65 + (250 / 62.5 + 500 / 60) = 77`, half of it is 38, and a survivor of dexterity 8 with no melee skill pays `38 + 38 - 4 = 72`.
#[test]
fn a_survivor_s_swing_costs_the_moves_its_weapon_takes() {
    // The tuple's third kind is the knife, not a crowbar. Naming it `crowbar` and asserting a crowbar's numbers was the first version of this test, and it failed with 72 for 92
    // because a hand-built fixture's order is not a promise about which item is in hand.
    let (mut world, _null, _rock, knife) = world_with_kinds();
    let survivor = cdda_sim::spawn_player(&mut world, (0, 0), 80, 0);
    world
        .entity_mut(survivor)
        .insert(cdda_sim::creature::Wielding(knife))
        .insert(cdda_sim::creature::Dexterity { base: 8 });
    let victim = cdda_sim::creature::spawn_creature(&mut world, "a rat", (1, 0, 0));
    cdda_sim::creature::set_moves(&mut world, survivor, 400);

    cdda_sim::creature::swing_what_the_character_holds(&mut world, survivor, victim);
    assert_eq!(
        cdda_sim::creature::moves_left(&world, survivor),
        328,
        "a knife swing costs seventy-two moves: 77 / 2 = 38, plus 38 for no melee skill, less 4 for dexterity 8"
    );
}

/// A survivor that lands a blow practises the skill it swung with.
///
/// Reference: `Character::melee_attack` calls `melee_train( *this, 2, std::min( 5, skill_training_cap ), cur_weap, ... )` after a hit (`melee.cpp:2222`), which is a practice of
/// the **weapon's** skill - not the melee skill, which is the general one. The gate swings a knife twenty times and reads the skill the swing is worth learning.
#[test]
fn a_landed_blow_teaches_the_skill_it_used() {
    let (mut world, _null, _rock, knife) = world_with_kinds();
    let survivor = cdda_sim::spawn_player(&mut world, (0, 0), 80, 0);
    world
        .entity_mut(survivor)
        .insert(cdda_sim::creature::Wielding(knife));
    // A knife's damage is all stab, so the skill it teaches is stabbing.
    let victim = cdda_sim::creature::spawn_creature(&mut world, "a rat", (1, 0, 0));
    world
        .entity_mut(victim)
        .insert(cdda_sim::creature::Health { hp: 10_000, max_hp: 10_000 });
    cdda_sim::creature::set_moves(&mut world, survivor, 100_000);

    assert_eq!(
        cdda_sim::creature::skill_level(&world, survivor, "stabbing"),
        0,
        "nothing practised yet"
    );
    for _ in 0..20 {
        cdda_sim::creature::swing_what_the_character_holds(&mut world, survivor, victim);
    }
    let stabbing = cdda_sim::creature::skill_level(&world, survivor, "stabbing");
    let exercise = world
        .get::<cdda_sim::creature::Exercise>(survivor)
        .and_then(|banked| banked.0.get("stabbing").copied())
        .unwrap_or(0);
    assert!(
        stabbing > 0 || exercise > 0,
        "twenty blows teach something: level {stabbing}, exercise {exercise}"
    );
    assert_eq!(
        cdda_sim::creature::skill_level(&world, survivor, "bashing"),
        0,
        "and nothing about a skill the knife does not use"
    );
}

/// A blow that misses still teaches, which is the reference's own rule.
///
/// Reference: `melee.cpp:713-716` trains after a **miss** exactly as `melee.cpp:2222` trains after a hit - the same `melee_train( *this, 2, std::min( 5,
/// skill_training_cap ), ... )`. The gate makes the target impossible to hit (a thousand points of dodge), so every swing misses, and the skill must still grow.
#[test]
fn a_blow_that_misses_still_teaches_the_skill_it_swung_with() {
    let (mut world, _null, _rock, knife) = world_with_kinds();
    let survivor = cdda_sim::spawn_player(&mut world, (0, 0), 80, 0);
    world
        .entity_mut(survivor)
        .insert(cdda_sim::creature::Wielding(knife));
    let victim = cdda_sim::creature::spawn_creature(&mut world, "the wind", (1, 0, 0));
    world
        .entity_mut(victim)
        .insert(cdda_sim::creature::Dodge { points: 1_000 })
        .insert(cdda_sim::creature::Health { hp: 10_000, max_hp: 10_000 });
    cdda_sim::creature::set_moves(&mut world, survivor, 100_000);

    for _ in 0..20 {
        cdda_sim::creature::swing_what_the_character_holds(&mut world, survivor, victim);
    }
    assert_eq!(
        cdda_sim::creature::health_of(&world, victim),
        Some(10_000),
        "nothing landed, which is what makes this a test about missing"
    );
    let exercise = world
        .get::<cdda_sim::creature::Exercise>(survivor)
        .and_then(|banked| banked.0.get("stabbing").copied())
        .unwrap_or(0);
    assert!(
        exercise > 0,
        "twenty misses still teach: exercise {exercise}"
    );
}

/// A survivor's swing flags what it hit, and a monster's blow flags itself.
///
/// Reference: two places, both in the attack paths. `Character::melee_attack` gives a non-avatar target `effect_hit_by_player` for **ten minutes** the moment it swings, before the
/// blow is resolved (`melee.cpp:595-599`, "Flag as attacked by us for AI"). `monster::melee_attack` gives **itself** `effect_hit_by_player` for **three turns** when the target is the
/// avatar (`monster.cpp:2130-2133`). The two durations differ by three orders of magnitude and that is the point: an AI remembers being attacked for a fight, and a monster remembers
/// attacking *you* for a moment.
#[test]
fn a_swing_flags_who_it_hit_and_who_it_swung_at() {
    let (mut world, _null, _rock, knife) = world_with_kinds();
    let flag = cdda_json::parse(br#"{"id":"hit_by_player","max_duration":"1 hours"}"#).expect("valid json");
    let cdda_json::Value::Object(entry) = flag else {
        panic!("an effect type is an object")
    };
    let kind = cdda_content::effects::read_one("hit_by_player", &entry);
    cdda_sim::effect_registry::set_effect_types(&mut world, std::slice::from_ref(&kind));

    let survivor = cdda_sim::spawn_player(&mut world, (0, 0), 80, 0);
    world
        .entity_mut(survivor)
        .insert(cdda_sim::creature::Wielding(knife));
    let monster = cdda_sim::creature::spawn_creature(&mut world, "a rat", (1, 0, 0));
    world
        .entity_mut(monster)
        .insert(cdda_sim::creature::Health { hp: 10_000, max_hp: 10_000 });
    cdda_sim::creature::set_moves(&mut world, survivor, 1_000);

    cdda_sim::creature::swing_what_the_character_holds(&mut world, survivor, monster);
    assert!(
        cdda_sim::applied_effects::has(&world, monster, "hit_by_player"),
        "the survivor's swing tells the world who it hit"
    );
    let ten_minutes = cdda_sim::applied_effects::worn(&world, monster)
        .into_iter()
        .find(|worn| worn.id == "hit_by_player")
        .map_or(0, |worn| worn.duration);
    assert_eq!(ten_minutes, 600, "for ten minutes, which is six hundred turns");

    // And the other direction: a monster swinging at the avatar flags itself, for three turns.
    let mut monster_type = {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/monsters/monsters.json");
        let text = std::fs::read(path).expect("the monster dump exists");
        let value = cdda_json::parse(&text).expect("the dump is valid JSON");
        let cdda_json::Value::Object(root) = value else {
            panic!("the dump's root is not an object")
        };
        let Some(cdda_json::Value::Array(prototypes)) = root.get("prototypes") else {
            panic!("the dump has no prototypes")
        };
        cdda_content::monsters::load_objects(prototypes)
            .expect("the dump loads")
            .into_iter()
            .find(|one| one.id == "mon_zombie")
            .expect("the corpus has a zombie")
    };
    monster_type.melee_dice = 1;
    let zombie = cdda_sim::creature::spawn_monster_of_type(&mut world, cdda_sim::MonsterKindId(1), &monster_type, (0, 1, 0), 100);
    let mut flagged = false;
    for _ in 0..12 {
        cdda_sim::creature::strike_at(&mut world, zombie, survivor);
        if cdda_sim::applied_effects::has(&world, zombie, "hit_by_player") {
            flagged = true;
        }
    }
    assert!(flagged, "a monster that attacks the avatar is flagged for three turns");
}

/// Swinging spends wind, and running out of it makes the next swing slower.
///
/// Reference: `Character::get_total_melee_stamina_cost` (spent per swing) and `Character::attack_speed`'s stamina penalty (`melee.cpp:2678-2685`), which doubles the moves a swing
/// costs once the character is spent. The gate drives the **store**: a 500 g knife costs 81 stamina a swing, so twenty blows take 1,620 of 3,500; and the same survivor drained to
/// nothing pays more moves for the same swing than a fresh one does.
#[test]
fn swinging_spends_wind_and_a_spent_survivor_swings_slower() {
    let (mut world, _null, _rock, knife) = world_with_kinds();
    let survivor = cdda_sim::spawn_player(&mut world, (0, 0), 80, 0);
    world
        .entity_mut(survivor)
        .insert(cdda_sim::creature::Wielding(knife))
        .insert(cdda_sim::creature::Stamina { cur: 3_500.0, max: 3_500.0 });
    let victim = cdda_sim::creature::spawn_creature(&mut world, "a rat", (1, 0, 0));
    world
        .entity_mut(victim)
        .insert(cdda_sim::creature::Health { hp: 100_000, max_hp: 100_000 });
    cdda_sim::creature::set_moves(&mut world, survivor, 100_000);

    let before = cdda_sim::creature::moves_left(&world, survivor);
    cdda_sim::creature::swing_what_the_character_holds(&mut world, survivor, victim);
    let fresh_cost = before - cdda_sim::creature::moves_left(&world, survivor);
    let wind = world
        .get::<cdda_sim::creature::Stamina>(survivor)
        .expect("the survivor has wind")
        .cur;
    assert!(
        (wind - (3_500.0 - 81.0)).abs() < 0.001,
        "a 500 g knife costs eighty-one of three and a half thousand: {wind}"
    );

    // Drained to nothing, the same swing costs more moves: the penalty is two at no wind.
    world
        .get_mut::<cdda_sim::creature::Stamina>(survivor)
        .expect("still there")
        .cur = 0.0;
    let before = cdda_sim::creature::moves_left(&world, survivor);
    cdda_sim::creature::swing_what_the_character_holds(&mut world, survivor, victim);
    let spent_cost = before - cdda_sim::creature::moves_left(&world, survivor);
    assert!(
        spent_cost > fresh_cost,
        "a spent survivor pays more for the same swing: {spent_cost} against {fresh_cost}"
    );
}

/// Spending more wind than there is makes a survivor winded, and the winded survivor recovers at a crawl.
///
/// Reference: `Character::mod_stamina` puts the body parts' windage effect on - `effect_winded`, **ten turns** - whenever the stamina change takes it below zero
/// (`character.cpp:6961-6968`), and `Character::update_stamina` recovers a **tenth** while that effect is on (`character.cpp:7033-7036`). The chain is what makes exhaustion
/// matter: swing past empty, and the next turns give back two instead of twenty.
#[test]
fn swinging_past_empty_makes_a_survivor_winded_and_slows_what_comes_back() {
    use bevy_ecs::prelude::*;

    let mut world = World::new();
    cdda_sim::singletons::put_rolls(&mut world, cdda_core_types::rng::Rolls::new(31));
    let mut level = cdda_core_types::level::Level::new();
    for x in 0..4 {
        for y in 0..4 {
            level.set_terrain(x, y, 1, 0);
        }
    }
    cdda_sim::load_level(&mut world, &level);
    let winded = cdda_json::parse(br#"{"id":"winded","max_duration":"1 hours"}"#).expect("valid json");
    let cdda_json::Value::Object(entry) = winded else {
        panic!("an effect type is an object")
    };
    let kind = cdda_content::effects::read_one("winded", &entry);
    cdda_sim::effect_registry::set_effect_types(&mut world, std::slice::from_ref(&kind));

    let survivor = cdda_sim::spawn_player(&mut world, (0, 0), 80, 0);
    world
        .entity_mut(survivor)
        .insert(cdda_sim::creature::Stamina { cur: 50.0, max: 3_500.0 });
    assert!(
        !cdda_sim::applied_effects::has(&world, survivor, "winded"),
        "nothing has winded the survivor yet"
    );

    // A spend of eighty-one out of fifty takes it past zero. Fifty is the point of the fixture: a hundred would have been left at nineteen, which is not past anything.
    cdda_sim::creature::spend_stamina(&mut world, survivor, -81);
    assert!(
        cdda_sim::applied_effects::has(&world, survivor, "winded"),
        "and going past empty winds the survivor"
    );
    let duration = cdda_sim::applied_effects::worn(&world, survivor)
        .into_iter()
        .find(|worn| worn.id == "winded")
        .map_or(0, |worn| worn.duration);
    assert_eq!(duration, 10, "for ten turns");

    // And the turn that follows gives a tenth of the rate back: two, not twenty.
    cdda_sim::turn::run_turn(&mut world);
    let wind = world
        .get::<cdda_sim::creature::Stamina>(survivor)
        .expect("the survivor still has wind")
        .cur;
    assert!(
        (wind - 2.0).abs() < 0.001,
        "a winded survivor gets two a turn, not twenty: {wind}"
    );
}
