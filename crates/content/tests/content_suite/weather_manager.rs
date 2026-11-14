//! The object that holds the weather between turns.
//!
//! Reference: `weather_manager::update_weather` (`weather.cpp:901-947`). Everything the last four rounds built hangs off it: it decides when to recompute, asks the condition
//! loop for an id, stores the point the generator produced, rolls the next change, and - the one place weather touches the map - tells the transparency caches to forget
//! themselves when the sight penalty changes.
//!
//! The two refactors worth noticing are the ones the reference *does not* make: its manager reads the option system, the UI state and the player's activity inside the update. The
//! port takes the rules as a value and returns what changed, so the caller can act on it.

#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::float_cmp,
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss
)]

use cdda_content::weather::{Change, Manager, Point, WeatherRules};

/// A rules table pinned to the shipped data's numbers where they matter.
struct Rules {
    /// The id the decision answers, in order of the turns it is asked about.
    decisions: std::cell::RefCell<Vec<String>>,
    sight: std::collections::BTreeMap<String, f32>,
    bounds: std::collections::BTreeMap<String, (i32, i32)>,
}

impl Rules {
    fn shipped() -> Rules {
        let mut sight = std::collections::BTreeMap::new();
        sight.insert("clear".to_string(), 1.0);
        sight.insert("sunny".to_string(), 1.0);
        sight.insert("fog".to_string(), 1.7);
        let mut bounds = std::collections::BTreeMap::new();
        for id in ["clear", "sunny", "fog"] {
            bounds.insert(id.to_string(), (300, 300));
        }
        Rules {
            decisions: std::cell::RefCell::new(Vec::new()),
            sight,
            bounds,
        }
    }
}

impl WeatherRules for Rules {
    fn decide(&self, _point: &Point) -> String {
        let mut decisions = self.decisions.borrow_mut();
        if decisions.is_empty() {
            return "clear".to_string();
        }
        decisions.remove(0)
    }
    fn duration_bounds(&self, id: &str) -> (i32, i32) {
        self.bounds.get(id).copied().expect("a known weather")
    }
    fn sight_penalty(&self, id: &str) -> f32 {
        self.sight.get(id).copied().expect("a known weather")
    }
}

fn point(temperature: f64) -> Point {
    Point {
        turn: 0,
        temperature,
        humidity: 50.0,
        pressure: 1015.0,
        windpower: 3,
        winddirection: 202,
    }
}

#[test]
fn nothing_happens_until_the_weather_is_due() {
    // Reference: `if( weather_id == WEATHER_NULL || calendar::turn >= nextweather )`. A manager that recomputed every turn would still produce the right id on a still day, and
    // would roll the duration three hundred times an hour - so the test counts the decisions as well as checking the answer.
    let rules = Rules::shipped();
    rules.decisions.borrow_mut().extend(["sunny".to_string()]);
    let mut manager = Manager::default();
    let mut roll = |_low: i32, _high: i32| 300;
    assert_eq!(
        manager.update(100, point(280.0), &rules, &mut roll),
        Change::Weather {
            sight_changed: true
        }
    );
    assert_eq!(manager.weather.as_deref(), Some("sunny"));
    assert_eq!(manager.next_turn, 400);
    for turn in [101, 200, 399] {
        assert_eq!(
            manager.update(turn, point(281.0), &rules, &mut roll),
            Change::None
        );
    }
    assert_eq!(
        manager.point.expect("a point").temperature,
        280.0,
        "an update that did not happen leaves the point alone"
    );
}

#[test]
fn the_first_update_counts_as_a_change_and_flushes_every_level() {
    // `old_weather` starts as the null id, whose sight penalty is zero, so the first real weather always differs - which is why the caches are dirty at the start of a game
    // without anything having changed in the weather.
    let rules = Rules::shipped();
    rules.decisions.borrow_mut().push("sunny".to_string());
    let mut manager = Manager::default();
    let mut roll = |_low: i32, _high: i32| 300;
    assert_eq!(
        manager.update(0, point(280.0), &rules, &mut roll),
        Change::Weather {
            sight_changed: true
        }
    );
}

