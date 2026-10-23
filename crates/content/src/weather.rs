//! Weather prototypes: what each kind of weather does to light, sight and sound.
//!
//! Reference: `weather_type` (`src/weather_type.h:73-127`) and its factory (`:128-140`). The numbers are **derived** rather than written in the data: `load` reads a handful of
//! flags and turns them into the multipliers the game then uses, which is why the port takes them from a dump rather than from `weather_type.json`.
//!
//! Gated on `--dump-weather`: **20** weather types.

use cdda_json::Value;

/// What falls from the sky.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Precipitation {
    /// Nothing.
    #[default]
    None,
    /// A drizzle at most.
    VeryLight,
    /// Rain or flurries.
    Light,
    /// A downpour or a blizzard.
    Heavy,
}

/// One weather type.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct WeatherType {
    /// Its id, e.g. `fog`.
    pub id: String,
    /// What the game calls it.
    pub name: String,
    /// What it does to ranged attacks.
    pub ranged_penalty: i32,
    /// What it does to sight: **at or above one**, because the number is the absorption multiplier `calc_transp` applies, so fog's `1.7` makes every square more opaque.
    pub sight_penalty: f32,
    /// How much it shifts the light level.
    pub light_modifier: i32,
    /// What it multiplies the light by.
    pub light_multiplier: f32,
    /// What it multiplies the sun by.
    pub sun_multiplier: f32,
    /// How far sound carries.
    pub sound_attn: i32,
    /// Whether it hurts to be out in it.
    pub dangerous: bool,
    /// What falls.
    pub precip: Precipitation,
    /// Whether it is rain rather than snow.
    pub rains: bool,
    /// Which weather wins when two are possible: the higher the priority.
    pub priority: i32,
    /// The weathers that must already hold before this one can (`weather_type.h`). Each becomes a chain: rain needs drizzle first, thunder needs rain.
    pub required_weathers: Vec<String>,
}

/// The weather types in the order the game tests them: by priority, and **stably**, so a tie keeps the order the data declared.
///
/// Reference: `weather_gen.cpp:319-341`, `sort_weather`. The reference uses `std::sort`, which is not stable, but the ties that exist in the data come out in
/// declaration order - `mist` and `fog` both carry 15 and stay adjacent in that order - so a stable sort reproduces it. `get_weather_conditions` lets the last
/// match win, which makes this order part of the weather's behaviour rather than a detail of how it is stored.
#[must_use]
pub fn sorted(types: &[WeatherType]) -> Vec<&WeatherType> {
    let mut ordered: Vec<&WeatherType> = types.iter().collect();
    ordered.sort_by_key(|kind| kind.priority);
    ordered
}

/// Reads the weather dump.
///
/// # Errors
///
/// Returns the reason the dump could not be read: a shape it does not understand, or a weather type with no id.
pub fn from_dump(value: &Value) -> Result<Vec<WeatherType>, String> {
    let Value::Object(root) = value else {
        return Err("the dump's root is not an object".to_string());
    };
    let Some(Value::Array(entries)) = root.get("prototypes") else {
        return Err("the dump has no prototypes array".to_string());
    };
    let mut types = Vec::with_capacity(entries.len());
    for entry in entries {
        let Value::Object(object) = entry else {
            return Err("a weather type is not an object".to_string());
        };
        let Some(Value::Str(id)) = object.get("id") else {
            return Err("a weather type has no id".to_string());
        };
        types.push(WeatherType {
            id: id.clone(),
            name: string_of(object.get("name")),
            ranged_penalty: int_of(object.get("ranged_penalty")),
            sight_penalty: float_of(object.get("sight_penalty")).unwrap_or(1.0),
            light_modifier: int_of(object.get("light_modifier")),
            light_multiplier: float_of(object.get("light_multiplier")).unwrap_or(1.0),
            sun_multiplier: float_of(object.get("sun_multiplier")).unwrap_or(1.0),
            sound_attn: int_of(object.get("sound_attn")),
            dangerous: matches!(object.get("dangerous"), Some(Value::Bool(true))),
            precip: match string_of(object.get("precip")).as_str() {
                "very_light" => Precipitation::VeryLight,
                "light" => Precipitation::Light,
                "heavy" => Precipitation::Heavy,
                _ => Precipitation::None,
            },
            rains: matches!(object.get("rains"), Some(Value::Bool(true))),
            priority: int_of(object.get("priority")),
            required_weathers: match object.get("required_weathers") {
                Some(Value::Array(list)) => list
                    .iter()
                    .filter_map(|id| match id {
                        Value::Str(id) => Some(id.clone()),
                        _ => None,
                    })
                    .collect(),
                _ => Vec::new(),
            },
        });
    }
    Ok(types)
}

/// One weather type by id.
#[must_use]
pub fn find<'a>(types: &'a [WeatherType], id: &str) -> Option<&'a WeatherType> {
    types.iter().find(|weather| weather.id == id)
}

fn int_of(value: Option<&Value>) -> i32 {
    match value {
        Some(Value::Int(number)) => i32::try_from(*number).unwrap_or(0),
        _ => 0,
    }
}

/// A number either way, as the game's single-precision float.
///
/// Both casts are the loaders' own: JSON writes an integral float as an integer, so `1` and `1.0` are the same number, and the reference keeps these as `float` rather than
/// `double`, which is why the narrowing is the faithful direction.
fn float_of(value: Option<&Value>) -> Option<f32> {
    #[allow(clippy::cast_precision_loss, clippy::cast_possible_truncation)]
    match value {
        Some(Value::Float(number)) => Some(*number as f32),
        Some(Value::Int(number)) => Some(*number as f32),
        _ => None,
    }
}

fn string_of(value: Option<&Value>) -> String {
    match value {
        Some(Value::Str(text)) => text.clone(),
        _ => String::new(),
    }
}
