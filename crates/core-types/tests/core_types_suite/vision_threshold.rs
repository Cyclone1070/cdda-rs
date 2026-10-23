//! How much light a particular pair of eyes needs before a square stops being blank.
//!
//! Reference: `Character::get_vision_threshold( light_level )` and its helper `threshold_for_range( range )` (`src/character.cpp:2716-2763`). The number decides the
//! last branch of `apparent_light_at`: below it a square is `BLANK`, at or above it the square is `LOW`.
//!
//! Two ideas meet here. **Range** is how far this character can see, and it sets a starting threshold through the same attenuation the light itself obeys -
//! `LIGHT_AMBIENT_MINIMAL` at the edge of that range is the dimmest thing visible there. **Dimming** is that threshold getting worse as the light around the
//! character gets brighter, because a lit room makes the dark beyond it harder to see into rather than easier.

#![allow(clippy::expect_used)]

use cdda_core_types::light::{
    AMBIENT_LIT, AMBIENT_LOW, Senses, threshold_for_range, vision_threshold,
};

#[track_caller]
fn assert_close(passed: f32, expected: f32, what: &str) {
    assert!(
        (passed - expected).abs() < 1e-5,
        "{what}: got {passed}, expected {expected}"
    );
}

fn eyes() -> Senses {
    Senses {
        perception: 5.0,
        night_vision: 0.0,
        bird_eye: false,
        gnv: false,
        limb_night_vis: 1.0,
        enchantment_night_vis: 1.0,
    }
}

#[test]
fn the_threshold_for_a_range_is_just_visible_light_at_that_range() {
    // `LIGHT_AMBIENT_MINIMAL / e^( range * LIGHT_TRANSPARENCY_OPEN_AIR ) - epsilon` (`character.cpp:2716-2720`): the same attenuation light obeys, so that
    // "the dimmest thing I can see at r squares" is a coherent statement. The epsilon means the answer is just below the light that would be *exactly* visible.
    assert_close(threshold_for_range(1.0), 3.573_673_5, "one square");
    assert_close(threshold_for_range(20.0), 1.943_151, "twenty squares");
    // And it falls with distance: a longer range means a dimmer threshold, not a brighter one.
    assert!(threshold_for_range(20.0) < threshold_for_range(1.0));
}

#[test]
fn in_the_dark_the_threshold_is_the_range_alone() {
    // Dimming is 1.0 at `LIGHT_AMBIENT_MINIMAL` and rises to 2.0 at `LIGHT_AMBIENT_LIT` (`character.cpp:2733-2734`). At the dim end there is nothing to dim *for*, so
    // perception five - range 5/3 - gives the range's own threshold.
    assert_close(
        vision_threshold(3.7, &eyes()),
        3.498_161_4,
        "perception 5 sees to 5/3 squares",
    );
}

#[test]
fn a_bright_room_makes_the_dark_beyond_it_harder_to_see_into() {
    // Dimming is only observable where the product stays under the cap, which for ordinary eyes is nowhere near full light - so the character here has bionic night
    // vision, whose wider range puts the threshold low enough to watch it climb.
    let mut bionic = eyes();
    bionic.gnv = true;
    assert_close(
        vision_threshold(3.7, &bionic),
        2.538_864,
        "in the dark, the widened range alone",
    );
    assert_close(
        vision_threshold(6.0, &bionic),
        3.465_750_4,
        "and at light 6.0 the dimming has made it worse",
    );
}

#[test]
fn the_threshold_is_capped_where_everything_stops_being_lit() {
    // `std::min( LIGHT_AMBIENT_LOW, ... )` (`character.cpp:2761-2762`). Ordinary eyes reach the cap almost immediately, which is why the cap is the value most characters
    // actually use: at full daylight and at half light the answer is the same three-and-a-half, and it sits *below* `LIGHT_AMBIENT_MINIMAL` - which is the reference's own
    // oddity, kept rather than tidied.
    assert_close(
        vision_threshold(AMBIENT_LIT, &eyes()),
        AMBIENT_LOW,
        "full daylight",
    );
    assert_close(
        vision_threshold(6.0, &eyes()),
        AMBIENT_LOW,
        "half light too",
    );
}

#[test]
fn a_range_is_never_shorter_than_one_square() {
    // `range = std::max( 1.0f, range * limb_score )` (`character.cpp:2757`): a character who can barely see still sees the square they are standing on. Perception nought
    // and perception three both come out at a range of one, so they are indistinguishable here - and the cap hides the value itself, which is why this test asks about
    // the equality rather than the number.
    let blind = Senses {
        perception: 0.0,
        ..eyes()
    };
    let just_visible = Senses {
        perception: 3.0,
        ..eyes()
    };
    assert_close(
        vision_threshold(3.7, &blind),
        vision_threshold(3.7, &just_visible),
        "both clamped to one square of range",
    );
}

#[test]
fn night_vision_widens_the_range_and_so_lowers_the_threshold() {
    // The mode bonuses (`character.cpp:2736-2747`) add to the range before the clamp: +2, +4.5 or +10 depending on the mode, and bird eye adds one on top.
    let mut cat = eyes();
    cat.night_vision = 4.5;
    cat.bird_eye = true;
    let dark = 3.7;
    assert_close(
        vision_threshold(dark, &cat),
        threshold_for_range(5.0 / 3.0 + 4.5 + 1.0),
        "range 5/3 + 5.5",
    );
    assert!(
        vision_threshold(dark, &cat) < vision_threshold(dark, &eyes()),
        "and a wider range means a dimmer threshold"
    );
}

#[test]
fn the_gnv_bonus_replaces_the_enchantment_and_does_not_stack_with_it() {
    // `if( worn_with_flag( GNV_EFFECT ) || has_flag( NIGHT_VISION ) ) range += 10; else range = enchantment->modify_value(...)` (`character.cpp:2749-2753`).
    // It is a branch, not two additions: a character with both the bionic and an enchantment gets the bionic's ten squares and the enchantment is ignored.
    let mut bionic = eyes();
    bionic.gnv = true;
    bionic.enchantment_night_vis = 3.0;
    assert_close(
        vision_threshold(3.7, &bionic),
        threshold_for_range(5.0 / 3.0 + 10.0),
        "10 added, not multiplied",
    );

    let mut enchanted = eyes();
    enchanted.enchantment_night_vis = 3.0;
    assert_close(
        vision_threshold(3.7, &enchanted),
        threshold_for_range(5.0),
        "and without the bionic the enchantment multiplies instead",
    );
}
