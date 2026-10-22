//! The vehicle parts nobody wrote down.
//!
//! Reference: `vehicles::parts::finalize()` (`src/veh_type.cpp:565-640`). After the `vehicle_part` entries have been loaded, the reference walks **every
//! item** and invents a part for each one that could be bolted to a mount:
//!
//! ```text
//! turret_<item id>, based on the generic turret part, with the gun's item as its base
//! ```
//!
//! The filter (`mountable_gun_filter`, `:473-491`) is: it has gun data, it is **not** a gun mod, and it carries none of six flags - a bionic weapon, something
//! that says `NO_TURRET`, something that cannot be unwielded, a pseudo item, a reload-and-shoot weapon or a strength-drawn bow.
//!
//! This is why the port's `vehicle_part` set was 462 against the reference's 701: the 239 parts nobody wrote down are exactly the mountable guns.

#![allow(clippy::expect_used, clippy::panic)]

use cdda_content::vehicles::turret_parts;

#[test]
fn the_reference_invents_a_part_for_every_mountable_gun() {
    let items = crate::corpus::items_loaded();
    let turrets = turret_parts(&items);
    assert_eq!(
        turrets.len(),
        239,
        "the count the reference's own registry implies"
    );
    assert!(
        turrets.iter().all(|part| part.starts_with("turret_")),
        "they are all named after the gun"
    );
}

#[test]
fn the_six_flags_that_disqualify_a_gun_are_the_references_own() {
    // A real gun from the corpus, then the same gun with one flag added at a time: each of the six takes the part away, and so does being a gun mod or not being
    // a gun at all. Mutating a loaded item rather than writing JSON by hand keeps the test honest about what a flag *is* - it is a set the loader filled.
    let items = crate::corpus::items_loaded();
    let Some(plain) = items
        .iter()
        .find(|item| {
            item.is_gun && !item.is_gunmod && !turret_parts(std::slice::from_ref(*item)).is_empty()
        })
        .cloned()
    else {
        panic!(
            "the corpus holds a gun that can be mounted: the first gun by load order is a reload-and-shoot pseudo weapon"
        );
    };
    let expected = format!("turret_{}", plain.id);
    assert_eq!(
        turret_parts(std::slice::from_ref(&plain)),
        vec![expected.clone()]
    );

    for flag in [
        "BIONIC_WEAPON",
        "NO_TURRET",
        "NO_UNWIELD",
        "PSEUDO",
        "RELOAD_AND_SHOOT",
        "STR_DRAW",
    ] {
        let mut disqualified = plain.clone();
        disqualified.flags.insert(flag.to_owned());
        assert!(
            turret_parts(&[disqualified]).is_empty(),
            "{flag} takes the turret part away"
        );
    }

    let mut gunmod = plain.clone();
    gunmod.is_gunmod = true;
    assert!(
        turret_parts(&[gunmod]).is_empty(),
        "a gun mod mounts nothing"
    );

    let mut not_a_gun = plain.clone();
    not_a_gun.is_gun = false;
    assert!(
        turret_parts(&[not_a_gun]).is_empty(),
        "no gun data, no turret"
    );
}
