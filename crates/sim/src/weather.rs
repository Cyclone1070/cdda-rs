//! The weather, as far as the light and the sight are concerned.
//!
//! Reference: `weather_type` (`src/weather_type.h:73-120`), whose `sight_penalty` is the number the transparency rule charges every square under open sky
//! (`calc_transp`, `lightmap.cpp:150-154`). The whole system - generators, transitions, precipitation, temperature - is a declared gap; this is the one number the light
//! already needed and was being handed by hand.
//!
//! **The number is above one and that is not a mistake.** The data gives rain 1.1, snow 1.3 and fog 1.7, because it raises a square's absorption rather than lowering a
//! transmission - the same backwards-looking direction as a field's translucency. A penalty below one would make bad weather brighten the world.

/// What the sky is doing, as far as the light is concerned.
#[derive(Debug, Clone, Copy, PartialEq, bevy_ecs::prelude::Resource)]
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
