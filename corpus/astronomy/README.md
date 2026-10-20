# The sun, as the reference computes it

`sun.json` is `--dump-astronomy`: 424 samples of the reference's own
`sun_azimuth_altitude` and `sun_light_at` (`src/calendar.cpp:237-402`), taken a week apart
across a year at three-hour intervals, at the reference's default location.

| Range | Value |
|---|---|
| Altitude | -69.83 to 68.57 degrees |
| Light | 0.0 to 125.0 |

Why a dump rather than trigonometry I could write from memory: the chain from a time point
to an altitude crosses right ascension, declination, sidereal time and the reference's own
season model, and a port with one wrong constant produces a plausible curve that is a few
minutes off at dawn. Minutes at dawn are exactly where the light-level boundary sits
(`docs/parity.md`, "Open-sky light").

So the astronomy is gated, not transcribed: the port has to reproduce these numbers.

Regenerate with:

    ./oracle/build/src/cataclysm --datadir oracle/src/data/ --userdir oracle/userdir \
        --seed 12345 --dump-astronomy corpus/astronomy/sun.json < /dev/null
