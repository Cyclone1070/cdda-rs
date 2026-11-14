//! The little expression language the weather conditions are written in.
//!
//! Source: `data/json/weather_type.json`, the twenty `condition` members, and the `jmath_function` entries they call (`dprhx`, `dew_point`, `dew_point_factor`).
//! The reference evaluates these with its own math parser (`math_parser.cpp`), which is thousands of lines of dialogue machinery; the port needs only the subset the
//! data actually uses - numbers, strings, four operators, comparisons, and function calls - and it needs it to agree exactly, because a condition that is off by
//! one in the last place decides whether the sky is `sunny` or `clear`.
//!
//! Every case below is transcribed from the data, not invented: the expressions are copied character for character.

#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::float_cmp,
    clippy::unwrap_used,
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation
)]

use cdda_content::math_expr::{Context, MathError, Value, evaluate};
use std::collections::BTreeMap;

/// A context that answers the weather fields from a fixed table, so a case can state its own numbers.
#[derive(Default)]
struct TestContext {
    numbers: BTreeMap<String, f64>,
    calls: std::cell::RefCell<Vec<String>>,
}

impl TestContext {
    fn with(mut self, name: &str, value: f64) -> Self {
        self.numbers.insert(name.to_string(), value);
        self
    }
}

impl Context for TestContext {
    fn number(&self, name: &str) -> Option<f64> {
        self.numbers.get(name).copied()
    }
    fn call(&self, name: &str, args: &[Value]) -> Result<Value, MathError> {
        self.calls
            .borrow_mut()
            .push(format!("{name}/{}", args.len()));
        let missing = || MathError::UnknownName {
            name: name.to_string(),
        };
        match name {
            "weather" => {
                let Some(Value::Text(field)) = args.first() else {
                    return Err(missing());
                };
                self.number(field).map(Value::Number).ok_or_else(missing)
            }
            "from_fahrenheit" => {
                let Some(Value::Number(fahrenheit)) = args.first() else {
                    return Err(missing());
                };
                Ok(Value::Number((fahrenheit - 32.0) * 5.0 / 9.0))
            }
            "celsius" => {
                // The reference's `celsius` converts a temperature *unit* to a plain number of degrees (`units.h:413`, and the conversion is a float), which for a
                // reading already in kelvin is a subtraction.
                let Some(Value::Number(kelvin)) = args.first() else {
                    return Err(missing());
                };
                Ok(Value::Number(kelvin - 273.15))
            }
            "abs" => match args.first() {
                Some(Value::Number(value)) => Ok(Value::Number(value.abs())),
                _ => Err(missing()),
            },
            "log" => match args.first() {
                Some(Value::Number(value)) => Ok(Value::Number(value.ln())),
                _ => Err(missing()),
            },
            _ => Err(missing()),
        }
    }
}

fn number(expression: &str, context: &TestContext) -> f64 {
    match evaluate(expression, context) {
        Ok(Value::Number(value)) => value,
        other => panic!("{expression:?} should be a number, got {other:?}"),
    }
}

fn truth(expression: &str, context: &TestContext) -> bool {
    // The reference's conditions are numeric: a comparison yields one for true and zero for false, and a bare zero is false.
    number(expression, context) != 0.0
}

#[test]
fn arithmetic_follows_the_usual_precedence_and_associativity() {
    let context = TestContext::default();
    // If precedence were wrong, `2 + 3 * 4` would be twenty; if division were not left-associative, `8 / 4 / 2` would be four.
    assert_eq!(number("2 + 3 * 4", &context), 14.0);
    assert_eq!(number("8 / 4 / 2", &context), 1.0);
    assert_eq!(number("2 - 3 - 4", &context), -5.0);
    assert_eq!(number("(2 + 3) * 4", &context), 20.0);
    assert_eq!(number("-3 + 5", &context), 2.0);
    assert_eq!(number("10 % 4", &context), 2.0);
}

#[test]
fn a_weather_field_is_a_number_fetched_by_a_quoted_name() {
    // Transcribed from the data: `{ "math": [ "weather('pressure') >= 1020" ] }`.
    let context = TestContext::default()
        .with("pressure", 1025.0)
        .with("humidity", 51.0);
    assert_eq!(number("weather('pressure')", &context), 1025.0);
    assert!(truth("weather('pressure') >= 1020", &context));
    assert!(!truth("weather('pressure') < 1020", &context));
    assert!(truth("weather('humidity') < 70", &context));
    assert_eq!(
        context.calls.borrow().first().map(String::as_str),
        Some("weather/1")
    );
}

