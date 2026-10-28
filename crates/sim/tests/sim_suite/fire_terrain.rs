//! Fire eating the ground it stands on.
//!
//! Transcribed from `field_processor_fd_fire` (`src/map_field.cpp:1051-1090`), including the order
//! of the dice: the `FLAMMABLE_HARD` check, then the wrecking check, then the ash.

#![allow(clippy::panic, clippy::expect_used)]

use cdda_core_types::rng::Rolls;
use cdda_sim::{TerrainBurn, TerrainFuel, burn_terrain};

/// The same rule written from the C++, for comparison.
fn reference(
    fuel: TerrainFuel,
    intensity: i32,
    windpower: i32,
    rolls: &mut Rolls,
) -> Option<TerrainBurn> {
    let (time_added, ashy) = if fuel.flammable {
        (i64::from(5 - intensity), false)
    } else if fuel.flammable_hard && rolls.one_in(3) {
        (i64::from(4 - intensity), false)
    } else if fuel.flammable_ash {
        (i64::from(5 - intensity), true)
    } else {
        return None;
    };
    let smoke = 2 + windpower / 5;
    let mut bashes = false;
    let mut ash_charges = None;
    if intensity > 1 && rolls.one_in(200 - intensity * 50) {
        bashes = true;
        if ashy {
            ash_charges = Some(rolls.rng(10, 1000));
        }
    }
    Some(TerrainBurn {
        time_added,
        smoke,
        bashes,
        ash_charges,
    })
}

fn combos() -> Vec<TerrainFuel> {
    let mut all = Vec::new();
    for mask in 0..8_u32 {
        all.push(TerrainFuel {
            flammable: mask & 1 != 0,
            flammable_hard: mask & 2 != 0,
            flammable_ash: mask & 4 != 0,
        });
    }
    all
}

#[test]
fn the_port_and_the_reference_agree_on_every_ground_type() {
    for intensity in 1..=3 {
        for windpower in [0, 5, 17] {
            for fuel in combos() {
                for attempt in 1..=12 {
                    let seed = crate::spread_seed(u32::try_from(attempt).unwrap_or(u32::MAX));
                    let mut a = Rolls::new(seed);
                    let expected = reference(fuel, intensity, windpower, &mut a);
                    let mut b = Rolls::new(seed);
                    let actual = burn_terrain(fuel, intensity, windpower, &mut b);
                    assert_eq!(
                        actual, expected,
                        "i {intensity}, wind {windpower}, {fuel:?}, seed {seed}"
                    );
                }
            }
        }
    }
}

#[test]
fn a_fire_on_plain_ground_gets_five_minus_its_intensity() {
    let fuel = TerrainFuel {
        flammable: true,
        ..TerrainFuel::default()
    };
    let mut rolls = Rolls::new(1);
    let burn = burn_terrain(fuel, 2, 0, &mut rolls).expect("plain ground burns");
    assert_eq!(burn.time_added, 3, "5 - 2");
    assert_eq!(burn.smoke, 2, "two smoke, no wind");
}

#[test]
fn hard_ground_burns_only_one_time_in_three_and_gives_less() {
    let fuel = TerrainFuel {
        flammable_hard: true,
        ..TerrainFuel::default()
    };
    let mut burned = 0;
    let mut refused = 0;
    for attempt in 1..=60 {
        let seed = crate::spread_seed(u32::try_from(attempt).unwrap_or(u32::MAX));
        let mut rolls = Rolls::new(seed);
        match burn_terrain(fuel, 1, 0, &mut rolls) {
            Some(burn) => {
                burned += 1;
                assert_eq!(burn.time_added, 3, "4 - 1");
            }
            None => refused += 1,
        }
    }
    assert!(
        burned > 0 && refused > 0,
        "{burned} burned, {refused} refused"
    );
}

#[test]
fn nothing_that_does_not_burn_gives_anything_and_draws_nothing() {
    let mut rolls = Rolls::new(1);
    let mut control = Rolls::new(1);
    assert_eq!(burn_terrain(TerrainFuel::default(), 3, 0, &mut rolls), None);
    assert_eq!(
        rolls.rng(1, 1000),
        control.rng(1, 1000),
        "the stream did not move"
    );
}

#[test]
fn ashy_ground_leaves_ash_only_when_it_is_wrecked() {
    let fuel = TerrainFuel {
        flammable_ash: true,
        ..TerrainFuel::default()
    };
    let mut ash_seen = false;
    let mut wrecked = false;
    for attempt in 1..=600 {
        let seed = crate::spread_seed(u32::try_from(attempt).unwrap_or(u32::MAX));
        let mut rolls = Rolls::new(seed);
        if let Some(burn) = burn_terrain(fuel, 2, 0, &mut rolls)
            && burn.bashes
        {
            wrecked = true;
            let charges = burn.ash_charges.expect("wrecked ashy ground leaves ash");
            assert!((10..=1000).contains(&charges), "seed {seed}: {charges}");
            ash_seen = true;
        }
    }
    assert!(
        wrecked && ash_seen,
        "some seed in 600 must wreck the ground"
    );
}

#[test]
fn wind_adds_smoke_one_point_per_five() {
    let fuel = TerrainFuel {
        flammable: true,
        ..TerrainFuel::default()
    };
    for (windpower, expected) in [(0, 2), (4, 2), (5, 3), (17, 5)] {
        let mut rolls = Rolls::new(1);
        let burn = burn_terrain(fuel, 1, windpower, &mut rolls).expect("burns");
        assert_eq!(burn.smoke, expected, "wind {windpower}");
    }
}
