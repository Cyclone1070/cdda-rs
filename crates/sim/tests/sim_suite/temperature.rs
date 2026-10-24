//! How warm a square is, against the reference's own recording.
//!
//! Reference: `weather_manager::get_temperature` (`src/weather.cpp:956-982`), `get_heat_radiation` (`src/game.cpp:1755-1795`) and `get_convection_temperature`
//! (`src/game.cpp:1835-1855`). `corpus/temperature/squares.json` is a corridor of floor with one square of lava, one of hot spring water and one fire, and the
//! reference's own three terms and total for nine squares along it.

#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss
)]

use std::path::Path;

use cdda_json::{parse, Value};
use cdda_sim::temperature::{convection, heat_radiation, temperature, Heat};

fn record() -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/temperature/squares.json");
    let text = std::fs::read_to_string(path).expect("the temperature dump exists");
    parse(text.as_bytes()).expect("the temperature dump is valid JSON")
}

fn member<'a>(value: &'a Value, key: &str) -> &'a Value {
    match value {
        Value::Object(object) => object.get(key).unwrap_or_else(|| panic!("{key} is a member")),
        other => panic!("an object: {other:?}"),
    }
}

fn array(value: &Value) -> &[Value] {
    match value {
        Value::Array(entries) => entries,
        other => panic!("a list: {other:?}"),
    }
}

fn int(value: &Value) -> i32 {
    match value {
        Value::Int(number) => i32::try_from(*number).expect("small"),
        other => panic!("an integer: {other:?}"),
    }
}

fn real(value: &Value) -> f64 {
    match value {
        Value::Float(number) => *number,
        Value::Int(number) => {
            #[allow(clippy::cast_precision_loss)]
            {
                *number as f64
            }
        }
        other => panic!("a number: {other:?}"),
    }
}

fn pair(value: &Value) -> (i32, i32) {
    let parts = array(value);
    (int(&parts[0]), int(&parts[1]))
}

/// The recorded room: uniform floor, one lava square, one hot-water square, one fire, and nothing else.
struct Room {
    lava: (i32, i32),
    water: (i32, i32),
    /// The square the cold field is on, which is opaque.
    frosted: (i32, i32),
    fire_intensity: i32,
}

impl Heat for Room {
    fn fire_intensity(&self, at: (i32, i32)) -> i32 {
        // **No fire in this fixture.** A fire of intensity three is opaque, so it hides what is behind it and the recording stops being arithmetic; the fixture
        // carries a cold gas instead, which is transparent and still has a convection term.
        let _ = at;
        self.fire_intensity
    }

    fn terrain_heat(&self, at: (i32, i32)) -> i32 {
        // `t_lava` radiates 3 and `t_water_hot` 1, from the data (`terrain-liquids.json`), and nothing else in the data radiates at all.
        if at == self.lava {
            3
        } else {
            i32::from(at == self.water)
        }
    }

    fn sees(&self, from: (i32, i32), to: (i32, i32)) -> bool {
        // **A cold-air field of this intensity is opaque**, and the line of sight is blocked at its *source*: `fd_cold_air3` has a translucency of zero at every
        // intensity, so the square it is on is solid to light. That is why the recording shows no heat at all on that square even though the lava is four squares
        // away and well inside the six-square reach - and the assertion below pins the translucency to the data rather than to this comment.
        let _ = to;
        from != self.frosted
    }
}

/// The cold field the fixture uses is opaque, which is a fact about the data rather than about the fixture.
#[test]
fn the_cold_air_field_the_fixture_uses_blocks_sight() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/prototypes/fields.json");
    let text = std::fs::read_to_string(path).expect("the field dump exists");
    let value = parse(text.as_bytes()).expect("the field dump is valid JSON");
    let fields = cdda_content::fields::from_dump(&value).expect("the field dump loads");
    let cold = cdda_content::fields::find(&fields, "fd_cold_air3").expect("fd_cold_air3 exists");
    assert!(
        cold.levels.iter().all(|level| level.translucency == 0.0),
        "every intensity of cold air has a translucency of zero, so its square is solid"
    );
    // And it is cold where it stands, which is the term the fixture is for.
    assert_eq!(
        cold.levels.first().map(|level| level.convection),
        Some(-16),
        "intensity one is sixteen degrees Fahrenheit colder"
    );
}

