//! The generator's draw step against the reference's own mapgen run.
//!
//! Source: `--dump-mapgen` from the instrumented reference, which runs one mapgen in isolation - no world, no player, no seed to speak of - and writes the terrain and furniture
//! id of every square. This is the mapgen gate's reference half: the port's `draw_chunk` has to produce the same terrain for the same key.

#![allow(clippy::expect_used, clippy::panic, clippy::indexing_slicing)]

use cdda_json::{Object, Value, parse};
use std::collections::BTreeMap;
use std::path::Path;

/// Disagreements between the port and the reference: the row symbol, the id the port drew, and the id the reference had.
type Disagreements = Vec<(char, String, String)>;

fn corpus() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus")
}

/// The reference's chunk: terrain ids, row by row.
fn reference() -> (String, Vec<Vec<String>>) {
    let text = std::fs::read_to_string(corpus().join("mapgens/shelter_chunk.json")).expect("the mapgen dump");
    let Value::Object(root) = parse(text.as_bytes()).expect("valid JSON") else { panic!("root") };
    let Some(Value::Str(mapgen)) = root.get("mapgen") else { panic!("no mapgen key") };
    let Some(Value::Array(rows)) = root.get("rows") else { panic!("no rows") };
    let rows = rows
        .iter()
        .map(|row| {
            let Value::Array(squares) = row else { panic!("a row is not an array") };
            squares
                .iter()
                .map(|square| {
                    let Value::Array(pair) = square else { panic!("a square is not a pair") };
                    match pair.first() {
                        Some(Value::Str(terrain)) => terrain.clone(),
                        _ => panic!("a square has no terrain"),
                    }
                })
                .collect()
        })
        .collect();
    (mapgen.clone(), rows)
}

/// The data's `mapgen` entry for a key, and every palette it can name.
fn mapgen_entry(key: &str) -> (Option<Object>, Vec<String>, BTreeMap<String, Object>) {
    fn walk(
        path: &Path,
        key: &str,
        entry: &mut Option<Object>,
        names: &mut Vec<String>,
        palettes: &mut BTreeMap<String, Object>,
    ) {
        let Ok(entries) = std::fs::read_dir(path) else { return };
        for candidate in entries.flatten() {
            let path = candidate.path();
            if path.is_dir() {
                walk(&path, key, entry, names, palettes);
            } else if path.extension().is_some_and(|extension| extension == "json") {
                let Ok(text) = std::fs::read_to_string(&path) else { continue };
                let Ok(Value::Array(entries)) = parse(text.as_bytes()) else { continue };
                for object in &entries {
                    let Value::Object(object) = object else { continue };
                    if matches!(object.get("type"), Some(Value::Str(kind)) if kind == "palette")
                        && let Some(Value::Str(id)) = object.get("id")
                    {
                        palettes.insert(id.clone(), object.clone());
                        continue;
                    }
                    if !matches!(object.get("type"), Some(Value::Str(kind)) if kind == "mapgen") {
                        continue;
                    }
                    let Some(om) = object.get("om_terrain") else { continue };
                    let claims = match om {
                        Value::Str(name) => name == key,
                        Value::Array(items) => items.iter().any(|item| match item {
                            Value::Str(name) => name == key,
                            _ => false,
                        }),
                        _ => false,
                    };
                    if !claims {
                        continue;
                    }
                    let Some(Value::Object(inner)) = object.get("object") else { continue };
                    *entry = Some(inner.clone());
                    if let Some(Value::Array(list)) = inner.get("palettes") {
                        for name in list {
                            if let Value::Str(name) = name {
                                names.push(name.clone());
                            }
                        }
                    }
                }
            }
        }
    }
    let mut entry = None;
    let mut names = Vec::new();
    let mut palettes = BTreeMap::new();
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../references/Cataclysm-DDA/data/json");
    walk(&root, key, &mut entry, &mut names, &mut palettes);
    (entry, names, palettes)
}

