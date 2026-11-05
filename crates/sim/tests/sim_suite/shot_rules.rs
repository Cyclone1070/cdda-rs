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
    let fired = cdda_sim::shot::fire_at(&mut world, shooter, &gun, &lines, (4, 0, 0), 18.0, None, None, 0.0, false);
    let after = cdda_sim::creature::moves_left(&world, shooter);
    assert_eq!(before - after, fired.cost, "the shot cost what the shot table says: {}", fired.cost);
    assert_eq!(
        fired.cost,
        cdda_sim::shot::cost_of_firing(&gun, &lines, 0, 100, 0),
        "which is the rifle line at no skill with a full wind"
    );
    assert!(fired.cost > 0, "and a shot is never free");

    // The same shot at bare ground uses a whole tile, so a small target is judged more harshly than nothing at all.
    let at_nothing = cdda_sim::shot::fire_at(&mut world, shooter, &gun, &lines, (4, 4, 0), 18.0, None, None, 0.0, false);
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
    let point_blank = cdda_sim::shot::fire_at(&mut world, shooter, &gun, &lines, (0, 0, 0), 18.0, Some(&round), None, 0.0, false);
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

/// A shot spends the gun's rounds, and a gun that cannot pay for one does not fire at all.
///
/// Reference: `fire_gun`'s `ammo_consume` and its `break` (`ranged.cpp:1167-1172`). The gate checks the **store**: two rounds in the gun means two shots, the third finds nothing, and a shot
/// that could not load costs no moves and hits nothing - which is the difference between "out of ammo" reading as a failure and reading as a free action.
#[test]
fn a_gun_that_cannot_pay_for_a_round_does_not_fire() {
    let read = |name: &str| {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("../../corpus/{name}"));
        let bytes = std::fs::read(path).expect("the corpus file exists");
        cdda_json::parse(&bytes).expect("valid JSON")
    };
    let guns = cdda_content::guns::load_dump(&read("items/guns.json")).expect("the guns load");
    let lines = cdda_content::skills::load_dump(&read("skills.json")).expect("the skills load");
    let gun = guns.iter().find(|gun| gun.id == "ruger_mini").expect("the Mini-14").clone();

    let mut world = bevy_ecs::prelude::World::new();
    cdda_sim::singletons::put_rolls(&mut world, cdda_core_types::rng::Rolls::new(99));
    let shooter = cdda_sim::creature::spawn_creature(&mut world, "the shooter", (0, 0, 0));
    world.entity_mut(shooter).insert(cdda_sim::creature::Moves { left: 500 });
    // The gun is an item entity with two rounds in it, which is where the port keeps "how many of this thing are here".
    let held = cdda_sim::drop_item(&mut world, (0, 0), cdda_sim::ItemTypeId(0), 2);
    world.entity_mut(held).insert(cdda_sim::creature::WieldedBy(shooter));

    let mut fired = 0;
    for _ in 0..3 {
        let shot = cdda_sim::shot::fire_at(&mut world, shooter, &gun, &lines, (4, 0, 0), 18.0, None, Some(held), 0.0, false);
        if shot.loaded {
            fired += 1;
        } else {
            assert_eq!(shot.cost, 0, "a shot that could not load costs nothing");
            assert!(shot.hit.is_none(), "and hits nothing");
        }
    }
    assert_eq!(fired, 2, "two rounds in the gun is two shots");
    assert_eq!(
        world.get::<cdda_sim::GroundItem>(held).map(|item| item.charges),
        Some(0),
        "and the gun is empty"
    );
}

/// How hard a shot kicks, worked by hand from the reference's own formula.
///
/// Reference: `item::gun_recoil` (`item.cpp:11232-11266`). The gate takes the corpus's Mini-14 - `recoil` **0**, `handling` **20** - and its .223 round, `recoil` **1500**, and prices the kick
/// for an eight-strength arm at a gun weight the test chooses, because weight is a separate item column the guns dump does not carry yet. **The handling factor is 2.0 at twenty**, so a light
/// gun kicks half as hard as its rounds would say.
#[test]
fn a_shot_s_kick_is_the_round_s_less_what_handling_and_weight_take_off() {
    use cdda_sim::shot::gun_recoil;

    // weight 333 g is 1.0 in the reference's units, so the factor is 1^0.8 * 2^1.2 = 2.297…
    let factor = 2.0_f64.powf(1.2);
    #[allow(clippy::cast_possible_truncation)]
    let expected = (1500.0 / factor) as i32;
    assert_eq!(gun_recoil(0, 1500, 20, 333.0, 8), expected, "1500 over the handling factor");
    assert!(
        (gun_recoil(0, 1500, 20, 333.0, 8) - 652).abs() <= 1,
        "which is about 652 of the round's 1500"
    );

    // Poor handling (10, so 1.0 in the reference's units) does not reach the reference's divide branch at all: factor 1.0 takes the **multiply** branch, and a gun that handles at unit
    // weight *doubles* its round's kick - which is the asymmetry the reference's `handling > 1.0` test builds.
    assert_eq!(gun_recoil(0, 1500, 10, 333.0, 8), 3000, "handling 10 is factor 1.0, which multiplies by two");
    // Weight enters the **factor** the kick is divided by, so at equal handling a heavier gun kicks **less** - which is the reference's own bias, spelled in its comment ("heavier weapons
    // benefit more from improved handling") and easy to get backwards if the weight is read as a penalty.
    let heavy = gun_recoil(0, 1500, 20, 20_000.0, 8);
    let light = gun_recoil(0, 1500, 20, 333.0, 8);
    assert!(heavy < light, "the heavier gun is steadier: {heavy} against {light}");
    // And the arm's strength **caps** how much weight counts, at 333 g a point: a stronger arm on the same heavy gun is steadier still.
    let strong = gun_recoil(0, 1500, 20, 20_000.0, 40);
    assert!(strong < heavy, "a stronger arm tames it further: {strong} against {heavy}");
    // A gun's own recoil adds to the round's rather than replacing it.
    assert!(gun_recoil(500, 1500, 20, 333.0, 8) > gun_recoil(0, 1500, 20, 333.0, 8), "the gun's own kick adds");
}

