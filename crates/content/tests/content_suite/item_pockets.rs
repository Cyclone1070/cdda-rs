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

/// One recorded pocket, in the units the port keeps. A plain record of the reference's own struct, flags and all.
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug)]
struct Recorded {
    items: Vec<String>,
    flags: Vec<String>,
    materials: Vec<String>,
    holster: bool,
    ablative: bool,
    sealed: bool,
    max_item_volume: i64,
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
            let list = |key: &str| match pocket.get(key) {
                Some(Value::Array(entries)) => entries
                    .iter()
                    .map(|entry| match entry {
                        Value::Str(id) => id.clone(),
                        other => panic!("{key} holds text: {other:?}"),
                    })
                    .collect::<Vec<_>>(),
                other => panic!("{key} is a list: {other:?}"),
            };
            specs.push(Recorded {
                items: list("items"),
                flags: list("flags"),
                materials: list("materials"),
                holster: flag("holster"),
                ablative: flag("ablative"),
                sealed: flag("sealed"),
                max_item_volume: whole("max_item_volume"),
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

/// How much of the recording actually exercises each rule, so that the gate cannot pass on a corpus that stopped stating the interesting fields.
#[derive(Default)]
struct Counts {
    items: usize,
    flags: usize,
    materials: usize,
    holsters: usize,
    ablative: usize,
    volumes: usize,
    sealed: usize,
}

impl Counts {
    fn count(&mut self, pocket: &Recorded) {
        self.items += usize::from(!pocket.items.is_empty());
        self.flags += usize::from(!pocket.flags.is_empty());
        self.materials += usize::from(!pocket.materials.is_empty());
        self.holsters += usize::from(pocket.holster);
        self.ablative += usize::from(pocket.ablative);
        self.volumes += usize::from(pocket.max_item_volume >= 0);
        self.sealed += usize::from(pocket.sealed);
    }
}

/// A list in alphabetical order, for comparing two orders that are both arbitrary.
fn sorted(ids: &[String]) -> Vec<String> {
    let mut sorted: Vec<String> = ids.to_vec();
    sorted.sort_unstable();
    sorted
}

/// What a comparison found: how many types and pockets were seen, and every pocket that disagreed.
#[derive(Default)]
struct Report {
    with_pockets: usize,
    limits: usize,
    counts: Counts,
    differing: Vec<(String, usize, String)>,
}

/// Compares every pocket the reference recorded with the pocket the port read for it.
fn compare() -> Report {
    let recorded = dump();
    assert_eq!(recorded.len(), 9_911, "a pocket dump of every type");
    let mut report = Report::default();
    for item in crate::corpus::items_loaded() {
        let Some(expected) = recorded.get(&item.id) else {
            continue;
        };
        report.with_pockets += usize::from(!expected.is_empty());
        let container_pockets: Vec<&cdda_content::pockets::PocketSpec> =
            item.pockets.iter().filter(|pocket| pocket.kind == "CONTAINER").collect();
        for (index, pocket) in expected.iter().enumerate() {
            report.counts.count(pocket);
            if pocket.max_length != 0 || pocket.min_length != 0 || pocket.min_volume != 0 {
                report.limits += 1;
            }
            let got = container_pockets.get(index).copied();
            if !same(got, pocket) {
                report
                    .differing
                    .push((item.id.clone(), index, describe(got, pocket)));
            }
        }
        if container_pockets.len() != expected.len() {
            report.differing.push((
                item.id.clone(),
                expected.len(),
                format!("the port has {} container pockets", container_pockets.len()),
            ));
        }
    }
    report
}

/// Whether the port's pocket is the reference's, field for field.
fn same(got: Option<&cdda_content::pockets::PocketSpec>, pocket: &Recorded) -> bool {
    got.is_some_and(|got| {
        got.kind == pocket.kind
            && got.watertight == pocket.watertight
            && got.airtight == pocket.airtight
            && got.rigid == pocket.rigid
            && got.max_volume_ml == pocket.max_volume
            && got.max_weight_grams == pocket.max_weight
            && got.max_length_mm == pocket.max_length
            && got.min_length_mm == pocket.min_length
            && got.min_volume_ml == pocket.min_volume
            && got.holster == pocket.holster
            && got.ablative == pocket.ablative
            && got.sealed == pocket.sealed
            && got.max_item_volume_ml == (pocket.max_item_volume >= 0).then_some(pocket.max_item_volume)
            // The three accept-lists are compared as **sets**: the reference keeps them in `flat_set`/`std::set` keyed by `string_id`, whose ordering is its
            // own interning order rather than an alphabetical one - which is why a wallet's flags come out `CREDIT_CARD_SHAPED` before `BANK_NOTE_SHAPED`.
            // No rule reads the order; every rule asks membership.
            && sorted(&got.items) == sorted(&pocket.items)
            && sorted(&got.flags) == sorted(&pocket.flags)
            && sorted(&got.materials) == sorted(&pocket.materials)
    })
}

/// Both sides of a disagreement, written out so that a failure names the field rather than the tuple.
fn describe(got: Option<&cdda_content::pockets::PocketSpec>, pocket: &Recorded) -> String {
    let Some(got) = got else {
        return format!("missing; the reference has {pocket:?}");
    };
    format!(
        "{} liquid={} gas={} rigid={} volume={} weight={} length={} min_length={} min_volume={} holster={} ablative={} sealed={} max_item_volume={:?} items={:?} flags={:?} materials={:?}",
        got.kind,
        got.watertight,
        got.airtight,
        got.rigid,
        got.max_volume_ml,
        got.max_weight_grams,
        got.max_length_mm,
        got.min_length_mm,
        got.min_volume_ml,
        got.holster,
        got.ablative,
        got.sealed,
        got.max_item_volume_ml,
        got.items,
        got.flags,
        got.materials
    )
}

#[test]
fn the_port_reads_every_pocket_the_reference_has() {
    let report = compare();
    // Measured, not guessed: the reference gives 1,146 of its 9,911 types pockets at all, 2,528 pockets between them, and 2,502 of those carry at least one of the
    // three compatibility limits - so both numbers are pinned here, and a corpus that quietly stopped describing pockets would fail rather than pass.
    assert_eq!(
        report.with_pockets, 1_146,
        "1,146 types have pockets; the port saw {}",
        report.with_pockets
    );
    assert_eq!(
        report.limits, 2_502,
        "2,502 pockets carry a limit; the port saw {}",
        report.limits
    );
    assert_eq!(
        report.differing,
        Vec::new(),
        "{} pockets disagree; the first twenty are {:#?}",
        report.differing.len(),
        report.differing.get(..20).unwrap_or(&report.differing)
    );
}

#[test]
fn the_accept_lists_and_the_one_item_rules_are_the_per_pocket_ones_the_reference_holds() {
    let report = compare();
    let counts = &report.counts;
    assert!(
        counts.items > 30 && counts.flags > 700 && counts.materials >= 3,
        "the corpus has to state accept-lists: it states {} item lists, {} flag lists and {} material lists",
        counts.items,
        counts.flags,
        counts.materials
    );
    assert!(
        counts.holsters > 800 && counts.ablative > 500 && counts.volumes > 60 && counts.sealed > 30,
        "the corpus has to state the one-item rules too: {} holsters, {} ablative, {} with a biggest item, {} sealed",
        counts.holsters,
        counts.ablative,
        counts.volumes,
        counts.sealed
    );
    assert_eq!(
        report.differing,
        Vec::new(),
        "{} pockets disagree; the first twenty are {:#?}",
        report.differing.len(),
        report.differing.get(..20).unwrap_or(&report.differing)
    );
}
