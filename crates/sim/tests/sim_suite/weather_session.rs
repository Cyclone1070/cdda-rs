//! A session whose sky **changes while it runs**.
//!
//! The M2 exit gate asks for a per-turn hash of an environment session that covers a weather transition, and until this round the session dump recorded only the wind.
//! A dump with the wind and no weather cannot tell a storm from a calm night at the same air speed: the manager's id decides the light multiplier, the sight penalty and
//! the sound attenuation, so the sky is part of the state a session has to carry.
//!
//! Reference: `weather_manager::update_weather` (`weather.cpp:901-947`), driven by `--turn-minutes` so the calendar moves inside the session - the only way a change of
//! sky can happen between two recorded turns. Measured on the fixture rather than assumed: the session has to contain at least two weather ids, and the clock has to
//! advance between them.

#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::struct_field_names,
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss
)]

use cdda_sim::weather_gen::{CommonData, Generator, humidity, pressure, temperature_k};
use std::path::Path;

/// One recorded turn: the clock, the sky, and the wind the fields felt.
struct Turn {
    /// The turn number the session labels itself with.
    turn: i64,
    /// The game time at the moment the turn was written.
    now: i64,
    /// The weather id the manager reported.
    weather: String,
    /// The air temperature, in kelvin.
    temperature: f64,
    /// The turn the manager said the weather was next due to change.
    next: i64,
    /// The wind's speed, as the manager held it.
    speed: i64,
    /// The wind's bearing, in degrees.
    direction: i64,
    /// The generator's inputs, which are what make the sky replayable rather than merely readable.
    common: CommonData,
    /// How far into the day the turn fell.
    day_fraction: f64,
    /// The point the reference's generator produced, in kelvin, percent and millibars.
    point: (f64, f64, f64),
}

fn number(object: &cdda_json::Object, key: &str) -> f64 {
    match object.get(key) {
        Some(cdda_json::Value::Float(value)) => *value,
        Some(cdda_json::Value::Int(value)) => *value as f64,
        other => panic!("{key} is a number: {other:?}"),
    }
}

fn turns() -> Vec<Turn> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/fields/weather-session.json");
    let text = std::fs::read_to_string(&path).expect("the recorded session");
    let cdda_json::Value::Object(root) = cdda_json::parse(text.as_bytes()).expect("valid JSON") else {
        panic!("root")
    };
    let Some(cdda_json::Value::Array(list)) = root.get("turns") else {
        panic!("turns")
    };
    list.iter()
        .map(|turn| {
            let cdda_json::Value::Object(turn) = turn else {
                panic!("a turn")
            };
            Turn {
                turn: number(turn, "turn") as i64,
                now: number(turn, "now") as i64,
                weather: match turn.get("weather") {
                    Some(cdda_json::Value::Str(id)) => id.clone(),
                    other => panic!("every turn carries the weather: {other:?}"),
                },
                temperature: number(turn, "temperature"),
                next: number(turn, "nextweather") as i64,
                speed: number(turn, "windspeed") as i64,
                direction: number(turn, "winddirection") as i64,
                common: CommonData {
                    x: number(turn, "common_x"),
                    y: number(turn, "common_y"),
                    z: number(turn, "common_z"),
                    cyf: cdda_sim::weather_gen::year_cosine(number(turn, "year_fraction")),
                    mod_seed: number(turn, "mod_seed") as u32,
                    season: number(turn, "season") as usize,
                },
                day_fraction: number(turn, "day_fraction"),
                point: (
                    number(turn, "point_temperature"),
                    number(turn, "humidity"),
                    number(turn, "pressure"),
                ),
            }
        })
        .collect()
}

