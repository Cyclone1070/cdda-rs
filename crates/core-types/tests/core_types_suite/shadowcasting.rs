//! The caster, checked against grids the reference printed.
//!
//! `corpus/shadowcasting/expected.txt` comes from the reference's own implementation
//! (`tests/shadowcasting_test.cpp:33-90`), extracted with a bounds-checked driver and
//! the eight segments the real caster uses. The file is read rather than transcribed
//! here, so there is exactly one copy of the expected output and no chance of a typo
//! passing as a fixture.

#![allow(clippy::expect_used, clippy::panic, clippy::indexing_slicing)]

use cdda_core_types::shadowcasting::{TransparencyGrid, cast_light};
use std::path::PathBuf;

const RADIUS: i32 = 5;

fn fixture_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../corpus/shadowcasting/expected.txt")
}

/// The committed grids, keyed by scenario name.
fn fixtures() -> Vec<(String, Vec<String>)> {
    let text = std::fs::read_to_string(fixture_path()).expect("the fixture file exists");
    let mut scenarios: Vec<(String, Vec<String>)> = Vec::new();
    for line in text.lines() {
        if let Some(name) = line.strip_prefix("scenario ") {
            scenarios.push((name.trim().to_owned(), Vec::new()));
        } else if !line.is_empty()
            && line.chars().all(|c| c == '#' || c == '.' || c == 'X')
            && let Some(last) = scenarios.last_mut()
        {
            last.1.push(line.to_owned());
        }
    }
    scenarios
}

#[test]
fn every_scenario_matches_the_reference_grid() {
    let scenarios = fixtures();
    assert!(
        scenarios.len() >= 5,
        "the fixture file should hold every scenario"
    );
    let mut checked = 0;
    for (name, expected) in &scenarios {
        let side = expected.len();
        // Solid cells come from the fixture itself: the grid says which cells block,
        // so the test cannot disagree with the file about the map.
        let mut cells = vec![1.0_f32; side * side];
        for (y, row) in expected.iter().enumerate() {
            for (x, cell) in row.chars().enumerate() {
                if cell == 'X' {
                    cells[x * side + y] = 0.0;
                }
            }
        }
        let grid = TransparencyGrid::new(side, cells);
        let middle = i32::try_from(side / 2).expect("a small grid");
        let origin = (middle, middle);
        let visible = cast_light(&grid, origin, RADIUS);
        let rendered: Vec<String> = (0..side)
            .map(|y| {
                (0..side)
                    .map(|x| {
                        let (Ok(xi), Ok(yi)) = (i32::try_from(x), i32::try_from(y)) else {
                            return '?';
                        };
                        if grid.at(xi, yi) == 0.0 {
                            'X'
                        } else if visible[x * side + y] {
                            '#'
                        } else {
                            '.'
                        }
                    })
                    .collect()
            })
            .collect();
        assert_eq!(
            &rendered, expected,
            "{name} differs from the reference grid"
        );
        checked += 1;
    }
    println!("shadowcasting: {checked} scenarios match the reference grids");
}
