//! What an enchantment does to a number.

#![allow(clippy::panic, clippy::expect_used, clippy::float_cmp, clippy::indexing_slicing)]

use cdda_sim::enchantments::{Enchantment, merge};

/// An enchantment adds, then multiplies what it has added to.
///
/// Reference: `enchant_cache::modify_value` (`magic_enchantment.cpp:1396-1400`) is two lines: `value += get_value_add( mod_val ); value *= 1.0 + get_value_multiply( mod_val );`
/// **The order is the rule** - a multiply of a half applied before the add would double a different number - and a key the enchantment does not mention is left exactly as it was, which is what the
/// reference's maps answer for a missing key.
#[test]
fn an_enchantment_adds_then_multiplies() {
    let mut enchantment = Enchantment::default();
    enchantment.add("STR", 2.0);
    enchantment.multiply("STR", 0.5);
    assert_eq!(enchantment.modify(10.0, "STR"), 18.0, "**ten plus two is twelve, and twelve times one and a half is eighteen**");

    assert_eq!(enchantment.modify(10.0, "DEX"), 10.0, "a number it does not mention is untouched");
    assert_eq!(Enchantment::default().modify(7.0, "STR"), 7.0, "and an enchantment with no values changes nothing");

    // An add alone multiplies by one, and a multiply alone adds nothing.
    let mut additive = Enchantment::default();
    additive.add("SPEED", 25.0);
    assert_eq!(additive.modify(100.0, "SPEED"), 125.0, "an add with no multiply is added to");
    let mut multiplicative = Enchantment::default();
    multiplicative.multiply("SPEED", 1.0);
    assert_eq!(multiplicative.modify(100.0, "SPEED"), 200.0, "**a multiply of one is a doubling**, because the reference states it as a fraction of the original");
}

/// Two rows for the same value do not sum: **the first one wins**, because the reference's loader uses `emplace`.
///
/// Reference: `load_add_and_multiply` (`magic_enchantment.cpp:204-232`): `add_map.emplace( value, add )` - a map's `emplace` inserts only when the key is absent, so a second row for the same
/// value is silently dropped rather than added to the first. The port keeps the same rule, because a port that summed the rows would answer a different number for every enchantment that names a
/// value twice.
#[test]
fn a_value_named_twice_keeps_its_first_row() {
    let mut enchantment = Enchantment::default();
    merge(&mut enchantment, "STR", Some(2.0), Some(0.5));
    merge(&mut enchantment, "STR", Some(100.0), Some(9.0));
    assert_eq!(enchantment.modify(0.0, "STR"), 3.0, "**the first row's two and a half is the one that counts**, not the second row's");
    assert_eq!(enchantment.add_of("STR"), 2.0, "the add map kept its first value");
    assert_eq!(enchantment.multiply_of("STR"), 0.5, "and so did the multiply map");
}
