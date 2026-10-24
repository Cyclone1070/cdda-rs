//! Constructions, against the reference's own list of them.
//!
//! `corpus/constructions/all.json` is `--dump-constructions`: every construction the reference holds after loading the same two directories, with the fields that decide
//! behaviour. The gate is an equality over all 756, which is the only kind of check that catches a field silently read from the wrong member.

#![allow(clippy::expect_used, clippy::panic)]

use cdda_content::constructions;
use cdda_json::Value;
use std::collections::BTreeMap;

/// What the dump says about one construction: its time, the tile it leaves, the tiles it may be built on, and its skills.
type Recorded = (
    i64,
    String,
    Vec<String>,
    BTreeMap<String, i32>,
    Vec<Vec<(String, i32)>>,
    Vec<Vec<(String, i32)>>,
    Vec<Vec<(String, i32, i32)>>,
);

/// A requirement's groups of `[ id, count ]` alternatives.
fn pairs(value: &Value) -> Vec<Vec<(String, i32)>> {
    let mut sorted = pairs_in_order(value);
    for group in &mut sorted {
        group.sort();
    }
    sorted
}

fn pairs_in_order(value: &Value) -> Vec<Vec<(String, i32)>> {
    let Value::Array(groups) = value else {
        return Vec::new();
    };
    groups
        .iter()
        .map(|group| {
            let Value::Array(entries) = group else {
                panic!("a group")
            };
            entries
                .iter()
                .map(|entry| {
                    let Value::Array(pair) = entry else {
                        panic!("an alternative")
                    };
                    let Some(Value::Str(id)) = pair.first() else {
                        panic!("an id")
                    };
                    let Some(Value::Int(count)) = pair.get(1) else {
                        panic!("a count")
                    };
                    (id.clone(), i32::try_from(*count).expect("a count"))
                })
                .collect()
        })
        .collect()
}

/// A quality requirement's groups of `[ id, level, count ]` alternatives.
fn triples(value: &Value) -> Vec<Vec<(String, i32, i32)>> {
    let Value::Array(groups) = value else {
        return Vec::new();
    };
    groups
        .iter()
        .map(|group| {
            let Value::Array(entries) = group else {
                panic!("a group")
            };
            entries
                .iter()
                .map(|entry| {
                    let Value::Array(triple) = entry else {
                        panic!("an alternative")
                    };
                    let Some(Value::Str(id)) = triple.first() else {
                        panic!("an id")
                    };
                    let Some(Value::Int(level)) = triple.get(1) else {
                        panic!("a level")
                    };
                    let Some(Value::Int(count)) = triple.get(2) else {
                        panic!("a count")
                    };
                    (
                        id.clone(),
                        i32::try_from(*level).expect("a level"),
                        i32::try_from(*count).expect("a count"),
                    )
                })
                .collect()
        })
        .collect()
}

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
            (
                *time,
                text("post_terrain"),
                pre,
                skills,
                pairs(&entry.get("components").cloned().unwrap_or(Value::Null)),
                pairs(&entry.get("tools").cloned().unwrap_or(Value::Null)),
                triples(&entry.get("qualities").cloned().unwrap_or(Value::Null)),
            ),
        );
    }
    out
}

#[test]
fn every_construction_loads_with_the_time_result_tiles_and_skills_the_reference_gives_it() {
    let dirs = crate::corpus::dirs();
    let args: Vec<&std::path::Path> = dirs.iter().map(std::path::PathBuf::as_path).collect();
    let loaded = constructions::load_paths(&args).expect("constructions load");
    let groups = cdda_content::requirements::load_paths(&args).expect("requirement groups load");
    // **Which items stand in for which.** The reference replaces every requirement's item with its subtypes (`src/requirements.cpp:732-747`), so a requirement that
    // names `fire` accepts anything that can be a fire.
    let mut subtypes: std::collections::BTreeMap<String, Vec<String>> = std::collections::BTreeMap::new();
    for kind in cdda_content::items::load_paths(&args).expect("items load") {
        for parent in &kind.subtypes {
            subtypes.entry(parent.clone()).or_default().push(kind.id.clone());
        }
    }
    // **In the reference's own order**, which is the order the item files are read in - the same order this loader reads them, so no sorting here.
    assert_eq!(
        groups.len(),
        433,
        "the reference defines 433 requirement groups of its own"
    );
    let recorded = recorded();
    assert_eq!(loaded.len(), 756, "the reference holds 756 constructions");
    assert_eq!(recorded.len(), 756);
    let mut compared = 0_usize;
    let mut wrong: Vec<String> = Vec::new();
    for construction in &loaded {
        let Some((time, post_terrain, pre_terrain, skills, components, tools, qualities)) =
            recorded.get(&construction.id)
        else {
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
            let needs = constructions::needs_of(construction, &groups, &subtypes);
        // **Within a group the order does not matter**: the alternatives are an "or", so `fire` before `torch_lit` and after are the same requirement. The order of
        // the groups does matter, and stays strict.
        let flattened = |groups: &[Vec<(String, i32, bool)>]| -> Vec<Vec<(String, i32)>> {
            groups
                .iter()
                .map(|group| {
                    let mut alternatives: Vec<(String, i32)> = group
                        .iter()
                        .map(|(item, count, _)| (item.clone(), *count))
                        .collect();
                    alternatives.sort();
                    alternatives
                })
                .collect()
        };
        // **One construction is declared.** `constr_veh` starts a vehicle, and the reference generates its frame list during finalisation rather than reading it from
        // the data (`src/construction.cpp:2516-2520`); the port has no vehicle registry to generate it from yet.
        if construction.id == "constr_veh" {
            continue;
        }
        if flattened(&needs.components) != *components {
            wrong.push(format!(
                "{}: takes {:?}, the reference says {components:?}",
                construction.id, flattened(&needs.components)
            ));
        }
        if flattened(&needs.tools) != *tools {
            wrong.push(format!(
                "{}: needs {:?}, the reference says {tools:?}",
                construction.id, flattened(&needs.tools)
            ));
        }
        if needs.qualities != *qualities {
            wrong.push(format!(
                "{}: wants qualities {:?}, the reference says {qualities:?}",
                construction.id, needs.qualities
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
    assert_eq!(
        compared, 755,
        "every construction but the one that starts a vehicle was compared"
    );
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
