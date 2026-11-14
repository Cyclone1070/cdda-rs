//! Every skill's attack-time line, read from the oracle's own dump.
//!
//! Reference: `time_info_t` (`src/skill.h:23-30`), dumped by `--dump-skills`.

#![allow(clippy::expect_used)]

/// Every skill's attack-time line loads, and the four the port prices a shot with are the dump's own numbers.
///
/// Reference: `time_info_t` (`src/skill.h:23-30`) as the reference resolved it, dumped by `--dump-skills`. **The numbers are not the struct's defaults**: marksmanship (`gun`) is 220/25/50,
/// while a rifle is 30/1/20 - so a port that assumed the defaults would price a rifle shot seven times too slow.
#[test]
fn every_skill_s_attack_time_line_loads_and_the_rifle_is_not_the_default() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/skills.json");
    let bytes = std::fs::read(path).expect("the skills dump exists");
    let value = cdda_json::parse(&bytes).expect("valid JSON");
    let skills = cdda_content::skills::load_dump(&value).expect("the dump loads");
    assert_eq!(skills.len(), 27, "every skill the reference has");

    let line = |id: &str| {
        let skill = skills
            .iter()
            .find(|skill| skill.id == id)
            .expect("the skill is in the dump");
        (
            skill.base_time,
            skill.time_reduction_per_level,
            skill.min_time,
        )
    };
    assert_eq!(
        line("rifle"),
        (30, 1, 20),
        "a rifle: thirty moves, one off a level, floor twenty"
    );
    assert_eq!(line("pistol"), (30, 1, 20), "and a pistol the same line");
    assert_eq!(
        line("archery"),
        (50, 3, 20),
        "archery is slower to learn and quicker to floor"
    );
    assert_eq!(
        line("gun"),
        (220, 25, 50),
        "marksmanship is the struct's default block, and the slowest line there is"
    );
    assert_eq!(line("throw"), (220, 25, 50), "throwing shares it");

    // The names are the reference's display names, not the ids: `rifle` is shown as `rifles`.
    let rifle = skills
        .iter()
        .find(|skill| skill.id == "rifle")
        .expect("a rifle");
    assert_eq!(
        rifle.name, "rifles",
        "the display name, which an id-keyed port would lose"
    );
}
