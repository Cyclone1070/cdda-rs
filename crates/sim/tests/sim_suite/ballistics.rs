//! Where a shot actually lands: the aim's error, and the square a wild miss walks to.
//!
//! Reference: `projectile_attack_roll` (`ballistics.cpp:204-231`) and the miss branch of `projectile_attack` (`ballistics.cpp:314-352`). The arithmetic is an isosceles
//! triangle - `iso_tangent` (`line.cpp:21-24`) - and the geometry caps a wild miss at thirty degrees and walks the range out by the square root of how far it missed.

use cdda_core_types::rng::Rolls;

#[test]
fn a_hit_that_misses_by_less_than_a_tile_lands_where_it_was_aimed() {
    use cdda_sim::ballistics::aim_of;

    // Sixty arcminutes is one degree: tan( half a degree ) * 10 squares * 2.
    let close = aim_of(60.0, 10.0, 1.0);
    assert!(
        (close.missed_by_tiles - 0.174_537).abs() < 1e-6,
        "a one-degree error over ten squares: {}",
        close.missed_by_tiles
    );
    assert!(
        (close.missed_by - 0.174_537).abs() < 1e-6,
        "which against a one-square target is the same number: {}",
        close.missed_by
    );

    // Six hundred arcminutes is ten degrees, and that is further than a one-square target is wide - so the miss caps at a whole miss.
    let wide = aim_of(600.0, 10.0, 1.0);
    assert!(
        (wide.missed_by_tiles - 1.749_77).abs() < 1e-4,
        "ten degrees over ten squares: {}",
        wide.missed_by_tiles
    );
    assert!(
        (wide.missed_by - 1.0).abs() < f64::EPSILON,
        "capped at a full miss: {}",
        wide.missed_by
    );

    // A target with no size is always a full miss, which is how the reference avoids dividing by zero.
    assert!(
        (aim_of(1.0, 5.0, 0.0).missed_by - 1.0).abs() < f64::EPSILON,
        "a point target cannot be hit"
    );
}

#[test]
fn a_shot_inside_the_target_square_never_moves_and_never_rolls() {
    use cdda_sim::ballistics::{Shot, landing};

    let shot = Shot {
        source: (0, 0, 0),
        target: (10, 0, 0),
        range: 10,
        aim: cdda_sim::ballistics::aim_of(30.0, 10.0, 2.0),
        no_overshoot: true,
    };
    let mut spent = Rolls::new(4_242);
    let where_it_landed = landing(&shot, &mut spent);
    assert_eq!(
        where_it_landed,
        (10, 0, 0),
        "a near miss still hits the square it aimed at"
    );

    // And no die was spent on it: a second stream at the same seed answers the same as the one just used.
    let mut untouched = Rolls::new(4_242);
    assert_eq!(
        spent.rng(0, 1_000),
        untouched.rng(0, 1_000),
        "a shot inside the target spends no dice"
    );
}

#[test]
fn a_wild_miss_walks_out_by_the_root_of_how_far_it_missed() {
    use cdda_sim::ballistics::{Shot, aim_of, landing};

    // Four tiles of miss at ten squares: the offset is the square root of four, so the shot lands between eight and twelve squares out.
    let aim = cdda_sim::ballistics::Aim {
        dispersion: 600.0,
        missed_by_tiles: 4.0,
        missed_by: 1.0,
    };
    let shot = Shot {
        source: (0, 0, 0),
        target: (10, 0, 0),
        range: 10,
        aim,
        no_overshoot: true,
    };
    let mut dice = Rolls::new(99);
    for _ in 0..64 {
        let landed = landing(&shot, &mut dice);
        // Each axis is rounded on its own - `roll_remainder` per coordinate, as the reference writes it - so the walk is a diamond with square corners rather than a circle.
        // The range drawn is between eight and twelve, and the turn is at most ten degrees here, so the axes stay inside those corners.
        assert!(
            i32::abs(landed.0) <= 12 && i32::abs(landed.1) <= 3,
            "a wild miss stays inside twelve along the aim and three across it, never at {landed:?}"
        );
        assert_ne!(landed, (0, 0, 0), "and never on the thrower");
    }

    // A shot aimed at its own square cannot happen, and the reference refuses it outright.
    let at_self = Shot { target: (0, 0, 0), ..shot };
    assert_eq!(
        landing(&at_self, &mut dice),
        (0, 0, 0),
        "a shot at your own square stays where it started"
    );
    let _ = aim_of(0.0, 0.0, 1.0);
}