/// Recoil accumulates with each shot and makes the next one worse.
///
/// Reference: `recoil += 5.0 * qty` (`ranged.cpp:1152-1154`), capped at `MAX_RECOIL` = 3000 (`game_constants.h:101`), and `total_gun_dispersion( gun, recoil_total(), ... )`
/// (`ranged.cpp:1108`) feeding that recoil into the shot's dispersion. **This is the rule that makes a burst worse than a series of single shots**, and the gate checks the store and the
/// shot's own aim rather than the arithmetic alone.
#[test]
fn recoil_accumulates_with_each_shot_and_widens_the_next() {
    use bevy_ecs::prelude::*;
    use cdda_sim::shot::{MAX_RECOIL, Recoil, recoil_after_a_shot};

    // The cap, and the fivefold scale that makes it matter.
    assert_eq!(recoil_after_a_shot(0, 100), 500, "a hundred-point kick adds five hundred");
    assert_eq!(recoil_after_a_shot(2_900, 100), MAX_RECOIL, "and the total stops at three thousand");
    assert_eq!(recoil_after_a_shot(0, 0), 0, "a kickless shot adds nothing");

    let read = |name: &str| {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("../../corpus/{name}"));
        let bytes = std::fs::read(path).expect("the corpus file exists");
        cdda_json::parse(&bytes).expect("valid JSON")
    };
    let guns = cdda_content::guns::load_dump(&read("items/guns.json")).expect("the guns load");
    let lines = cdda_content::skills::load_dump(&read("skills.json")).expect("the skills load");
    let rounds = cdda_content::ammo::load_dump(&read("items/ammo.json")).expect("the ammo loads");
    let gun = guns.iter().find(|gun| gun.id == "ruger_mini").expect("the Mini-14").clone();
    let round = rounds.iter().find(|round| round.id == "223").expect("the .223").clone();

    let mut world = World::new();
    cdda_sim::singletons::put_rolls(&mut world, cdda_core_types::rng::Rolls::new(777));
    let shooter = cdda_sim::creature::spawn_creature(&mut world, "the shooter", (0, 0, 0));
    world
        .entity_mut(shooter)
        .insert(cdda_sim::creature::Strength { base: 8 })
        .insert(cdda_sim::creature::Dexterity { base: 8 })
        .insert(cdda_sim::creature::Perception { base: 8 })
        .insert(cdda_sim::creature::Moves { left: 1_000 });

    // One shot at bare ground, then a second: the first leaves recoil behind, and the second is aimed with it.
    let first = cdda_sim::shot::fire_at(&mut world, shooter, &gun, &lines, (5, 0, 0), 18.0, Some(&round), None, 3180.0, false);
    let carried = world.get::<Recoil>(shooter).map_or(0, |recoil| recoil.points);
    assert!(carried > 0, "the shot left recoil behind: {carried}");
    assert_eq!(
        carried,
        recoil_after_a_shot(0, 123),
        "and it is five times the kick a Mini-14 with a .223 makes: **123**, because its 3,180 g is capped at the arm's 8 * 333 g"
    );

    let second = cdda_sim::shot::fire_at(&mut world, shooter, &gun, &lines, (5, 0, 0), 18.0, Some(&round), None, 3180.0, false);
    assert!(
        world.get::<Recoil>(shooter).map_or(0, |recoil| recoil.points) > carried,
        "and the second shot adds to it"
    );
    assert!(
        second.aim.dispersion > first.aim.dispersion,
        "the second shot is aimed with the first one's recoil in it: {} against {}",
        second.aim.dispersion,
        first.aim.dispersion
    );
}

/// A bow resets the shooter's aim to the cap instead of adding to it, and a gun does not.
///
/// Reference: `ranged.cpp:1230-1233` - `if( gun.has_flag( flag_RELOAD_AND_SHOOT ) ) { recoil = MAX_RECOIL; }`. The data says which weapons those are: a **longbow** and a **compound bow**
/// state `RELOAD_AND_SHOOT`, and a rifle states nothing at all - so this is the flag's rule and not the weapon's size.
#[test]
fn a_bow_starts_its_aim_over_and_a_rifle_carries_its_recoil() {
    use cdda_sim::shot::{MAX_RECOIL, recoil_after_firing};

    assert_eq!(recoil_after_firing(0, 123, true), MAX_RECOIL, "a bow resets to the cap whatever its kick");
    assert_eq!(recoil_after_firing(2_000, 123, true), MAX_RECOIL, "even from a shooter who was already shaking");
    assert_eq!(recoil_after_firing(0, 123, false), 615, "a gun adds five times its kick");
    assert_eq!(recoil_after_firing(2_990, 123, false), MAX_RECOIL, "and never passes the cap");
    // The two rules disagree for every input, which is why the flag has to be read rather than assumed.
    assert_ne!(
        recoil_after_firing(0, 123, true),
        recoil_after_firing(0, 123, false),
        "a bow and a rifle with the same kick do different things"
    );
}

