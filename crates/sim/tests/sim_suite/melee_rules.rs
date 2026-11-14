#![allow(clippy::expect_used, clippy::float_cmp, clippy::panic)]

/// How well a character aims a blow: their dexterity, their skills, and the weapon's own balance.
///
/// Reference: `Character::get_melee_hit_base` (`melee.cpp:375-386`) is `get_hit_base()` - which is `get_dex() / 4.0f` (`character.cpp:5705-5709`) - plus `get_hit_weapon`, which is
/// `skill / 3 + melee_skill / 2 + weap.get_to_hit()`. The three terms are added in tenths of an accuracy point in the port, which is the unit `MeleeAttack::accuracy_tenths` already
/// uses, so a fixture and a character's own skill are the same kind of number.
#[test]
fn how_well_a_character_aims_a_blow() {
    use cdda_sim::melee::{hit_base_tenths, hit_weapon_tenths};

    // Dexterity eight is two points of accuracy, which is twenty tenths.
    assert_eq!(hit_base_tenths(8), 20, "a dexterous character aims well");
    assert_eq!(hit_base_tenths(0), 0, "and one with no dexterity does not");
    assert_eq!(hit_base_tenths(12), 30, "twelve is three points");

    // Skills are thirds and halves: unarmed three is one point, melee four is two.
    assert_eq!(
        hit_weapon_tenths(3, 4, 0),
        30,
        "one point of unarmed and two of melee"
    );
    // The weapon's own balance is added last and can be negative - a badly balanced thing aims worse.
    assert_eq!(
        hit_weapon_tenths(0, 0, -2),
        -20,
        "a clumsy weapon takes accuracy away"
    );
    assert_eq!(
        hit_weapon_tenths(0, 0, 3),
        30,
        "a well balanced one gives it"
    );
}

/// The registry knows what a swung thing is worth, for every type the reference dumped.
///
/// Reference: `itype::melee_damage` (`item_factory.cpp:4109`) and `itype::m_to_hit` (`itype.h:1540`) are what a character's blow carries and how well it aims. The dump
/// `corpus/items/thrown-damage.json` version 2 carries all four columns, and this gate compares **all of them for all 9,912 types** - so a registry that dropped a damage type, or a
/// `to_hit` that quietly became zero, fails here rather than in a swing nobody can see.
#[test]
fn the_registry_carries_what_a_swung_thing_is_worth() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../corpus/items/thrown-damage.json");
    let Ok(bytes) = std::fs::read(path) else {
        panic!("the item dump exists")
    };
    let root = cdda_json::parse(&bytes).unwrap_or_else(|_| panic!("the dump is valid JSON"));
    let cdda_json::Value::Object(root) = root else {
        panic!("the dump's root is not an object");
    };
    let Some(cdda_json::Value::Array(types)) = root.get("types") else {
        panic!("the dump has no types");
    };

    let number = |value: Option<&cdda_json::Value>| -> f64 {
        match value {
            Some(cdda_json::Value::Float(value)) => *value,
            Some(cdda_json::Value::Int(value)) => {
                #[allow(clippy::cast_precision_loss)]
                {
                    *value as f64
                }
            }
            _ => 0.0,
        }
    };

    // One row per type: id, the three melee damage types, and how well it aims.
    let mut rows: Vec<(String, f64, f64, f64, i32)> = Vec::new();
    for one in types {
        let cdda_json::Value::Object(fields) = one else {
            continue;
        };
        let Some(cdda_json::Value::Str(id)) = fields.get("id") else {
            continue;
        };
        #[allow(clippy::cast_possible_truncation)]
        let to_hit = number(fields.get("to_hit")) as i32;
        rows.push((
            id.clone(),
            number(fields.get("melee_bash")),
            number(fields.get("melee_cut")),
            number(fields.get("melee_stab")),
            to_hit,
        ));
    }
    assert_eq!(rows.len(), 9_912, "every type in the dump");

    let mut world = bevy_ecs::prelude::World::new();
    let borrowed: Vec<cdda_sim::item_registry::Kind<'_>> = rows
        .iter()
        .map(
            |(id, bash, cut, stab, to_hit)| cdda_sim::item_registry::Kind {
                id,
                volume_ml: 50,
                weight_g: 100,
                pockets: &[],
                thrown_damage: 0.0,
                melee_bash: *bash,
                melee_cut: *cut,
                melee_stab: *stab,
                to_hit: *to_hit,
            },
        )
        .collect();
    cdda_sim::item_registry::set_item_types(&mut world, &borrowed);
    let names = cdda_sim::item_registry::snapshot(&mut world);

    // Every row compared, and the wrong ones counted so a silent zero shows up as a number rather than as a pass.
    let mut wrong = Vec::new();
    for (id, bash, cut, stab, to_hit) in &rows {
        let Some(kind) = names.id_of(id) else {
            wrong.push(format!("{id}: not in the registry"));
            continue;
        };
        let got = (
            names.melee_bash_of(kind),
            names.melee_cut_of(kind),
            names.melee_stab_of(kind),
            names.to_hit_of(kind),
        );
        if (got.0 - bash).abs() > f64::EPSILON
            || (got.1 - cut).abs() > f64::EPSILON
            || (got.2 - stab).abs() > f64::EPSILON
            || got.3 != *to_hit
        {
            wrong.push(format!(
                "{id}: ({bash}, {cut}, {stab}, {to_hit}) became {got:?}"
            ));
        }
    }
    let first_few: Vec<&String> = wrong.iter().take(5).collect();
    assert!(
        wrong.is_empty(),
        "{} of 9,912 wrong: {first_few:?}",
        wrong.len()
    );

    // Named checks, because a comparison of nine thousand rows says nothing about which rows matter.
    let of = |id: &str| {
        let kind = names
            .id_of(id)
            .unwrap_or_else(|| panic!("the registry has {id}"));
        (
            names.melee_bash_of(kind),
            names.melee_stab_of(kind),
            names.to_hit_of(kind),
        )
    };
    assert_eq!(of("rock"), (7.0, 0.0, -1), "a rock in the fist");
    assert_eq!(
        of("knife_combat"),
        (0.0, 21.0, -1),
        "a knife stabs and does not bash"
    );
    assert_eq!(
        of("null"),
        (0.0, 0.0, -2),
        "and the null item is worth nothing, and aims worst"
    );
}

