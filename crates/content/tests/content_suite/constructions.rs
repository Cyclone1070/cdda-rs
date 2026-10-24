//! Constructions, against the reference's own list of them.
//!
//! `corpus/constructions/all.json` is `--dump-constructions`: every construction the reference holds after loading the same two directories, with the fields that decide
//! behaviour. The gate is an equality over all 756, which is the only kind of check that catches a field silently read from the wrong member.

#![allow(clippy::expect_used, clippy::panic)]

use cdda_content::constructions;
use cdda_json::Value;
use std::collections::BTreeMap;

/// What the dump says about one construction: its time, the tile it leaves, the tiles it may be built on, and its skills.
type Recorded = (i64, String, Vec<String>, BTreeMap<String, i32>);

/// The dump, by id.
fn recorded() -> BTreeMap<String, Recorded> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let bytes = std::fs::read(root.join("corpus/constructions/all.json")).expect("the dump");
    let Value::Object(object) = cdda_json::parse(&bytes).expect("valid JSON") else {
        panic!("the dump is an object")
    };
    let Some(Value::Array(all)) = object.get("constructions") else {
        panic!("the dump holds constructions")
    };
    let mut out = BTreeMap::new();
    for entry in all {
        let Value::Object(entry) = entry else {
            panic!("a construction")
        };
        let text = |key: &str| match entry.get(key) {
            Some(Value::Str(value)) => value.clone(),
            other => panic!("{key} is text: {other:?}"),
        };
        let skills = match entry.get("skills") {
            Some(Value::Array(pairs)) => pairs
                .iter()
                .map(|pair| {
                    let Value::Array(pair) = pair else {
                        panic!("a pair")
                    };
                    let Some(Value::Str(name)) = pair.first() else {
                        panic!("a skill name")
                    };
                    let Some(Value::Int(level)) = pair.get(1) else {
                        panic!("a level")
                    };
                    (name.clone(), i32::try_from(*level).expect("a level"))
                })
                .collect(),
            other => panic!("skills: {other:?}"),
        };
        let pre = match entry.get("pre_terrain") {
            Some(Value::Array(names)) => names
                .iter()
                .map(|name| match name {
                    Value::Str(name) => name.clone(),
                    other => panic!("a terrain: {other:?}"),
                })
                .collect(),
            other => panic!("pre_terrain: {other:?}"),
        };
        let Some(Value::Int(time)) = entry.get("time") else {
            panic!("time")
        };
        out.insert(
            text("id"),
            (*time, text("post_terrain"), pre, skills),
        );
    }
    out
}

#[test]
fn every_construction_loads_with_the_time_result_tiles_and_skills_the_reference_gives_it() {
    let dirs = crate::corpus::dirs();
    let args: Vec<&std::path::Path> = dirs.iter().map(std::path::PathBuf::as_path).collect();
    let loaded = constructions::load_paths(&args).expect("constructions load");
    let recorded = recorded();
    assert_eq!(loaded.len(), 756, "the reference holds 756 constructions");
    assert_eq!(recorded.len(), 756);
    let mut compared = 0_usize;
    let mut wrong: Vec<String> = Vec::new();
    for construction in &loaded {
        let Some((time, post_terrain, pre_terrain, skills)) = recorded.get(&construction.id) else {
            wrong.push(format!("{}: the reference has no such construction", construction.id));
            continue;
        };
        if construction.time != *time {
            wrong.push(format!(
                "{}: {} moves, the reference says {time}",
                construction.id, construction.time
            ));
        }
        if construction.post_terrain != *post_terrain {
            wrong.push(format!(
                "{}: leaves {}, the reference says {post_terrain}",
                construction.id, construction.post_terrain
            ));
        }
        if construction.pre_terrain != *pre_terrain {
            wrong.push(format!(
                "{}: built on {:?}, the reference says {pre_terrain:?}",
                construction.id, construction.pre_terrain
            ));
        }
        if construction.required_skills != *skills {
            wrong.push(format!(
                "{}: wants {:?}, the reference says {skills:?}",
                construction.id, construction.required_skills
            ));
        }
        compared += 1;
    }
    assert_eq!(compared, 756, "every construction was compared");
    assert!(wrong.is_empty(), "{} differences: {wrong:#?}", wrong.len());
}

#[test]
fn a_duration_string_is_read_in_the_units_the_reference_reads_it_in() {
    // `to_moves<int>( read_from_json_string<time_duration> )` (`construction.cpp:2216-2219`): one turn is one second, and one second is a hundred moves. Every
    // construction that survives the gate above is proof of the arithmetic; these are the units spelled out.
    assert_eq!(constructions::parse_duration("1 h"), 3_600);
    assert_eq!(constructions::parse_duration("1 h 30 m"), 5_400);
    assert_eq!(constructions::parse_duration("30 m"), 1_800);
    assert_eq!(constructions::parse_duration("45 s"), 45);
    assert_eq!(constructions::parse_duration("2 t"), 2);
    assert_eq!(constructions::parse_duration("1 d"), 86_400);
    assert_eq!(constructions::parse_duration("0 s"), 0);
    assert_eq!(
        constructions::duration_in_moves(&Value::Str("1 m".to_owned())),
        6_000,
        "a minute is sixty seconds and every second is a hundred moves"
    );
    assert_eq!(
        constructions::duration_in_moves(&Value::Int(500)),
        500,
        "a bare number is already in moves"
    );
}
