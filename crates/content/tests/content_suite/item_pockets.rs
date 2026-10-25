//! Every type's pockets, replayed against the reference's own record.
//!
//! `corpus/items/pockets.json` is `--dump-pockets`: for each of the reference's 9,911 item types that has pockets, every pocket it holds — in order — with what it is
//! for, whether a liquid or a gas may go in, whether it keeps its own shape, how much it holds, how much it takes, and the three compatibility limits.
//!
//! **The recording is the CONTAINER pockets, and only those**: the dump walks `get_all_contained_pockets`, which is `get_pockets( is_type( CONTAINER ) )`
//! (`src/item_contents.cpp:2142-2147`) - so a tool whose only pocket is a `MAGAZINE` has none, and the port must filter the same way to be compared with it. The port's
//! loader reads **every** pocket, because every pocket is data that later rules will ask about, and the test says how many more it holds.
//!
//! The limits are the reason this gate exists. Two of them are read straight from the data and one is **computed**: a pocket that states no longest side takes an
//! item whose length is at most the diagonal of a cube of its own capacity, which is `default_length_from_volume( capacity ) * sqrt( 2 )`. A port that copied the data
//! and skipped the computation would think a 500 ml pocket takes anything at all, when the reference stops at 113 mm.

#![allow(clippy::expect_used, clippy::panic)]

use cdda_json::Value;
use std::collections::BTreeMap;

/// One recorded pocket, in the units the port keeps.
struct Recorded {
    kind: String,
    watertight: bool,
    airtight: bool,
    rigid: bool,
    max_volume: i64,
    max_weight: i64,
    max_length: i64,
    min_length: i64,
    min_volume: i64,
}

fn dump() -> BTreeMap<String, Vec<Recorded>> {
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/items/pockets.json");
    let bytes =
        std::fs::read(&path).unwrap_or_else(|_| panic!("the recording at {}", path.display()));
    let Value::Object(root) = cdda_json::parse(&bytes).expect("valid json") else {
        panic!("the dump is an object")
    };
    let Some(Value::Array(types)) = root.get("types") else {
        panic!("the dump holds types")
    };
    let mut recorded = BTreeMap::new();
    for entry in types {
        let Value::Object(entry) = entry else {
            panic!("a type")
        };
        let Some(Value::Str(id)) = entry.get("id") else {
            panic!("an id")
        };
        let Some(Value::Array(pockets)) = entry.get("pockets") else {
            panic!("pockets")
        };
        let mut specs = Vec::new();
        for pocket in pockets {
            let Value::Object(pocket) = pocket else {
                panic!("a pocket")
            };
            let text = |key: &str| match pocket.get(key) {
                Some(Value::Str(value)) => value.clone(),
                other => panic!("{key} is text: {other:?}"),
            };
            let whole = |key: &str| match pocket.get(key) {
                Some(Value::Int(value)) => *value,
                other => panic!("{key} is a whole number: {other:?}"),
            };
            let flag = |key: &str| matches!(pocket.get(key), Some(Value::Bool(true)));
            specs.push(Recorded {
                kind: text("kind"),
                watertight: flag("watertight"),
                airtight: flag("airtight"),
                rigid: flag("rigid"),
                max_volume: whole("max_volume"),
                max_weight: whole("max_weight"),
                max_length: whole("max_length"),
                min_length: whole("min_length"),
                min_volume: whole("min_volume"),
            });
        }
        recorded.insert(id.clone(), specs);
    }
    recorded
}

#[test]
fn the_port_reads_every_pocket_the_reference_has() {
    let recorded = dump();
    assert_eq!(recorded.len(), 9_911, "a pocket dump of every type");

    let types = crate::corpus::items_loaded();
    let mut with_pockets = 0;
    let mut limits = 0;
    let mut differing: Vec<(String, usize, String)> = Vec::new();
    for item in &types {
        let Some(expected) = recorded.get(&item.id) else {
            continue;
        };
        if !expected.is_empty() {
            with_pockets += 1;
        }
        let container_pockets: Vec<&cdda_content::pockets::PocketSpec> =
            item.pockets.iter().filter(|pocket| pocket.kind == "CONTAINER").collect();
        for (index, pocket) in expected.iter().enumerate() {
            if pocket.max_length != 0 || pocket.min_length != 0 || pocket.min_volume != 0 {
                limits += 1;
            }
            let got = container_pockets.get(index).copied();
            let same = got.is_some_and(|got| {
                got.kind == pocket.kind
                    && got.watertight == pocket.watertight
                    && got.airtight == pocket.airtight
                    && got.rigid == pocket.rigid
                    && got.max_volume_ml == pocket.max_volume
                    && got.max_weight_grams == pocket.max_weight
                    && got.max_length_mm == pocket.max_length
                    && got.min_length_mm == pocket.min_length
                    && got.min_volume_ml == pocket.min_volume
            });
            if !same {
                differing.push((
                    item.id.clone(),
                    index,
                    got.map_or_else(
                        || "missing".to_owned(),
                        |got| {
                            format!(
                                "{} liquid={} gas={} rigid={} volume={} weight={} length={} min_length={} min_volume={}",
                                got.kind,
                                got.watertight,
                                got.airtight,
                                got.rigid,
                                got.max_volume_ml,
                                got.max_weight_grams,
                                got.max_length_mm,
                                got.min_length_mm,
                                got.min_volume_ml
                            )
                        },
                    ),
                ));
            }
        }
        if container_pockets.len() != expected.len() {
            differing.push((
                item.id.clone(),
                expected.len(),
                format!("the port has {} container pockets", container_pockets.len()),
            ));
        }
    }
    // Measured, not guessed: the reference gives 1,146 of its 9,911 types pockets at all, 2,528 pockets between them, and 2,502 of those carry at least one of the three
    // compatibility limits - so both numbers are pinned here, and a corpus that quietly stopped describing pockets would fail rather than pass.
    assert_eq!(
        with_pockets, 1_146,
        "1,146 types have pockets; the port saw {with_pockets}"
    );
    assert_eq!(
        limits, 2_502,
        "2,502 pockets carry a limit; the port saw {limits}"
    );
    assert!(
        differing.is_empty(),
        "{} pockets disagree; the first twenty are {:#?}",
        differing.len(),
        differing.get(..20).unwrap_or(&differing)
    );
}
