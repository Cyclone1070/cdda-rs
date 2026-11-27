//! The zone types the data holds.
//!
//! **Counts prove non-vacuity**: the data defines 64 `LOOT_ZONE` objects with 64 unique ids, and every one of them can be personal. The type tag is `LOOT_ZONE` rather than
//! `zone_type` - the game's own historical name for the file - which is why a loader that looked for `zone_type` would read none of them.

#![allow(clippy::expect_used)]

use cdda_content::zone_types;

fn data_roots() -> Vec<std::path::PathBuf> {
    let reference = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../references/Cataclysm-DDA/data");
    vec![reference.join("core"), reference.join("json")]
}

#[test]
fn every_zone_type_in_the_data_is_read() {
    let roots = data_roots();
    let paths: Vec<&std::path::Path> = roots.iter().map(std::path::PathBuf::as_path).collect();
    let zones = zone_types::load_paths(&paths).expect("the zone types load");
    assert_eq!(zones.kinds.len(), 64, "the data defines 64 zone types");
    assert_eq!(zones.by_id.len(), 64, "all of them have unique ids");
    let corpse = zones.by_id.get("LOOT_CORPSE").expect("the corpse zone");
    assert_eq!(corpse.name, "Loot: Corpses", "**the name the player sees**");
    assert!(corpse.can_be_personal, "**a zone one character can own**");
    assert!(
        corpse.description.contains("corpse"),
        "**and the description says what it is for: {}**",
        corpse.description
    );
    // **The activities' zone types are the data's**, which is what makes `get_zone_for_act` a table of ids rather than an invention: every one it names must exist.
    for id in [
        "LOOT_CORPSE",
        "LOOT_WOOD",
        "VEHICLE_DECONSTRUCT",
        "VEHICLE_REPAIR",
        "CHOP_TREES",
        "CONSTRUCTION_BLUEPRINT",
        "FARM_PLOT",
        "FISHING_SPOT",
        "MINING",
        "MOPPING",
        "DISASSEMBLE",
    ] {
        assert!(
            zones.by_id.contains_key(id),
            "**the data defines the zone type {id}**"
        );
    }
}
