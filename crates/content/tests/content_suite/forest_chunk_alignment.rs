//! A trace that names its own chunk, and what that settles.
//!
//! Every feature-trace line now begins with the chunk it belongs to, so the 576 lines of a chunk are a **lookup** instead
//! of a guess. That removes the doubt of the previous round and leaves the finding standing on its own feet: even for the
//! chunk the dump describes, most squares hold a terrain their recorded palette could not have produced.

#![allow(
    clippy::panic,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss
)]

use std::collections::BTreeMap;

use cdda_content::overmap_special::read_region_palettes;

struct Line {
    chunk: (i64, i64, i64),
    x: i32,
    y: i32,
    palette: String,
}

fn trace() -> Vec<Line> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../corpus/overmap/reference-chunk-water-features.txt");
    std::fs::read_to_string(&path)
        .expect("the trace")
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            let fields: Vec<&str> = line.split_whitespace().collect();
            Line {
                chunk: (
                    fields[0].parse().expect("chunk x"),
                    fields[1].parse().expect("chunk y"),
                    fields[2].parse().expect("chunk z"),
                ),
                x: fields[3].parse().expect("x"),
                y: fields[4].parse().expect("y"),
                palette: fields[6].to_owned(),
            }
        })
        .collect()
}

fn cells() -> Vec<String> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../corpus/maps/reference-chunk-water.json");
    let cdda_json::Value::Object(root) =
        cdda_json::parse(&std::fs::read(&path).expect("the chunk")).expect("valid json")
    else {
        panic!("an object");
    };
    let Some(cdda_json::Value::Array(rows)) = root.get("rows") else {
        panic!("rows");
    };
    let mut out = Vec::new();
    for row in rows {
        let cdda_json::Value::Array(row) = row else {
            panic!("a row");
        };
        for cell in row {
            let cdda_json::Value::Str(text) = cell else {
                panic!("a terrain id");
            };
            out.push(text.clone());
        }
    }
    out
}

fn palettes() -> BTreeMap<String, Vec<(String, i32)>> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../references/Cataclysm-DDA/data/json/regional_map_settings.json");
    let root =
        cdda_json::parse(&std::fs::read(&path).expect("the region settings")).expect("valid json");
    read_region_palettes(&root)
}

fn reachable(palettes: &BTreeMap<String, Vec<(String, i32)>>, name: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut pending = vec![name.to_owned()];
    while let Some(current) = pending.pop() {
        assert!(pending.len() < 4096, "the palette chain walked too far");
        match palettes.get(&current) {
            Some(entries) => pending.extend(entries.iter().map(|(entry, _)| entry.clone())),
            None => out.push(current),
        }
    }
    out.sort_unstable();
    out.dedup();
    out
}

#[test]
fn the_chunk_a_line_belongs_to_is_written_on_the_line() {
    let lines = trace();
    assert_eq!(lines.len(), 576);
    // Every line of this chunk names the same chunk, and it is the chunk the dump describes: the dump's square is
    // [129,206] and the trace's identifier is its submap origin, twice that.
    assert!(
        lines.iter().all(|line| line.chunk == (258, 412, 0)),
        "one chunk"
    );
    // Measured: this is a **wet** woodland, so most of its ground cover comes from the swamp palette - 117 squares - and
    // only seven from the dry one. My first version expected a hundred and three of the dry palette, carrying the previous
    // chunk's numbers into this one.
    assert_eq!(
        lines
            .iter()
            .filter(|line| line.palette == "t_region_groundcover_swamp")
            .count(),
        117
    );
    assert_eq!(
        lines
            .iter()
            .filter(|line| line.palette == "t_region_groundcover_forest")
            .count(),
        7
    );
}

#[test]
fn most_squares_do_not_hold_a_terrain_their_recorded_palette_could_produce() {
    // With alignment settled, the finding of the previous round is no longer an artefact of which block was read: **260
    // of 576** squares hold a terrain their palette could not produce. The placed terrain therefore comes from somewhere
    // this trace does not record, and the palette weights cannot be checked against this chunk until that is traced.
    let palettes = palettes();
    let lines = trace();
    let cells = cells();
    let mut inside = 0;
    for line in &lines {
        let cell = &cells[(line.y * 24 + line.x) as usize];
        if reachable(&palettes, &line.palette).contains(cell) {
            inside += 1;
        }
    }
    assert_eq!(
        inside, 316,
        "{inside} of 576 squares hold a terrain from their own palette"
    );
    assert_eq!(576 - inside, 260);
}

#[test]
fn the_palettes_a_chunk_draws_are_counted_without_reference_to_the_map() {
    // The half that is trustworthy whatever the map does: what the raffle chose. This chunk is wet woodland, so its draws
    // differ from the dry chunk's - which is itself the check that the trace is describing this chunk and not another.
    let lines = trace();
    let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
    for line in &lines {
        *counts.entry(line.palette.as_str()).or_insert(0) += 1;
    }
    assert_eq!(
        counts.get("t_region_groundcover_swamp").copied(),
        Some(117),
        "wet ground cover"
    );
    assert_eq!(
        counts.get("t_region_groundcover_forest").copied(),
        Some(7),
        "and a little dry"
    );
    assert!(
        counts.len() >= 3,
        "several palettes draw the chunk: {counts:?}"
    );
}