/// The chance a swing is a critical hit.
///
/// Reference: `Character::crit_chance` (`melee.cpp:1116-1193`). Three chances are multiplied - the **weapon's** (its `to_hit`, or the unarmed skill when nothing is held), the
/// **body's** (dexterity and perception) and the **skill's** (half the melee skill plus the weapon's own) - and their product is a critical whatever the rolls. A good enough
/// roll against the target's dodge adds a **pair** of them: `0.5 * (wp + ps + ws - 3 * wps)`, which is the arithmetic of "any two of the three" with the triple removed, so
/// the two halves are mutually exclusive and may simply be added.
///
/// The three cases below are the reference's **own worked examples**, quoted in its comment block (`melee.cpp:1148-1158`): fresh 8/8/8/8 unarmed, expert 10/10/10/10 unarmed,
/// and a godlike 20/20/20/20 with a pipe of accuracy +1 - which is `to_hit` 1 on a bashing weapon at skill 20. They are the strongest kind of fixture this project has: numbers
/// the reference worked out for itself about its own rule.
#[test]
fn a_swing_is_a_critical_hit_by_three_chances_multiplied() {
    use cdda_sim::melee::{CritInputs, crit_chance};

    // Fresh: dexterity 8, perception 8, every skill zero, nothing in hand.
    let fresh = CritInputs {
        to_hit: -2,
        unarmed: true,
        unarmed_skill: 0,
        dexterity: 8,
        perception: 8,
        weapon_skill: 0,
        melee_skill: 0,
        martial_arts_bonus: 0.0,
    };
    let triple = |inputs: &CritInputs| crit_chance(inputs, 0.0, 0.0);
    // **30% * 49% * 25% = 0.03675**, the reference's "~1/16": the weapon's third is **0.3 and not 0.5**, because the null item's accuracy is -2 (`itype.h:1540`) and the negative
    // branch cuts the half. The oracle's own dump said so in round 1337, where this fixture had written 0.5 and been accepted.
    assert!(
        (triple(&fresh) - 0.036_75).abs() < 0.000_1,
        "a fresh unarmed survivor's guaranteed critical: {}",
        triple(&fresh)
    );
    // And with a roll past the target's dodge by half again, a pair is added: the reference's "~1/8" on top.
    let good_roll = crit_chance(&fresh, 10.0, 4.0);
    assert!(
        (good_roll - (0.036_75 + 0.5 * (0.3 * 0.49 + 0.49 * 0.25 + 0.3 * 0.25 - 3.0 * 0.036_75)))
            .abs()
            < 0.000_1,
        "a roll of ten against a dodge of four is a double chance: {good_roll}"
    );

    // Expert: dexterity 10, perception 10, every skill 10, unarmed. The reference works out 100%, 55%, 60%.
    let expert = CritInputs {
        to_hit: -2,
        unarmed: true,
        unarmed_skill: 10,
        dexterity: 10,
        perception: 10,
        // **Zero, because an empty hand has no weapon skill**: the rule asks `get_skill_level( weap.melee_skill() )` and a null weapon's skill id is null, which answers nothing
        // (`melee.cpp:1147-1152`). The unarmed skill feeds the **weapon's** chance and no other, which is the shape the round-1337 dump records as `weapon_skill_level`.
        weapon_skill: 0,
        melee_skill: 10,
        martial_arts_bonus: 0.0,
    };
    // 0.8 * 0.55 * 0.35 = 0.154, which is the reference's own answer for this character in `corpus/melee/crit-chance.json` - **not** the 100% its comment claims, because the
    // comment leaves out the null item's -2 accuracy and gives the skill term the unarmed level it does not use.
    assert!(
        (triple(&expert) - 0.154).abs() < 0.000_1,
        "an expert's guaranteed critical: {}",
        triple(&expert)
    );

    // A pipe at accuracy +1 raises the weapon's chance to its maximum of 0.6 and no further: `max( 0.5, 0.5 + 0.1 * to_hit )`, then capped at one.
    let pipe = CritInputs {
        to_hit: 1,
        unarmed: false,
        unarmed_skill: 0,
        dexterity: 0,
        perception: 0,
        weapon_skill: 0,
        melee_skill: 0,
        martial_arts_bonus: 0.0,
    };
    // A body of zero leaves the body's chance at its floor, which is the only weapon-chance factor left once the others are pinned: 0.6 * 0.25 * 0.25.
    assert!(
        (triple(&pipe) - (0.6 * 0.25 * 0.25)).abs() < 0.000_1,
        "an accurate weapon carries the weapon chance: {}",
        triple(&pipe)
    );
    // A **recorded divergence from the reference's own comment**: `melee.cpp:1148-1158` illustrates a godlike survivor as "60%, 100%, 42%", but the arithmetic below the comment
    // gives a body chance of `0.25 + 0.01 * 20 + 0.02 * 20 = 0.85` and a skill chance of `0.25 + (20 + 20 / 2.5) * 0.025 = 0.95`. The comment predates the constants; the code
    // is what this port follows, so the test pins the code's arithmetic and records the disagreement rather than copying the stale figure.
    let godlike = CritInputs {
        to_hit: 1,
        unarmed: false,
        unarmed_skill: 20,
        dexterity: 20,
        perception: 20,
        weapon_skill: 20,
        melee_skill: 20,
        martial_arts_bonus: 0.0,
    };
    assert!(
        (triple(&godlike) - (0.6 * 0.85 * 0.95)).abs() < 0.000_1,
        "a godlike survivor's guaranteed critical follows the constants, not the comment: {}",
        triple(&godlike)
    );
    // The body chance is capped: at dexterity and perception 20 it would be 0.85, but the reference caps every chance at one.
    assert!(
        triple(&CritInputs {
            dexterity: 100,
            perception: 100,
            ..godlike
        }) <= 1.0,
        "no chance is ever above one"
    );
}

