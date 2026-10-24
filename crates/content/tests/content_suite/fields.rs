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

#[test]
fn only_two_fields_in_the_data_can_spawn_anything() {
    let all = dump();
    let spawners: Vec<(&str, i32, String)> = all
        .iter()
        .flat_map(|field| {
            field
                .levels
                .iter()
                .filter(|level| level.spawn_chance > 0)
                .map(move |level| {
                    (
                        field.id.as_str(),
                        level.spawn_chance,
                        level.spawn_group.clone(),
                    )
                })
        })
        .collect();
    assert_eq!(spawners.len(), 2, "spawning levels: {spawners:?}");
    assert!(
        spawners
            .iter()
            .any(|(id, _, group)| *id == "fd_tindalos_rift" && group == "GROUP_TINDALOS")
    );
    assert!(
        spawners
            .iter()
            .all(|(_, chance, group)| *chance > 0 && !group.is_empty())
    );
}

#[test]
fn the_dump_carries_the_light_absorption_of_every_intensity() {
    // `translucency` is what `calc_transp` multiplies into a square's value (`lightmap.cpp:158-165`), and it is **not** the same question as the `transparent` flag beside
    // it: smoke at intensity two is marked opaque and still has a translucency of ten, so it dims a ray rather than stopping it. The dump carries both, so the loader can
    // tell the difference instead of guessing from the flag.
    let loaded = dump();

    let smoke = fields::find(&loaded, "fd_smoke").expect("smoke");
    let absorption: Vec<f32> = smoke
        .levels
        .iter()
        .map(|level| level.translucency)
        .collect();
    assert_eq!(absorption, vec![1.0, 10.0, 0.0], "thin, thick, and solid");
    assert!(
        smoke.levels.get(2).map(|level| level.translucency) == Some(0.0),
        "and the thickest layer is solid, which is what blinds a line of sight"
    );
}

#[test]
fn the_dump_carries_what_the_radiation_and_wandering_processors_read() {
    // **The two processors that need no subsystem at all** (`src/map_field.cpp:664-690`). `field_processor_extra_radiation` rolls
    // `rng(extra_radiation_min, extra_radiation_max)` per intensity level and adds it to the square's radiation, and `field_processor_wandering_field` places the
    // type's `wandering_field` at the same intensity around the field. A dump without them would leave a port able to say what a gas *is* but not what it *does* -
    // which is how these two stayed unimplemented while twenty others were ported.
    let all = dump();
    let wandering: Vec<(&str, &str)> = all
        .iter()
        .filter(|field| field.wandering_field != "fd_null" && !field.wandering_field.is_empty())
        .map(|field| (field.id.as_str(), field.wandering_field.as_str()))
        .collect();
    assert_eq!(
        wandering,
        vec![
            ("fd_gas_vent", "fd_toxic_gas"),
            ("fd_tindalos_rift", "fd_tindalos_gas"),
            ("fd_smoke_vent", "fd_smoke"),
        ],
        "three vents wander, each into a named gas"
    );
    // Radiation is the other way round: one field emits it, and only at its higher intensities - `fd_nuke_gas` at levels two to four.
    let radiating: Vec<(&str, i32, i32)> = all
        .iter()
        .flat_map(|field| {
            field
                .levels
                .iter()
                .filter(|level| level.extra_radiation_max > 0)
                .map(move |level| (field.id.as_str(), level.extra_radiation_min, level.extra_radiation_max))
        })
        .collect();
    assert_eq!(
        radiating,
        vec![("fd_nuke_gas", 0, 1), ("fd_nuke_gas", 0, 2), ("fd_nuke_gas", 0, 3)],
        "one field radiates, and each of its three intensities radiates more than the last"
    );
}