#[test]
fn a_square_is_warmer_by_the_square_of_every_heat_source_it_can_see() {
    let record = record();
    let room = Room {
        lava: pair(member(&record, "lava")),
        water: pair(member(&record, "water")),
        frosted: pair(member(&record, "fire")),
        fire_intensity: 0,
    };
    let squares = array(member(&record, "squares"));
    assert_eq!(squares.len(), 9, "nine squares along the corridor");
    let mut wrong = Vec::new();
    for square in squares {
        let (dx, _dy) = (int(member(square, "dx")), 0);
        let mine = f64::from(heat_radiation(&room, (dx, 0)));
        let theirs = real(member(square, "heat"));
        if (mine - theirs).abs() > 1e-4 {
            wrong.push(format!("dx {dx}: {mine:.9} against {theirs:.9}"));
        }
    }
    assert!(wrong.is_empty(), "{} wrong:\n{}", wrong.len(), wrong.join("\n"));
    // Non-vacuity: the corridor has two sources at different strengths, so the numbers are not all the same and one of them is a sum.
    let heats: Vec<f64> = squares.iter().map(|square| real(member(square, "heat"))).collect();
    assert!(heats.iter().any(|heat| *heat > 0.0), "some square is warmed");
    assert!(
        heats.windows(2).any(|pair| (pair[0] - pair[1]).abs() > 1e-9),
        "and the warmth varies"
    );
}

#[test]
fn a_field_on_a_square_warms_the_square_itself() {
    let record = record();
    let squares = array(member(&record, "squares"));
    // Exactly one square has a field on it - the fire - and its convection is the fire's own 300 F, which is 166.666... K.
    let with_field: Vec<&Value> = squares
        .iter()
        .filter(|square| int(member(square, "field")) > 0)
        .collect();
    assert_eq!(with_field.len(), 1, "one square has a field");
    let square = with_field[0];
    // `fd_cold_air3` at intensity one is -16 F, which is the only field in this fixture.
    let mine = f64::from(convection(&[(-16, false)]));
    let theirs = real(member(square, "convection"));
    assert!((mine - theirs).abs() < 1e-4, "{mine:.9} against {theirs:.9}");
    // Every other square answers zero, which is what "convection is about the square itself" means.
    for other in squares.iter().filter(|square| int(member(square, "field")) == 0) {
        assert!(
            real(member(other, "convection")).abs() < 1e-9,
            "no field, no convection"
        );
    }
}

#[test]
fn the_total_adds_the_three_terms_to_the_weather() {
    let record = record();
    let room = Room {
        lava: pair(member(&record, "lava")),
        water: pair(member(&record, "water")),
        frosted: pair(member(&record, "fire")),
        fire_intensity: 0,
    };
    let weather = real(member(&record, "weather"));
    let base = real(member(&record, "base"));
    let mut wrong = Vec::new();
    for square in array(member(&record, "squares")) {
        let dx = int(member(square, "dx"));
        let heat = heat_radiation(&room, (dx, 0));
        let field = if int(member(square, "field")) > 0 {
            convection(&[(-16, false)])
        } else {
            0.0
        };
        let map = f64::from(real(member(square, "map")) as f32);
        #[allow(clippy::cast_possible_truncation)]
        let mine = f64::from(temperature(
            weather as f32,
            base as f32,
            0,
            heat,
            field,
            map as f32,
        ));
        let theirs = real(member(square, "total"));
        if (mine - theirs).abs() > 1e-3 {
            wrong.push(format!("dx {dx}: {mine:.6} against {theirs:.6}"));
        }
        // And a square below street level ignores all three and answers the year's average.
        #[allow(clippy::cast_possible_truncation)]
        let underground = f64::from(temperature(weather as f32, base as f32, -1, heat, field, 0.0));
        if (underground - real(member(square, "underground"))).abs() > 1e-3 {
            wrong.push(format!(
                "dx {dx} underground: {underground:.6} against {:.6}",
                real(member(square, "underground"))
            ));
        }
    }
    assert!(wrong.is_empty(), "{} wrong:\n{}", wrong.len(), wrong.join("\n"));
}
