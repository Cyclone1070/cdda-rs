//! What a material resists, which is what an item's armour is made of.
//!
//! Reference: `material::resist( damage_type )` (`src/material.h`), fed by the `resist` object's `bash` and `cut`
//! entries in `materials.json`. `item::_resist` (`item.cpp:9199-9231`) multiplies that by the armour entry's
//! thickness per part, so a material's number is the first factor of every protection in the game - and a wrong
//! default here would make every garment either useless or invulnerable.

#![allow(clippy::panic, clippy::expect_used, clippy::float_cmp)]

use cdda_content::materials::Material;

use crate::corpus;

fn find(materials: &[Material], id: &str) -> Material {
    materials
        .iter()
        .find(|material| material.id == id)
        .unwrap_or_else(|| panic!("the data has no {id}"))
        .clone()
}

#[test]
fn a_metal_resists_more_than_flesh() {
    let materials = corpus::materials_loaded();
    assert!(
        materials.len() > 20,
        "the data holds {} materials, which is too few",
        materials.len()
    );

    let steel = find(&materials, "steel");
    assert_eq!(
        steel.bash_resist, 6.0,
        "steel resists six per millimetre of bash"
    );
    assert_eq!(steel.cut_resist, 6.0, "and six of cut");

    let flesh = find(&materials, "flesh");
    assert_eq!(
        flesh.bash_resist, 1.0,
        "flesh resists one per millimetre of bash"
    );
    assert_eq!(flesh.cut_resist, 1.0, "and one of cut");
    assert!(
        flesh.bash_resist < steel.bash_resist,
        "which is a sixth of steel"
    );
}

#[test]
fn a_material_that_states_no_resistance_resists_nothing() {
    let materials = corpus::materials_loaded();
    let insta = materials
        .iter()
        .find(|material| material.bash_resist == 0.0 && material.cut_resist == 0.0);
    assert!(
        insta.is_some(),
        "some material in the data resists nothing, which is what a missing `resist` entry must look like"
    );
}
