//! The caret in the data's expressions is a **power**, and it binds and associates the way the reference's parser does.
//!
//! Reference: `binary_ops` (`math_parser_impl.h:328-344`): `< <= > >= == !=` at one, `+ -` at two, `* / %` at three, `^` at **four**, and `^` is the only one of them
//! that associates to the **right**. `math_opers::math_pow` is `std::pow`. The data writes `((-1) ^ rand(1))` in `traps.json`, which reads as a random sign rather than
//! as a bitwise exclusive or - and a port that read it as an operator it did not know refused the whole effect.

#![allow(clippy::panic)]

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