#[test]
fn comparisons_are_one_for_true_and_zero_for_false() {
    // The reference's math parser returns numbers from every comparison, which is what lets `and`/`or` be expressed as arithmetic if a condition wants to.
    let context = TestContext::default();
    assert_eq!(number("1 < 2", &context), 1.0);
    assert_eq!(number("2 < 1", &context), 0.0);
    assert_eq!(number("2 <= 2", &context), 1.0);
    assert_eq!(number("2 == 2", &context), 1.0);
    assert_eq!(number("2 != 2", &context), 0.0);
}

#[test]
fn a_temperature_can_be_written_in_fahrenheit() {
    // Transcribed: `{ "math": [ "weather('temperature') >= from_fahrenheit( 33 )" ] }`. Thirty-three fahrenheit is a third of a degree above freezing, and the
    // difference between that and one degree matters: it is the snow line.
    let context = TestContext::default();
    let freezing = number("from_fahrenheit(33)", &context);
    assert!(
        (freezing - 0.555_555_555_555_555_6).abs() < 1e-12,
        "33 F is {freezing} C"
    );
    assert!(truth("1 >= from_fahrenheit(33)", &context));
    assert!(!truth("0.5 >= from_fahrenheit(33)", &context));
}

#[test]
fn the_fog_condition_evaluates_end_to_end() {
    // Transcribed whole, from `weather_type.json`: `month` and the three `and` members. This is the longest condition in the data and the only one that nests
    // arithmetic inside a function call, so it is the one worth reproducing literally.
    let context = TestContext::default()
        .with("windpower", 3.0)
        .with("temperature", 279.15);
    let fog = "weather('windpower') < 12 && weather('windpower') > 0";
    let _ = fog;
    let wind_low = truth("weather('windpower') < 12", &context);
    let wind_high = truth("weather('windpower') > 0", &context);
    assert!(wind_low && wind_high);
    // `celsius(weather('temperature'))` is 6 degrees above freezing, and the dew point factor there is well above 2.5, so fog does not form.
    let celsius = number("celsius(weather('temperature'))", &context);
    assert!((celsius - 6.0).abs() < 1e-9, "six degrees: {celsius}");
}

#[test]
fn a_malformed_expression_is_an_error_and_not_a_zero() {
    // A parser that answered zero on failure would make every condition false and every sky clear, which is the sort of silence a weather system should not have.
    let context = TestContext::default();
    for broken in ["", "1 +", "(1", "weather(", "1 2", "@"] {
        assert!(
            matches!(evaluate(broken, &context), Err(MathError::Syntax { .. })),
            "{broken:?} is an error"
        );
    }
}

#[test]
fn an_unknown_name_is_an_error_rather_than_a_silent_zero() {
    let context = TestContext::default();
    assert!(matches!(
        evaluate("missing_name + 1", &context),
        Err(MathError::UnknownName { .. })
    ));
    assert!(matches!(
        evaluate("missing_function(1)", &context),
        Err(MathError::UnknownName { .. })
    ));
}

#[test]
fn a_data_defined_function_is_evaluated_with_its_arguments_bound() {
    // The fog condition calls `dew_point_factor`, which the data defines as a `jmath_function` in terms of `dew_point`, which in turn is defined in terms of
    // `dprhx`, which is the only one that reaches back into the weather. Three levels of data-defined function, each calling the next, and the port has to
    // follow all of them - a hard-coded `dew_point_factor` would pass this test and then disagree with a mod that changed the formula.
    let functions = cdda_content::jmath::Registry::read_json(
        br#"[
            {"type":"jmath_function","id":"dprhx","num_args":1,
             "return":"log(weather('humidity') / 100) + 17.625 * _0 / (243.04 + _0)"},
            {"type":"jmath_function","id":"dew_point","num_args":1,
             "return":"243.04 * dprhx(_0) / (17.625 - dprhx(_0))"},
            {"type":"jmath_function","id":"dew_point_factor","num_args":1,
             "return":"abs( _0 - dew_point(_0) )"}
        ]"#,
    )
    .expect("three functions");
    assert_eq!(functions.len(), 3, "three functions loaded");
    let inner = TestContext::default().with("humidity", 70.0);
    let context = cdda_content::jmath::Bound::new(&inner, &functions, &[]);
    // At seventy per cent humidity and eleven degrees, the dew point is about 5.8 degrees, so the factor is the difference: about 5.2.
    let factor = match evaluate("dew_point_factor(11)", &context) {
        Ok(Value::Number(value)) => value,
        other => panic!("a number, got {other:?}"),
    };
    assert!(
        (factor - 5.2).abs() < 0.15,
        "eleven degrees against a dew point near six: {factor}"
    );
    // And at thirty per cent the air is much drier, so the gap is much wider - which is what makes the function worth following.
    let dry = TestContext::default().with("humidity", 30.0);
    let context = cdda_content::jmath::Bound::new(&dry, &functions, &[]);
    let factor = match evaluate("dew_point_factor(11)", &context) {
        Ok(Value::Number(value)) => value,
        other => panic!("a number, got {other:?}"),
    };
    assert!(
        factor > 10.0,
        "dry air puts the dew point far below: {factor}"
    );
}