#[test]
fn a_new_weather_with_the_same_sight_penalty_does_not_flush_the_caches() {
    // Reference: `if( weather_id->sight_penalty != old_weather->sight_penalty )`. `sunny` and `clear` both absorb 1.0, so the sky can change without the map being redrawn - and a
    // port that flushed on every change would be correct and slow, which is the kind of bug that never shows up in a test that only checks the answer.
    let rules = Rules::shipped();
    rules
        .decisions
        .borrow_mut()
        .extend(["sunny".to_string(), "clear".to_string()]);
    let mut manager = Manager::default();
    let mut roll = |_low: i32, _high: i32| 300;
    assert!(matches!(
        manager.update(0, point(280.0), &rules, &mut roll),
        Change::Weather {
            sight_changed: true
        }
    ));
    assert_eq!(
        manager.update(400, point(280.0), &rules, &mut roll),
        Change::Weather {
            sight_changed: false
        }
    );
    assert_eq!(manager.weather.as_deref(), Some("clear"));
}

#[test]
fn a_new_weather_that_changes_the_sight_penalty_flushes_the_caches() {
    let rules = Rules::shipped();
    rules
        .decisions
        .borrow_mut()
        .extend(["sunny".to_string(), "fog".to_string()]);
    let mut manager = Manager::default();
    let mut roll = |_low: i32, _high: i32| 300;
    assert!(matches!(
        manager.update(0, point(280.0), &rules, &mut roll),
        Change::Weather {
            sight_changed: true
        }
    ));
    assert_eq!(
        manager.update(400, point(280.0), &rules, &mut roll),
        Change::Weather {
            sight_changed: true
        }
    );
}

#[test]
fn the_manager_rolls_the_duration_from_the_type_it_chose() {
    // The bounds come from the weather that was just chosen, not from the one that was there before - the reference reads `weather_id->duration_min` *after* assigning the new id.
    let rules = Rules::shipped();
    rules
        .decisions
        .borrow_mut()
        .extend(["sunny".to_string(), "clear".to_string()]);
    let mut manager = Manager::default();
    let mut asked = Vec::new();
    let mut roll = |low: i32, high: i32| {
        asked.push((low, high));
        300
    };
    manager.update(0, point(280.0), &rules, &mut roll);
    assert_eq!(
        asked,
        [(300, 300)],
        "the first update rolled once, between the chosen type's bounds"
    );
}

#[test]
fn lightning_is_cleared_by_an_update_and_set_by_nothing_here() {
    // Reference: `lightning_active = false;` inside the update, and only the lightning effect sets it back. A port that forgot this line would leave a thunderstorm flashing after
    // the weather had passed.
    let rules = Rules::shipped();
    rules
        .decisions
        .borrow_mut()
        .extend(["sunny".to_string(), "sunny".to_string()]);
    let mut manager = Manager::default();
    let mut roll = |_low: i32, _high: i32| 300;
    manager.update(0, point(280.0), &rules, &mut roll);
    manager.lightning_active = true;
    manager.update(400, point(280.0), &rules, &mut roll);
    assert!(!manager.lightning_active, "an update clears the flash");
}

/// The rules as the recorded series saw them: the ids in the order the reference chose them, and the sight penalties from the prototype dump.
struct Recorded {
    ids: std::cell::RefCell<Vec<String>>,
    sight: std::collections::BTreeMap<String, f32>,
}

impl WeatherRules for Recorded {
    fn decide(&self, _point: &Point) -> String {
        let mut ids = self.ids.borrow_mut();
        assert!(!ids.is_empty(), "the recording ran out of weathers");
        ids.remove(0)
    }
    fn duration_bounds(&self, _weather: &str) -> (i32, i32) {
        // Every type in the shipped data takes the five-minute default, which is the number the trace drew every hour.
        (300, 300)
    }
    fn sight_penalty(&self, weather: &str) -> f32 {
        self.sight.get(weather).copied().unwrap_or(1.0)
    }
}