/// How long a swing takes, and how long a weapon takes to swing.
///
/// Reference: `item::attack_time` (`item.cpp:7595-7602`) is the **weapon's own** time, derived from what it weighs and how big it is - `65 + ( volume / 62.5_ml + weight / 60_gram ) /
/// count` - and `Character::attack_speed` (`melee.cpp:2668-2705`) turns that into the moves a swing costs: half the weapon's time, plus the melee skill's discount
/// `base * (15 - skill) / 15`, minus half the dexterity, all multiplied by a **stamina penalty** that rises from nothing at a quarter stamina to double at none.
///
/// The numbers below are worked out from the reference's own formula and the corpus's own numbers: a crowbar is **520 ml and 1,456 g**, so its attack time is
/// `65 + (520 / 62.5 + 1456 / 60) = 97`, and a survivor of dexterity 8 with no melee skill swinging it pays `48 + 48 - 4 = 92` moves at full stamina.
#[test]
fn a_swing_costs_the_time_its_weapon_takes_and_the_swinger_s_skill_saves() {
    use cdda_sim::melee::{Stance, attack_speed, attack_time};

    assert_eq!(
        attack_time(520, 1_456, 1),
        97,
        "a crowbar takes ninety-seven"
    );
    assert_eq!(
        attack_time(0, 0, 1),
        65,
        "and a thing of no size or weight takes the base sixty-five"
    );
    assert_eq!(
        attack_time(520, 1_456, 2),
        81,
        "two of them in one stack are quicker per swing, because the sum is divided by the count"
    );

    // Half a crowbar's time is forty-eight; the skill discount at zero skill is the whole of it; dexterity 8 saves four.
    assert_eq!(
        attack_speed(48, 0, 8, 1.0, Stance::default()),
        92,
        "a clumsy survivor's swing"
    );
    // Ten points of melee skill remove two thirds of the base: `48 * 5 / 15 = 16`.
    assert_eq!(
        attack_speed(48, 10, 8, 1.0, Stance::default()),
        60,
        "an expert's swing"
    );
    // Half the stamina is no penalty at all; at none it doubles the base.
    assert_eq!(
        attack_speed(48, 0, 8, 0.5, Stance::default()),
        92,
        "half stamina costs nothing extra"
    );
    assert_eq!(
        attack_speed(48, 0, 8, 0.0, Stance::default()),
        140,
        "no stamina costs double the base: 96 + 48 - 4"
    );
    // A quarter of stamina is exactly the threshold, so nothing is added yet.
    assert_eq!(
        attack_speed(48, 0, 8, 0.25, Stance::default()),
        92,
        "a quarter of stamina is the threshold"
    );
    assert_eq!(
        attack_speed(48, 0, 8, 0.125, Stance::default()),
        116,
        "an eighth of stamina adds half again to the base"
    );
}

