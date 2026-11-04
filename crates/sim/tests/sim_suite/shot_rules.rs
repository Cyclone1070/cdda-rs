//! What a gunshot costs: the skill's line and the wind's cliff.
//!
//! Reference: `time_to_attack` (`ranged.cpp:2299-2308`) and `RAS_time` (`ranged.cpp:2310-2322`). Both are arithmetic over data the port names rather than data it has, so the gate works the
//! numbers out by hand and says where they come from.

#![allow(clippy::panic, clippy::expect_used)]

/// A skill's line: `base_time` down by `time_reduction_per_level` a level, floored at `min_time`.
///
/// Reference: `time_to_attack` (`ranged.cpp:2299-2308`). The numbers are the **oracle's own** (`--dump-skills`): **archery** is `base_time` 50, reduction 3, `min_time` 20; **rifle** and
/// **pistol** are 30, 1, 20; and **marksmanship** (`gun`) is 220, 25, 50.
#[test]
fn a_gun_s_attack_time_is_its_skill_s_line_floored() {
    use cdda_sim::shot::time_to_attack;

    // Archery: 50 - 3 a level, floor 20.
    assert_eq!(time_to_attack(50, 3, 20, 0), 50, "a beginner pays the base time");
    assert_eq!(time_to_attack(50, 3, 20, 5), 35, "five levels is fifteen moves off");
    assert_eq!(time_to_attack(50, 3, 20, 10), 20, "and ten levels is exactly the floor");
    assert_eq!(time_to_attack(50, 3, 20, 15), 20, "which no amount of skill goes below");
    // Rifle: 30 - 1 a level, floor 20 - a slower line that reaches its floor at ten.
    assert_eq!(time_to_attack(30, 1, 20, 10), 20, "a rifle's floor at ten");
    assert_eq!(time_to_attack(30, 1, 20, 5), 25, "and 30 - 5 halfway");
    // Marksmanship: 220 - 25 a level, floor 50 - the default block, and four times a rifle.
    assert_eq!(time_to_attack(220, 25, 50, 0), 220, "an unskilled shot with no marksmanship");
    assert_eq!(time_to_attack(220, 25, 50, 10), 50, "and its floor at ten");
}

/// Firing while out of wind costs two moves a percent below a quarter, and nothing above it.
///
/// Reference: `RAS_time` (`ranged.cpp:2312-2316`). The gate pins the **cliff**: twenty-five percent pays nothing, twenty-four pays two, and none pays fifty. A port that made this a straight
/// line from full wind would be slower in the middle of the bar and faster at the bottom, and no single case would say which.
#[test]
fn a_winded_shooter_is_slower_to_fire_but_only_below_a_quarter_wind() {
    use cdda_sim::shot::{cost_of_a_shot, winded_cost};

    assert_eq!(winded_cost(100), 0, "full wind costs nothing extra");
    assert_eq!(winded_cost(25), 0, "and a quarter is still free");
    assert_eq!(winded_cost(24), 2, "one percent below the cliff costs two moves");
    assert_eq!(winded_cost(10), 30, "ten percent costs thirty");
    assert_eq!(winded_cost(0), 50, "and no wind costs fifty");

    // The whole shot: archery at skill five with a full wind and a round fetched at no cost.
    assert_eq!(cost_of_a_shot(50, 3, 20, 5, 100, 0), 35, "the skill's line alone");
    // The same shot out of wind, with the round taking twenty moves to reach the chamber.
    assert_eq!(
        cost_of_a_shot(50, 3, 20, 5, 0, 20),
        105,
        "35 for the line, 50 for the wind and 20 for the reach"
    );
}

