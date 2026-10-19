//! Physical quantities, translated from the reference's own tests.
//!
//! Reference: `references/Cataclysm-DDA/tests/units_test.cpp` (line numbers in
//! comments) and `references/Cataclysm-DDA/src/units.h`. C++ has overloaded
//! user-defined literals; the port exposes same-named macros, so `1_gram`
//! becomes `gram!(1)`.

// The reference compares exact doubles for conversions that are exact by
// construction (`1.0_liter == 1000.0_ml`); approximations are written explicitly
// where the reference uses `Approx`.
#![allow(clippy::float_cmp)]

use cdda_core_types::units::{
    Angle, Energy, Length, Mass, Money, Power, Temperature, Volume, atan2, cos, display_energy,
    from_celsius, from_celsius_delta, from_cent, from_fahrenheit, from_fahrenheit_delta, from_gram,
    from_joule, from_joule_per_gram, from_kelvin, from_kelvin_delta, from_kilogram, from_kilojoule,
    from_kilowatt, from_kusd, from_milligram, from_millijoule, from_milliliter, from_millimeter,
    from_milliwatt, from_radians, from_usd, from_watt, round_to_multiple_of, sin, to_celsius,
    to_celsius_delta, to_cent, to_centimeter, to_fahrenheit, to_fahrenheit_delta, to_gram,
    to_joule, to_joule_per_gram, to_kelvin, to_kelvin_delta, to_kilogram, to_kilojoule,
    to_kilometer, to_kilowatt, to_kusd, to_liter, to_meter, to_milligram, to_millijoule,
    to_milliliter, to_millimeter, to_milliwatt, to_usd, to_watt,
};
use cdda_core_types::{
    arcmin, cent, centimeter, degrees, gram, joule, kilogram, kilojoule, kilometer, kilowatt, kusd,
    liter, meter, milligram, millijoule, milliliter, millimeter, milliwatt, pi_radians, radians,
    usd, watt,
};

#[test]
fn mass_units_have_correct_ratios() {
    // units_test.cpp:17-20.
    assert_eq!(gram!(1), milligram!(1000));
    assert_eq!(kilogram!(1), gram!(1000));
    assert_eq!(gram!(1.0), milligram!(1000.0));
    assert_eq!(kilogram!(1.0), gram!(1000.0));

    // The integer literal yields the canonical `mass` type from units_fwd.h:29
    // (int64_t milligrams), not a quantity over the literal's own integer type.
    let one_gram: Mass = gram!(1);
    assert_eq!(one_gram, from_milligram(1000_i64));
    assert_eq!(one_gram, from_gram(1_i64));
    assert_eq!(kilogram!(1), from_kilogram(1_i64));
}

#[test]
fn quantities_compare_by_base_value() {
    // units.h:78-95: comparators look at the stored base value only.
    assert!(milligram!(999) < gram!(1));
    assert!(gram!(1) <= gram!(1));
    assert!(gram!(2) > gram!(1));
    assert!(gram!(2) >= gram!(1));
    assert_ne!(gram!(1), milligram!(999));
}

#[test]
fn mass_conversions_truncate_toward_zero() {
    // units.h:357-361: `to_gram` returns the quantity's own value type, so the
    // `double` division is narrowed back to int64_t and 1500 mg is 1 g.
    assert_eq!(to_gram(from_milligram(1500_i64)), 1);
    assert_eq!(to_gram(from_milligram(999_i64)), 0);
    assert_eq!(to_gram(from_milligram(-1500_i64)), -1);
    // units.h:351-355: milligrams come back unchanged.
    assert_eq!(to_milligram(from_milligram(1500_i64)), 1500);
    // units.h:363-366: kilograms are a `double`, so the fraction survives.
    assert_eq!(to_kilogram(from_milligram(1500_i64)), 1500.0 / 1_000_000.0);
}

#[test]
fn volume_units_have_correct_ratios() {
    // units_test.cpp:15-16.
    assert_eq!(liter!(1), milliliter!(1000));
    assert_eq!(liter!(1.0), milliliter!(1000.0));

    // Integer literals produce the canonical `volume` type from units_fwd.h:16.
    let one_litre: Volume = liter!(1);
    assert_eq!(one_litre, from_milliliter(1000));
}