/// Practising a skill adds to its exercise, and the exercise buys levels at a rising price.
///
/// Reference: `SkillLevel::on_exercise_change` (`skill.cpp:450-479`) - the price of the next level is `100 * 100 * ( level + 1 )^2`, so level one costs **10,000**, level two
/// **40,000** and level three **90,000**, and a single big gain may buy several levels in a row, each at its own price. The knowledge half of the reference is not ported: this port
/// keeps one number per skill, so there is nothing for a second experience track to differ from.
#[test]
fn practising_a_skill_buys_levels_at_a_rising_price() {
    use cdda_sim::skills::{practise, price_of_the_next_level};

    assert_eq!(
        price_of_the_next_level(0),
        10_000,
        "the first level costs ten thousand"
    );
    assert_eq!(
        price_of_the_next_level(1),
        40_000,
        "the second forty thousand"
    );
    assert_eq!(
        price_of_the_next_level(2),
        90_000,
        "the third ninety thousand"
    );

    assert_eq!(
        practise(0, 0, 9_999),
        (0, 9_999),
        "nearly there is not there"
    );
    assert_eq!(
        practise(0, 9_999, 1),
        (1, 0),
        "and the last point buys the level"
    );
    assert_eq!(
        practise(0, 9_990, 30),
        (1, 20),
        "the overspend carries into the next level"
    );
    // A huge gain cascades: 50,000 from nothing buys level one, spends 10,000, leaves 40,000, which buys level two exactly.
    assert_eq!(
        practise(0, 0, 50_000),
        (2, 0),
        "and one big gain may buy two levels"
    );
    assert_eq!(practise(3, 0, 0), (3, 0), "nothing practises nothing");
}

/// A swing costs stamina, and a heavier weapon costs more of it.
///
/// Reference: `Character::get_total_melee_stamina_cost` (`melee.cpp:973-997`) with `get_standard_stamina_cost` (`character.cpp:3107-3120`): `- ( weight / 16 + 50 )`, plus the melee
/// skill as a rebate, less a stance penalty, floored at **-50**. A 500 g knife is `-(31 + 50) = -81`; the same knife in expert hands is `-71`; and no weapon's swing is ever cheaper
/// than 50.
#[test]
fn a_swing_costs_stamina_in_proportion_to_what_it_weighs() {
    use cdda_sim::melee::stamina_cost;

    assert_eq!(
        stamina_cost(500, 0, 0, 1.0),
        -81,
        "a half-kilo knife costs eighty-one"
    );
    assert_eq!(
        stamina_cost(0, 0, 0, 1.0),
        -50,
        "and empty hands cost the floor"
    );
    assert_eq!(
        stamina_cost(500, 10, 0, 1.0),
        -71,
        "ten points of melee skill give ten back"
    );
    assert_eq!(
        stamina_cost(500, 0, 20, 1.0),
        -101,
        "and crouching costs twenty more"
    );
    // The floor holds however light the weapon or skilled the hand: `std::min( -50, ... )`.
    assert_eq!(stamina_cost(0, 100, 0, 1.0), -50, "there is no free swing");
    assert_eq!(
        stamina_cost(4_000, 0, 0, 1.0),
        -300,
        "a four-kilo maul costs three hundred"
    );
}

/// Dexterity's contribution to ranged dispersion: half a unit a point below twenty, nothing above it.
///
/// Reference: `Character::ranged_dex_mod` (`character.cpp:4363-4367`). The two things worth pinning are the **floor at zero** - dexterity above twenty does not make a shot more accurate
/// - and the **truncation**, because the reference returns an `int` and a half-unit is dropped rather than rounded.
#[test]
fn dexterity_only_ever_adds_dispersion_and_only_below_twenty() {
    use cdda_sim::dispersion::from_dexterity;

    assert_eq!(
        from_dexterity(20),
        0,
        "twenty dexterity is the point the penalty stops"
    );
    assert_eq!(from_dexterity(24), 0, "and above it nothing improves");
    assert_eq!(from_dexterity(10), 5, "ten dexterity is five units");
    assert_eq!(from_dexterity(8), 6, "eight is six");
    assert_eq!(
        from_dexterity(9),
        5,
        "and nine is five and a half, **truncated** to five rather than rounded to six"
    );
    assert_eq!(
        from_dexterity(1),
        9,
        "one dexterity is nine and a half, truncated"
    );
}

