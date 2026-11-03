//! The flag list is the **data's** list, not a list this port typed.
//!
//! Reference: `json_flag::get_all()` (`flag.h:406`), read by `f_foreach` when the data says `{"foreach": "ids", "target": "flag"}`
//! (`npctalk.cpp:7041-7058`). `corpus/flags.json` is the reference's own answer, so the count is the reference's count.

#![allow(clippy::expect_used, clippy::panic)]

use bevy_ecs::prelude::*;

fn installed() -> World {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/flags.json");
    let text = std::fs::read_to_string(path).expect("the flags dump");
    let value = cdda_json::parse(text.as_bytes()).expect("json");
    let cdda_json::Value::Object(root) = &value else {
        panic!("the dump is an object");
    };
    let Some(cdda_json::Value::Array(rows)) = root.get("flags") else {
        panic!("the dump carries a flags array");
    };
    let flags: Vec<String> = rows
        .iter()
        .filter_map(|row| match row {
            cdda_json::Value::Str(flag) => Some(flag.clone()),
            _ => None,
        })
        .collect();
    let mut world = World::new();
    cdda_sim::flag_registry::install(&mut world, flags);
    world
}

#[test]
fn the_port_carries_every_flag_the_reference_declares() {
    let world = installed();
    let flags = cdda_sim::flag_registry::flags_of(&world);
    assert_eq!(flags.len(), 731, "the reference declares 731 flags");
    assert!(
        cdda_sim::flag_registry::has(&world, "BELTED"),
        "and a flag a rule names is among them"
    );
    assert!(
        !cdda_sim::flag_registry::has(&world, "NOT_A_FLAG"),
        "and one it does not is not"
    );
}
