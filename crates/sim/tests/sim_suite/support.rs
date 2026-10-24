//! The small map `clear_path` is asked about, laid out the way the reference's fixture was.
//!
//! It has to be the same map or the recorded answers mean nothing: a 31-square floor at z=0 with a wall column at `x = 5` from `y = -4` to `y = 4`, and a second level
//! above it that is floor everywhere except the column at `x = 8`, which is open air.

#![allow(clippy::expect_used)]

use cdda_sim::LineOfFire;

/// A cost window's answer for one square.
#[derive(Debug, Clone, Copy)]
enum Square {
    /// `t_floor`: passable at cost two, and it supports whatever stands on it.
    Floor,
    /// `t_concrete_wall`: impassable.
    Wall,
    /// `t_open_air`: passable, but nothing to stand on.
    OpenAir,
}

/// The fixture map, with the same coordinates the harness used (the reference's origin is `(0, 0, 0)` here).
pub struct LineOfFireMap {
    squares: Vec<([i32; 3], Square)>,
}

impl LineOfFire for LineOfFireMap {
    fn in_bounds(&self, square: [i32; 3]) -> bool {
        let [x, y, z] = square;
        (0..=1).contains(&z) && (-20..=20).contains(&x) && (-20..=20).contains(&y)
    }

    fn move_cost(&self, square: [i32; 3]) -> i32 {
        match self.at(square) {
            Square::Wall => 0,
            // `t_floor` and `t_open_air` are both passable at cost two; only the second has nothing to stand on.
            Square::Floor | Square::OpenAir => 2,
        }
    }

    fn has_floor_or_support(&self, square: [i32; 3]) -> bool {
        matches!(self.at(square), Square::Floor)
    }
}

impl LineOfFireMap {
    fn at(&self, square: [i32; 3]) -> Square {
        self.squares
            .iter()
            .find(|(at, _)| *at == square)
            .map_or(Square::OpenAir, |(_, square)| *square)
    }
}

/// The map the `--dump-clear-path` fixture built, square for square.
#[must_use]
pub fn line_of_fire_map() -> LineOfFireMap {
    let mut squares = Vec::new();
    for dx in -15..=15 {
        for dy in -15..=15 {
            // The wall goes in place of the floor rather than on top of it: the map answers with the first square it finds, so a later entry would never be seen.
            let ground = if dx == 5 && (-4..=4).contains(&dy) {
                Square::Wall
            } else {
                Square::Floor
            };
            squares.push(([dx, dy, 0], ground));
            squares.push(([dx, dy, 1], if dx == 8 { Square::OpenAir } else { Square::Floor }));
        }
    }
    LineOfFireMap { squares }
}