/// A round's own spread is drawn as an angle, capped at thirty degrees, and only displaces the aim when it is wide across several tiles.
///
/// Reference: `ballistics.cpp:395-405`. The gate pins the three parts a port gets wrong: **the draw is absolute** (the sign of a normal sample carries no meaning here), **the cap is
/// thirty degrees** (1,800 arcmin, not 30 arcmin), and **`max_spread` must reach one** before the target square moves - so a birdshot's 300 arcmin is wide yet a one-tile shot never
/// scatters.
#[test]
fn a_round_s_spread_is_absolute_capped_and_only_moves_the_aim_across_tiles() {
    use cdda_core_types::rng::Rolls;
    use cdda_sim::shot::{shot_spread_arcmin, spread_across_tiles};

    let mut dice = Rolls::new(1234);
    // Every draw from a 300-arcmin round is between nothing and 300, and none is negative: the reference takes the absolute value.
    let mut seen_wide = false;
    for _ in 0..200 {
        let drawn = shot_spread_arcmin(300, &mut dice);
        assert!(drawn >= 0.0, "an absolute draw: {drawn}");
        assert!(drawn <= 300.0 || (drawn - 1800.0).abs() < f64::EPSILON, "within the round's own spread, or at the cap: {drawn}");
        if drawn > 100.0 {
            seen_wide = true;
        }
    }
    assert!(seen_wide, "and two hundred draws from three hundred arcmin reach past a hundred");
    assert!(shot_spread_arcmin(0, &mut dice).abs() < f64::EPSILON, "a round with no spread draws nothing");

    // One tile: no displacement at all, however wide. Two hundred arcmin across two tiles: 0.058 - under one, so still no displacement.
    assert!(spread_across_tiles(1800.0, 1).abs() < f64::EPSILON, "one tile never scatters");
    let two = spread_across_tiles(200.0, 2);
    assert!(two > 0.0 && two < 1.0, "two tiles and two hundred arcmin is a tenth of a square: {two}");
    // Enough tiles at a wide enough arc does pass one, which is the only case the reference moves the target square.
    assert!(spread_across_tiles(1800.0, 10) >= 1.0, "ten tiles at the cap is more than a square: {}", spread_across_tiles(1800.0, 10));
}

/// A wide round scatters off a distant square and never off a near one, and the shot reports it as a miss.
///
/// Reference: `ballistics.cpp:395-405`, where a `max_spread` of one or more rotates the target square. **This is the wiring round 1160 left undone**: the pellet's own wander is drawn from the
/// round in the gun and judged against the range, so a birdshot fired at a zombie across twenty squares can miss a target it was aimed at, and one fired across two cannot.
#[test]
fn a_birdshot_scatters_off_a_distant_square_and_not_off_a_near_one() {
    use bevy_ecs::prelude::*;
    use cdda_core_types::rng::Rolls;

    let read = |name: &str| {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("../../corpus/{name}"));
        let bytes = std::fs::read(path).expect("the corpus file exists");
        cdda_json::parse(&bytes).expect("valid JSON")
    };
    let guns = cdda_content::guns::load_dump(&read("items/guns.json")).expect("the guns load");
    let lines = cdda_content::skills::load_dump(&read("skills.json")).expect("the skills load");
    let rounds = cdda_content::ammo::load_dump(&read("items/ammo.json")).expect("the ammo loads");
    // The gun names the **ammunition type** it takes (`shot`), not the round (`shot_bird`); the test passes the round to `fire_at` directly, which is what the port's loader does with
    // `gun.ammo` when the two ids happen to agree.
    let gun = guns.iter().find(|gun| gun.id == "pipe_shotgun").expect("a shotgun").clone();
    assert_eq!(gun.ammo, "shot", "the pipe shotgun takes the `shot` ammunition type");
    let bird = rounds.iter().find(|round| round.id == "shot_bird").expect("birdshot").clone();
    assert_eq!(bird.shot_spread, 300, "birdshot is three hundred arcmin wide");

    // One shooter, one world: only the range changes, which is what makes the two runs a differential.
    let fire = |range: i32, seed: u32| {
        let mut world = World::new();
        cdda_sim::singletons::put_rolls(&mut world, Rolls::new(seed));
        let shooter = cdda_sim::creature::spawn_creature(&mut world, "the shooter", (0, 0, 0));
        world
            .entity_mut(shooter)
            .insert(cdda_sim::creature::Strength { base: 8 })
            .insert(cdda_sim::creature::Dexterity { base: 20 })
            .insert(cdda_sim::creature::Perception { base: 20 })
            .insert(cdda_sim::creature::Moves { left: 1_000 });
        let target = cdda_sim::creature::spawn_creature(&mut world, "the target", (range, 0, 0));
        world.entity_mut(target).insert(cdda_sim::creature::Health { hp: 80, max_hp: 80 });
        // A `NEVER_JAMS` gun, so the misfire roll round 1172 added does not eat one of the four seeds' draws.
        let held = world.spawn((cdda_sim::shot::NeverJams, cdda_sim::Position { x: 0, y: 0, z: 0 })).id();
        cdda_sim::shot::fire_at(&mut world, shooter, &gun, &lines, (range, 0, 0), 18.0, Some(&bird), Some(held), 3_000.0, false)
    };

    // Four seeds: at twenty squares at least one pellet wanders a whole square, and at two squares none ever can - `spread * (2 - 1)` is under one for any draw within 300 arcmin.
    let mut scattered_far = 0;
    let mut scattered_near = 0;
    for seed in [11_u32, 22, 33, 44] {
        if fire(20, seed).aim.missed_by >= 1.0 {
            scattered_far += 1;
        }
        if fire(2, seed).aim.missed_by >= 1.0 && fire(2, seed).hit.is_none() {
            scattered_near += 1;
        }
    }
    assert!(scattered_far > 0, "a birdshot at twenty squares scatters at least once in four seeds: {scattered_far}");
    assert!(scattered_near < 4, "and at two squares it is not the scatter doing the missing: {scattered_near}");
}

