//! Which zone an activity works in.
//!
//! Reference: `get_zone_for_act` (`activity_item_handling.cpp:2642-2688`).

#![allow(clippy::expect_used)]

use bevy_ecs::prelude::World;

/// **Eleven activities work in eleven zones.** Each line of the table is pinned, because a job sent to the wrong zone is a person walking to the wrong place all day - and the
/// table is the only thing that decides where a multiple activity looks first.
#[test]
fn a_multiple_activity_works_in_its_own_zone() {
    for (activity, zone) in [
        ("ACT_VEHICLE_DECONSTRUCTION", "VEHICLE_DECONSTRUCT"),
        ("ACT_VEHICLE_REPAIR", "VEHICLE_REPAIR"),
        ("ACT_MULTIPLE_CHOP_TREES", "CHOP_TREES"),
        ("ACT_MULTIPLE_CONSTRUCTION", "CONSTRUCTION_BLUEPRINT"),
        ("ACT_MULTIPLE_FARM", "FARM_PLOT"),
        ("ACT_MULTIPLE_BUTCHER", "LOOT_CORPSE"),
        ("ACT_MULTIPLE_CHOP_PLANKS", "LOOT_WOOD"),
        ("ACT_MULTIPLE_FISH", "FISHING_SPOT"),
        ("ACT_MULTIPLE_MINE", "MINING"),
        ("ACT_MULTIPLE_MOP", "MOPPING"),
        ("ACT_MULTIPLE_DIS", "DISASSEMBLE"),
    ] {
        assert_eq!(
            cdda_sim::multiple_work::zone_for_activity(activity),
            Some(zone),
            "**{activity} works in {zone}**"
        );
    }
    // **Reading and crafting work where they are, not in a zone**, and so does an activity nobody mapped: the reference answers the undefined zone for them.
    for activity in [
        "ACT_MULTIPLE_READ",
        "ACT_CRAFT",
        "ACT_SOCIALIZE",
        "ACT_UNHEARD_OF",
    ] {
        assert_eq!(
            cdda_sim::multiple_work::zone_for_activity(activity),
            None,
            "**{activity} works in no zone**"
        );
    }
}

/// **Every zone the table names is one the data defines.** This is the property that keeps the table honest as it grows: a typo in a zone id is a job that can never find work,
/// and it would fail here rather than in a playthrough.
#[test]
fn every_zone_the_table_names_is_in_the_data() {
    let reference = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../references/Cataclysm-DDA/data");
    let roots = [reference.join("core"), reference.join("json")];
    let paths: Vec<&std::path::Path> = roots.iter().map(std::path::PathBuf::as_path).collect();
    let loaded = cdda_content::zone_types::load_paths(&paths).expect("the zone types load");
    let mut world = World::new();
    cdda_sim::multiple_work::set_zone_types(&mut world, loaded.kinds);
    for (_, zone) in [
        ("ACT_VEHICLE_DECONSTRUCTION", "VEHICLE_DECONSTRUCT"),
        ("ACT_VEHICLE_REPAIR", "VEHICLE_REPAIR"),
        ("ACT_MULTIPLE_CHOP_TREES", "CHOP_TREES"),
        ("ACT_MULTIPLE_CONSTRUCTION", "CONSTRUCTION_BLUEPRINT"),
        ("ACT_MULTIPLE_FARM", "FARM_PLOT"),
        ("ACT_MULTIPLE_BUTCHER", "LOOT_CORPSE"),
        ("ACT_MULTIPLE_CHOP_PLANKS", "LOOT_WOOD"),
        ("ACT_MULTIPLE_FISH", "FISHING_SPOT"),
        ("ACT_MULTIPLE_MINE", "MINING"),
        ("ACT_MULTIPLE_MOP", "MOPPING"),
        ("ACT_MULTIPLE_DIS", "DISASSEMBLE"),
    ] {
        assert!(
            cdda_sim::multiple_work::zone_type_named(&world, zone).is_some(),
            "**the data defines the zone type {zone}**"
        );
    }
}