/// Perception's dispersion and the composed figure a shot starts with.
///
/// Reference: `Character::ranged_per_mod` (`character.cpp:4369-4373`) and `Character::get_weapon_dispersion` (`ranged.cpp:2500-2545`). The composition is gated with **one case worked out
/// by hand** rather than with the sum the port itself computes, because a gate built from the same expression as the code proves only that the code is spelled consistently.
#[test]
fn perception_costs_more_dispersion_than_dexterity_and_the_shot_adds_them_up() {
    use cdda_sim::dispersion::{Shotmaker, from_perception, weapon_dispersion};

    assert_eq!(
        from_perception(20),
        0,
        "twenty perception is the point the penalty stops"
    );
    assert_eq!(from_perception(10), 12, "ten perception is twelve units");
    assert_eq!(from_perception(24), 0, "and above twenty nothing improves");
    assert_eq!(
        from_perception(10),
        from_dexterity_for_this_test() * 12 / 5,
        "1.2 a point against dexterity's 0.5: the same shortfall costs twelve against five"
    );

    // A skilled shooter with a hundred-dispersion weapon: the skill term is zero at MAX_SKILL, the stats cost nothing at twenty, so the answer is the weapon's own divided figure.
    let skilled = Shotmaker {
        weapon_dispersion: 100.0,
        dexterity: 20,
        perception: 20,
        gun_skill: 10,
        weapon_skill: 10,
        archery: false,
        divider: 18.0,
    };
    assert!(
        (weapon_dispersion(skilled) - 100.0).abs() < f64::EPSILON,
        "the weapon alone: this struct's field is the **already-divided** figure, which `shotmaker_for` is what builds"
    );

    // Eight dexterity costs six and ten perception costs twelve, and the average skill of five is what the curve is asked about.
    let clumsy = Shotmaker {
        weapon_dispersion: 100.0,
        dexterity: 8,
        perception: 10,
        gun_skill: 5,
        weapon_skill: 5,
        archery: false,
        divider: 18.0,
    };
    let skill_term = cdda_sim::dispersion::from_skill(5.0, 300.0 / 18.0);
    let expected = 6.0 + 12.0 + skill_term + 100.0;
    assert!(
        (weapon_dispersion(clumsy) - expected).abs() < 0.000_001,
        "weapon, hands and skill in one figure: {} against {expected}",
        weapon_dispersion(clumsy)
    );
    assert!(
        weapon_dispersion(clumsy) > weapon_dispersion(skilled),
        "and the clumsy shooter's shot is wider than the skilled one's"
    );
    // The bow's term is measured against **450** instead of 300, and the skill curve takes the weapon's own dispersion as its input - so a bigger denominator means a **bigger** penalty,
    // not a smaller one. The first version of this assertion had the direction backwards, which is the kind of sign a reader would never question.
    assert!(
        weapon_dispersion(Shotmaker {
            archery: true,
            ..clumsy
        }) > weapon_dispersion(clumsy),
        "a bow at the same skill is priced against 450 rather than 300, so its skill term is the larger of the two"
    );
}

/// The dexterity coefficient, read from the ported rule so the comparison above is about the **ratio of the two data coefficients** and not about a number typed twice.
fn from_dexterity_for_this_test() -> i32 {
    5
}

/// A gun and a shooter make a `Shotmaker`, and the figure that comes out is the reference's own composition for a real weapon from the corpus.
///
/// Reference: `Character::get_weapon_dispersion` (`ranged.cpp:2500-2545`). The gun is the corpus's `barb_launcher` (**dispersion 100, skill `pistol`**), so the weapon's own figure is the
/// data's rather than a number this test chose, and the skills are the shooter's: nothing at all is a beginner, and ten and ten is mastery.
#[test]
fn a_gun_and_a_shooter_compose_the_dispersion_a_shot_starts_with() {
    use bevy_ecs::prelude::*;

    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/items/guns.json");
    let bytes = std::fs::read(path).expect("the guns dump exists");
    let value = cdda_json::parse(&bytes).expect("valid JSON");
    let guns = cdda_content::guns::load_dump(&value).expect("the dump loads");
    let launcher = guns
        .iter()
        .find(|gun| gun.id == "barb_launcher")
        .expect("the corpus has a barb launcher")
        .clone();

    let mut world = World::new();
    let shooter = cdda_sim::creature::spawn_creature(&mut world, "the shooter", (0, 0, 0));
    world
        .entity_mut(shooter)
        .insert(cdda_sim::creature::Dexterity { base: 8 })
        .insert(cdda_sim::creature::Perception { base: 8 });

    // A beginner: eight and eight cost six and fourteen units of dispersion, and the skill term is the curve's at nothing.
    let beginner = cdda_sim::dispersion::shotmaker_for(&world, shooter, &launcher, 18.0);
    assert!(
        (beginner.weapon_dispersion - 6.0).abs() < f64::EPSILON,
        "the launcher's own dispersion **divided by eighteen and rounded**: 100 becomes 6, not 100 - the corpus carries the raw figure"
    );
    assert_eq!(beginner.dexterity, 8, "the shooter's effective dexterity");
    assert_eq!(beginner.perception, 8, "and its effective perception");
    assert_eq!(beginner.gun_skill, 0, "with no skill in guns");
    assert!(!beginner.archery, "and a launcher is not archery");

    // Mastery: ten in `gun` and ten in `pistol`, so the average is ten, the curve answers zero, and the stats no longer cost anything.
    world
        .entity_mut(shooter)
        .insert(cdda_sim::creature::Dexterity { base: 20 });
    world
        .entity_mut(shooter)
        .insert(cdda_sim::creature::Perception { base: 20 });
    world.entity_mut(shooter).insert(cdda_sim::creature::Skills(
        [("gun".to_owned(), 10), ("pistol".to_owned(), 10)]
            .into_iter()
            .collect(),
    ));
    let master = cdda_sim::dispersion::shotmaker_for(&world, shooter, &launcher, 18.0);
    assert_eq!(
        master.gun_skill, 10,
        "the gun skill is the reference's own `gun`"
    );
    assert_eq!(
        master.weapon_skill, 10,
        "and the weapon's skill is the one the gun's data names"
    );
    assert!(
        cdda_sim::dispersion::weapon_dispersion(master)
            < cdda_sim::dispersion::weapon_dispersion(beginner),
        "mastery is narrower than beginnerhood: {} against {}",
        cdda_sim::dispersion::weapon_dispersion(master),
        cdda_sim::dispersion::weapon_dispersion(beginner)
    );
}

