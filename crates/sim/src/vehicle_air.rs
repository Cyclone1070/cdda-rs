#![allow(clippy::indexing_slicing)]

//! What a vehicle's body resists the air with: the column scan behind `coeff_air_drag`.
//!
//! Reference: `vehicle::coeff_air_drag` (`vehicle.cpp:4515-4660`) and the `inside` flag it reads (`vehicle::refresh_insides`, `vehicle.cpp:7525-7575`).
//!
//! The rule looks at the vehicle one **column** at a time - a column is a row of the mount grid across the vehicle's width - and asks what is the furthest
//! forward instance of each thing in that column: a protrusion, a half board, a windshield, a roof, a ramp. Then it prices each column against an ideal shape
//! ("the least drag vehicle has halfboard, windshield, seat with roof, windshield, halfboard and is twice as long as it is wide") and prorates by width.

use bevy_ecs::prelude::*;

use crate::vehicles::{Vehicle, part_spec};

// The scan is indexed by mount column, and every index it uses is bounded by the vehicle's own parts: the column comes from `y - min_y`, which is in
// `0..width` by construction, and the inside flags are one per part in part order.

/// The empty column's sentinels and the marks the reference uses for "nothing here yet".
///
/// Reference: `minrow`/`maxrow` (`vehicle.cpp:4495-4496`). A `d_check_max` that fails stores `minrow`, and a `d_check_min` that fails stores `maxrow`, so a
/// column's field is only ever the column's own furthest instance once something in it tested true.
const MINROW: i32 = -122;
const MAXROW: i32 = 122;

/// The air drag constants, as the reference writes them.
const C_AIR_BASE: f64 = 0.25;
const C_AIR_MOD: f64 = 0.1;
const BASE_HEIGHT: f64 = 1.4;
const AISLE_HEIGHT: f64 = 0.6;
const FULLBOARD_HEIGHT: f64 = 0.5;
const ROOF_HEIGHT: f64 = 0.1;
const WINDMILL_HEIGHT: f64 = 0.7;
const SAIL_HEIGHT: f64 = 0.8;
const ROTOR_HEIGHT: f64 = 0.6;

/// What the air weighs, in kilograms per cubic metre.
const AIR_DENSITY: f64 = 1.29;

/// The lowest coefficient the reference will report.
///
/// Reference: `std::max( 0.1, ... )` (`vehicle.cpp:4655`) - a floor, not a fudge: some drag is always there.
const FLOOR: f64 = 0.1;

/// One column of the mount grid, as the reference tallies it.
///
/// Reference: `drag_column` (`vehicle.cpp:4497-4513`), whose defaults are load-bearing: every "most forward" measure starts at `minrow` (nothing) and the two
/// "least forward" measures start at `maxrow`.
#[derive(Debug, Clone, Copy)]
struct DragColumn {
    pro: i32,
    hboard: i32,
    fboard: i32,
    aisle: i32,
    seat: i32,
    exposed: i32,
    roof: i32,
    shield: i32,
    turret: i32,
    panel: i32,
    windmill: i32,
    sail: i32,
    rotor: i32,
    last: i32,
    lastpart: i32,
}

impl Default for DragColumn {
    fn default() -> Self {
        Self {
            pro: MINROW,
            hboard: MINROW,
            fboard: MINROW,
            aisle: MINROW,
            seat: MINROW,
            exposed: MINROW,
            roof: MINROW,
            shield: MINROW,
            turret: MINROW,
            panel: MINROW,
            windmill: MINROW,
            sail: MINROW,
            rotor: MINROW,
            last: MAXROW,
            lastpart: MAXROW,
        }
    }
}

/// How wide one tile of a vehicle is, in metres.
///
/// Reference: `tile_to_width` (`vehicle.cpp:4484-4493`): `0.5 + 0.4 * tiles` up to five tiles, then `2.5 + 0.15 * ( tiles - 5 )`, and 0.1 for a vehicle with no
/// width at all.
fn tile_to_width(tiles: i32) -> f64 {
    if tiles < 1 {
        0.1
    } else if tiles < 6 {
        0.5 + 0.4 * f64::from(tiles)
    } else {
        2.5 + 0.15 * f64::from(tiles - 5)
    }
}