/// A round with a count fires that many pellets, and a cartridge's count of one fires one.
///
/// Reference: `for( int j = 0; j < proj.count; ++j )` (`ballistics.cpp:388`) with `proj.count` from `islot_ammo::count` - **twenty** for birdshot, one for a .223. This is the rule that makes
/// a shotgun a shotgun: the same shot step, the same price, twenty times the blow if the pellets land.
#[test]
fn a_shotgun_fires_its_round_s_whole_count_and_a_rifle_fires_one() {
    use bevy_ecs::prelude::*;
    use cdda_core_types::rng::Rolls;

    let read = |name: &str| {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("../../corpus/{name}"));
        let bytes = std::fs::read(path).expect("the corpus file exists");
        cdda_json::parse(&bytes).expect("valid JSON")
    };
    let guns = cdda_content::guns::load_dump(&read("items/guns.json")).expect("the guns load");
    let lines = cdda_content::skills::load_dump(&read("skills.json")).expect("the skills load");
    let rounds = cdda_content::ammo::load_dump(&read("items/ammo.json")).expect("the ammo loads");
    let round = |id: &str| rounds.iter().find(|round| round.id == id).expect("the round is in the dump").clone();
    let gun = |id: &str| guns.iter().find(|gun| gun.id == id).expect("the gun is in the dump").clone();
    assert_eq!(round("shot_bird").count, 20, "birdshot is twenty pellets");
    assert_eq!(round("223").count, 1, "a cartridge is one");

    // The same shooter and the same dice, once with a shotgun and once with a rifle, at one square - so the count is the only thing that differs.
    let fire = |gun: &cdda_content::guns::GunType, round: &cdda_content::ammo::AmmoType| {
        let mut world = World::new();
        cdda_sim::singletons::put_rolls(&mut world, Rolls::new(99));
        let shooter = cdda_sim::creature::spawn_creature(&mut world, "the shooter", (0, 0, 0));
        world
            .entity_mut(shooter)
            .insert(cdda_sim::creature::Strength { base: 8 })
            .insert(cdda_sim::creature::Dexterity { base: 20 })
            .insert(cdda_sim::creature::Perception { base: 20 })
            .insert(cdda_sim::creature::Moves { left: 1_000 });
        let target = cdda_sim::creature::spawn_creature(&mut world, "the target", (1, 0, 0));
        world.entity_mut(target).insert(cdda_sim::creature::Health { hp: 500, max_hp: 500 });
        // `NEVER_JAMS`, because round 1172 put a misfire roll ahead of every shot and this gate is about the pellet count rather than the dice.
        let held = world.spawn((cdda_sim::shot::NeverJams, cdda_sim::Position { x: 0, y: 0, z: 0 })).id();
        let fired = cdda_sim::shot::fire_at(&mut world, shooter, gun, &lines, (1, 0, 0), 18.0, Some(round), Some(held), 3_000.0, false);
        fired.hurt
    };

    let shotgun_hurt = fire(&gun("pipe_shotgun"), &round("shot_bird"));
    let rifle_hurt = fire(&gun("ruger_mini"), &round("223"));
    assert!(rifle_hurt > 0, "the rifle's single pellet lands at one square: {rifle_hurt}");
    assert_eq!(rifle_hurt, 39, "and does the .223's whole blow");
    assert!(
        shotgun_hurt > rifle_hurt,
        "while twenty pellets do far more: {shotgun_hurt} against {rifle_hurt}"
    );
    assert!(
        shotgun_hurt > 39,
        "which is more than any one pellet could: {shotgun_hurt}"
    );
}

/// A shot's noise is the gun's figure plus the round's, and the round carries almost all of it.
///
/// Reference: `item::gun_noise` (`ranged.cpp:2394-2409`) - `noise = type->gun->loudness`, `+= ammo->loudness` when the gun has a round, then clamped at zero. **The corpus says the gun's own
/// figure is zero for a rifle and the round's is not**, which is the fact a port that read only the gun would get exactly backwards.
#[test]
fn a_shot_s_noise_comes_from_the_round_and_not_from_the_gun() {
    use cdda_sim::shot::gun_noise;

    let read = |name: &str| {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("../../corpus/{name}"));
        let bytes = std::fs::read(path).expect("the corpus file exists");
        cdda_json::parse(&bytes).expect("valid JSON")
    };
    let guns = cdda_content::guns::load_dump(&read("items/guns.json")).expect("the guns load");
    let rounds = cdda_content::ammo::load_dump(&read("items/ammo.json")).expect("the ammo loads");
    let gun = |id: &str| guns.iter().find(|gun| gun.id == id).expect("the gun is in the dump").clone();
    let round = |id: &str| rounds.iter().find(|round| round.id == id).expect("the round is in the dump").clone();

    let rifle = gun("ruger_mini");
    let cartridge = round("223");
    assert_eq!(rifle.loudness, 0, "the rifle itself states no loudness at all");
    assert!(cartridge.loudness > 100, "while the .223 is loud: {}", cartridge.loudness);
    assert_eq!(
        gun_noise(rifle.loudness, cartridge.loudness),
        cartridge.loudness,
        "so the shot's noise is the round's, to the point"
    );

    let bird = round("shot_bird");
    assert!(bird.loudness > 0, "and birdshot states its own: {}", bird.loudness);
    assert_eq!(gun_noise(0, bird.loudness), bird.loudness, "the same rule, another round");

    // The floor, which is the one branch: a round that somehow stated negative loudness cannot make a shot quieter than nothing.
    assert_eq!(gun_noise(0, -50), 0, "a negative round is floored at zero");
    assert_eq!(gun_noise(10, 5), 15, "and the two add when both state a figure");
}