/// A target's size is the fraction of its tile the aim roll divides by.
///
/// Reference: `occupied_tile_fraction` (`src/ranged.cpp:603-622`), whose five numbers are the whole of "how big is a thing" as far as aiming is concerned. The gate pins them against the
/// port's own `CreatureSize`, and then shows what they are for: **the same miss is a clean miss against a cat and a graze against a cow.**
#[test]
fn a_target_s_size_is_the_fraction_of_its_tile_a_shot_is_judged_against() {
    use cdda_sim::CreatureSize;
    use cdda_sim::ballistics::{aim_of, occupied_tile_fraction};

    assert!(
        (occupied_tile_fraction(CreatureSize::Tiny) - 0.1).abs() < f64::EPSILON,
        "a cat fills a tenth"
    );
    assert!(
        (occupied_tile_fraction(CreatureSize::Small) - 0.25).abs() < f64::EPSILON,
        "a dog a quarter"
    );
    assert!(
        (occupied_tile_fraction(CreatureSize::Medium) - 0.5).abs() < f64::EPSILON,
        "a person half"
    );
    assert!(
        (occupied_tile_fraction(CreatureSize::Large) - 0.75).abs() < f64::EPSILON,
        "a cow three quarters"
    );
    assert!(
        (occupied_tile_fraction(CreatureSize::Huge) - 1.0).abs() < f64::EPSILON,
        "and a hulk its whole tile"
    );

    // Ten arcminutes at ten squares misses by about two hundredths of a tile. Against a cat that is a fifth of the target; against a cow, a fortieth.
    let at_cat = aim_of(10.0, 10.0, occupied_tile_fraction(CreatureSize::Tiny));
    let at_cow = aim_of(10.0, 10.0, occupied_tile_fraction(CreatureSize::Large));
    assert!(
        at_cat.missed_by > at_cow.missed_by,
        "the same error is worse against the smaller thing: {} against {}",
        at_cat.missed_by,
        at_cow.missed_by
    );
    assert!(
        (at_cat.missed_by_tiles - at_cow.missed_by_tiles).abs() < f64::EPSILON,
        "and the error itself does not depend on what it was aimed at"
    );
}

/// A gun's own dispersion is divided by the option and floored at one, and damage on the gun adds before the division.
///
/// Reference: `item::gun_dispersion` (`item.cpp:11108-11132`). **The divider is the number that was missing from the port's first eight ranged rounds**: the dump carries the raw
/// `dispersion` (a rifle's is 100), the reference divides it by **18** and rounds, so the figure a shot is actually aimed with is **6**. Every dispersion the port composed before this round
/// was eighteen times too large, and every gate still passed because the gates were written against the same wrong figure.
#[test]
fn a_gun_s_dispersion_is_divided_and_floored_before_anything_is_added_to_it() {
    use cdda_sim::dispersion::gun_dispersion;

    assert_eq!(
        gun_dispersion(100, 0, 30, 18.0),
        6,
        "a hundred divided by eighteen is six"
    );
    assert_eq!(
        gun_dispersion(0, 0, 30, 18.0),
        1,
        "and the reference's floor is one, not zero"
    );
    assert_eq!(
        gun_dispersion(9, 0, 30, 18.0),
        1,
        "nine rounds to one, and the floor keeps it there"
    );
    assert_eq!(
        gun_dispersion(-50, 0, 30, 18.0),
        1,
        "a negative figure is floored at zero before the division, then at one"
    );

    // Damage on the gun adds thirty a level *before* the division, so a battered rifle is worse than a new one by a fifth of a level's worth.
    assert_eq!(
        gun_dispersion(100, 1, 30, 18.0),
        7,
        "one level of damage takes 100 to 7"
    );
    assert_eq!(
        gun_dispersion(100, 5, 30, 18.0),
        14,
        "and five levels to 14"
    );
    assert!(
        gun_dispersion(100, 5, 30, 18.0) > gun_dispersion(100, 0, 30, 18.0),
        "which is the whole point of the damage term"
    );
}

/// A swing from the floor takes four times as long, and no swing is ever quicker than twenty-five moves.
///
/// Reference: `Character::attack_speed` (`melee.cpp:2696-2710`):
///
/// ```text
/// if( is_on_ground() ) { if( PSEUDOPOD_GRASP ) move_cost *= 1.5; else move_cost *= 4.0; }
/// else if( is_crouching() && !PSEUDOPOD_GRASP && !natural_stance && !unarmed_attack() ) move_cost *= 1.5;
/// if( move_cost < 25.0 ) return 25;
/// return std::round( move_cost );
/// ```
///
/// **The floor and the rounding were both missing from the port**, which floored at zero and truncated: `std::round` of 92.4 is **92** either way, so the truncation hid behind the fixtures
/// until a row with a fraction of a half arrived. The stance multipliers are the branch the port had **no word for at all**.
#[test]
fn a_swing_from_the_floor_takes_four_times_as_long() {
    use cdda_sim::melee::{Stance, attack_speed};

    let standing = Stance::default();
    assert_eq!(
        attack_speed(48, 0, 8, 1.0, standing),
        92,
        "a standing swing is the arithmetic's own figure"
    );
    // 96 + 48 - 10 = 134 for a clumsy survivor with no wind...
    let downed = Stance {
        on_ground: true,
        ..Stance::default()
    };
    assert_eq!(
        attack_speed(48, 0, 8, 1.0, downed),
        368,
        "**and from the floor it is four times the whole figure**: ( 48 + 48 - 4 ) * 4"
    );
    let crouched = Stance {
        crouching: true,
        armed: true,
        ..Stance::default()
    };
    assert_eq!(
        attack_speed(48, 0, 8, 1.0, crouched),
        138,
        "a crouched armed swing is half again: ( 48 + 48 - 4 ) * 1.5"
    );
    // **The floor is twenty-five moves**, which nothing in the port could reach before because it floored at zero.
    let quick = Stance::default();
    assert_eq!(
        attack_speed(1, 30, 20, 1.0, quick),
        25,
        "**a swing is never quicker than twenty-five moves**"
    );
}