#[test]
fn the_manager_walks_the_recorded_day_turn_for_turn() {
    // The manager against the day the reference recorded: it must recompute on exactly the turns the reference recomputed, land on exactly the next turn it landed on, and report a
    // sight change on exactly the updates whose weather changed the sight penalty. The id is taken from the recording rather than decided, because the decision has its own gate -
    // what is being checked here is the *bookkeeping* around it.
    let prototypes = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../corpus/prototypes/weather.json"),
    )
    .expect("the prototype dump");
    let cdda_json::Value::Object(root) =
        cdda_json::parse(prototypes.as_bytes()).expect("valid JSON")
    else {
        panic!("root")
    };
    let Some(cdda_json::Value::Array(kinds)) = root.get("prototypes") else {
        panic!("prototypes")
    };
    let mut sight = std::collections::BTreeMap::new();
    for kind in kinds {
        let cdda_json::Value::Object(kind) = kind else {
            panic!("a kind")
        };
        let Some(cdda_json::Value::Str(id)) = kind.get("id") else {
            panic!("an id")
        };
        let penalty = match kind.get("sight_penalty") {
            Some(cdda_json::Value::Float(value)) => *value as f32,
            Some(cdda_json::Value::Int(value)) => *value as f32,
            other => panic!("a sight penalty: {other:?}"),
        };
        sight.insert(id.clone(), penalty);
    }

    let series = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../corpus/weather/series-24h.json"),
    )
    .expect("the recorded series");
    let cdda_json::Value::Object(root) = cdda_json::parse(series.as_bytes()).expect("valid JSON")
    else {
        panic!("root")
    };
    let Some(cdda_json::Value::Array(steps)) = root.get("steps") else {
        panic!("steps")
    };
    let field = |step: &cdda_json::Object, key: &str| -> f64 {
        match step.get(key) {
            Some(cdda_json::Value::Float(value)) => *value,
            Some(cdda_json::Value::Int(value)) => *value as f64,
            other => panic!("{key} is a number: {other:?}"),
        }
    };
    let mut ids = Vec::new();
    for step in steps {
        let cdda_json::Value::Object(step) = step else {
            panic!("a step")
        };
        match step.get("weather") {
            Some(cdda_json::Value::Str(id)) => ids.push(id.clone()),
            other => panic!("a weather id: {other:?}"),
        }
    }
    let rules = Recorded {
        ids: std::cell::RefCell::new(ids),
        sight,
    };
    let mut manager = Manager::default();
    let mut roll = |low: i32, high: i32| {
        assert_eq!(
            (low, high),
            (300, 300),
            "the shipped data's five-minute default"
        );
        300
    };
    let mut updates = 0;
    let mut flushes = 0;
    let mut previous_penalty: Option<f32> = None;
    for step in steps {
        let cdda_json::Value::Object(step) = step else {
            panic!("a step")
        };
        let turn = field(step, "turn") as i64;
        let expected_next = field(step, "next_weather_turn") as i64;
        let id = match step.get("weather") {
            Some(cdda_json::Value::Str(id)) => id.clone(),
            other => panic!("an id: {other:?}"),
        };
        let change = manager.update(
            turn,
            Point {
                turn,
                temperature: field(step, "temperature_k"),
                humidity: field(step, "humidity"),
                pressure: field(step, "pressure"),
                windpower: field(step, "windpower") as i32,
                winddirection: field(step, "winddirection") as i32,
            },
            &rules,
            &mut roll,
        );
        assert!(
            matches!(change, Change::Weather { .. }),
            "turn {turn} was due: the reference recomputed every hour of the recording"
        );
        updates += 1;
        assert_eq!(
            manager.weather.as_deref(),
            Some(id.as_str()),
            "turn {turn} chose the recorded weather"
        );
        assert_eq!(
            manager.next_turn, expected_next,
            "turn {turn} scheduled the recorded next turn"
        );
        let penalty = rules.sight_penalty(&id);
        let expected_sight = previous_penalty != Some(penalty);
        if let Change::Weather { sight_changed } = change {
            assert_eq!(
                sight_changed, expected_sight,
                "turn {turn} sight change against the recorded penalty"
            );
            if sight_changed {
                flushes += 1;
            }
        }
        previous_penalty = Some(penalty);
    }
    assert_eq!(updates, 24, "every recorded hour was an update");
    assert_eq!(
        flushes, 1,
        "only the first update changed the sight penalty, because sunny and clear absorb the same light"
    );
}
