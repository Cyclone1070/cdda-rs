//! The overmap terrain instances the reference builds from its types.
//!
//! Source: `--dump-overmap-terrains`, which writes every instance the world can hold together with the type it came from and the two facts
//! that decided how many instances that type became - whether it rotates and whether it draws lines.
//!
//! The reference has 6,269 terrain types and 24,218 instances, so almost four of every five overmap ids exist only as a rule: `north`,
//! `east`, `south`, `west` around a rotatable type, or one of the sixteen line suffixes around a linear one. Getting that rule wrong is the
//! difference between an overmap that loads and one that reports thousands of unknown terrains.

#![allow(clippy::expect_used, clippy::panic)]

use cdda_json::{Value, parse};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

fn corpus() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus")
}

fn data() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../references/Cataclysm-DDA/data/json")
}

/// One instance from the reference: its id, its type, and what its type decided.
fn reference() -> (BTreeSet<String>, BTreeMap<String, (bool, bool)>) {
    let text = std::fs::read_to_string(corpus().join("overmap_terrains.json")).expect("the dump");
    let Value::Object(root) = parse(text.as_bytes()).expect("valid JSON") else { panic!("root") };
    let Some(Value::Array(instances)) = root.get("instances") else { panic!("no instances") };
    let mut ids = BTreeSet::new();
    let mut types = BTreeMap::new();
    for instance in instances {
        let Value::Object(instance) = instance else { panic!("an instance is an object") };
        let (Some(Value::Str(id)), Some(Value::Str(kind))) = (instance.get("id"), instance.get("type")) else {
            panic!("an instance has an id and a type")
        };
        let Some(Value::Bool(rotatable)) = instance.get("rotatable") else { panic!("rotatable") };
        let Some(Value::Bool(linear)) = instance.get("linear") else { panic!("linear") };
        ids.insert(id.clone());
        types.insert(kind.clone(), (*rotatable, *linear));
    }
    (ids, types)
}

#[test]
fn the_port_derives_every_overmap_terrain_instance_the_reference_holds() {
    let (expected, _) = reference();
    // The module this asks is the one the digest gate already trusts, `overmap.rs`, which reads the types and derives the instances. This
    // test adds what a digest cannot show: the reference's own per-type facts, and the reason when one of them is wrong.
    let loaded = cdda_content::overmap::load_paths(&[data().as_path()]).expect("the overmap terrain data loads");
    let got: BTreeSet<String> = loaded.terrains.clone();

    let missing: Vec<&String> = expected.difference(&got).take(8).collect();
    let extra: Vec<&String> = got.difference(&expected).take(8).collect();
    assert_eq!(
        (got.len(), expected.len()),
        (expected.len(), expected.len()),
        "the port derives the reference's {} instances, not {}; missing {missing:?}, extra {extra:?}",
        expected.len(),
        got.len()
    );
    assert!(missing.is_empty() && extra.is_empty(), "missing {missing:?}, extra {extra:?}");
}

#[test]
fn each_type_rotates_and_draws_lines_the_way_the_reference_says() {
    let (_, expected) = reference();
    let loaded = cdda_content::overmap::load_paths(&[data().as_path()]).expect("the overmap terrain data loads");
    let types = &loaded.types;
    assert_eq!(types.len(), expected.len(), "the port reads every terrain type");
    let mut wrong = Vec::new();
    for kind in types {
        let want = expected.get(&kind.id).unwrap_or_else(|| panic!("{} is in the reference", kind.id));
        let got = (kind.rotatable(), kind.linear());
        if got != *want && wrong.len() < 8 {
            wrong.push((kind.id.clone(), got, *want));
        }
    }
    assert!(wrong.is_empty(), "rotatable/linear disagree for {wrong:?}");
}