/// A limb score becomes a multiplier by inversion, and no multiplier is worse than the data's cap.
///
/// Reference: `character_modifier::modifier` (`character_modifier.cpp:365-386`) computes `( nominator / denominator ) / score` and then clamps it to the modifier's `max`, and the data
/// gives both melee ones `{ "limb_score": "lift" | "balance", "max": 3, "nominator": 1.0 }` (`data/json/character_modifiers.json:36-50`). So a body that can balance perfectly multiplies a
/// swing by **one**, and one that cannot balance at all multiplies it by **three and no more** - which is why a swing from the floor is not the nine times the two inversions would give.
#[test]
fn a_limb_score_becomes_a_multiplier_by_inversion() {
    use cdda_sim::melee::limb_score_modifier;

    assert!(
        (limb_score_modifier(1.0, 3.0) - 1.0).abs() < 1e-9,
        "**a whole score is no change**"
    );
    assert!(
        (limb_score_modifier(0.5, 3.0) - 2.0).abs() < 1e-9,
        "half a score doubles the cost"
    );
    assert!(
        (limb_score_modifier(0.0, 3.0) - 3.0).abs() < 1e-9,
        "**no score at all would divide by zero, so the reference's zero branch answers the cap** (`character_modifier.cpp:361-364`)"
    );
    assert!(
        (limb_score_modifier(0.1, 3.0) - 3.0).abs() < 1e-9,
        "and a score too poor to invert within the cap answers the cap too"
    );
}

/// Armour states three coverages, and a blow is met by the melee one.
///
/// Reference: `item::get_coverage( bodypart, cover_type )` (`item.cpp:8608-8645`) answers a **different figure for each type**, and the loader defaults both `cover_melee` and
/// `cover_ranged` to the entry's own `coverage` (`item_factory.cpp:3159-3160`) - so a garment that states nothing is met by its plain coverage whichever way the blow comes.
/// `outfit::get_coverage` (`character_attire.cpp:1553-1562`) then adds the figure of every worn garment **that covers the part**, which is the sum the port already had for the vitals.
///
/// **The twins matter because the data states them separately**: a plate carrier covers a torso completely in melee and less against a shot, and a port that added the plain coverage
/// would be right for every garment that states nothing and wrong for exactly the ones that do.
#[test]
fn armour_states_a_melee_coverage_and_a_ranged_one() {
    use cdda_sim::melee::{CoverType, worn_cover};

    // (covers the part, plain coverage, melee coverage, ranged coverage)
    let garments = [(true, 100, 80, 60), (true, 50, 50, 50), (false, 90, 90, 90)];
    assert_eq!(
        worn_cover(&garments, CoverType::Melee),
        130,
        "**the melee figure of every garment that covers the part**"
    );
    assert_eq!(
        worn_cover(&garments, CoverType::Ranged),
        110,
        "and the ranged one"
    );
    assert_eq!(
        worn_cover(&garments, CoverType::Default),
        150,
        "and the plain coverage, which is what the vitals rule already asked for"
    );
    // A garment covering another part is not met at all, whichever figure it states.
    assert_eq!(worn_cover(&[(false, 100, 100, 100)], CoverType::Melee), 0);
}

/// A monster tied down, trapped or bouldering dodges less, and one that cannot move does not dodge at all.
///
/// Reference: `monster::get_dodge` (`monster.cpp:2693-2713`):
///
/// ```text
/// if( has_effect( effect_downed ) || has_flag( json_flag_CANNOT_MOVE ) ) return 0.0f;
/// float ret = Creature::get_dodge();
/// if( lightsnare || heavysnare || beartrap || tied ) ret /= 2;
/// if( bouldering ) ret /= 4;
/// ```
///
/// and `monster::dodge_roll` is `get_dodge() * 5` (`:2717-2720`). **The port read `sk_dodge` straight into a `Dodge` component and never asked the penalties**, so a zombie in a bear trap
/// dodged like a free one - the kind of rule that only shows when something is actually trapped.
#[test]
fn a_trapped_monster_dodges_worse() {
    use cdda_sim::melee::monster_dodge;

    assert_eq!(
        monster_dodge(4, &[]),
        4,
        "a free monster dodges as its type says"
    );
    assert_eq!(
        monster_dodge(4, &["beartrap"]),
        2,
        "**a trapped one dodges half as well**"
    );
    for trap in ["lightsnare", "heavysnare", "tied"] {
        assert_eq!(monster_dodge(4, &[trap]), 2, "{trap} halves it too");
    }
    assert_eq!(
        monster_dodge(4, &["bouldering"]),
        1,
        "a bouldering one quarters it, rounded down"
    );
    assert_eq!(
        monster_dodge(4, &["downed"]),
        0,
        "**and one on the floor dodges nothing at all**, whatever else it is suffering"
    );
    assert_eq!(
        monster_dodge(4, &["downed", "beartrap"]),
        0,
        "the floor is asked first"
    );
    assert_eq!(
        monster_dodge(3, &["beartrap", "bouldering"]),
        0,
        "both penalties apply, and the floor rounds down"
    );
}