/// A shot is a sound the world hears, placed where the shooter stands and as loud as its round.
///
/// Reference: `ranged.cpp:1180-1190` calls `sounds::sound( pos, gun_noise.volume, ... )` after the shot. **This is the wiring round 1164 left undone**, and the gate asks the store rather than
/// the return value: a `Sound` entity exists, it is at the shooter's square, and its volume is the round's own.
#[test]
fn a_shot_puts_a_sound_in_the_world_at_the_shooter_s_square() {
    use bevy_ecs::prelude::*;
    use cdda_core_types::rng::Rolls;

    let read = |name: &str| {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("../../corpus/{name}"));
        let bytes = std::fs::read(path).expect("the corpus file exists");
        cdda_json::parse(&bytes).expect("valid JSON")
    };
    let guns = cdda_content::guns::load_dump(&read("items/guns.json")).expect("the guns load");
    let lines = cdda_content::skills::load_dump(&read("skills.json")).expect("the skills load");
    let rounds = cdda_content::ammo::load_dump(&read("items/ammo.json")).expect("the ammo loads");
    let gun = guns.iter().find(|gun| gun.id == "ruger_mini").expect("the Mini-14").clone();
    let cartridge = rounds.iter().find(|round| round.id == "223").expect("the .223").clone();

    let mut world = World::new();
    cdda_sim::singletons::put_rolls(&mut world, Rolls::new(7));
    let shooter = cdda_sim::creature::spawn_creature(&mut world, "the shooter", (3, 4, 0));
    world
        .entity_mut(shooter)
        .insert(cdda_sim::creature::Strength { base: 8 })
        .insert(cdda_sim::creature::Dexterity { base: 8 })
        .insert(cdda_sim::creature::Perception { base: 8 })
        .insert(cdda_sim::creature::Moves { left: 1_000 });

    assert!(cdda_sim::sounds::heard_this_turn(&mut world).is_empty(), "nothing has been heard yet");
    // The gun is given `NEVER_JAMS` **because round 1172 added a misfire roll ahead of every shot**: a durability-8 gun jams once in 1,536 tries, and this gate is about the sound rather than
    // about the dice. A scenario that wants a specific shot has to say the gun cannot jam.
    let held = world.spawn((cdda_sim::shot::NeverJams, cdda_sim::Position { x: 3, y: 4, z: 0 })).id();
    let fired = cdda_sim::shot::fire_at(&mut world, shooter, &gun, &lines, (6, 4, 0), 18.0, Some(&cartridge), Some(held), 3_180.0, false);
    assert!(fired.loaded, "the shot fired");

    let heard = cdda_sim::sounds::heard_this_turn(&mut world);
    assert_eq!(heard.len(), 1, "and it made exactly one sound");
    let Some(sound) = heard.first() else {
        return;
    };
    assert_eq!(sound.volume, cartridge.loudness, "as loud as the round: {}", cartridge.loudness);
    assert_eq!(sound.position, (3, 4, 0), "and at the shooter's own square, not the target's");
    assert_eq!(fired.noise, cartridge.loudness, "and the report says the same number the world heard");
}

