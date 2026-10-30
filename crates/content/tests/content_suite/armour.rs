//! What a worn thing takes off a blow.
//!
//! Reference: `item::_resist` (`item.cpp:9199-9231`). The rule has two parts that a tidier version would merge and
//! get wrong: a material only counts when the hit's roll beats its coverage, and what it contributes is its
//! resistance **times its thickness in millimetres** - not its resistance alone.

#![allow(clippy::panic, clippy::expect_used, clippy::float_cmp)]

use cdda_content::armour::{Layer, layers_on, resist_of};

use crate::corpus;

#[test]
fn a_layer_only_counts_when_the_hit_meets_it() {
    let layer = Layer {
        resist: 6.0,
        thickness: 0.5,
        cover: 60,
    };
    assert_eq!(
        resist_of(&[layer], 59),
        3.0,
        "a roll of fifty-nine beats a coverage of sixty, so steel at six per millimetre contributes three"
    );
    assert_eq!(
        resist_of(&[layer], 60),
        0.0,
        "and sixty does not: the test is `<`, so the boundary is the hit's to lose"
    );
}

#[test]
fn layers_add_and_thickness_multiplies() {
    let thin = Layer {
        resist: 6.0,
        thickness: 0.25,
        cover: 100,
    };
    let thick = Layer {
        resist: 1.0,
        thickness: 2.0,
        cover: 100,
    };
    assert_eq!(resist_of(&[thin], 0), 1.5, "a quarter of a millimetre of steel");
    assert_eq!(resist_of(&[thick], 0), 2.0, "two millimetres of flesh");
    assert_eq!(resist_of(&[thin, thick], 0), 3.5, "and both together");
    assert_eq!(resist_of(&[], 0), 0.0, "nothing in the way protects nothing");
}

#[test]
fn a_real_garment_is_worth_what_its_materials_and_thickness_say() {
    let materials = corpus::materials_loaded();
    let items = corpus::items_loaded();
    // Every garment in the data that covers a torso with a material the loader describes.
    let mut checked = 0;
    for item in &items {
        let layers = layers_on(item, &materials, "torso", false);
        if layers.is_empty() {
            continue;
        }
        checked += 1;
        let total = resist_of(&layers, 0);
        assert!(
            total >= 0.0,
            "{} protects {total}, which cannot be negative",
            item.id
        );
        // The sum must be exactly what the layers say, layer by layer - a garment that reported a different
        // number here would be one whose protection and whose data disagree.
        let expected: f64 = layers
            .iter()
            .map(|layer| layer.resist * f64::from(layer.thickness))
            .sum();
        assert_eq!(total, expected, "{} does not add up", item.id);
    }
    assert!(
        checked >= 50,
        "only {checked} garments cover a torso, which is too few to prove anything"
    );
}
