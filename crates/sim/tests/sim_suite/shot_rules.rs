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