#[test]
fn volume_conversions_keep_fractions_in_litres_only() {
    // units.h:319-328: `to_milliliter` is exact, `to_liter` is a double.
    assert_eq!(to_milliliter(liter!(1)), 1000);
    assert_eq!(to_liter(liter!(1)), 1.0);
    assert_eq!(to_liter(milliliter!(1500)), 1.5);
}

#[test]
fn energy_units_have_correct_ratios() {
    // units_test.cpp:21-24.
    assert_eq!(joule!(1), millijoule!(1000));
    assert_eq!(kilojoule!(1), joule!(1000));
    assert_eq!(joule!(1.0), millijoule!(1000.0));
    assert_eq!(kilojoule!(1.0), joule!(1000.0));

    // units_test.cpp:33-35: the literals agree with the named conversions.
    let canonical: Energy = millijoule!(1);
    assert_eq!(canonical, from_millijoule(1_i64));
    assert_eq!(joule!(1), from_joule(1_i64));
    assert_eq!(kilojoule!(1), from_kilojoule(1_i64));
}

#[test]
fn energy_conversions_truncate_and_clamp() {
    // units.h:529-545: whole joules, then whole kilojoules.
    assert_eq!(to_millijoule(joule!(1)), 1000);
    assert_eq!(to_joule(from_millijoule(1500_i64)), 1);
    assert_eq!(to_kilojoule(from_millijoule(1_999_000_i64)), 1);

    // units.h:508-516 clamps at `max / 1000` before scaling, so a huge input
    // stops at the largest multiple of 1000 that still fits.
    assert_eq!(
        to_millijoule(from_joule(i64::MAX)),
        (i64::MAX / 1000) * 1000
    );
}

#[test]
fn power_units_have_correct_ratios() {
    // units_test.cpp:46-48.
    let canonical: Power = milliwatt!(1);
    assert_eq!(canonical, from_milliwatt(1_i64));
    assert_eq!(watt!(1), from_watt(1_i64));
    assert_eq!(kilowatt!(1), from_kilowatt(1_i64));
}

#[test]
fn money_units_have_correct_ratios() {
    // units_test.cpp:25-28.
    assert_eq!(usd!(1), cent!(100));
    assert_eq!(kusd!(1), usd!(1000));
    assert_eq!(usd!(1.0), cent!(100.0));
    assert_eq!(kusd!(1.0), usd!(1000.0));

    // Integer literals produce the canonical `money` type from units_fwd.h:53.
    let one_dollar: Money = usd!(1);
    assert_eq!(one_dollar, from_cent(100));
}

#[test]
fn money_conversions_truncate_toward_zero() {
    // units.h:614-630: cents are exact; dollars and kilodollars are whole.
    assert_eq!(to_cent(usd!(1)), 100);
    assert_eq!(to_usd(cent!(150)), 1);
    assert_eq!(to_kusd(cent!(150_000)), 1);
    assert_eq!(to_cent(from_usd(1)), 100);
    assert_eq!(to_cent(from_kusd(1)), 100_000);
}

#[test]
fn power_conversions_truncate_toward_zero() {
    // units.h:576-592: whole watts, then whole kilowatts.
    assert_eq!(to_milliwatt(watt!(1)), 1000);
    assert_eq!(to_watt(from_milliwatt(1500_i64)), 1);
    assert_eq!(to_kilowatt(from_milliwatt(1_999_000_i64)), 1);
}

#[test]
fn length_units_have_correct_ratios() {
    // units.h:632-658: every metric prefix is a power of ten millimetres.
    assert_eq!(centimeter!(1), millimeter!(10));
    assert_eq!(meter!(1), millimeter!(1000));
    assert_eq!(kilometer!(1), meter!(1000));

    // Integer literals produce the canonical `length` type from units_fwd.h:59.
    let one_metre: Length = meter!(1);
    assert_eq!(one_metre, from_millimeter(1000));
}

#[test]
fn length_conversions_truncate_toward_zero() {
    // units.h:660-682: millimetres are exact, the larger units are whole.
    assert_eq!(to_millimeter(meter!(1)), 1000);
    assert_eq!(to_centimeter(millimeter!(15)), 1);
    assert_eq!(to_meter(millimeter!(1500)), 1);
    assert_eq!(to_kilometer(millimeter!(999_999)), 0);
}

