//! The weather, as a tick and as far as the light and the sight are concerned.
//!
//! Reference: `weather_type` (`src/weather_type.h:73-120`), whose `sight_penalty` is the number the transparency rule charges every square under open sky
//! (`calc_transp`, `lightmap.cpp:150-154`). The whole system - generators, transitions, precipitation, temperature - is a declared gap; this is the one number the light
//! already needed and was being handed by hand.
//!
//! **The number is above one and that is not a mistake.** The data gives rain 1.1, snow 1.3 and fog 1.7, because it raises a square's absorption rather than lowering a
//! transmission - the same backwards-looking direction as a field's translucency. A penalty below one would make bad weather brighten the world.

/// What the sky is doing, as far as the light is concerned.
#[derive(Debug, Clone, Copy, PartialEq, bevy_ecs::prelude::Component)]
pub struct Weather {
    /// How much more a square under open sky absorbs while this weather lasts.
    sight_penalty: f32,
}

impl Weather {
    /// A clear sky, which costs nothing.
    #[must_use]
    pub fn clear() -> Self {
        Self { sight_penalty: 1.0 }
    }

    /// Weather with a sight penalty, the number a `weather_type` prototype carries.
    ///
    /// A value below one is refused rather than clamped: the reference's data never has one, and a caller that passes `0.5` has the direction of the multiplier backwards.
    ///
    /// # Panics
    ///
    /// Panics if the penalty is below one, because that would mean weather that brightens the world.
    #[must_use]
    pub fn with_sight_penalty(sight_penalty: f32) -> Self {
        assert!(
            sight_penalty >= 1.0,
            "a sight penalty below one would make bad weather brighter than clear air"
        );
        Self { sight_penalty }
    }

    /// The sky a loaded weather type describes.
    ///
    /// A type whose penalty is zero is a type nothing wrote one for - the data has exactly one, and it means "no weather in force" rather than "better than clear air" - so it
    /// becomes [`Weather::clear`] instead of tripping the guard above. That is the reference's own reading: the multiplier is one when no weather applies.
    #[must_use]
    pub fn from_type(weather: &cdda_content::weather::WeatherType) -> Self {
        if weather.sight_penalty >= 1.0 {
            Self::with_sight_penalty(weather.sight_penalty)
        } else {
            Self::clear()
        }
    }

    /// The penalty the transparency rule applies to every square in the open.
    #[must_use]
    pub fn sight_penalty(self) -> f32 {
        self.sight_penalty
    }
}

impl Default for Weather {
    fn default() -> Self {
        Self::clear()
    }
}

/// What one turn's weather is computed from.
#[derive(Debug, Clone, Copy)]
pub struct Forecast<'a> {
    /// The region's settings.
    pub generator: &'a crate::weather_gen::Generator,
    /// Where and when, as the noise field wants it.
    pub common: crate::weather_gen::CommonData,
    /// How far through the day the turn is, as a fraction, which the temperature's daily cosine needs.
    pub day_fraction: f64,
    /// The turn itself.
    pub turn: i64,
}

/// The weather as the game runs it: a manager, the wind's remembered direction, and the rules.
///
/// Reference: the reference keeps all three as globals - `weather_manager::weather_id`, the generator's `static current_winddir`, and the weather registry - and the tick that joins
/// them is `weather_manager::update_weather`. Holding them together is what makes the order testable: the point is computed first, then the sky decided from it, then the next change
/// booked, and each step can only see the ones before it.
#[derive(Debug, Clone)]
pub struct System {
    /// The weather held between turns.
    pub manager: cdda_content::weather::Manager,
    /// The direction the wind last blew from, remembered across turns like the reference's static.
    pub wind: crate::weather_gen::WindDirection,
    /// The types, their conditions, and the order they are tested in.
    pub rules: cdda_content::weather::Rules,
}

impl System {
    /// A system with no weather yet.
    #[must_use]
    pub fn new(rules: cdda_content::weather::Rules) -> System {
        System {
            manager: cdda_content::weather::Manager::default(),
            wind: crate::weather_gen::WindDirection::default(),
            rules,
        }
    }

