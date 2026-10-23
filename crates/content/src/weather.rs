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
    /// The shortest the weather can last, in turns.
    pub duration_min: i32,
    /// The longest it can last, in turns.
    pub duration_max: i32,
}

/// What a weather type lasts when the data says nothing: five minutes (`weather_type.cpp:120-122`).
///
/// Every type in the shipped data takes this default, which makes a drawn duration of three hundred turns look like a constant until a mod says otherwise.
pub const DEFAULT_DURATION_TURNS: i32 = 300;

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
            duration_min: duration_of(object, "duration_min"),
            duration_max: duration_of(object, "duration_max"),
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

/// The weather a set of satisfied conditions produces (`weather_gen.cpp:196-215`).
///
/// The loop starts at `clear` and walks the types in the order [`sorted`] gives them. A type applies when its own requirements are met - it names none, or it names the
/// current answer - **and** its condition holds; the last type that applies is the answer, which is why the order is behaviour and not presentation.
///
/// The condition is passed in rather than evaluated here: each one is a dialogue expression, and the dialogue machine is a phase of its own. Taking it as a parameter
/// keeps this loop honest - it can be gated on transcribed chains today, and it does not need rewriting when the conditions arrive.
#[must_use]
pub fn conditions(order: &[&WeatherType], satisfied: &mut dyn FnMut(&WeatherType) -> bool) -> String {
    let mut current = "clear".to_string();
    for kind in order {
        let follows = kind.required_weathers.is_empty() || kind.required_weathers.contains(&current);
        if follows && satisfied(kind) {
            current.clone_from(&kind.id);
        }
    }
    current
}

/// One of a type's duration bounds, defaulting to five minutes.
///
/// The reference reads these as a `time_duration`, which the dump writes as whole turns; a fractional turn is not a shape the data uses, and truncating it would be
/// the reference's own conversion if it ever did.
#[allow(clippy::cast_possible_truncation)]
fn duration_of(object: &cdda_json::Object, key: &str) -> i32 {
    match object.get(key) {
        Some(Value::Int(turns)) => i32::try_from(*turns).unwrap_or(DEFAULT_DURATION_TURNS),
        Some(Value::Float(turns)) => *turns as i32,
        _ => DEFAULT_DURATION_TURNS,
    }
}

/// When the weather is next due to change (`weather.cpp:901-925`).
///
/// Reference: `weather_manager::update_weather` recomputes only when there is no weather yet or the turn has reached `nextweather`, and each recompute sets
/// `nextweather = turn + rng( weather_id->duration_min, weather_id->duration_max )`. The port keeps the same two facts in a value, so the rule can be checked at
/// its boundary instead of being inferred from a whole session.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Schedule {
    /// The weather now, or `None` before the first update.
    pub weather: Option<String>,
    /// The turn the weather is next due to change.
    pub next_turn: i64,
}

impl Schedule {
    /// Whether this turn recomputes the weather.
    #[must_use]
    pub fn needs_update(&self, turn: i64) -> bool {
        self.weather.is_none() || turn >= self.next_turn
    }

    /// Records a new weather and the duration it was rolled to last.
    pub fn reschedule(&mut self, weather: String, turn: i64, duration: i32) {
        self.weather = Some(weather);
        self.next_turn = turn + i64::from(duration);
    }
}

/// The numbers a weather update produces (`w_point`).
///
/// The reference's `w_point` also carries the location and the seed that produced it; the port keeps the four numbers a manager and the dialogue conditions read, and takes the
/// rest as arguments where they are needed.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Point {
    /// The temperature at the surface, in kelvin.
    pub temperature: f64,
    /// Relative humidity, as a percentage.
    pub humidity: f64,
    /// Pressure, in millibars.
    pub pressure: f64,
    /// Wind strength.
    pub windpower: i32,
    /// Wind direction in degrees.
    pub winddirection: i32,
}

/// What an update did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Change {
    /// The weather was not due, so nothing happened.
    None,
    /// The weather was recomputed.
    Weather {
        /// Whether the sight penalty changed, which is the signal to mark every transparency cache dirty (`weather.cpp:938-941`).
        sight_changed: bool,
    },
}

/// What the manager needs to know about the data and the rules, supplied by the caller.
///
/// The reference reads the same facts from globals - the weather registry, the option system, the regional settings. Passing them in is what lets the manager be tested against
/// recorded numbers without a world, and it is the same discipline the dice follow.
pub trait WeatherRules {
    /// The weather the current conditions produce.
    fn decide(&self, point: &Point) -> String;
    /// The shortest and longest a weather can last, in turns.
    fn duration_bounds(&self, weather: &str) -> (i32, i32);
    /// How much light the weather absorbs, which is `1.0` for a clear sky.
    fn sight_penalty(&self, weather: &str) -> f32;
}

/// The weather, held between turns (`weather_manager`).
///
/// The reference keeps one of these in a global and fills it from options, overrides and the player's position. The port keeps the parts that have behaviour: the id, the point it
/// was decided from, the turn it is next due, and the lightning flag.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Manager {
    /// The weather now, or `None` before the first update.
    pub weather: Option<String>,
    /// The point the last update produced.
    pub point: Option<Point>,
    /// The turn the weather is next due to change.
    pub next_turn: i64,
    /// Whether a flash is in progress. Cleared by every update, set by the lightning effect and by nothing else.
    pub lightning_active: bool,
}

impl Manager {
    /// Recomputes the weather if it is due (`weather.cpp:901-947`).
    ///
    /// The order is the reference's: the new point replaces the old one, the id is decided, the point's numbers are stored, the lightning is cleared, and **then** the next change is
    /// rolled from the bounds of the id that was just chosen.
    ///
    /// The null id the reference starts with has a sight penalty of zero, so the first update always reports a sight change. That is why the map's caches are clean at the start of
    /// a game without the weather having done anything.
    pub fn update(
        &mut self,
        turn: i64,
        point: Point,
        rules: &dyn WeatherRules,
        roll: &mut dyn FnMut(i32, i32) -> i32,
    ) -> Change {
        let due = self.weather.is_none() || turn >= self.next_turn;
        if !due {
            return Change::None;
        }
        let previous = self.weather.clone();
        let chosen = rules.decide(&point);
        let (low, high) = rules.duration_bounds(&chosen);
        // The point is stored before the decision is asked for in the reference, but the decision reads the local `w`; storing it here first is what a later reader of
        // `manager.point` expects either way.
        self.point = Some(point);
        self.weather = Some(chosen.clone());
        self.lightning_active = false;
        self.next_turn = turn + i64::from(roll(low, high));
        let changed = previous.as_deref() != Some(chosen.as_str());
        // Exact comparison on purpose: the reference compares the two penalties with `!=`, so a weather that differs by one part in a million still dirties every cache. A margin
        // would leave a map drawn under the wrong absorption.
        #[allow(clippy::float_cmp)]
        let sight_changed = match &previous {
            None => true,
            Some(old) => rules.sight_penalty(old) != rules.sight_penalty(&chosen),
        };
        debug_assert!(!sight_changed || changed, "a sight change without a weather change would be a bug in the rules");
        Change::Weather { sight_changed }
    }
}
