//! Quantities written as text, which is how the content states weight, volume
//! and price.
//!
//! Reference: `read_from_json_string_common` (`units.h:1207-1265`). The grammar
//! is stricter than it looks: every term is a whole number followed by a unit,
//! terms add, and a decimal point is not a number.

use cdda_core_types::units::{read_mass, read_money, read_volume};

#[test]
fn a_term_is_a_whole_number_and_a_unit() {
    assert_eq!(read_mass("1 kg"), Ok(1_000_000));
    assert_eq!(read_mass("250 g"), Ok(250_000));
    assert_eq!(read_mass("7 mg"), Ok(7));
}

#[test]
fn terms_add_up_and_spacing_is_optional_around_the_number() {
    assert_eq!(read_mass("1 kg 250 g"), Ok(1_250_000));
    assert_eq!(read_mass("  2 kg   "), Ok(2_000_000));
    assert_eq!(read_volume("1 L 500 ml"), Ok(1_500));
}

#[test]
fn a_sign_is_allowed_and_applies_to_its_own_term() {
    assert_eq!(read_mass("-1 g"), Ok(-1_000));
    assert_eq!(read_mass("1 kg -250 g"), Ok(750_000));
}

#[test]
fn a_decimal_point_is_not_a_number() {
    // "1.5 L" would need a fractional number, which the reference does not read.
    assert!(read_volume("1.5 L").is_err());
}

#[test]
fn a_missing_or_unknown_unit_is_an_error() {
    assert!(read_mass("12").is_err());
    assert!(read_mass("12 lb").is_err());
    assert!(read_mass("").is_err());
    assert!(read_money("3").is_err());
}

#[test]
fn money_knows_the_plural_before_the_singular() {
    // "cents" has to be read as itself, not as "cent" followed by a stray "s".
    assert_eq!(read_money("7 cents"), Ok(7));
    assert_eq!(read_money("7 cent"), Ok(7));
    assert_eq!(read_money("2 dollars"), Ok(200));
    assert_eq!(read_money("3 USD"), Ok(300));
}
