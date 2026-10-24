# Temperature

What a square's temperature is made of, recorded from the reference.

    ./oracle/build/src/cataclysm --datadir oracle/src/data/ --userdir oracle/userdir \
        --seed 12345 --dump-temperature corpus/temperature/squares.json < /dev/null

`weather_manager::get_temperature` (`src/weather.cpp:956-982`) answers the weather's temperature plus three terms, and the dump records each term separately: a port that only
recorded the total could be wrong in one of them and right by accident in the others.

| Term | Where it comes from |
|---|---|
| `heat` | `get_heat_radiation` (`src/game.cpp:1755-1795`): six squares around, `6 F * intensity^2 / square distance` per fire or radiating terrain the square can see |
| `convection` | `get_convection_temperature` (`src/game.cpp:1835-1855`): the fields **on** the square, 300 F for a fire and negative for cold air |
| `map` | the submap's own `temperature_mod` (`src/map.cpp:4936-4949`), which is zero everywhere in this fixture |

A square below street level answers the generator's `base_temperature` and none of the three.

## What the fixture had to avoid, twice

**A fire is a bad heat source to record.** Level-three fire has a translucency of 0.1, which `calc_transp` multiplies into a square's light, so a corridor with a fire in it
recorded heat that rose and fell by no rule of distance at all: the fire was visible from one square and not from the next. The fixture uses lava (`heat_radiation` 3) and hot
spring water (1) - terrain, which does not block sight - and a **cold gas** on the last square to exercise convection.

**And a cold gas is opaque.** `fd_cold_air3` has a translucency of zero at every intensity, so the square it sits on is solid to light and sees nothing: the recording shows no
heat at all there, even though the lava is four squares away and inside the six-square reach. That asymmetry is a rule, not a defect, and the port's gate pins the
translucency to the data before relying on it.