    /// Runs one turn of the weather (`weather.cpp:901-947`).
    ///
    /// Nothing is computed unless the weather is due - the reference computes `w` inside the `if` - so a still hour costs nothing and, more importantly, **rolls no dice**: the
    /// reference's own trace shows the hours that did not recompute drawing nothing at all.
    ///
    /// `roll` supplies every die in the reference's order: the wind's three, the wind direction's check, and the duration. `sector` supplies the wind direction itself when the check
    /// fires, because the reference draws it from a distribution over the engine rather than through `rng`, so no traced draw stands for it.
    pub fn tick(
        &mut self,
        forecast: &Forecast,
        roll: &mut dyn FnMut(i32, i32) -> i32,
        sector: &mut dyn FnMut(usize) -> usize,
    ) -> cdda_content::weather::Change {
        let turn = forecast.turn;
        if !self.manager.needs_update(turn) {
            return cdda_content::weather::Change::None;
        }
        let temperature = crate::weather_gen::temperature_k(
            forecast.generator,
            &forecast.common,
            forecast.day_fraction,
        );
        let humidity = crate::weather_gen::humidity(forecast.generator, &forecast.common);
        let pressure = crate::weather_gen::pressure(forecast.generator, &forecast.common);
        let windpower =
            crate::weather_gen::wind_power(forecast.generator, &forecast.common, pressure, roll);
        // `one_in( W * 2160 )` is true without rolling when its chance is one or less, which is why a calm hour draws nothing here and the trace agrees.
        let chance = windpower.saturating_mul(2160);
        let check = if chance <= 1 {
            None
        } else {
            Some(roll(0, chance - 1))
        };
        if self.wind.needs_redraw(windpower, check) {
            self.wind.set_from_sector(sector(forecast.common.season));
        }
        let point = cdda_content::weather::Point {
            turn,
            temperature,
            humidity,
            pressure,
            windpower,
            winddirection: self.wind.degrees.unwrap_or(0),
        };
        self.manager.update(turn, point, &self.rules, roll)
    }
}

/// The weather as the world holds it: the system, plus the two numbers a turn hands to everything else.
///
/// The reference keeps the same facts in `weather_manager`'s globals and reads them where they are needed - `get_weather().windspeed` in the field sweep, the type's `sight_penalty`
/// in the light. Putting them in a resource is what lets a turn take one borrow instead of reaching for a singleton, and it is where the *last computed* values live between the
/// hourly updates: a turn that does not recompute still has a wind to spread smoke with.
#[derive(Debug, Clone, bevy_ecs::prelude::Component)]
pub struct Climate {
    /// The tick itself: manager, remembered wind direction, and rules.
    pub system: System,
    /// The wind the last update reported, which is what the field sweep and the fires use.
    windpower: i32,
    /// The weather's id, for anything that wants to name what the sky is doing.
    weather: Option<String>,
    /// The absorption the light is cast under.
    sight_penalty: f32,
}

impl Climate {
    /// A climate with no weather yet: clear air, no wind, and the rules it will decide by.
    #[must_use]
    pub fn new(rules: cdda_content::weather::Rules) -> Climate {
        Climate {
            system: System::new(rules),
            windpower: 0,
            weather: None,
            sight_penalty: 1.0,
        }
    }

    /// The wind the last update reported.
    #[must_use]
    pub fn windpower(&self) -> i32 {
        self.windpower
    }

    /// Puts a point in the manager, for a caller that has the numbers from somewhere other than a tick.
    ///
    /// This is the test-and-replay door: a scenario that must answer `weather('temperature')` at a known turn does not need a whole weather simulation to say
    /// what the sky was doing.
    pub fn set_point_for_reading(&mut self, point: cdda_content::weather::Point) {
        self.system.manager.point = Some(point);
    }

    /// The last point the weather computed, if it has computed one.
    ///
    /// Reference: `weather_manager::weather_precise`, the global the math parser reaches for when it answers `weather('temperature')`. The manager already keeps
    /// it, so the port answers from there rather than copying it out.
    #[must_use]
    pub fn point(&self) -> Option<cdda_content::weather::Point> {
        self.system.manager.point
    }