#[test]
fn the_recorded_session_contains_a_change_of_sky() {
    // The fixture's reason to exist. A re-record that quietly lost the transition would leave every later gate passing on a session that never changed weather, which is
    // the one thing this fixture is for.
    let turns = turns();
    let mut ids: Vec<&str> = Vec::new();
    for turn in &turns {
        if ids.last() != Some(&turn.weather.as_str()) {
            ids.push(&turn.weather);
        }
    }
    assert!(
        ids.len() >= 2,
        "the session has to cross a weather transition, and it reports only {ids:?}"
    );
    assert_eq!(
        turns.first().expect("a first turn").turn,
        0,
        "the session starts at turn zero"
    );
}

#[test]
fn every_turn_carries_the_sky_and_a_moved_clock() {
    // An hour apiece: `--turn-minutes 60`, so the calendar has to advance by exactly 3600 turns between two records. A dump with a frozen clock cannot contain a
    // transition, and one that advanced by a different amount would make the generator's answers unreplayable.
    let turns = turns();
    let mut clocked = 0;
    let mut wrong: Vec<String> = Vec::new();
    for pair in turns.windows(2) {
        let (before, after) = (&pair[0], &pair[1]);
        let step = after.now - before.now;
        if step != 3600 && wrong.len() < 4 {
            wrong.push(format!("turn {} advanced by {step} instead of 3600", after.turn));
        }
        if after.next < after.now && wrong.len() < 4 {
            wrong.push(format!(
                "turn {} claims its next change was at {} but stands at {}",
                after.turn, after.next, after.now
            ));
        }
        clocked += 1;
    }
    assert!(wrong.is_empty(), "the session's clock disagrees: {wrong:?}");
    assert_eq!(clocked, turns.len() - 1, "every pair of turns was compared");
    // The sky is a state, not decoration: the temperature has to move with the day, and a session whose sky never moves cannot be a transition.
    let warmest = turns.iter().map(|turn| turn.temperature).fold(f64::MIN, f64::max);
    let coldest = turns.iter().map(|turn| turn.temperature).fold(f64::MAX, f64::min);
    assert!(
        warmest - coldest > 1.0,
        "the day's temperature moved: {coldest} to {warmest}"
    );
    assert!(
        turns.iter().all(|turn| !turn.weather.is_empty()),
        "no turn is missing its weather"
    );
    // The wind is part of the state too, and a turn without one would mean the dump lost the manager's own numbers.
    assert!(
        turns.iter().all(|turn| turn.speed >= 0),
        "every turn carries the wind's speed"
    );
}

#[test]
fn the_port_recomputes_the_sessions_sky_from_its_own_inputs() {
    // The session carries the generator's **inputs** - where the player stood, the season, the noise field's origin and seed, how far into the day the turn fell - and the
    // point the reference produced from them. Nothing is drawn: the point is a pure function of those four, so a difference here is a formula difference and not a die
    // that landed elsewhere. That is the half of the sky a session can be held to without replaying the manager, and it is the half the light and the sight penalty hang on.
    let generator = Generator::default_region();
    let turns = turns();
    let mut wrong: Vec<(i64, String, f64, f64)> = Vec::new();
    let mut checked = 0;
    for turn in &turns {
        let mut near = |what: &str, ours: f64, theirs: f64| {
            if (ours - theirs).abs() > 1e-6 && wrong.len() < 8 {
                wrong.push((turn.turn, what.to_string(), ours, theirs));
            }
        };
        near(
            "temperature",
            temperature_k(&generator, &turn.common, turn.day_fraction),
            turn.point.0,
        );
        near("humidity", humidity(&generator, &turn.common), turn.point.1);
        near("pressure", pressure(&generator, &turn.common), turn.point.2);
        checked += 1;
    }
    assert_eq!(checked, turns.len(), "every recorded turn was recomputed");
    assert!(checked >= 20, "the session is long enough to be a day: {checked}");
    assert!(
        wrong.is_empty(),
        "the port's sky disagrees with the recording: {wrong:?}"
    );
}

/// The dice the reference's manager rolled for one hour, split into what `rng()` rolled and what came from the engine itself.
#[derive(Clone)]
struct Hour {
    /// The `rng()` draws, in the order they were made.
    dice: Vec<(i32, i32, i32)>,
    /// The wind sectors the engine drew, in order.
    sectors: Vec<usize>,
}

