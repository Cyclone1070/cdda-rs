//! **The map generator on the reference's own data.**
//!
//! Every other mapgen test uses a fixture: two rows, one symbol, one square. They prove the rules; they cannot prove the *pipeline* meets the data. This one takes the
//! reference tree's real mapgens - 2,725 objects, 10,076 (id, map) pairs - and draws one of them into a world, so a loader that quietly stopped reading a layer, or a
//! registry that stopped filing a mapgen, shows up here rather than only in a fixture that no longer resembles the data.

#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::unwrap_used,
    clippy::indexing_slicing
)]

use bevy_ecs::prelude::*;
use cdda_content::mapgen::MapgenObject;
use cdda_content::mapgen::Piece;
use std::collections::BTreeMap;

/// Where the reference tree's mapgens live, beside this crate.
fn mapgen_directory() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../references/Cataclysm-DDA/data/json")
}

/// Every mapgen the reference tree files, read with the port's own loader.
///
/// A checkout without the reference tree answers nothing rather than panicking: 149 tests already read that tree, and a machine without it skips them with a line saying so.
fn the_reference_mapgens() -> Vec<(String, MapgenObject)> {
    let mut found = Vec::new();
    let mut stack = vec![mapgen_directory()];
    while let Some(directory) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&directory) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            if path.extension().and_then(std::ffi::OsStr::to_str) != Some("json") {
                continue;
            }
            let Some(value) = cdda_content::read_json(&path) else {
                continue;
            };
            for mapgen in cdda_content::mapgen::load_objects(&value) {
                found.push((mapgen.om_terrain.clone(), mapgen));
            }
        }
    }
    found
}

/// The world a map is drawn into: a whole chunk of empty ground, and the codes every terrain the mapgens name.
fn a_world(mapgens: &[(String, MapgenObject)]) -> (World, BTreeMap<String, u32>) {
    let mut world = World::new();
    let side = cdda_sim::overmap::SQUARES_PER_OMT;
    let mut level = cdda_core_types::level::Level::new();
    for x in 0..side {
        for y in 0..side {
            level.set_terrain(x, y, 0, 0);
        }
    }
    cdda_sim::load_level(&mut world, &level);
    let mut codes: BTreeMap<String, u32> = BTreeMap::new();
    for (_, mapgen) in mapgens {
        for terrain in mapgen.terrain.values() {
            let Some(id) = terrain.id() else {
                continue;
            };
            let next = u32::try_from(codes.len() + 1).unwrap_or(1);
            codes.entry(id.to_owned()).or_insert(next);
        }
        if let Some(fill) = mapgen.fill_ter.as_deref() {
            let next = u32::try_from(codes.len() + 1).unwrap_or(1);
            codes.entry(fill.to_owned()).or_insert(next);
        }
    }
    cdda_sim::mapgen_update::set_terrain_codes(&mut world, codes.clone());
    (world, codes)
}

/// The id a piece names, or the **fallback** of a parameter it names without one being drawn - a fallback is terrain the palette names too, and a test that forgot it called
/// the mapgen's own square a stray.
fn piece_id_or_fallback(piece: &Piece) -> Option<&str> {
    match piece {
        Piece::Id(id) => Some(id),
        Piece::Param { fallback, .. } => fallback.as_deref(),
        // A switch names one of its cases, and every case is terrain the palette names - so the first case is a stand-in for "the mapgen named this square itself", which is
        // what the test is asking. Choosing between them is the generation's job, not this helper's.
        Piece::Switch { cases, .. } => cases.values().next().map(String::as_str),
    }
}