/// What a vehicle's body resists the air with.
///
/// Reference: `vehicle::coeff_air_drag` (`vehicle.cpp:4515-4660`). `None` for a vehicle that is not in the store at all; every real vehicle gets a number, and a
/// vehicle with no parts answers the floor.
#[must_use]
pub fn coeff_air_drag(world: &World, vehicle: Entity) -> Option<f64> {
    let built = world.get::<Vehicle>(vehicle)?;
    if built.parts.is_empty() {
        return Some(FLOOR);
    }
    let (min_x, min_y, max_x, max_y) = bounds(built)?;
    let width = max_y - min_y + 1;
    let length = max_x - min_x + 1;
    let columns = scan_columns(world, vehicle, built, (min_x, min_y), width);

    let mut height = 0.0;
    let mut drag = 0.0;
    for column in &columns {
        let (c_drag, c_height) = tally(column, length, width);
        drag += c_drag;
        height += c_height;
    }
    let columns_count = f64::from(width);
    height /= columns_count;
    drag /= columns_count;
    let cross_area = height * tile_to_width(width);

    // F_air = c_air * cross_area * 1/2 * air_density * v^2.
    Some((drag * cross_area * 0.5 * AIR_DENSITY).max(FLOOR))
}

/// The mount grid the scan walks: the lowest and highest squares any part stands on.
///
/// Reference: `mount_min`/`mount_max`, grown over every part that is not removed (`vehicle.cpp:6650-6680`).
fn bounds(built: &Vehicle) -> Option<(i32, i32, i32, i32)> {
    let min_x = built.parts.iter().map(|part| part.x).min()?;
    let min_y = built.parts.iter().map(|part| part.y).min()?;
    let max_x = built.parts.iter().map(|part| part.x).max()?;
    let max_y = built.parts.iter().map(|part| part.y).max()?;
    Some((min_x, min_y, max_x, max_y))
}

/// One pass over the structure parts, filling a column per row of the mount grid.
///
/// Reference: the loop over `all_parts_at_location( part_location_structure )` (`vehicle.cpp:4560-4594`). Each structure part owns its column and brings the whole
/// stack standing on its mount, because a seat, a roof and a door all matter to the air even though none of them is a structure part itself.
fn scan_columns(
    world: &World,
    vehicle: Entity,
    built: &Vehicle,
    corner: (i32, i32),
    width: i32,
) -> Vec<DragColumn> {
    let (min_x, min_y) = corner;
    let mut columns = vec![DragColumn::default(); usize::try_from(width).unwrap_or(0)];
    for index in 0..built.parts.len() {
        let Some(part) = built.parts.get(index) else {
            continue;
        };
        let Some(spec) = part_spec(world, vehicle, index) else {
            continue;
        };
        if spec.location != "structure" {
            continue;
        }
        let Some(column) = columns.get_mut(usize::try_from(part.y - min_y).unwrap_or(0)) else {
            continue;
        };
        let stack = crate::vehicles::parts_at(world, vehicle, part.x, part.y);
        // `d_protrusion`: a protrusion only counts when it is alone on its mount, and this is a "least forward" test, so a column keeps its earliest instance.
        let alone = stack
            .first()
            .and_then(|first| part_spec(world, vehicle, *first))
            .is_some_and(|spec| spec.flags.contains("PROTRUSION"));
        let protrusion = stack.len() <= 1 && alone;
        column.pro = column
            .pro
            .min(if protrusion { part.x - min_x } else { MAXROW });
        for at in stack {
            let Some(other) = built.parts.get(at) else {
                continue;
            };
            let Some(other_spec) = part_spec(world, vehicle, at) else {
                continue;
            };
            let available = !other.broken;
            let flags = &other_spec.flags;
            let here = other.x - min_x;
            let exposed = other_spec.location == "center"
                && !(other.inside
                    || flags.contains("NO_ROOF_NEEDED")
                    || flags.contains("WINDSHIELD")
                    || flags.contains("OPENABLE"));
            let reach = |test: bool| if test { here } else { MINROW };
            column.hboard = column.hboard.max(reach(flags.contains("HALF_BOARD")));
            column.fboard = column.fboard.max(reach(flags.contains("FULL_BOARD")));
            column.aisle = column.aisle.max(reach(flags.contains("AISLE")));
            column.shield = column
                .shield
                .max(reach(flags.contains("WINDSHIELD") && available));
            column.seat = column
                .seat
                .max(reach(flags.contains("SEAT") || flags.contains("BED")));
            column.turret = column.turret.max(reach(
                other_spec.location == "on_roof" && !flags.contains("SOLAR_PANEL"),
            ));
            column.roof = column.roof.max(reach(flags.contains("ROOF")));
            column.panel = column.panel.max(reach(flags.contains("SOLAR_PANEL")));
            column.windmill = column.windmill.max(reach(flags.contains("WIND_TURBINE")));
            column.rotor = column.rotor.max(reach(flags.contains("ROTOR")));
            column.sail = column.sail.max(reach(flags.contains("WIND_POWERED")));
            column.exposed = column.exposed.max(reach(exposed));
            column.last = column.last.min(
                if flags.contains("LOW_FINAL_AIR_DRAG") || flags.contains("HALF_BOARD") {
                    here
                } else {
                    MAXROW
                },
            );
            column.lastpart = column.lastpart.min(here);
        }
    }
    columns
}