    /// The weather's id.
    #[must_use]
    pub fn weather(&self) -> Option<&str> {
        self.weather.as_deref()
    }

    /// The absorption the light is cast under.
    #[must_use]
    pub fn sight_penalty(&self) -> f32 {
        self.sight_penalty
    }
}

impl Default for Climate {
    fn default() -> Self {
        Climate::new(cdda_content::weather::Rules::default())
    }
}

/// Runs the weather for one turn and hands the world what changed.
///
/// The order is the reference's: the tick decides, then the map is told whether its light is stale, then the numbers a turn reads are stored. The weather resource is updated from the
/// **same** rules the decision used, so a world cannot end up drawing fog at a clear sky's brightness.
///
/// Returns the change, so a caller can react to a weather that just turned dangerous without asking again.
pub fn tick_climate(
    world: &mut bevy_ecs::prelude::World,
    forecast: &Forecast,
    roll: &mut dyn FnMut(i32, i32) -> i32,
    sector: &mut dyn FnMut(usize) -> usize,
) -> cdda_content::weather::Change {
    let Some(mut climate) = crate::singletons::climate(world) else {
        return cdda_content::weather::Change::None;
    };
    let change = climate.system.tick(forecast, roll, sector);
    if let cdda_content::weather::Change::Weather { .. } = change {
        if let Some(point) = climate.system.manager.point {
            climate.windpower = point.windpower;
        }
        // The stored weather is copied out before it is written back: `climate` is a borrowed
        // component now, so the source and the destination cannot both be held at once.
        let named = climate.system.manager.weather.clone();
        climate.weather = named;
        if let Some(id) = climate.weather.clone() {
            let penalty =
                cdda_content::weather::WeatherRules::sight_penalty(&climate.system.rules, &id);
            climate.sight_penalty = penalty;
        }
    }
    let penalty = climate.sight_penalty;
    // `climate` is a `Mut` handed out by `world`, so the borrow ends when the guard is dropped at
    // the end of this scope; the reads below take their own.
    crate::apply_weather_change(world, change);
    crate::singletons::set_weather(world, crate::Weather::with_sight_penalty(penalty.max(1.0)));
    change
}

/// The wind a turn should spread its fields and fires by.
///
/// A world with a [`Climate`] has exactly one answer, and it is the weather's. A world without one - a session replay that supplies its own wind, a unit test of a single rule - falls
/// back to what its caller passed, which is why the fallback is explicit instead of a zero.
#[must_use]
pub fn wind_of(world: &bevy_ecs::prelude::World, fallback: i32) -> i32 {
    match crate::singletons::climate_read(world) {
        Some(climate) => climate.windpower,
        None => fallback,
    }
}

/// The name of the weather that is happening, which is what a dialogue asks about.
///
/// Reference: `f_is_weather` (`condition.cpp:1812-1818`) compares `get_weather().weather_id` with the id the guard names. The port's
/// [`Weather`] carries only the sight penalty the light needs, so the id lives beside it as its own component: the id is data, the
/// penalty is a number, and a rule that wants one should not have to carry the other.
#[derive(Debug, Clone, PartialEq, Eq, Default, bevy_ecs::prelude::Component)]
pub struct WeatherId {
    /// The `weather_type` id, as the data spells it.
    pub id: String,
}

/// Says what the sky is called, adding the component if the world has not named it yet.
pub fn set_id(world: &mut bevy_ecs::prelude::World, id: &str) {
    let entity = crate::singletons::singleton::<WeatherId, crate::singletons::TheWeather>(world);
    if let Some(mut held) = world.get_mut::<WeatherId>(entity) {
        held.id.clone_from(&id.to_owned());
        return;
    }
    world
        .entity_mut(entity)
        .insert(WeatherId { id: id.to_owned() });
}

/// What the sky is called, or nothing when nobody has named it.
#[must_use]
pub fn id_of(world: &bevy_ecs::prelude::World) -> Option<String> {
    world
        .iter_entities()
        .find_map(|entity| entity.get::<WeatherId>().map(|held| held.id.clone()))
}