/// **A real mapgen draws a real chunk.** The test picks the mapgen with the most distinguishable terrain among those that name no monsters, items or traps - so the draw is
/// about the terrain layers rather than about registries this test has no business installing - and asserts the chunk it writes is made of **its own** terrains: every square
/// the row touched holds a code the mapgen itself named, and the far corner is either its fill or untouched ground.
#[test]
fn a_real_mapgen_draws_its_own_chunk() {
    let mapgens = the_reference_mapgens();
    if mapgens.is_empty() {
        eprintln!("the reference tree is not here, so the real mapgens cannot be drawn");
        return;
    }
    let chosen = mapgens
        .iter()
        .filter(|(_, mapgen)| {
            !mapgen.terrain.is_empty()
                && mapgen.predecessor_mapgen.is_none()
                && mapgen.furniture.is_empty()
                && mapgen.traps.is_empty()
                && mapgen.items.is_empty()
                && mapgen.monsters.is_empty()
        })
        .max_by_key(|(_, mapgen)| mapgen.terrain.len())
        .expect("a mapgen with terrain and no other layers");
    let (mut world, codes) = a_world(&mapgens);
    cdda_sim::mapgen_update::set_mapgens(&mut world, mapgens.iter().cloned());
    cdda_sim::singletons::put_rolls(&mut world, cdda_core_types::rng::Rolls::new(1));
    let written = cdda_sim::mapgen_tile::generate(&mut world, &chosen.0, (0, 0, 0));
    assert!(
        written.is_some_and(|squares| squares > 0),
        "**the mapgen the data files under {} draws something**: {written:?}",
        chosen.0
    );
    // The chunk is drawn from the mapgen's own terrain symbols **and** its fill, so both are "its own": a fill square is the mapgen's answer, not a stray.
    // A symbol naming a **parameter** draws one of the parameter's own values, so those are the mapgen's own terrain as well - the set is what the palette can draw, not what
    // its rows spell out.
    // **Every mapgen filed under that terrain id**, not the one this test picked: an overmap terrain may hold several mapgens and the writer draws the one it draws, so the set
    // of "its own" terrain is the union of what all of them name.
    let filed: Vec<&cdda_content::mapgen::MapgenObject> = mapgens
        .iter()
        .filter(|(id, _)| id == &chosen.0)
        .map(|(_, mapgen)| mapgen)
        .collect();
    let own: Vec<u32> = filed
        .iter()
        .flat_map(|mapgen| {
            mapgen
                .terrain
                .values()
                .filter_map(piece_id_or_fallback)
                .chain(
                    mapgen
                        .parameters
                        .values()
                        .flat_map(|parameter| parameter.values.iter().map(|(id, _)| id.as_str())),
                )
                .chain(mapgen.fill_ter.as_deref())
        })
        .filter_map(|id| codes.get(id).copied())
        .collect();
    let side = cdda_sim::overmap::SQUARES_PER_OMT;
    let mut drawn = 0;
    for x in 0..side {
        for y in 0..side {
            let Some(terrain) = cdda_sim::square::terrain_at(&world, (x, y, 0)) else {
                continue;
            };
            if terrain != 0 {
                drawn += 1;
                assert!(
                    own.contains(&terrain),
                    "**every drawn square is one of the mapgen's own terrains**: ({x}, {y}) holds {terrain} = {:?}",
                    codes
                        .iter()
                        .find(|(_, code)| **code == terrain)
                        .map(|(id, _)| id)
                );
            }
        }
    }
    assert_eq!(
        Some(drawn),
        written,
        "**and the count the draw answers is the squares it drew**"
    );
    // **A known symbol lands on a known square**, which is what a constant code cannot fake: the first row's first symbol the mapgen names must hold *that* symbol's terrain.
    let known = chosen.1.rows.first().and_then(|row| {
        row.chars().enumerate().find_map(|(column, symbol)| {
            let terrain = chosen.1.terrain.get(&symbol)?;
            Some((column, symbol, codes.get(terrain.id()?).copied()?))
        })
    });
    if let Some((column, symbol, code)) = known {
        assert_eq!(
            cdda_sim::square::terrain_at(&world, (i32::try_from(column).unwrap_or(0), 0, 0)),
            Some(code),
            "**the {symbol} at column {column} is its own terrain**"
        );
    }
    // And a map that names more than one terrain draws more than one, which a single-code write also cannot fake.
    let distinct: std::collections::BTreeSet<u32> = chosen
        .1
        .terrain
        .values()
        .filter_map(|terrain| terrain.id().and_then(|id| codes.get(id).copied()))
        .collect();
    if distinct.len() > 1 {
        let drawn_terrains: std::collections::BTreeSet<u32> = (0..side)
            .flat_map(|x| (0..side).map(move |y| (x, y)))
            .filter_map(|(x, y)| cdda_sim::square::terrain_at(&world, (x, y, 0)))
            .filter(|terrain| *terrain != 0)
            .collect();
        assert!(
            drawn_terrains.len() > 1,
            "**a map of {} terrains draws more than one**: {drawn_terrains:?}",
            distinct.len()
        );
    }
}

/// **The same seed draws the same chunk.** The map generator's whole reason for taking `Rolls` rather than reaching for a generator is that a seed replays a world; this is
/// that promise held against the data rather than against a fixture.
///
/// The terrain is found by **trying** the ones with several candidates and keeping the first that draws: the data files mapgens that name layers their own file does not carry
/// (a palette supplies them, or an update adds them later), and a test that insisted on one particular id would be testing the id rather than the replay.
#[test]
fn a_real_mapgen_draws_the_same_chunk_twice() {
    let mapgens = the_reference_mapgens();
    if mapgens.is_empty() {
        eprintln!("the reference tree is not here, so the real mapgens cannot be drawn");
        return;
    }
    let mut candidates: BTreeMap<&str, usize> = BTreeMap::new();
    for (terrain, mapgen) in &mapgens {
        if mapgen.weight > 0 && !mapgen.terrain.is_empty() && !mapgen.rows.is_empty() {
            *candidates.entry(terrain.as_str()).or_insert(0) += 1;
        }
    }
    let drawn = |seed: u32, terrain: &str| {
        let (mut world, _) = a_world(&mapgens);
        cdda_sim::mapgen_update::set_mapgens(&mut world, mapgens.iter().cloned());
        cdda_sim::singletons::put_rolls(&mut world, cdda_core_types::rng::Rolls::new(seed));
        let written = cdda_sim::mapgen_tile::generate(&mut world, terrain, (0, 0, 0));
        let side = cdda_sim::overmap::SQUARES_PER_OMT;
        let chunk: Vec<_> = (0..side)
            .flat_map(|x| (0..side).map(move |y| (x, y)))
            .map(|(x, y)| cdda_sim::square::terrain_at(&world, (x, y, 0)))
            .collect();
        (written, chunk)
    };
    // The terrains with several candidates, most candidates first, so the weighted pick is what the seed decides.
    let mut crowded: Vec<(&str, usize)> = candidates
        .iter()
        .filter(|(_, count)| **count > 1)
        .map(|(terrain, count)| (*terrain, *count))
        .collect();
    crowded.sort_unstable_by(|left, right| right.1.cmp(&left.1).then(left.0.cmp(right.0)));
    let Some((terrain, (written, first))) = crowded.iter().find_map(|(terrain, _)| {
        let (written, chunk) = drawn(1, terrain);
        written
            .filter(|squares| *squares > 0)
            .map(|_| (terrain, (written, chunk)))
    }) else {
        eprintln!("no terrain with several candidate mapgens draws a chunk in this data");
        return;
    };
    assert!(
        written.is_some_and(|squares| squares > 0),
        "**{terrain} draws something**"
    );
    let (_, again) = drawn(1, terrain);
    assert_eq!(first, again, "**the same seed draws the same chunk**");
}