#[test]
fn angle_units_have_correct_ratios() {
    // units_test.cpp:42-44.
    assert_eq!(core::f64::consts::PI * radians!(1), pi_radians!(1));
    assert_eq!(pi_radians!(2), degrees!(360));
    assert_eq!(arcmin!(60), degrees!(1));

    // Integer literals produce the canonical `angle` type from units_fwd.h:65,
    // a double quantity of radians.
    let quarter_turn: Angle = degrees!(90);
    assert_eq!(quarter_turn, from_radians(core::f64::consts::FRAC_PI_2));
}

#[test]
fn angles_feed_trig_functions() {
    // units_test.cpp:309-317.
    assert_eq!(sin(radians!(0)), 0.0);
    assert!((sin(pi_radians!(0.5)) - 1.0).abs() < 1e-12);
    assert!((sin(degrees!(270)) + 1.0).abs() < 1e-12);
    assert!((cos(pi_radians!(1)) + 1.0).abs() < 1e-12);
    assert!((cos(degrees!(360)) - 1.0).abs() < 1e-12);
    assert_eq!(atan2(0.0, -1.0), pi_radians!(1));
    assert_eq!(atan2(0.0, 1.0), radians!(0));
    assert_eq!(atan2(1.0, 0.0), degrees!(90));
    assert_eq!(atan2(-1.0, 0.0), -degrees!(90));
}

#[test]
fn rounding_to_the_nearest_multiple() {
    // units_test.cpp:322-332, against `round_to_multiple_of`
    // (`src/units_utility.h:40-45`): `std::lround`, so ties go away from zero.
    for (angle, expected) in [
        (0, 0),
        (1, 0),
        (7, 0),
        (8, 15),
        (15, 15),
        (360, 360),
        (-1, 0),
        (-7, 0),
        (-8, -15),
        (-15, -15),
        (-360, -360),
    ] {
        assert_eq!(
            round_to_multiple_of(degrees!(angle), degrees!(15)),
            degrees!(expected),
            "rounding {angle} degrees to the nearest 15"
        );
    }
}

#[test]
fn energy_display_picks_whole_units() {
    // units_test.cpp:404-412, through `units::display` (`src/units.cpp:161-176`).
    assert_eq!(display_energy(from_millijoule(1)), "1 mJ");
    assert_eq!(display_energy(from_millijoule(1000)), "1 J");
    assert_eq!(display_energy(from_millijoule(1001)), "1001 mJ");
    assert_eq!(display_energy(from_millijoule(1_000_000)), "1 kJ");
    assert_eq!(display_energy(from_millijoule(1_000_001)), "1 kJ");
    assert_eq!(display_energy(from_millijoule(1_001_000)), "1001 J");
    assert_eq!(display_energy(from_millijoule(1_001_001)), "1001001 mJ");
    assert_eq!(
        display_energy(from_millijoule(2_147_483_648)),
        "2147483648 mJ"
    );
    assert_eq!(
        display_energy(from_millijoule(4_294_967_296)),
        "4294967296 mJ"
    );
}

#[test]
fn quantities_add_and_subtract_in_their_base_unit() {
    // units.h:98-147: addition, subtraction and negation of one dimension.
    assert_eq!(joule!(1) + millijoule!(1), from_millijoule(1001_i64));
    assert_eq!(kilojoule!(1) - joule!(500), joule!(500));
    assert_eq!(-(joule!(1)), from_joule(-1_i64));

    let mut total = gram!(1);
    total += milligram!(500);
    assert_eq!(total, from_milligram(1500_i64));
    total -= gram!(1);
    assert_eq!(total, milligram!(500));
}

#[test]
fn quantities_scale_by_a_scalar() {
    // units.h:199-266: scalar on either side, and division by a scalar.
    assert_eq!(joule!(1) * 4, joule!(4));
    assert_eq!(4 * joule!(1), joule!(4));
    assert_eq!(kilojoule!(1) / 4, joule!(250));
    assert_eq!(radians!(1) / 2.0, radians!(0.5));
}

/// Catch2's `Approx` as used by the reference's temperature cases: the stored
/// type is `float` (`units_fwd.h:31`), so 1e-3 is the margin the reference
/// allows at this magnitude.
fn approx(actual: f32, expected: f32) -> bool {
    (actual - expected).abs() <= 1e-3
}

