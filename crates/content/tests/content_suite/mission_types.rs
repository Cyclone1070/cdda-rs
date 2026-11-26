//! The mission definitions the data holds.
//!
//! **Counts prove non-vacuity**: the data defines 306 `mission_definition` objects with 306 unique ids, and **141 of them name a follow-up** - every one of which lands on a
//! mission that exists, which is the check that matters, because `clear_mission`'s whole rule is "offer the follow-up next".

#![allow(clippy::expect_used)]

use cdda_content::mission_types;

fn data_roots() -> Vec<std::path::PathBuf> {
    let reference = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../references/Cataclysm-DDA/data");
    vec![reference.join("core"), reference.join("json")]
}

#[test]
fn every_mission_in_the_data_is_read() {
    let roots = data_roots();
    let paths: Vec<&std::path::Path> = roots.iter().map(std::path::PathBuf::as_path).collect();
    let missions = mission_types::load_paths(&paths).expect("the missions load");
    assert_eq!(missions.kinds.len(), 306, "the data defines 306 missions");
    assert_eq!(missions.by_id.len(), 306, "all of them have unique ids");
    let sorted = {
        let mut ids = missions
            .kinds
            .iter()
            .map(|kind| kind.id.clone())
            .collect::<Vec<_>>();
        ids.sort();
        ids
    };
    assert_eq!(
        missions
            .kinds
            .iter()
            .map(|kind| kind.id.clone())
            .collect::<Vec<_>>(),
        sorted,
        "**the store is sorted by id, so a lookup and a listing cannot disagree**"
    );
}

#[test]
fn a_missions_fields_are_the_datas() {
    let roots = data_roots();
    let paths: Vec<&std::path::Path> = roots.iter().map(std::path::PathBuf::as_path).collect();
    let missions = mission_types::load_paths(&paths).expect("the missions load");
    // The starting assassination is the one a new game can begin with: goal, difficulty and value are all stated by the data, and the name is a `{ "str": ... }` object.
    let kind = missions
        .named("MISSION_ASSASSINATION")
        .expect("the starting mission exists");
    assert_eq!(kind.goal, "MGOAL_KILL_MONSTER", "**its goal is read**");
    assert_eq!(kind.difficulty, 4, "**its difficulty is read**");
    assert_eq!(kind.value, 0, "**its value is read**");
    assert_eq!(
        kind.name, "Finish your mission",
        "**and its name, out of the text object**"
    );
    assert_eq!(
        kind.origins,
        vec!["ORIGIN_GAME_START".to_owned()],
        "**and where it may come from**"
    );
}

#[test]
fn every_follow_up_leads_to_a_mission_that_exists() {
    let roots = data_roots();
    let paths: Vec<&std::path::Path> = roots.iter().map(std::path::PathBuf::as_path).collect();
    let missions = mission_types::load_paths(&paths).expect("the missions load");
    let with_follow_up = missions
        .kinds
        .iter()
        .filter(|kind| kind.follow_up.is_some())
        .count();
    assert_eq!(with_follow_up, 141, "the data chains 141 missions");
    let mut dangling = Vec::new();
    for kind in &missions.kinds {
        if let Some(next) = &kind.follow_up
            && missions.named(next).is_none()
        {
            dangling.push(format!("{} -> {next}", kind.id));
        }
    }
    assert!(
        dangling.is_empty(),
        "**a chained mission that names a mission the data does not define would break clear_mission: {dangling:?}**"
    );
}