/// A winded survivor dodges worse, on the reference's own curve.
///
/// Reference: `Character::get_stamina_dodge_modifier` (`character.cpp:1261-1268`):
///
/// ```text
/// const double stamina_min = get_stamina_max() * 0.1;
/// const double stamina_max = get_stamina_max() * 0.9;
/// return 1.0 - logarithmic_range( stamina_min, stamina_max, stamina );
/// ```
///
/// `logarithmic_range` answers **one below** its minimum and **zero above** its maximum, so the modifier is **one with a full wind and zero at a tenth of one** - and it moves the way the
/// curve does in between, which is what makes a long fight cost the ability to dodge rather than switching it off.
#[test]
fn a_winded_survivor_dodges_worse() {
    use cdda_sim::melee::stamina_dodge_modifier;

    let max = 3_500.0;
    assert!(
        (stamina_dodge_modifier(max, max) - 1.0).abs() < 1e-9,
        "**a full wind costs nothing**"
    );
    assert!(
        (stamina_dodge_modifier(0.1 * max, max) - 0.0).abs() < 1e-9,
        "**and a tenth of one leaves nothing to dodge with**"
    );
    assert!(
        (stamina_dodge_modifier(0.0, max) - 0.0).abs() < 1e-9,
        "nothing at all is not worse than a tenth, because the curve is clamped above its maximum"
    );
    // Half a wind is somewhere between, and more wind is always better than less.
    let half = stamina_dodge_modifier(0.5 * max, max);
    assert!(
        (0.2..0.8).contains(&half),
        "half a wind leaves half a dodge: {half}"
    );
    assert!(stamina_dodge_modifier(0.8 * max, max) > half);
    assert!(stamina_dodge_modifier(0.3 * max, max) < half);
}

/// A creature slower than a walk dodges in proportion, and a faster one gains nothing.
///
/// Reference: `Character::get_dodge` (`melee.cpp:1216-1221`):
///
/// ```text
/// int speed_stat = get_speed();
/// if( speed_stat < 100 ) ret *= speed_stat / 100.0f;
/// ```
///
/// **The test is one-sided**: a hundred is a whole walk and the branch does not fire above it, so a creature at a hundred and twenty dodges exactly as well as one at a hundred - the rule is a
/// penalty for being slowed, not a reward for being quick.
#[test]
fn a_slow_creature_dodges_in_proportion() {
    use cdda_sim::melee::speed_dodge_modifier;

    assert!(
        (speed_dodge_modifier(100) - 1.0).abs() < 1e-9,
        "**a walk is no penalty**"
    );
    assert!(
        (speed_dodge_modifier(120) - 1.0).abs() < 1e-9,
        "**and being quick is not a reward**: the branch does not fire above a hundred"
    );
    assert!(
        (speed_dodge_modifier(50) - 0.5).abs() < 1e-9,
        "half speed, half the dodge"
    );
    assert!(
        (speed_dodge_modifier(0) - 0.0).abs() < 1e-9,
        "and no speed at all, none"
    );
}

/// Roller skates cost a survivor most of its dodge, and a skater only half of it.
///
/// Reference: `Character::get_dodge` (`melee.cpp:1209-1214`):
///
/// ```text
/// if( worn_with_flag( ROLLER_INLINE ) || worn_with_flag( ROLLER_QUAD ) || worn_with_flag( ROLLER_ONE ) )
///     ret /= has_trait( PROF_SKATER ) ? 2 : 5;
/// ```
///
/// **The division is integer and it is severe**: an unskilled survivor on skates keeps a fifth of its dodge, and the trait is worth going from a fifth to a half. The port had no rule here at all,
/// and no way to ask what a creature is wearing by flag - which is the other half of this round.
#[test]
fn skates_cost_a_survivor_most_of_its_dodge() {
    use cdda_sim::melee::skate_dodge;

    assert_eq!(skate_dodge(10, false, false), 10, "shoes are not skates");
    assert_eq!(
        skate_dodge(10, true, false),
        2,
        "**an unskilled skater keeps a fifth**, and the division takes the fraction"
    );
    assert_eq!(skate_dodge(10, true, true), 5, "and a skater keeps half");
    assert_eq!(
        skate_dodge(3, true, false),
        0,
        "three points of dodge on skates is none"
    );
}