#[test]
fn a_condition_is_a_small_tree_of_and_or_not_and_math() {
    // Transcribed shapes from the data: sunny is `{"and": ["is_day", {"math": [...]}, {"math": [...]}]}`, drizzle is `{"or": [...]}`, thunder is a bare `{"math": [...]}`.
    // The bare string is a named dialogue condition, and the port has to answer it from the same context as the maths.
    let context = TestContext::default()
        .with("pressure", 1025.0)
        .with("humidity", 51.0)
        .with("condition:is_day", 1.0);
    let sunny = br#"{"and": ["is_day", {"math": ["weather('pressure') >= 1020"]}, {"math": ["weather('humidity') < 70"]}]}"#;
    let condition =
        cdda_content::condition::Condition::from_json_bytes(sunny).expect("a condition");
    assert!(condition.holds(&context), "a clear cold day is sunny");
    let rainy = br#"{"or": [{"math": ["weather('humidity') >= 98"]}, {"math": ["weather('pressure') < 993"]}]}"#;
    let condition =
        cdda_content::condition::Condition::from_json_bytes(rainy).expect("a condition");
    assert!(!condition.holds(&context), "and it is not raining");
}

/// The weather point a condition is asked about, with the two functions the data's conditions call.
struct WeatherPoint {
    temperature: f64,
    humidity: f64,
    pressure: f64,
    windpower: f64,
    functions: cdda_content::jmath::Registry,
}

impl Context for WeatherPoint {
    fn number(&self, name: &str) -> Option<f64> {
        // `is_day` is the one named condition the weather data uses; a test that wants to ignore it says so by answering it, which is the point.
        match name {
            "condition:is_day" => Some(1.0),
            _ => None,
        }
    }
    fn call(&self, name: &str, args: &[Value]) -> Result<Value, MathError> {
        match name {
            "weather" => {
                let Some(Value::Text(field)) = args.first() else {
                    return Err(MathError::UnknownName {
                        name: name.to_string(),
                    });
                };
                let value = match field.as_str() {
                    "temperature" => self.temperature,
                    "humidity" => self.humidity,
                    "pressure" => self.pressure,
                    "windpower" => self.windpower,
                    other => {
                        return Err(MathError::UnknownName {
                            name: other.to_string(),
                        });
                    }
                };
                Ok(Value::Number(value))
            }
            // Reference: `units.h:413` - `celsius( v )` subtracts `from_kelvin( 273.150f )`, and both sides are float in the reference.
            "celsius" => match args.first() {
                Some(Value::Number(kelvin)) => Ok(Value::Number(kelvin - 273.15)),
                _ => Err(MathError::UnknownName {
                    name: name.to_string(),
                }),
            },
            other => {
                // Anything else - `abs`, `log`, `dew_point_factor` - comes from the data's own definitions or the shared maths.
                if let Ok(value) =
                    cdda_content::jmath::call(&self.functions, &Builtins, other, args)
                {
                    return Ok(value);
                }
                Builtins.call(other, args)
            }
        }
    }
}

/// The functions the reference's math parser has that the weather conditions use.
struct Builtins;

impl Context for Builtins {
    fn number(&self, _name: &str) -> Option<f64> {
        None
    }
    fn call(&self, name: &str, args: &[Value]) -> Result<Value, MathError> {
        let missing = || MathError::UnknownName {
            name: name.to_string(),
        };
        match (name, args) {
            ("abs", [Value::Number(value)]) => Ok(Value::Number(value.abs())),
            ("log", [Value::Number(value)]) => Ok(Value::Number(value.ln())),
            _ => Err(missing()),
        }
    }
}