/// A burst fires until the gun runs dry, and each shot is worse aimed than the one before it.
///
/// Reference: `ranged.cpp:1130-1240` - `for( curshot < burst_size )` with the rounds spent inside the loop and a `break` when the gun cannot fire. **The two claims are different**: the first is
/// about the loop stopping (a two-round gun asked for three shots fires two), the second is about the recoil the loop carries (the third shot's dispersion is wider than the first's).
#[test]
fn a_burst_fires_until_the_gun_runs_dry_and_gets_worse_as_it_goes() {
    use bevy_ecs::prelude::*;
    use cdda_core_types::rng::Rolls;

    let read = |name: &str| {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("../../corpus/{name}"));
        let bytes = std::fs::read(path).expect("the corpus file exists");
        cdda_json::parse(&bytes).expect("valid JSON")
    };
    let guns = cdda_content::guns::load_dump(&read("items/guns.json")).expect("the guns load");
    let lines = cdda_content::skills::load_dump(&read("skills.json")).expect("the skills load");
    let rounds = cdda_content::ammo::load_dump(&read("items/ammo.json")).expect("the ammo loads");
    let gun = guns.iter().find(|gun| gun.id == "ruger_mini").expect("the Mini-14").clone();
    let cartridge = rounds.iter().find(|round| round.id == "223").expect("the .223").clone();

    let mut world = World::new();
    cdda_sim::singletons::put_rolls(&mut world, Rolls::new(4242));
    let shooter = cdda_sim::creature::spawn_creature(&mut world, "the shooter", (0, 0, 0));
    world
        .entity_mut(shooter)
        .insert(cdda_sim::creature::Strength { base: 8 })
        .insert(cdda_sim::creature::Dexterity { base: 20 })
        .insert(cdda_sim::creature::Perception { base: 20 })
        .insert(cdda_sim::creature::Moves { left: 1_000 });
    // A gun entity holding **two** rounds, so a burst of three must stop at two.
    let held = world
        .spawn((
            cdda_sim::GroundItem {
                kind: cdda_sim::ItemTypeId(7),
                charges: 2,
                age: 0,
                burnt: 0,
                damage: 0,
                burn: None,
            },
            cdda_sim::Position { x: 0, y: 0, z: 0 },
        ))
        .id();

    let burst = cdda_sim::shot::fire_burst(&mut world, shooter, &gun, &lines, (8, 0, 0), 18.0, Some(&cartridge), Some(held), 3_180.0, false, 3, false);
    assert_eq!(burst.len(), 2, "two rounds in the gun, three asked for: two shots and a stop");
    assert!(burst.iter().all(|shot| shot.loaded), "and both of them fired");
    assert_eq!(
        world.get::<cdda_sim::GroundItem>(held).map(|item| item.charges),
        Some(0),
        "the gun is empty afterwards"
    );
    // The recoil the first shot left is in the second shot's dispersion.
    assert!(
        burst.last().map(|shot| shot.aim.dispersion) > burst.first().map(|shot| shot.aim.dispersion),
        "the second shot is aimed worse than the first: {:?} against {:?}",
        burst.last().map(|shot| shot.aim.dispersion),
        burst.first().map(|shot| shot.aim.dispersion)
    );
    // A burst of one is one shot, and a burst of none is none.
    assert_eq!(cdda_sim::shot::fire_burst(&mut world, shooter, &gun, &lines, (8, 0, 0), 18.0, Some(&cartridge), None, 3_180.0, false, 1, false).len(), 1, "one shot asked for");
    assert!(cdda_sim::shot::fire_burst(&mut world, shooter, &gun, &lines, (8, 0, 0), 18.0, Some(&cartridge), None, 3_180.0, false, 0, false).is_empty(), "and none for none");
}

/// A burst trains the shooter's skills, and a weapon that says it will not train marksmanship trains neither.
///
/// Reference: `ranged.cpp:1207-1213` - the base `gun` skill by `(hits + 1) * 5` unless the weapon carries `WONT_TRAIN_MARKSMANSHIP`, and the weapon's own skill by `(units + 1) * 5`, where
/// `units` is the hits except for a **launcher**, which trains by shots fired. The gate checks the store's skill bank, because that is where practice lands.
#[test]
fn a_burst_trains_the_skills_it_should_and_a_flagged_weapon_trains_none() {
    use bevy_ecs::prelude::*;
    use cdda_core_types::rng::Rolls;

    let read = |name: &str| {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("../../corpus/{name}"));
        let bytes = std::fs::read(path).expect("the corpus file exists");
        cdda_json::parse(&bytes).expect("valid JSON")
    };
    let guns = cdda_content::guns::load_dump(&read("items/guns.json")).expect("the guns load");
    let lines = cdda_content::skills::load_dump(&read("skills.json")).expect("the skills load");
    let rounds = cdda_content::ammo::load_dump(&read("items/ammo.json")).expect("the ammo loads");
    let gun = guns.iter().find(|gun| gun.id == "ruger_mini").expect("the Mini-14").clone();
    let cartridge = rounds.iter().find(|round| round.id == "223").expect("the .223").clone();

    let exercise_of = |wont_train: bool| {
        let mut world = World::new();
        cdda_sim::singletons::put_rolls(&mut world, Rolls::new(808));
        let shooter = cdda_sim::creature::spawn_creature(&mut world, "the shooter", (0, 0, 0));
        world
            .entity_mut(shooter)
            .insert(cdda_sim::creature::Strength { base: 8 })
            .insert(cdda_sim::creature::Dexterity { base: 20 })
            .insert(cdda_sim::creature::Perception { base: 20 })
            .insert(cdda_sim::creature::Moves { left: 1_000 });
        let target = cdda_sim::creature::spawn_creature(&mut world, "the target", (1, 0, 0));
        world.entity_mut(target).insert(cdda_sim::creature::Health { hp: 500, max_hp: 500 });
        let made = cdda_sim::shot::fire_burst(&mut world, shooter, &gun, &lines, (1, 0, 0), 18.0, Some(&cartridge), None, 3_180.0, false, 2, wont_train);
        assert_eq!(made.len(), 2, "two shots were asked for and the gun was loaded");
        let banked = world.get::<cdda_sim::creature::Exercise>(shooter).cloned().unwrap_or_default();
        (banked.0.get("gun").copied().unwrap_or(0), banked.0.get("rifle").copied().unwrap_or(0))
    };

    let (gun_skill, rifle_skill) = exercise_of(false);
    assert!(gun_skill > 0, "the base gun skill was trained: {gun_skill}");
    assert!(rifle_skill > 0, "and the rifle skill with it: {rifle_skill}");
    // At least five a burst for the base skill, because `( hits + 1 ) * 5` counts the shot even when it misses.
    assert!(gun_skill >= 5, "the base skill gains at least five a burst: {gun_skill}");

    let (flagged_gun, flagged_rifle) = exercise_of(true);
    assert_eq!(flagged_gun, 0, "a weapon flagged WONT_TRAIN_MARKSMANSHIP trains no gun skill");
    assert!(flagged_rifle > 0, "but its own weapon skill still trains, because that rule is not the flagged one: {flagged_rifle}");
}

