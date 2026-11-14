//! **A hit on a vehicle part is three rules in a row, and two of them surprise.**
//!
//! Reference: `vehicle::damage_direct` (`vehicle.cpp:7925-7945`). A blow smaller than a **threshold** does nothing at all - not less, nothing - because a puncture-sized
//! hit on a car door is a scratch the game does not track; a blow at or over it has the part's flat **armour** subtracted; and only then are hit points lost. `pure` damage is
//! the exception to both: it goes through the threshold and past the armour, which is why the data marks its type `no_resist`.
//!
//! The numbers below are the reference's, and the parts are the dump's: the seat has 300 hit points, so its threshold is `min(20, 300/10) = 20`, and it shaves 5 off a bash.

#![allow(clippy::expect_used, clippy::panic)]

use cdda_content::vehicles::PartSpec;
use cdda_sim::vehicle_damage::{Outcome, hit};

fn part(durability: i32, bash_armour: f32) -> PartSpec {
    let mut spec = PartSpec {
        id: "test_part".to_owned(),
        durability,
        ..PartSpec::default()
    };
    spec.damage_reduction.insert("bash".to_owned(), bash_armour);
    spec.damage_reduction.insert("pure".to_owned(), 0.0);
    spec
}

#[test]
fn a_blow_below_the_threshold_does_nothing_at_all() {
    let seat = part(300, 5.0);
    assert_eq!(
        hit(&seat, 300, 19, "bash", false),
        Outcome::Ignored,
        "**19 against a threshold of 20 is a scratch**: the reference returns the damage unconsumed and not one hit point is lost"
    );
    assert_eq!(
        hit(&seat, 300, 20, "bash", false),
        Outcome::Applied {
            net: 15,
            hp_after: 285,
            broke: false
        },
        "at the threshold the armour comes off: 20 - 5 = 15, so 285 hit points remain"
    );
}

#[test]
fn a_thin_part_has_a_threshold_of_a_tenth_of_its_hit_points() {
    let thin = part(40, 5.0);
    // `tsh = min(20, durability / 10)`, and 40 / 10 = 4, so 3 is ignored and 4 is not.
    assert_eq!(hit(&thin, 40, 3, "bash", false), Outcome::Ignored);
    assert_eq!(
        hit(&thin, 40, 4, "bash", false),
        Outcome::Applied {
            net: 0,
            hp_after: 40,
            broke: false
        },
        "**4 is at the threshold but the armour eats all of it**: nothing is lost and the blow is still not ignored"
    );
}

#[test]
fn pure_damage_both_ignores_the_threshold_and_passes_the_armour() {
    let seat = part(300, 5.0);
    assert_eq!(
        hit(&seat, 300, 19, "pure", true),
        Outcome::Applied {
            net: 19,
            hp_after: 281,
            broke: false
        },
        "**19 pure damage is not a scratch**: `pure` is `no_resist`, and the threshold does not apply to it"
    );
    assert_eq!(
        hit(&seat, 10, 19, "pure", true),
        Outcome::Applied {
            net: 19,
            hp_after: 0,
            broke: true
        },
        "and a part taken to zero hit points is broken rather than gone"
    );
}