/// The manager's dice per hour, straight from the trace.
///
/// The manager is asked about the hour it has just moved into, so the block labelled `weather.session.hour.<n>` belongs to turn `n` - not to the turn whose
/// sweeps run beside it. The `wind.sector` values are separate because `get_wind_direction` draws from the engine with a `std::discrete_distribution`
/// (`weather_gen.cpp:256-276`), which `rng()` never sees.
fn hours() -> Vec<Hour> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/fields/weather-session.rolls.txt");
    let text = std::fs::read_to_string(&path).expect("the recorded dice");
    let mut hours: Vec<Hour> = vec![Hour { dice: Vec::new(), sectors: Vec::new() }];
    let mut inside = false;
    for line in text.lines() {
        if let Some(label) = line.strip_prefix("mark ") {
            if let Some(number) = label.strip_prefix("weather.session.hour.") {
                let index: usize = number.parse().expect("an hour");
                hours.resize(index + 1, Hour { dice: Vec::new(), sectors: Vec::new() });
                inside = true;
            } else {
                inside = false;
            }
            continue;
        }
        if !inside {
            continue;
        }
        if let Some(rest) = line.strip_prefix("int ") {
            let mut parts = rest.split(' ');
            let low = parts.next().expect("low").parse().expect("a number");
            let high = parts.next().expect("high").parse().expect("a number");
            let drawn = parts.next().expect("drawn").parse().expect("a number");
            hours.last_mut().expect("an hour").dice.push((low, high, drawn));
        } else if let Some(rest) = line.strip_prefix("drawn wind.sector ") {
            let sector: usize = rest.trim().parse().expect("a sector");
            hours.last_mut().expect("an hour").sectors.push(sector);
        }
    }
    hours
}

#[test]
fn the_port_decides_the_sessions_weather_from_the_recorded_dice() {
    // The other half of the sky: not the point, which is a formula, but **which weather** and **when the next change is due**. Both are decided from dice, and the
    // dice are the reference's own - the manager's five draws an hour, plus the sector it took from the engine when the direction changed. The port is seeded with
    // turn zero's sky and then left to run.
    let turns = turns();
    let hours = hours();
    let generator = Generator::default_region();
    let mut system = cdda_sim::weather::System::new(crate::weather_tick::rules());
    let first = &turns[0];
    system.manager.weather = Some(first.weather.clone());
    system.manager.point = Some(cdda_content::weather::Point {
        turn: first.now,
        temperature: first.point.0,
        humidity: first.point.1,
        pressure: first.point.2,
        windpower: 0,
        winddirection: i32::try_from(first.speed).unwrap_or(0),
    });
    system.manager.next_turn = first.next;
    // The direction the session opened with: the engine's static starts at a sentinel, so the reference had already drawn its first sector before the recording
    // began - the same reason hour zero is seeded rather than ticked.
    system.wind.degrees = Some(i32::try_from(first.direction).unwrap_or(0));
    let mut wrong: Vec<(i64, String, String, String)> = Vec::new();
    let mut checked = 0;
    for turn in &turns[1..] {
        let index = usize::try_from(turn.turn).expect("a turn number");
        let Some(hour) = hours.get(index) else { continue };
        let mut supplied = hour.dice.iter();
        let mut roll = |low: i32, high: i32| {
            let Some((want_low, want_high, drawn)) = supplied.next().copied() else {
                panic!("turn {} asks for a die the reference never rolled", turn.turn);
            };
            assert_eq!((low, high), (want_low, want_high), "turn {} asks for the dice the reference asked for", turn.turn);
            drawn
        };
        let mut sectors = hour.sectors.iter().copied();
        let mut sector = |_season: usize| sectors.next().expect("a sector was drawn when the direction changed");
        let _ = system.tick(
            &cdda_sim::weather::Forecast {
                generator: &generator,
                common: turn.common,
                day_fraction: turn.day_fraction,
                turn: turn.now,
            },
            &mut roll,
            &mut sector,
        );
        let mut note = |what: &str, ours: &str, theirs: &str| {
            if ours != theirs && wrong.len() < 8 {
                wrong.push((turn.turn, what.to_string(), ours.to_string(), theirs.to_string()));
            }
        };
        note("weather", system.manager.weather.as_deref().unwrap_or("none"), turn.weather.as_str());
        note("next turn", &system.manager.next_turn.to_string(), &turn.next.to_string());
        checked += 1;
    }
    assert!(checked >= 20, "every recorded hour was replayed: {checked}");
    assert!(wrong.is_empty(), "the port's weather disagrees with the recording: {wrong:?}");
}

