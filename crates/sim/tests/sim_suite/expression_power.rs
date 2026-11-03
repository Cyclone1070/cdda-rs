//! The caret in the data's expressions is a **power**, and it binds and associates the way the reference's parser does.
//!
//! Reference: `binary_ops` (`math_parser_impl.h:328-344`): `< <= > >= == !=` at one, `+ -` at two, `* / %` at three, `^` at **four**, and `^` is the only one of them
//! that associates to the **right**. `math_opers::math_pow` is `std::pow`. The data writes `((-1) ^ rand(1))` in `traps.json`, which reads as a random sign rather than
//! as a bitwise exclusive or - and a port that read it as an operator it did not know refused the whole effect.

#![allow(clippy::panic, clippy::expect_used)]

use bevy_ecs::prelude::*;

/// One expression, and what it comes to.
fn value_of(text: &str) -> f64 {
    let mut world = World::new();
    let speaker = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
    let Some(expression) = cdda_content::dialogue::read_text_expression(text) else {
        panic!("the port reads {text}");
    };
    cdda_sim::conditions::value(&world, speaker, None, &expression).unwrap_or(f64::NAN)
}

#[test]
fn the_caret_is_a_power_and_binds_tighter_than_a_product() {
    assert!((value_of("2 ^ 3") - 8.0).abs() < f64::EPSILON, "two cubed is eight");
    assert!(
        (value_of("2 * 3 ^ 2") - 18.0).abs() < f64::EPSILON,
        "and it binds tighter than a product: 2 * 9, not 36"
    );
    assert!(
        (value_of("2 ^ 3 ^ 2") - 512.0).abs() < f64::EPSILON,
        "and associates to the right: 2 ^ 9, not 8 ^ 2"
    );
    assert!(
        (value_of("(-1) ^ 1") + 1.0).abs() < f64::EPSILON,
        "which is how the data makes a random sign"
    );
}

/// The data's ternary chooses a branch, and only the branch it takes is evaluated.
///
/// Reference: `?` and `:` are in `binary_ops` at precedence **zero** (`math_parser_impl.h:329-330`), right-associative - and the reference's parser rewrites a nested
/// ternary into parens on the way (`math_parser.cpp:678-682`), so `0?0?-1:-2:1` groups as `0?(0?-1:-2):1` and comes to **one**. The data writes
/// `BILE_STINK_DURATION = u_effect_duration('bile_stink') > 0 ? … : 600`, which is the shape a port must carry.
#[test]
fn a_ternary_chooses_one_branch() {
    assert!((value_of("1 ? 5 : 7") - 5.0).abs() < f64::EPSILON, "a true test takes the first branch");
    assert!((value_of("0 ? 5 : 7") - 7.0).abs() < f64::EPSILON, "and a false one the second");
    assert!(
        (value_of("0 ? 0 ? -1 : -2 : 1") - 1.0).abs() < f64::EPSILON,
        "and a nested ternary groups the way the reference rewrites it"
    );
    assert!(
        (value_of("2 * 3 > 5 ? 10 : 20") - 10.0).abs() < f64::EPSILON,
        "and it is looser than every comparison: (2 * 3 > 5) ? 10 : 20"
    );
}

/// A duration rolled between two ends the world works out.
///
/// Reference: `get_duration_or_var` reads each end of a range as a `dbl_or_var` (`npctalk.cpp:3426-3450`), so `["1 minutes", {"math": [...]}]` is a range whose top the
/// storm's own strength decides - which is how the portal storms set their light's length.
#[test]
fn a_range_may_have_computed_ends() {
    let text = r#"{"custom_light_level": 125, "length": ["1 minutes", {"math": ["60 * 2"]}]}"#;
    let value = cdda_json::parse(text.as_bytes()).expect("json");
    let Some(effect) = cdda_content::dialogue::read_effect(&value) else {
        panic!("the port reads a range with computed ends");
    };
    let cdda_content::dialogue::Effect::CustomLightLevel { length, .. } = effect else {
        panic!("it reads as the light effect");
    };
    assert!(
        matches!(
            length,
            cdda_content::dialogue::EffectDuration::RangeComputed { .. }
        ),
        "and the range is the computed one: {length:?}"
    );
}