#[test]
fn temperatures_in_different_units_match() {
    // units_test.cpp:338-348.
    assert!(approx(to_kelvin(from_kelvin(273.150)), 273.150));
    assert!(approx(to_kelvin(from_celsius(0.0)), 273.150));
    assert!(approx(to_kelvin(from_fahrenheit(32.0)), 273.150));

    // `units::from_celsius( 0 )` with an integer argument.
    assert!(approx(to_kelvin(from_celsius(0.0)), 273.150));
    assert!(approx(to_kelvin(from_fahrenheit(32.0)), 273.150));

    // `units_test.cpp:345` writes this as `273.150_K`.
    let absolute: Temperature = from_kelvin(273.150);
    assert!(approx(to_kelvin(absolute), 273.150));

    assert!(approx(to_fahrenheit(from_kelvin(100.0)), -279.67));
    assert!(approx(to_celsius(from_kelvin(100.0)), -173.15));
}

#[test]
fn temperature_deltas_convert_between_units() {
    // units_test.cpp:355-370: a delta has no offset, only the scale differs.
    assert_eq!(to_kelvin_delta(from_kelvin_delta(10.0)), 10.0);
    assert_eq!(to_kelvin_delta(from_celsius_delta(10.0)), 10.0);
    assert_eq!(to_kelvin_delta(from_fahrenheit_delta(18.0)), 10.0);

    assert_eq!(to_kelvin_delta(from_kelvin_delta(0.0)), 0.0);
    assert_eq!(to_kelvin_delta(from_celsius_delta(0.0)), 0.0);
    assert_eq!(to_kelvin_delta(from_fahrenheit_delta(0.0)), 0.0);

    assert_eq!(to_kelvin_delta(from_kelvin_delta(-10.0)), -10.0);
    assert_eq!(to_kelvin_delta(from_celsius_delta(-10.0)), -10.0);
    assert_eq!(to_kelvin_delta(from_fahrenheit_delta(-18.0)), -10.0);

    assert_eq!(to_kelvin_delta(from_kelvin_delta(100.1)), 100.1);
    assert_eq!(to_kelvin_delta(from_celsius_delta(100.1)), 100.1);
    assert!(approx(
        to_kelvin_delta(from_fahrenheit_delta(180.18)),
        100.1
    ));

    // The reverse directions from units.h:472-484.
    assert_eq!(to_celsius_delta(from_celsius_delta(10.0)), 10.0);
    assert_eq!(to_fahrenheit_delta(from_fahrenheit_delta(18.0)), 18.0);

    // units_test.cpp:374-377: subtracting two absolute temperatures.
    assert_eq!(
        to_kelvin_delta(from_kelvin(100.0) - from_kelvin(90.0)),
        10.0
    );
    assert_eq!(to_kelvin_delta(from_kelvin_delta(10.0)), 10.0);
    assert_eq!(to_kelvin_delta(from_celsius_delta(10.0)), 10.0);
    assert_eq!(to_kelvin_delta(from_fahrenheit_delta(18.0)), 10.0);
}

#[test]
fn temperature_plus_delta_stays_absolute() {
    // units_test.cpp:380-390.
    assert_eq!(to_kelvin(from_kelvin(10.0) + from_kelvin_delta(10.0)), 20.0);
    assert_eq!(to_kelvin(from_kelvin_delta(10.0) + from_kelvin(10.0)), 20.0);
    assert_eq!(
        to_celsius(from_celsius(10.0) + from_celsius_delta(10.0)),
        20.0
    );
    assert!(approx(
        to_fahrenheit(from_fahrenheit(10.0) + from_fahrenheit_delta(10.0)),
        20.0
    ));

    let mut temp = from_kelvin(22.5);
    temp += from_kelvin_delta(10.3);
    assert_eq!(to_kelvin(temp), 32.8);
}

#[test]
fn specific_energy_round_trips() {
    // units_test.cpp:394-399; the template keeps the argument's own value type.
    assert_eq!(to_joule_per_gram(from_joule_per_gram(100)), 100);
    // `units_test.cpp:398` passes `100.1`, a double, and compares it with
    // `Approx`; the template stores and returns it unchanged, so it is exact.
    assert_eq!(to_joule_per_gram(from_joule_per_gram(100.1)), 100.1);
}