/// A gun is priced by **its own skill's** line, looked up in the dumped table - and a line the table does not have falls back to the reference's struct defaults.
///
/// Reference: `Character::fire_gun` (`ranged.cpp:1060`) reads `firing.gun->skill_used->time_to_attack()`; the lines are the corpus's (`--dump-skills`, round 1143). The gate uses the corpus's
/// own guns, so the skill each one names is the data's: a **rifle** is priced at 30/1/20 and a **bow** at archery's 50/3/20, which is the difference round 1143's correction was about.
#[test]
fn a_gun_is_priced_by_the_line_of_the_skill_its_own_data_names() {
    use cdda_sim::shot::{cost_of_firing, unknown_skill_line};

    let read = |name: &str| {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("../../corpus/{name}"));
        let bytes = std::fs::read(path).expect("the corpus file exists");
        cdda_json::parse(&bytes).expect("valid JSON")
    };
    let guns = cdda_content::guns::load_dump(&read("items/guns.json")).expect("the guns load");
    let skills = cdda_content::skills::load_dump(&read("skills.json")).expect("the skills load");

    let naive = |id: &str| {
        guns.iter().find(|gun| gun.id == id).expect("the corpus has it").clone()
    };
    let rifle = naive("ruger_mini");
    assert_eq!(rifle.skill, "rifle", "the corpus says what a Mini-14 is fired with");
    // Rifle's line is 30 - 1 a level, floor 20: at skill four the shot costs 26 moves.
    assert_eq!(cost_of_firing(&rifle, &skills, 4, 100, 0), 26, "a rifle shot at skill four");
    // Out of wind it costs thirty more, because the wind term is added after the line.
    assert_eq!(cost_of_firing(&rifle, &skills, 4, 10, 0), 56, "and thirty more with no wind");

    // A bow names archery, whose line is 50 - 3 a level: at skill four that is 38, not 26.
    let bow = naive("longbow");
    assert_eq!(bow.skill, "archery", "and a longbow names archery");
    assert_eq!(
        cost_of_firing(&bow, &skills, 4, 100, 0),
        38,
        "archery is slower to draw than a rifle is to shoulder - which is the mistake round 1143 caught"
    );
    assert!(
        cost_of_firing(&bow, &skills, 4, 100, 0) > cost_of_firing(&rifle, &skills, 4, 100, 0),
        "so the same skill level buys a rifle shot sooner than a bow shot"
    );

    // A skill the table does not have is priced at the reference's struct defaults rather than at zero or at a rifle's line.
    let mut stranger = rifle.clone();
    stranger.skill = "a-skill-nobody-dumped".to_owned();
    let (base, reduction, min) = unknown_skill_line();
    assert_eq!(cost_of_firing(&stranger, &skills, 0, 100, 0), base, "the fallback is the default base time");
    assert_eq!(
        cost_of_firing(&stranger, &skills, 10, 100, 0),
        min,
        "and its floor"
    );
    // The line reaches its floor at seven (220 - 175 = 45, floored to 50), which is why the fallback is a slow line and not a rifle's.
    assert_eq!(
        base - reduction * 7,
        45,
        "the raw line at seven levels"
    );
    assert!(
        base - reduction * 7 < min,
        "which is already under the floor, so the floor is what the shooter pays"
    );
}

/// Firing a gun spends the moves, rolls the shot's own error, and says which creature it reached.
///
/// Reference: `Character::fire_gun` (`ranged.cpp:1050-1200`) - the dispersion composed by `get_weapon_dispersion`, the moves charged by `mod_moves( -attack_moves )`, and the aim rolled by
/// `projectile_attack_roll` - plus that rule's own convention that **a fraction missed of one is a miss**. The gate stands the shooter between a small thing and a big one, fires the **same
/// gun** at each from the same square on the same dice, and reads the store's moves in between.
#[test]
fn firing_a_gun_spends_moves_and_judges_the_shot_by_what_stands_there() {
    use bevy_ecs::prelude::*;

    let read = |name: &str| {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("../../corpus/{name}"));
        let bytes = std::fs::read(path).expect("the corpus file exists");
        cdda_json::parse(&bytes).expect("valid JSON")
    };
    let guns = cdda_content::guns::load_dump(&read("items/guns.json")).expect("the guns load");
    let lines = cdda_content::skills::load_dump(&read("skills.json")).expect("the skills load");
    let gun = guns.iter().find(|gun| gun.id == "ruger_mini").expect("the corpus has the Mini-14").clone();

    let mut world = World::new();
    cdda_sim::singletons::put_rolls(&mut world, cdda_core_types::rng::Rolls::new(4_242));
    let shooter = cdda_sim::creature::spawn_creature(&mut world, "the shooter", (0, 0, 0));
    world
        .entity_mut(shooter)
        .insert(cdda_sim::creature::Dexterity { base: 8 })
        .insert(cdda_sim::creature::Perception { base: 8 })
        .insert(cdda_sim::creature::Moves { left: 200 });

    // A small thing four squares away, and the same gun fired at it: the moves fall by the price the corpus's own tables give.
    let beast = cdda_sim::creature::spawn_creature(&mut world, "a small thing", (4, 0, 0));
    world.entity_mut(beast).insert(cdda_sim::creature::Creature {
        name: "a small thing".to_owned(),
        size: cdda_sim::CreatureSize::Tiny,
    });
    world.entity_mut(beast).insert(cdda_sim::creature::Health { hp: 10, max_hp: 10 });

    let before = cdda_sim::creature::moves_left(&world, shooter);
    let fired = cdda_sim::shot::fire_at(&mut world, shooter, &gun, &lines, (4, 0, 0), 18.0, None);
    let after = cdda_sim::creature::moves_left(&world, shooter);
    assert_eq!(before - after, fired.cost, "the shot cost what the shot table says: {}", fired.cost);
    assert_eq!(
        fired.cost,
        cdda_sim::shot::cost_of_firing(&gun, &lines, 0, 100, 0),
        "which is the rifle line at no skill with a full wind"
    );
    assert!(fired.cost > 0, "and a shot is never free");

    // The same shot at bare ground uses a whole tile, so a small target is judged more harshly than nothing at all.
    let at_nothing = cdda_sim::shot::fire_at(&mut world, shooter, &gun, &lines, (4, 4, 0), 18.0, None);
    assert!(
        fired.aim.missed_by >= at_nothing.aim.missed_by,
        "the small thing is at least as hard to hit as empty ground: {} against {}",
        fired.aim.missed_by,
        at_nothing.aim.missed_by
    );
    assert!(
        at_nothing.hit.is_none(),
        "and firing at an empty square hits nothing, whatever the aim says"
    );

    // Now the blow itself: a target **on the shooter's own square**, which is zero range and therefore a certain hit whatever the dice say. The round is the corpus's .223.
    let rounds = cdda_content::ammo::load_dump(&read("items/ammo.json")).expect("the ammo loads");
    let round = rounds.iter().find(|round| round.id == "223").expect("the corpus has a .223").clone();
    let standing = cdda_sim::creature::spawn_creature(&mut world, "a thing in the way", (0, 0, 0));
    world
        .entity_mut(standing)
        .insert(cdda_sim::creature::Creature { name: "a thing in the way".to_owned(), size: cdda_sim::CreatureSize::Medium })
        .insert(cdda_sim::creature::Health { hp: 100, max_hp: 100 });
    let point_blank = cdda_sim::shot::fire_at(&mut world, shooter, &gun, &lines, (0, 0, 0), 18.0, Some(&round));
    assert_eq!(point_blank.hit, Some(standing), "at zero range the shot reaches what is there");
    assert_eq!(point_blank.hurt, 39, "and does the cartridge's thirty-nine, with no armour to stop it");
    assert_eq!(
        cdda_sim::creature::health_of(&world, standing),
        Some(61),
        "so the creature is left with sixty-one of its hundred"
    );
}