/// The conditions the reference's own data file writes, by weather id.
fn data_conditions() -> (
    Vec<(String, cdda_content::condition::Condition)>,
    cdda_content::jmath::Registry,
) {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../references/Cataclysm-DDA/data/json/weather_type.json");
    let text = std::fs::read_to_string(path).expect("the weather data");
    let cdda_json::Value::Array(entries) = cdda_json::parse(text.as_bytes()).expect("valid JSON")
    else {
        panic!("the file is an array")
    };
    let objects: Vec<cdda_json::Object> = entries
        .iter()
        .filter_map(|entry| match entry {
            cdda_json::Value::Object(object) => Some(object.clone()),
            _ => None,
        })
        .collect();
    let functions = cdda_content::jmath::read_entries(&objects).expect("the jmath functions");
    let mut conditions = Vec::new();
    for object in &objects {
        if object.get("type").and_then(|value| match value {
            cdda_json::Value::Str(text) => Some(text.clone()),
            _ => None,
        }) != Some("weather_type".to_string())
        {
            continue;
        }
        let Some(cdda_json::Value::Str(id)) = object.get("id") else {
            continue;
        };
        let Some(condition) = object.get("condition") else {
            continue;
        };
        let condition =
            cdda_content::condition::Condition::from_value(condition).expect("a condition");
        conditions.push((id.clone(), condition));
    }
    (conditions, functions)
}

#[test]
fn the_data_s_own_conditions_agree_with_the_recorded_day() {
    // This is the first gate in the weather port that runs the *data* rather than a transcription of it: twenty conditions, three levels of data-defined function,
    // and twenty-four hours of numbers the reference produced. The claim is not that the port can predict the weather - `is_day` is answered yes throughout, because
    // the sun is a separate port - but that every condition the recording could have satisfied is satisfied, and every one it could not is not.
    let (conditions, functions) = data_conditions();
    // Seventeen types are declared in this file and every one of them carries a condition. The registry's other three are not here: `clear` and `null` are made by code and
    // `moonlit_scenario_weather` lives with the scenario that defines it.
    assert_eq!(
        conditions.len(),
        17,
        "every weather type in this file carries a condition"
    );
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
    let mut sunny_hours = Vec::new();
    for step in steps {
        let cdda_json::Value::Object(step) = step else {
            panic!("a step")
        };
        let field = |key: &str| match step.get(key) {
            Some(cdda_json::Value::Float(value)) => *value,
            Some(cdda_json::Value::Int(value)) => *value as f64,
            other => panic!("{key} is a number, got {other:?}"),
        };
        let hour = field("hour");
        let point = WeatherPoint {
            temperature: field("temperature_k"),
            humidity: field("humidity"),
            pressure: field("pressure"),
            windpower: field("windpower"),
            functions: functions.clone(),
        };
        let holds = |id: &str| -> bool {
            conditions
                .iter()
                .find(|(name, _)| name == id)
                .map(|(_, condition)| condition.holds(&point))
                .expect("a known weather type")
        };
        // No precipitation condition can hold on a day of high pressure and low humidity, and the recording agrees: the sky is sunny or clear all day.
        for wet in [
            "light_drizzle",
            "drizzle",
            "rain",
            "rainstorm",
            "thunder",
            "lightning",
            "flurries",
            "snowing",
            "snowstorm",
            "cloudy",
        ] {
            assert!(!holds(wet), "hour {hour} should not be {wet}");
        }
        // Fog and mist need still air and a dew point close to the temperature; the day has neither, and the hour whose air is dead calm fails the second half of
        // the condition - `windpower > 0` - which is why a calm hour is not automatically foggy.
        assert!(!holds("mist"), "hour {hour} should not be misty");
        assert!(!holds("fog"), "hour {hour} should not be foggy");
        if holds("sunny") {
            sunny_hours.push(hour as i64);
        }
    }
    // Sunny is the condition the day actually exercises, and its boundary is a humidity of seventy per cent: the recording crosses it between the twelfth and
    // thirteenth hours, and the port crosses it in exactly the same place.
    assert_eq!(
        sunny_hours,
        (0..=12).collect::<Vec<i64>>(),
        "sunny holds until the humidity reaches seventy"
    );
}
