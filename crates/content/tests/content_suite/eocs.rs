//! Effect-on-conditions, defined and inline.
//!
//! Reference: `effect_on_conditions::load_inline_eoc`
//! (`src/effect_on_condition.cpp:119-133`). An entry of type
//! `effect_on_condition` defines one by name; an *inline* one is an object sitting
//! in a slot such as `activated_eocs` or `true_eocs`, and the reference registers
//! it under its own `id` as it loads. Both kinds are the same registry.

use cdda_content::eocs;
use std::collections::BTreeSet;
use std::path::PathBuf;

fn reference_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../references/Cataclysm-DDA")
}

#[allow(clippy::expect_used, clippy::panic)]
fn reference_ids(registry: &str) -> BTreeSet<String> {
    let corpus = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../corpus/prototypes/ids.json");
    let text = std::fs::read_to_string(&corpus).expect("id corpus");
    let parsed = cdda_json::parse(text.as_bytes()).expect("corpus parses");
    let cdda_json::Value::Object(root) = parsed else {
        panic!("corpus is an object");
    };
    let Some(cdda_json::Value::Array(registries)) = root.get("registries") else {
        panic!("corpus has a registry list");
    };
    for entry in registries {
        let cdda_json::Value::Object(entry) = entry else {
            continue;
        };
        if entry.get("type") == Some(&cdda_json::Value::Str(registry.to_owned())) {
            let Some(cdda_json::Value::Array(ids)) = entry.get("ids") else {
                panic!("registry {registry} has ids");
            };
            return ids
                .iter()
                .filter_map(|id| match id {
                    cdda_json::Value::Str(text) => Some(text.clone()),
                    _ => None,
                })
                .collect();
        }
    }
    panic!("registry {registry} is in the corpus");
}

#[allow(clippy::expect_used, clippy::panic)]
#[test]
fn every_effect_on_condition_is_known_by_name() {
    let root = reference_root();
    let started = std::time::Instant::now();
    let loaded = eocs::load_paths(&[&root.join("data/core"), &root.join("data/json")])
        .expect("the effect files load");
    let theirs = reference_ids("effect_on_condition");

    let missing: Vec<&String> = theirs.difference(&loaded.ids).take(8).collect();
    let invented: Vec<&String> = loaded.ids.difference(&theirs).take(8).collect();
    println!(
        "effect_on_condition: {} ours, {} reference ({} defined, {} inline), {} missing, {} invented in {:.1}s",
        loaded.ids.len(),
        theirs.len(),
        loaded.defined,
        loaded.inline,
        theirs.difference(&loaded.ids).count(),
        loaded.ids.difference(&theirs).count(),
        started.elapsed().as_secs_f64()
    );
    assert!(
        missing.is_empty() && invented.is_empty(),
        "missing {missing:?}, invented {invented:?}"
    );
}
