//! Field prototypes, against the reference's own dump.

#![allow(clippy::expect_used, clippy::panic)]

use cdda_content::fields::{self, FieldType};
use cdda_json::parse;
use std::path::Path;

fn dump() -> Vec<FieldType> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/prototypes/fields.json");
    let text = std::fs::read_to_string(path).expect("the field dump exists");
    let value = parse(text.as_bytes()).expect("the dump is valid JSON");
    fields::from_dump(&value).expect("the dump loads")
}

#[test]
fn the_port_finds_every_field_the_reference_defines() {
    assert_eq!(dump().len(), 145, "field types");
}

#[test]
fn the_number_of_levels_is_the_maximum_intensity() {
    let all = dump();
    let smoke = fields::find(&all, "fd_smoke").expect("smoke exists");
    assert_eq!(smoke.max_intensity(), 3, "smoke has three intensities");
    let fire = fields::find(&all, "fd_fire").expect("fire exists");
    assert!(fire.max_intensity() >= 1, "fire has at least one");
    for field in &all {
        assert!(!field.levels.is_empty(), "{} must have levels", field.id);
    }
}

#[test]
fn a_level_carries_what_the_runtime_draws_and_costs() {
    let all = dump();
    let fire = fields::find(&all, "fd_fire").expect("fire exists");
    let first = fire.levels.first().expect("fire has a first level");
    assert!(first.dangerous, "intensity 1 fire is dangerous");
    assert!(!first.color.is_empty(), "and has a colour");
    // Thin smoke lets light through; thick smoke does not. That is why a smoke field dims a room
    // without extinguishing it, and it is data, not a rule the port can invent.
    let smoke = fields::find(&all, "fd_smoke").expect("smoke exists");
    let transparent: Vec<bool> = smoke.levels.iter().map(|l| l.transparent).collect();
    assert_eq!(
        transparent,
        vec![true, false, false],
        "thin smoke is see-through, thick smoke is not"
    );
    // Fire outranks smoke when both are on one tile (priority 4 against 8 means smoke wins).
    let fire = fields::find(&all, "fd_fire").expect("fire exists");
    assert!(
        smoke.priority > fire.priority,
        "smoke draws over fire: {} > {}",
        smoke.priority,
        fire.priority
    );
}

#[test]
fn a_field_with_no_levels_is_refused() {
    let value =
        parse(br#"{"prototypes":[{"id":"fd_x","priority":1,"levels":[]}]}"#).expect("valid JSON");
    assert!(
        fields::from_dump(&value).is_err(),
        "a field with no levels cannot be drawn"
    );
}

#[test]
fn the_processor_data_arrives_with_each_field() {
    // These three are what the remaining processors read: gas spread, underwater aging, and the
    // slow self-upgrade that only one field in the data uses.
    let all = dump();
    let smoke = fields::find(&all, "fd_smoke").expect("smoke exists");
    assert_eq!(smoke.percent_spread, 10, "smoke spreads one turn in ten");
    let underwater = all
        .iter()
        .find(|field| field.underwater_age_speedup > 0)
        .expect("something ages faster in water");
    assert!(underwater.underwater_age_speedup > 0);
    let upgrades: Vec<&str> = all
        .iter()
        .filter(|field| field.levels.iter().any(|level| level.upgrade_chance > 0))
        .map(|field| field.id.as_str())
        .collect();
    assert_eq!(
        upgrades,
        vec!["fd_fatigue"],
        "exactly one field can upgrade itself"
    );
}