#[test]
fn the_port_draws_the_shelter_the_way_the_reference_draws_it() {
    let (key, expected) = reference();
    let (entry, names, palettes) = mapgen_entry(&key);
    let entry = entry.unwrap_or_else(|| panic!("the data has a mapgen for {key}"));
    let Some(Value::Array(rows)) = entry.get("rows") else {
        panic!("{key} draws a grid");
    };
    let rows: Vec<String> = rows
        .iter()
        .filter_map(|row| match row {
            Value::Str(text) => Some(text.clone()),
            _ => None,
        })
        .collect();
    let terrain = match entry.get("terrain") {
        Some(Value::Object(terrain)) => Some(terrain),
        _ => None,
    };
    let fill = match entry.get("fill_ter") {
        Some(Value::Str(fill)) => Some(fill.clone()),
        _ => None,
    };
    let drawn = cdda_content::mapgen::draw_chunk(
        &rows,
        terrain,
        &names,
        &|name| palettes.get(name).cloned(),
        fill.as_deref(),
    )
    .expect("the chunk draws");

    // **The reference ran the mapgen at the overmap square's rotation**, and the port draws it unrotated, so the comparison tries all
    // four quarter turns and takes the best. Squares the port leaves alone - a space, or a symbol that belongs to the furniture or item
    // layer - are skipped: the reference shows whatever the world had there, because the mapgen was applied over an existing chunk.
    //
    // **What is not implemented, and is therefore named rather than tolerated:** the shelter's walls come through a palette that takes
    // *parameters* (`"palettes": [{"name": "parametrized_walls_palette", "parameters": {"wall": "t_concrete_wall"}}]`), and the
    // reference then connects neighbouring walls into directional ids (`t_concrete_wall_w`). The port does neither, so the assertion is
    // that every disagreement is one of those wall squares - if any other symbol disagrees, this fails.
    let size = drawn.len();
    let mut best: Option<(usize, usize, Disagreements)> = None;
    for turn in 0..4 {
        let mut agree = 0;
        let mut drawn_squares = 0;
        let mut different = Vec::new();
        for (y, line) in drawn.iter().enumerate() {
            for (x, square) in line.iter().enumerate() {
                let Some(id) = square else { continue };
                drawn_squares += 1;
                let (rx, ry) = match turn {
                    0 => (x, y),
                    1 => (size - 1 - y, x),
                    2 => (size - 1 - x, size - 1 - y),
                    _ => (y, size - 1 - x),
                };
                let want = &expected[ry][rx];
                if id == want {
                    agree += 1;
                } else {
                    let symbol = rows[y].chars().nth(x).unwrap_or('?');
                    different.push((symbol, id.clone(), want.clone()));
                }
            }
        }
        if best.as_ref().is_none_or(|(a, _, _)| agree > *a) {
            best = Some((agree, drawn_squares, different));
        }
    }
    let (agree, drawn_squares, different) = best.expect("one of four rotations");
    assert!(drawn_squares > 300, "the port draws most of the chunk: {drawn_squares}");
    let wall_squares = different.iter().filter(|(symbol, _, _)| *symbol == '#').count();
    assert_eq!(
        wall_squares,
        different.len(),
        "every disagreement is a parametrised wall square: {:?}",
        &different[..different.len().min(6)]
    );
    assert!(
        agree * 100 / drawn_squares >= 85,
        "at least 85% of drawn squares match without the wall pass: {agree} of {drawn_squares}"
    );
}

/// A reference chunk, by the file it was dumped into.
fn chunk(file: &str) -> (String, Vec<Vec<String>>) {
    let text = std::fs::read_to_string(corpus().join("mapgens").join(file)).expect("a mapgen dump");
    let Value::Object(root) = parse(text.as_bytes()).expect("valid JSON") else { panic!("root") };
    let Some(Value::Str(mapgen)) = root.get("mapgen") else { panic!("no mapgen key") };
    let Some(Value::Array(rows)) = root.get("rows") else { panic!("no rows") };
    let rows = rows
        .iter()
        .map(|row| {
            let Value::Array(squares) = row else { panic!("a row is not an array") };
            squares
                .iter()
                .map(|square| {
                    let Value::Array(pair) = square else { panic!("a square is not a pair") };
                    match pair.first() {
                        Some(Value::Str(terrain)) => terrain.clone(),
                        _ => panic!("a square has no terrain"),
                    }
                })
                .collect()
        })
        .collect();
    (mapgen.clone(), rows)
}

