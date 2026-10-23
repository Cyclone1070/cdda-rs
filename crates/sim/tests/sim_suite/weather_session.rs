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
