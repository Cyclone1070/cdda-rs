//! The specials loader against the reference's own data.

#![allow(clippy::panic, clippy::expect_used, clippy::indexing_slicing)]

use cdda_content::overmap_special::{load_specials, read_special};
use cdda_json::Value;

fn specials() -> Vec<cdda_content::overmap_special::OvermapSpecial> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../references/Cataclysm-DDA/data/json/overmap/overmap_special");
    let mut documents = Vec::new();
    let mut paths: Vec<_> = std::fs::read_dir(&root)
        .expect("the specials directory")
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "json")
        })
        .collect();
    paths.sort();
    for path in paths {
        let bytes = std::fs::read(&path).expect("a specials file");
        documents.push(cdda_json::parse(&bytes).expect("valid json"));
    }
    load_specials(&documents)
}

#[test]
fn every_special_in_the_reference_is_read() {
    let specials = specials();
    assert_eq!(
        specials.len(),
        258,
        "the corpus holds 258 specials across eleven files"
    );
    assert!(
        specials.iter().all(|special| !special.id.is_empty()),
        "every one has an id"
    );
    // Two describe their shape as a graph of named nodes for the mutable generator instead of a list of tiles.
    let mutable = specials
        .iter()
        .filter(|special| special.subtype.as_deref() == Some("mutable"))
        .count();
    assert_eq!(mutable, 2, "two mutable specials");
    let with_tiles = specials
        .iter()
        .filter(|special| !special.overmaps.is_empty())
        .count();
    assert_eq!(with_tiles, 256, "and 256 have a fixed shape of tiles");
}

#[test]
fn a_special_is_a_shape_of_terrain_tiles() {
    let specials = specials();
    let farm = specials
        .iter()
        .find(|special| special.id == "quiet_farm")
        .expect("quiet_farm is in the corpus");
    assert_eq!(farm.overmaps.len(), 11, "eleven tiles");
    assert_eq!(farm.overmaps[0].offset, (0, -1, 0));
    assert_eq!(farm.overmaps[0].terrain, "rural_road_forest_ns");
    assert_eq!(farm.occurrences, (10, 100));
    assert!(
        farm.overmaps
            .iter()
            .any(|tile| tile.terrain == "unvitrified_farm_0")
    );
}

#[test]
fn placement_rules_are_read_and_defaulted_the_reference_way() {
    let specials = specials();
    let rotatable = specials.iter().filter(|special| special.rotate).count();
    assert_eq!(
        rotatable,
        258 - 15,
        "fifteen specials refuse to rotate, and the rest may"
    );
    // One special states `[-1, 1000]` explicitly, which is also the default, so the count of *differing* values is
    // 225 while 226 entries carry the key. Both numbers are pinned because confusing them is easy.
    let with_distance = specials
        .iter()
        .filter(|special| special.city_distance != (-1, 1000))
        .count();
    assert_eq!(
        with_distance, 225,
        "225 differ from the default, 226 state it"
    );
    let with_sizes = specials
        .iter()
        .filter(|special| special.city_sizes != (0, 1000))
        .count();
    assert_eq!(with_sizes, 131, "131 state a city size range");
    // Twenty-six specials may **never** be placed on their own: they exist to be reached from somewhere else (the
    // world itself, a campus's parts), and their occurrences are `[0, 0]`. Reading that as "rare" rather than "never"
    // would put a campus in every town.
    let never = specials
        .iter()
        .filter(|special| special.occurrences == (0, 0))
        .count();
    assert_eq!(never, 26, "twenty-six are placed only by something else");
}

#[test]
fn unique_specials_say_so_in_their_flags() {
    let specials = specials();
    let unique = specials
        .iter()
        .filter(|special| special.flags.iter().any(|flag| flag == "OVERMAP_UNIQUE"))
        .count();
    assert!(
        unique > 0,
        "some specials may appear once per overmap: {unique}"
    );
    assert!(
        specials
            .iter()
            .all(|special| special.flags.iter().all(|flag| !flag.is_empty()))
    );
}

#[test]
fn connections_are_read_with_their_offsets() {
    let specials = specials();
    let with_connections = specials
        .iter()
        .filter(|special| !special.connections.is_empty())
        .count();
    assert_eq!(with_connections, 155, "155 specials join the road network");
    let highway = specials
        .iter()
        .find(|special| !special.connections.is_empty())
        .expect("one of them");
    assert!(!highway.connections[0].connection.is_empty());
    assert!(!highway.connections[0].terrain.is_empty());
}

#[test]
fn something_that_is_not_a_special_is_not_read_as_one() {
    let entry = cdda_json::parse(br#"{"type":"overmap_terrain","id":"field","name":"field"}"#)
        .expect("valid json");
    assert!(read_special(&entry).is_none());
    // A special with no `overmaps` at all still reads, but has no shape - the loader does not invent tiles for it.
    let bare = cdda_json::parse(br#"{"type":"overmap_special","id":"x","occurrences":[1,2]}"#)
        .expect("valid json");
    let special = read_special(&bare).expect("a special");
    assert!(special.overmaps.is_empty(), "no tiles, no shape");
    assert_eq!(special.occurrences, (1, 2));
    let Value::Object(_) = bare else {
        panic!("an object")
    };
}