/// A shot's blow is the gun's own plus the round's, and armour stops it capped at the blow.
///
/// Reference: `item::gun_damage( with_ammo = true )` (`item.cpp:11160-11180`) adds the round's damage to the weapon's, and `absorb_hit` (`monster.cpp:2085`) takes the resistance off capped
/// at the blow. The gate uses the corpus's own numbers: a **longbow's six stab plus an arrow's** and a **Mini-14's zero plus a .223's thirty-nine** - which is the whole finding of rounds
/// 1146 and 1147, as arithmetic.
#[test]
fn a_shot_s_blow_is_its_gun_s_plus_its_round_s_and_armour_takes_it_off() {
    use cdda_sim::shot::blow_of;

    let read = |name: &str| {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("../../corpus/{name}"));
        let bytes = std::fs::read(path).expect("the corpus file exists");
        cdda_json::parse(&bytes).expect("valid JSON")
    };
    let guns = cdda_content::guns::load_dump(&read("items/guns.json")).expect("the guns load");
    let rounds = cdda_content::ammo::load_dump(&read("items/ammo.json")).expect("the ammo loads");
    let gun = |id: &str| guns.iter().find(|gun| gun.id == id).expect("the gun is in the dump").clone();
    let round = |id: &str| rounds.iter().find(|round| round.id == id).expect("the round is in the dump").clone();

    // A rifle carries no blow of its own; its cartridge carries all of it.
    let rifle = blow_of(&gun("ruger_mini"), Some(&round("223")));
    assert_eq!(rifle.stab, 39, "the .223's thirty-nine, and nothing from the gun");
    assert_eq!(rifle.bash, 0, "and no bash, because a cartridge is not a rock");
    assert_eq!(
        blow_of(&gun("ruger_mini"), None).stab,
        0,
        "the same gun with no round in it does nothing at all, which is what an unloaded rifle should do"
    );

    // A bow carries its own blow and its arrow adds to it.
    let bow_alone = blow_of(&gun("longbow"), None);
    assert_eq!(bow_alone.stab, 6, "a longbow's own six stab");
    let arrow = round("arrow_wood");
    let bow_and_arrow = blow_of(&gun("longbow"), Some(&arrow));
    assert_eq!(
        bow_and_arrow.stab,
        bow_alone.stab + arrow.damage_stab,
        "the arrow adds to the limbs rather than replacing them: 6 + {}",
        arrow.damage_stab
    );

    // A thrown rock is a round in this table too, and it is bash.
    let rock = blow_of(&gun("feral_human_thrown_rock"), Some(&round("rock")));
    assert!(rock.bash > 0, "a thrown rock is bash: {}", rock.bash);
}
