//! Which of a mapgen's members the port reads, and which it does not.
//!
//! The refusal censuses count programs that stop. A mapgen member the loader ignores **stops nothing**: the chunk is drawn without its toilets, its vending machines or its
//! nested rooms and nothing complains - round 288 found 298 uses of a member the port had never read. This test is the instrument for that class of gap: it walks the data,
//! collects every member a mapgen or a palette writes, and requires each one to be either **read** or **named as a gap**, so a member can only be ignored on purpose.

#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// The members the loader reads out of a mapgen's `object`, each one proven by a test.
///
/// This list is checked against the loader by `every_member_the_loader_reads_is_in_the_ledger` below, so it cannot drift into a claim about code that reads something else.
const READ: &[&str] = &[
    "rows",
    "terrain",
    "furniture",
    "traps",
    "items",
    "place_monster",
    "place_fields",
    "toilets",
    "liquids",
    "place_items",
    "place_loot",
    "sealed_item",
    "vendingmachines",
    "signs",
    "gaspumps",
    "nested",
    "place_terrain",
    "place_furniture",
    "place_traps",
    "item",
    "fields",
    "computers",
    "remove_all",
    "monster",
    "monsters",
    "rotation",
    "set",
    "place_zones",
    "place_monsters",
    "place_vehicles",
    "place_ter_furn_transforms",
    "parameters",
    "mapping",
    "rubble",
    "place_rubble",
    "place_nested",
    "place_item",
    "place_signs",
    "place_liquids",
    "fill_ter",
    "predecessor_mapgen",
    "fallback_predecessor_mapgen",
    "mapgensize",
    "palettes",
];

/// The members the loader ignores, each with the reason it is ignored - and each with the reference's own site where the reason is not obvious.
const NAMED_GAPS: &[&str] = &[
    // A nested mapgen with its own weighted list, and the machinery to choose one and stitch it into the chunk.
    // The parameterised form of every symbol map: `place_terrain`, `place_furniture`, `place_traps`, `place_items`, ... draw over a **rectangle** rather than from
    // rows, and each is the symbol map's rule with a range. The data writes both; the port reads the row form.
    "place_toilets",
    "place_vendingmachines",
    "place_computers",
    "place_graffiti",
    "place_corpses",
    "place_variables",
    // A spawn point: the monster group, its density and the map's spawn inventory, which is a subsystem of its own.
    // Furniture with contents or text: each needs the thing it holds or says before it can be placed honestly.
    // Vehicles are P8.
    "vehicles",
    "remove_vehicles",
    // Clearing a square needs the removal phase and every layer it clears.
    // Rubble is a terrain and an item together.
    // NPCs need templates and the NPC store, which is P7.
    "npcs",
    "remove_npcs",
    // Terrain and furniture that change into each other (`ter_furn_transform`), which is a loader of its own.
    "ter_furn_transforms",
    // A mapgen's own parameters and the size it is drawn at, which the overmap special supplies (P9).
    // Factions and the owner of a chunk are P9.
    "faction_owner",
    // A predecessor that only applies when the request asks for one (`expects_predecessor`), and its fallback.
];

/// Five more the census found once the first thirty-one were named.
const MORE_GAPS: &[&str] = &[
    // The symbol form of a field patch (the coordinate form, `place_fields`, is read).
    // A mapgen's own flags, which the overmap rules read (P9).
    "flags",
    // The single-item symbol map, whose group form (`items`) is read.
    // The rectangle form of `npcs`.
    "place_npcs",
];

/// Members that are **comments**, not placements: the data's own `//` keys.
const COMMENTS: &[&str] = &["//", "//1", "//2", "//3", "//4"];

/// Every member name the data writes inside a mapgen's or a palette's `object`.
fn members_in_the_data() -> BTreeSet<String> {
    let data = root().join("references/Cataclysm-DDA/data/json");
    let files = cdda_content::json_files_in_load_order(&data).expect("the data tree is readable");
    let mut found = BTreeSet::new();
    for path in files {
        let Some(text) = std::fs::read_to_string(&path).ok() else {
            continue;
        };
        let Ok(value) = cdda_json::parse(text.as_bytes()) else {
            continue;
        };
        let cdda_json::Value::Array(objects) = value else {
            continue;
        };
        for object in objects {
            let cdda_json::Value::Object(object) = object else {
                continue;
            };
            let kind = match object.get("type") {
                Some(cdda_json::Value::Str(kind)) => kind.as_str(),
                _ => continue,
            };
            if kind != "mapgen" && kind != "palette" {
                continue;
            }
            let Some(cdda_json::Value::Object(inner)) = object.get("object") else {
                continue;
            };
            for (member, _) in inner.members() {
                found.insert(member.clone());
            }
        }
    }
    found
}

/// **Every member the data writes is read or named.** A member that is neither would be ignored silently - which is what a toilet was for 298 uses.
#[test]
fn every_mapgen_member_the_data_writes_is_read_or_named() {
    let found = members_in_the_data();
    assert!(
        found.len() > 30,
        "**the census must see the data**: it found {} members",
        found.len()
    );
    let read: BTreeSet<&str> = READ.iter().copied().collect();
    let named: BTreeSet<&str> = NAMED_GAPS.iter().chain(MORE_GAPS.iter()).copied().collect();
    let commented: BTreeSet<&str> = COMMENTS.iter().copied().collect();
    let unclassified: Vec<&String> = found
        .iter()
        .filter(|member| {
            !read.contains(member.as_str())
                && !named.contains(member.as_str())
                && !commented.contains(member.as_str())
        })
        .collect();
    assert!(
        unclassified.is_empty(),
        "**{} members are neither read nor named as gaps**: {unclassified:?}\n\
         read: {}, named: {}, in the data: {}",
        unclassified.len(),
        read.len(),
        named.len(),
        found.len()
    );
    // And the ledger is not allowed to grow stale in the other direction: a name in it that the data never writes is one nobody can check.
    let stale: Vec<&str> = read
        .iter()
        .chain(named.iter())
        .filter(|member| !found.contains(**member))
        .copied()
        .collect();
    assert!(
        stale.is_empty(),
        "**these names are in the ledger and not in the data**: {stale:?}"
    );
    // The two lists must not overlap: a member that is both read and a gap is a disagreement about the same square.
    let both: Vec<&&str> = read.intersection(&named).collect();
    assert!(both.is_empty(), "**read and named at once**: {both:?}");
}