/// A filthy gun misfires and a badly made one can too, and a `NEVER_JAMS` gun never does.
///
/// Reference: `Character::handle_gun_damage` (`ranged.cpp:700-800`) - the dirt branch `x_in_y( dirt^3, 10^12 )` and the mechanical one `one_in( ( 2 << durability ) * 3 )`. **The gate counts
/// draws rather than trusting one**: a rule that returns false always would pass a single-shot check, so the counts are what make the claim.
#[test]
fn a_filthy_gun_misfires_and_a_never_jams_gun_never_does() {
    use cdda_core_types::rng::Rolls;
    use cdda_sim::shot::misfires;

    // Dirt at the reference's maximum of ten thousand is a **certainty**: 10,000^3 over 10^12 is one.
    let mut dice = Rolls::new(5);
    assert!(misfires(10_000, 8, false, &mut dice), "dirt at the cap misfires every time");
    // No dirt at all leaves only the mechanical dice, and a durable gun rarely rolls it: count a thousand.
    let mut dice = Rolls::new(5);
    let mut jams = 0;
    for _ in 0..1_000 {
        if misfires(0, 8, false, &mut dice) {
            jams += 1;
        }
    }
    assert!(jams > 0, "a durability-8 gun still misfires sometimes in a thousand shots: {jams}");
    assert!(jams < 50, "but rarely: {jams} in a thousand, against the one in 1,536 the shift says");
    // A worse made gun jams more often, and the two runs differ only in durability.
    let mut dice = Rolls::new(5);
    let mut rough = 0;
    for _ in 0..1_000 {
        if misfires(0, 3, false, &mut dice) {
            rough += 1;
        }
    }
    assert!(rough > jams, "a durability-3 gun is worse: {rough} against {jams}");
    // And the flag skips both branches, even at maximum dirt.
    let mut dice = Rolls::new(5);
    assert!(!misfires(10_000, 8, true, &mut dice), "NEVER_JAMS ignores the dirt entirely");
}

/// A filthy gun does not fire at all, and the round it would have used is still in it.
///
/// Reference: `if( !handle_gun_damage( gun ) ) break;` (`ranged.cpp:1078`) - the check is **before** the projectile and before the round is spent, so a misfire costs nothing but the attempt. The
/// gate sets the gun's dirt to the reference's maximum, where the chance is a certainty, so the claim does not depend on a lucky seed.
#[test]
fn a_filthy_gun_misfires_instead_of_firing_and_keeps_its_round() {
    use bevy_ecs::prelude::*;
    use cdda_core_types::rng::Rolls;

    let read = |name: &str| {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("../../corpus/{name}"));
        let bytes = std::fs::read(path).expect("the corpus file exists");
        cdda_json::parse(&bytes).expect("valid JSON")
    };
    let guns = cdda_content::guns::load_dump(&read("items/guns.json")).expect("the guns load");
    let lines = cdda_content::skills::load_dump(&read("skills.json")).expect("the skills load");
    let rounds = cdda_content::ammo::load_dump(&read("items/ammo.json")).expect("the ammo loads");
    let gun = guns.iter().find(|gun| gun.id == "ruger_mini").expect("the Mini-14").clone();
    let cartridge = rounds.iter().find(|round| round.id == "223").expect("the .223").clone();

    let fire = |dirt: i32, never_jams: bool| {
        let mut world = World::new();
        cdda_sim::singletons::put_rolls(&mut world, Rolls::new(3));
        let shooter = cdda_sim::creature::spawn_creature(&mut world, "the shooter", (0, 0, 0));
        world
            .entity_mut(shooter)
            .insert(cdda_sim::creature::Strength { base: 8 })
            .insert(cdda_sim::creature::Dexterity { base: 20 })
            .insert(cdda_sim::creature::Perception { base: 20 })
            .insert(cdda_sim::creature::Moves { left: 1_000 });
        let target = cdda_sim::creature::spawn_creature(&mut world, "the target", (1, 0, 0));
        world.entity_mut(target).insert(cdda_sim::creature::Health { hp: 500, max_hp: 500 });
        let held = world
            .spawn((
                cdda_sim::GroundItem {
                    kind: cdda_sim::ItemTypeId(7),
                    charges: 5,
                    age: 0,
                    burnt: 0,
                    damage: 0,
                    burn: None,
                },
                cdda_sim::Position { x: 0, y: 0, z: 0 },
                cdda_sim::shot::Dirt { points: dirt },
            ))
            .id();
        if never_jams {
            world.entity_mut(held).insert(cdda_sim::shot::NeverJams);
        }
        let fired = cdda_sim::shot::fire_at(&mut world, shooter, &gun, &lines, (1, 0, 0), 18.0, Some(&cartridge), Some(held), 3_180.0, false);
        let left = world.get::<cdda_sim::GroundItem>(held).map(|item| item.charges);
        (fired, left)
    };

    let (fired, left) = fire(cdda_sim::shot::MAX_DIRT, false);
    assert!(fired.misfired, "a gun at maximum dirt misfires");
    assert!(!fired.loaded || fired.hurt == 0, "and nothing was hurt: {}", fired.hurt);
    assert_eq!(left, Some(5), "and the round is still in the gun: {left:?}");
    assert_eq!(fired.cost, 0, "and no moves were spent: {}", fired.cost);
    assert!(fired.hit.is_none(), "and nothing was hit");

    // The clean case carries `NEVER_JAMS`, **so the gate does not depend on the dice**: a durability-8 gun misfires once in 1,536 shots, and a seed that happened to roll it would make this
    // assertion flake rather than fail. The claim being tested is that a gun which *can* misfire does so at maximum dirt and a gun which cannot never does.
    let (clean, left) = fire(0, true);
    assert!(!clean.misfired, "a gun that cannot jam fires even with a seed that would jam another");
    assert_eq!(left, Some(4), "and spends its round: {left:?}");
    assert!(clean.hurt > 0, "and lands on the target it was aimed at: {}", clean.hurt);
}

