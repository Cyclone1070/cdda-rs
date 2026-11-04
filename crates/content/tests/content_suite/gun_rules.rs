//! The gun table: what the reference resolved for every weapon that can be fired.
//!
//! Reference: `itype::gun` (`itype.h`), dumped by `--dump-guns` after the reference applied inheritance, mods and overrides.

#![allow(clippy::expect_used)]

/// Every gun in the oracle's dump loads, and the corpus's own numbers come through unchanged.
///
/// Reference: `itype::gun` (`itype.h`) as the reference resolved it, dumped by `--dump-guns`. The gate counts what it saw, because a loader that quietly returned nothing would
/// otherwise pass every claim about the guns it read.
#[test]
fn the_gun_dump_loads_every_gun_and_keeps_its_numbers() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/items/guns.json");
    let bytes = std::fs::read(path).expect("the guns dump exists");
    let value = cdda_json::parse(&bytes).expect("the dump is valid JSON");
    let guns = cdda_content::guns::load_dump(&value).expect("the dump loads");
    assert_eq!(guns.len(), 406, "every gun the reference dumped");
    // Version 2 added the weapon's own blow, and the column says something the port needed to learn: **a firearm's own damage is zero**, because a gun's blow is its cartridge's. Bows are the
    // exception and state theirs - a longbow's six stab is not ammunition's, it is the limbs'.
    assert_eq!(guns.iter().filter(|gun| gun.damage > 0.0).count(), 102, "102 weapons carry a blow of their own");
    assert_eq!(guns.iter().filter(|gun| gun.damage == 0.0).count(), 266, "and the rest - every firearm among them - leave it to their ammo");
    // Thirty-eight state a **negative** blow, which is the data's way of saying "less than nothing": a .22 pistol and a thrown rock are among them. A port that clamped the sum at zero
    // would silently make those weapons as good as a fist.
    assert_eq!(
        guns.iter().filter(|gun| gun.damage < 0.0).count(),
        38,
        "38 weapons state a negative blow, and the loader keeps the sign"
    );

    // One row in full, against the dump's own numbers: a loader that renamed or reordered a column would pass a count and fail this.
    let launcher = guns
        .iter()
        .find(|gun| gun.id == "barb_launcher")
        .expect("the corpus has a barb launcher");
    assert_eq!(launcher.dispersion, 100, "its own dispersion");
    assert_eq!(launcher.skill, "pistol", "fired with the pistol skill");
    assert_eq!(launcher.range, 12, "range twelve");
    assert_eq!(launcher.recoil, 0, "no recoil");
    assert_eq!(launcher.ammo, "barb", "and the barb it fires");

    // The data states a zero dispersion for seven guns and a **range of zero** for 271 - most of them modular guns whose parts carry the numbers, and some of them monsters' natural
    // attacks. That is the data, not the loader: the dump reports the field, and a port that "fixed" either number would be inventing one.
    assert_eq!(
        guns.iter().filter(|gun| gun.dispersion == 0).count(),
        7,
        "seven guns state no dispersion of their own"
    );
    assert_eq!(
        guns.iter().filter(|gun| gun.range == 0).count(),
        271,
        "and 271 state no range"
    );

    // The dump's skills are the reference's own, not a list this test could have guessed: six firing skills and one throw.
    let skills: std::collections::BTreeSet<&str> = guns.iter().map(|gun| gun.skill.as_str()).collect();
    for skill in ["archery", "launcher", "pistol", "rifle", "shotgun", "smg"] {
        assert!(skills.contains(skill), "the dump says a gun is fired with `{skill}`");
    }
    // And the launchers that state no ammo at all are in the count rather than dropped.
    assert!(
        guns.iter().any(|gun| gun.ammo.is_empty()),
        "a gun with no ammo type is still a gun"
    );
}