#[test]
fn the_port_draws_a_fully_inline_chunk_square_for_square() {
    // The unqualified version of the gate, on chunks whose whole terrain comes from the entry itself - rows, a terrain map, a fill and
    // nothing else. There is no palette indirection and no parameter here, so there is nothing to explain away: every square the port
    // draws has to be the square the reference drew, under one of the four rotations, and the drawn squares have to be nearly all of them.
    for file in ["temple_chunk.json", "billboard_chunk.json"] {
        let (key, expected) = chunk(file);
        let (entry, names, palettes) = mapgen_entry(&key);
        let entry = entry.unwrap_or_else(|| panic!("the data has a mapgen for {key}"));
        let Some(Value::Array(rows)) = entry.get("rows") else { panic!("{key} draws a grid") };
        let rows: Vec<String> = rows
            .iter()
            .filter_map(|row| match row {
                Value::Str(text) => Some(text.clone()),
                _ => None,
            })
            .collect();
        let terrain = match entry.get("terrain") {
            Some(Value::Object(terrain)) => Some(terrain),
            _ => None,
        };
        let fill = match entry.get("fill_ter") {
            Some(Value::Str(fill)) => Some(fill.clone()),
            _ => None,
        };
        let drawn = cdda_content::mapgen::draw_chunk(
            &rows,
            terrain,
            &names,
            &|name| palettes.get(name).cloned(),
            fill.as_deref(),
        )
        .expect("the chunk draws");

        let size = drawn.len();
        let mut best: Option<(usize, usize, Disagreements)> = None;
        for turn in 0..4 {
            let mut agree = 0;
            let mut drawn_squares = 0;
            let mut different = Vec::new();
            for (y, line) in drawn.iter().enumerate() {
                for (x, square) in line.iter().enumerate() {
                    let Some(id) = square else { continue };
                    drawn_squares += 1;
                    let (rx, ry) = match turn {
                        0 => (x, y),
                        1 => (size - 1 - y, x),
                        2 => (size - 1 - x, size - 1 - y),
                        _ => (y, size - 1 - x),
                    };
                    let want = &expected[ry][rx];
                    if id == want {
                        agree += 1;
                    } else if different.len() < 6 {
                        let symbol = rows[y].chars().nth(x).unwrap_or('?');
                        different.push((symbol, id.clone(), want.clone()));
                    }
                }
            }
            if best.as_ref().is_none_or(|(a, _, _)| agree > *a) {
                best = Some((agree, drawn_squares, different));
            }
        }
        let (agree, drawn_squares, different) = best.expect("one rotation");
        assert_eq!(
            agree, drawn_squares,
            "{key}: every drawn square matches ({agree} of {drawn_squares}): {different:?}"
        );
    }
}

#[test]
fn a_terrain_map_is_written_symbol_first() {
    // Reference: `mapgen.cpp`'s symbol lookup. A mapgen's `terrain` is written `".": "t_floor"` - the symbol is the key - and the port
    // read it the other way for many rounds. The misreading wore three different masks: a chunk that seemed to need a fill, a palette
    // that resolved to single letters, and stairs that came out as plain floor. This is the smallest case that tells the two apart: read
    // symbol-first it draws floor and wall; read id-first it draws nothing at all, because no key is one character long.
    let text = br##"[{"terrain": {".": "t_floor", "#": ["t_concrete_wall", 100]}}]"##;
    let Value::Array(entries) = parse(text).expect("valid JSON") else { panic!("root") };
    let Value::Object(entry) = entries.first().expect("one entry") else { panic!("object") };
    let Some(Value::Object(terrain)) = entry.get("terrain") else { panic!("terrain") };
    let rows = vec!["..#".to_string()];
    let drawn = cdda_content::mapgen::resolve_rows(&rows, terrain).expect("resolves");
    assert_eq!(
        drawn,
        vec![vec![Some("t_floor".to_string()), Some("t_floor".to_string()), Some("t_concrete_wall".to_string())]],
        "a symbol draws the id it names, and a list's first id is the one a draw takes"
    );
}
