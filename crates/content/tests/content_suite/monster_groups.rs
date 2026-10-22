//! Monster groups: which monsters a spawn point may produce.
//!
//! The gate is a transcription of the reference's own data: every `monstergroup` object under `data/json`, keyed by its id,
//! with how many entries it holds. The corpus file was written by reading those files directly, so a loader that misses a
//! directory, a mod, or a repeated id fails here rather than quietly spawning nothing.

#![allow(clippy::panic, clippy::expect_used, clippy::indexing_slicing)]

use std::collections::BTreeMap;

fn corpus() -> BTreeMap<String, usize> {
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/monsters/groups.json");
    let text = std::fs::read_to_string(&path).expect("the group transcription");
    let value = cdda_json::parse(text.as_bytes()).expect("valid json");
    let cdda_json::Value::Object(fields) = value else {
        panic!("an object of id to entry count")
    };
    fields
        .members()
        .iter()
        .map(|(id, count)| {
            let cdda_json::Value::Int(number) = count else {
                panic!("a count for {id}")
            };
            (id.clone(), usize::try_from(*number).unwrap_or(0))
        })
        .collect()
}

#[test]
fn every_group_in_the_data_is_loaded_with_all_of_its_entries() {
    let expected = corpus();
    assert_eq!(expected.len(), 401, "the data defines this many groups");
    let dirs = [std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../references/Cataclysm-DDA/data/json")];
    let dirs: Vec<&std::path::Path> = dirs.iter().map(std::path::PathBuf::as_path).collect();
    let groups = cdda_content::monster_groups::load_paths(&dirs).expect("the groups load");
    let ours: BTreeMap<String, usize> = groups
        .iter()
        .map(|group| (group.id.clone(), group.entries.len()))
        .collect();
    let invented: Vec<&String> = ours
        .keys()
        .filter(|id| !expected.contains_key(*id))
        .collect();
    let missing: Vec<&String> = expected
        .keys()
        .filter(|id| !ours.contains_key(*id))
        .collect();
    assert!(invented.is_empty(), "invented groups: {invented:?}");
    assert!(missing.is_empty(), "missing groups: {missing:?}");
    // A group repeated in the data - a mod appending to a core group - keeps every entry, which is the point of loading by
    // id rather than by object.
    let wrong: Vec<(&String, &usize, &usize)> = expected
        .iter()
        .filter_map(|(id, want)| {
            let got = ours.get(id)?;
            (*got != *want).then_some((id, got, want))
        })
        .collect();
    assert!(wrong.is_empty(), "entry counts differ: {wrong:?}");
}

#[test]
fn an_entry_keeps_the_monster_it_names_and_its_weight() {
    // Weights decide which monster comes out, so an entry without one is not an entry. `GROUP_GOO` is the worked example from
    // the data: six entries, the heaviest of them a blob at 693.
    let dirs = [std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../references/Cataclysm-DDA/data/json")];
    let dirs: Vec<&std::path::Path> = dirs.iter().map(std::path::PathBuf::as_path).collect();
    let groups = cdda_content::monster_groups::load_paths(&dirs).expect("the groups load");
    let goo = groups
        .iter()
        .find(|group| group.id == "GROUP_GOO")
        .expect("GROUP_GOO");
    assert_eq!(goo.entries.len(), 6);
    let heaviest = goo
        .entries
        .iter()
        .max_by_key(|entry| entry.weight)
        .expect("an entry");
    assert_eq!(
        heaviest.kind,
        cdda_content::monster_groups::EntryKind::Monster("mon_blob".to_owned())
    );
    assert_eq!(heaviest.weight, 693);
    assert!(
        goo.entries.iter().all(|entry| match &entry.kind {
            cdda_content::monster_groups::EntryKind::Monster(id)
            | cdda_content::monster_groups::EntryKind::Group(id) => !id.is_empty(),
        }),
        "every entry names a monster or a group: {:?}",
        goo.entries
    );
}