/// Every ordinary shot adds one point of dirt, and ordinary ammunition stops fouling at 7,150 while black powder goes to the maximum.
///
/// Reference: `ranged.cpp:864-872`. **The two caps are the whole point**: 10,000 is where a misfire is certain, and 7,150 is where *ordinary* ammunition stops - so a gun fired with cartridges
/// can never get as bad as one fired with black powder, which is a rule a single "cap at the maximum" would lose.
#[test]
fn a_shot_dirties_the_gun_by_one_until_ordinary_ammunition_stops_at_7150() {
    use cdda_sim::shot::{MAX_DIRT, dirt_after_a_shot};

    assert_eq!(dirt_after_a_shot(0, false, 0, false), 1, "a clean gun takes one point");
    assert_eq!(dirt_after_a_shot(100, false, 0, false), 101, "and one more with every shot");
    assert_eq!(dirt_after_a_shot(7_149, false, 0, false), 7_150, "until the ordinary cap");
    assert_eq!(dirt_after_a_shot(7_150, false, 0, false), 7_150, "where it stops, well short of the maximum");
    assert_eq!(dirt_after_a_shot(MAX_DIRT, false, 0, false), MAX_DIRT, "and nothing goes past the maximum");

    // Black powder jumps by `200 - tolerance * 2` and is allowed to the top.
    assert_eq!(dirt_after_a_shot(0, true, 0, false), 201, "black powder adds two hundred and one");
    assert_eq!(dirt_after_a_shot(0, true, 5, false), 191, "less the weapon's tolerance, two a point");
    assert_eq!(dirt_after_a_shot(9_950, true, 0, false), MAX_DIRT, "and reaches the maximum, where cartridges cannot");
    assert!(dirt_after_a_shot(0, true, 200, false) >= 1, "a tolerance past the additive still leaves one point: {}", dirt_after_a_shot(0, true, 200, false));

    // A non-fouling round leaves the gun exactly as it was, which is the branch the flag exists for.
    assert_eq!(dirt_after_a_shot(500, false, 0, true), 500, "NON_FOULING adds nothing");
    assert_eq!(dirt_after_a_shot(0, true, 0, true), 0, "not even black powder");
}

/// Every shot leaves the gun dirtier, so a gun that keeps firing tends towards a misfire.
///
/// Reference: `ranged.cpp:864-872`. The gate asks the **store** after the shot rather than the rule's return value, because the claim is that the number on the gun's entity moved - which is what
/// makes round 1171's misfire reachable in play rather than only in a hand-built world.
#[test]
fn every_shot_leaves_the_gun_dirtier() {
    use bevy_ecs::prelude::*;
    use cdda_core_types::rng::Rolls;

    let read = |name: &str| {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("../../corpus/{name}"));
        let bytes = std::fs::read(path).expect("the corpus file exists");
        cdda_json::parse(&bytes).expect("valid JSON")
    };
    let guns = cdda_content::guns::load_dump(&read("items/guns.json")).expect("the guns load");
    let lines = cdda_content::skills::load_dump(&read("skills.json")).expect("the skills load");
    let rounds = cdda_content::ammo::load_dump(&read("items/ammo.json")).expect("the ammo loads");
    let gun = guns.iter().find(|gun| gun.id == "ruger_mini").expect("the Mini-14").clone();
    let cartridge = rounds.iter().find(|round| round.id == "223").expect("the .223").clone();

    let mut world = World::new();
    cdda_sim::singletons::put_rolls(&mut world, Rolls::new(91));
    let shooter = cdda_sim::creature::spawn_creature(&mut world, "the shooter", (0, 0, 0));
    world
        .entity_mut(shooter)
        .insert(cdda_sim::creature::Strength { base: 8 })
        .insert(cdda_sim::creature::Dexterity { base: 20 })
        .insert(cdda_sim::creature::Perception { base: 20 })
        .insert(cdda_sim::creature::Moves { left: 1_000 });
    let held = world
        .spawn((
            cdda_sim::GroundItem {
                kind: cdda_sim::ItemTypeId(7),
                charges: 10,
                age: 0,
                burnt: 0,
                damage: 0,
                burn: None,
            },
            cdda_sim::Position { x: 0, y: 0, z: 0 },
            cdda_sim::shot::NeverJams,
        ))
        .id();
    assert_eq!(world.get::<cdda_sim::shot::Dirt>(held).map(|dirt| dirt.points), None, "a new gun carries no dirt component at all");

    for expected in 1..=3 {
        let fired = cdda_sim::shot::fire_at(&mut world, shooter, &gun, &lines, (5, 0, 0), 18.0, Some(&cartridge), Some(held), 3_180.0, false);
        assert!(!fired.misfired, "the gun cannot jam, so the shot fired");
        assert_eq!(
            world.get::<cdda_sim::shot::Dirt>(held).map(|dirt| dirt.points),
            Some(expected),
            "and after shot {expected} the gun is that much dirtier"
        );
    }
}
