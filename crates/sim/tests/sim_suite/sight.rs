//! Fine detail vision, against the reference's own arithmetic.

#![allow(clippy::panic, clippy::expect_used, clippy::indexing_slicing)]

/// A lamp in the hand beats the same light on the floor, and nothing beats full light.
///
/// Reference: `Character::fine_detail_vision_mod()` (`character.cpp:2916-2947`): `own_light = max( 1, 10 - active - 2 )` and `ambient = max( 1, 10 - ambient + 1 )`, and the answer is the **smaller**
/// - both are penalties, so the best light wins. **The character's own light is worth three more than ambient**, and both are floored at one, so there is no bonus for light beyond ambient.
#[test]
fn a_lamp_beats_the_floor_and_nothing_beats_full_light() {
    use cdda_sim::sight::fine_detail_vision_mod;

    assert!((fine_detail_vision_mod(true, 10.0, 10.0) - 11.0).abs() < 1e-6, "**blind is eleven**, whatever the light");
    assert!(
        (fine_detail_vision_mod(false, 0.0, 10.0) - 1.0).abs() < 1e-6,
        "**full ambient light is enough on its own**: the ambient side is floored at one, and the smaller answer wins"
    );
    assert!(
        (fine_detail_vision_mod(false, 0.0, 0.0) - 8.0).abs() < 1e-6,
        "in the dark with nothing in hand, the character's own side is 10 - 0 - 2 = 8, which beats the ambient 11"
    );
    assert!((fine_detail_vision_mod(false, 10.0, 0.0) - 1.0).abs() < 1e-6, "a bright lamp: floored at one, never zero");
    assert!(
        (fine_detail_vision_mod(false, 10.0, 10.0) - 1.0).abs() < 1e-6,
        "full light both ways is the best possible and not better"
    );
    // The three-point advantage, stated directly: the same brightness does more good in the hand.
    let in_hand = fine_detail_vision_mod(false, 5.0, 0.0);
    let on_floor = fine_detail_vision_mod(false, 0.0, 5.0);
    assert!(in_hand < on_floor, "**light you carry is better than light you stand in**: {in_hand} against {on_floor}");
}