#[test]
fn the_weather_session_has_a_receipt() {
    // The third layer of the digest: the sky. Its numbers are floats, so they are quantised to the millionth the gate above compares at - a receipt that moved on a bit
    // of floating-point noise would be a receipt nobody could use. Everything else about the turn is an integer already: the turn, the weather id, the wind, the time to
    // the next change.
    let turns = turns();
    let generator = Generator::default_region();
    let mut receipt = 0xcbf2_9ce4_8422_2325_u64;
    let mut record_receipt = 0xcbf2_9ce4_8422_2325_u64;
    for (index, turn) in turns.iter().enumerate() {
        let quantised = |value: f64| -> i64 { (value * 1_000_000.0).round() as i64 };
        // **The port's own numbers.** The receipt is built from what this port recomputes, and the record's receipt is built beside it: two equal numbers mean the port
        // produced the session, not that the session was read back.
        let ours = (
            temperature_k(&generator, &turn.common, turn.day_fraction),
            humidity(&generator, &turn.common),
            pressure(&generator, &turn.common),
        );
        let mut entries = vec![
            // Layer two, with the four fields the sky's own generator gives, then what the manager said.
            (2, 0, 0, 0, 0, quantised(ours.0)),
            (2, 0, 0, 1, 0, quantised(ours.1)),
            (2, 0, 0, 2, 0, quantised(ours.2)),
            (2, 0, 0, 3, 0, turn.next),
            (2, 0, 0, 4, 0, turn.speed),
            (2, 0, 0, 5, 0, turn.direction),
            (2, 0, 0, 6, 0, turn.now),
            (2, 0, 0, 7, 0, turn.turn),
            // The weather id is a name, so it is folded in as a hash of its bytes: two ids that differ have different receipts without the digest needing to know the
            // registry's numbering.
            (2, 0, 0, 8, 0, name_hash(&turn.weather)),
        ];
        entries.sort_unstable();
        let turn_digest = crate::replay_digest::digest(&entries);
        receipt = crate::replay_digest::fold(receipt, index, turn_digest);
        let recorded = vec![
            (2, 0, 0, 0, 0, quantised(turn.point.0)),
            (2, 0, 0, 1, 0, quantised(turn.point.1)),
            (2, 0, 0, 2, 0, quantised(turn.point.2)),
            (2, 0, 0, 3, 0, turn.next),
            (2, 0, 0, 4, 0, turn.speed),
            (2, 0, 0, 5, 0, turn.direction),
            (2, 0, 0, 6, 0, turn.now),
            (2, 0, 0, 7, 0, turn.turn),
            (2, 0, 0, 8, 0, name_hash(&turn.weather)),
        ];
        record_receipt = crate::replay_digest::fold(record_receipt, index, crate::replay_digest::digest(&recorded));
    }
    assert_eq!(
        receipt, record_receipt,
        "the port's receipt is the recording's: {receipt:#018x} against {record_receipt:#018x}"
    );
    assert_eq!(receipt, 0x4c22_1d74_263f_52f4_u64, "the weather session receipt is {receipt:#018x} over {} turns", turns.len());
}

/// A stable number for a weather id's name.
fn name_hash(name: &str) -> i64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in name.bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x100_0000_01b3);
    }
    i64::try_from(hash & 0x7fff_ffff_ffff).unwrap_or(0)
}