/// What one column costs in drag, and how tall it stands.
///
/// Reference: the tally loop (`vehicle.cpp:4596-4644`), where each "worse than the ideal shape" adds a tenth and the two good shapes take one away.
fn tally(column: &DragColumn, length: i32, width: i32) -> (f64, f64) {
    let mut c_air = C_AIR_BASE;
    c_air += if column.pro > column.hboard {
        C_AIR_MOD
    } else {
        0.0
    };
    c_air += if column.hboard.max(column.fboard).max(column.shield) > column.hboard {
        2.0 * C_AIR_MOD
    } else {
        0.0
    };
    c_air += if column.shield < column.seat {
        3.0 * C_AIR_MOD
    } else {
        0.0
    };
    c_air += if column.exposed > MINROW {
        3.0 * C_AIR_MOD
    } else {
        0.0
    };
    c_air -= if length >= 2 * width { C_AIR_MOD } else { 0.0 };
    c_air -= if column.last == column.lastpart {
        C_AIR_MOD
    } else {
        0.0
    };
    c_air += if column.turret > MINROW {
        3.0 * C_AIR_MOD
    } else {
        0.0
    };
    c_air += if column.windmill > MINROW {
        5.0 * C_AIR_MOD
    } else {
        0.0
    };
    c_air += if column.rotor > MINROW {
        6.0 * C_AIR_MOD
    } else {
        0.0
    };
    c_air += if column.sail > MINROW {
        7.0 * C_AIR_MOD
    } else {
        0.0
    };

    let mut height = BASE_HEIGHT;
    height += if column.roof > MINROW {
        ROOF_HEIGHT
    } else {
        0.0
    };
    height += if column.aisle > MINROW {
        AISLE_HEIGHT
    } else {
        0.0
    };
    height += if column.fboard > MINROW {
        FULLBOARD_HEIGHT
    } else {
        0.0
    };
    height += if column.turret > MINROW {
        2.0 * ROOF_HEIGHT
    } else {
        0.0
    };
    height += if column.panel > MINROW {
        ROOF_HEIGHT
    } else {
        0.0
    };
    height += if column.windmill > MINROW {
        WINDMILL_HEIGHT
    } else {
        0.0
    };
    height += if column.rotor > MINROW {
        ROTOR_HEIGHT
    } else {
        0.0
    };
    height += if column.sail > MINROW {
        SAIL_HEIGHT
    } else {
        0.0
    };
    (c_air, height)
}
