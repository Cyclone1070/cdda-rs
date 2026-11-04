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

/// Every round's own blow loads, and it is the blow a firearm does not carry.
///
/// Reference: `islot_ammo::damage` (`itype.h:1090-1101`), dumped by `--dump-ammo`. **The gun table's damage is zero for every firearm** (round 1146), so this table is where a shot's blow
/// actually lives - and the gate says so with a number rather than with a sentence.
#[test]
fn every_round_s_own_blow_loads_and_it_is_the_blow_a_gun_does_not_carry() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/items/ammo.json");
    let bytes = std::fs::read(path).expect("the ammo dump exists");
    let value = cdda_json::parse(&bytes).expect("valid JSON");
    let ammo = cdda_content::ammo::load_dump(&value).expect("the dump loads");
    assert_eq!(ammo.len(), 736, "every round the reference has");

    let round = |id: &str| {
        ammo.iter().find(|round| round.id == id).expect("the round is in the dump")
    };
    let rifle = round("223");
    assert_eq!(rifle.damage_bullet, 39, "a .223 round is thirty-nine bullet");
    assert_eq!(rifle.count, 1, "and it is one projectile");
    assert!(
        rifle.shot_damage.abs() < f64::EPSILON,
        "the per-pellet figure says nothing for a rifle round - which is why the port reads `damage`"
    );

    // Birdshot: fifty bullet over twenty pellets, with three hundred arcminutes between them.
    let bird = round("shot_bird");
    assert_eq!(bird.damage_bullet, 50, "birdshot's blow");
    assert_eq!(bird.count, 20, "spread over twenty pellets");
    assert_eq!(bird.shot_spread, 300, "and three hundred arcminutes of spread");
    assert!(bird.count > rifle.count, "which is what makes it shot and not a bullet");

    // Not every round with a blow is a bullet: **429 rounds do damage and 391 of them are bullet**, and the thirty-eight that are not are exactly the ones a port would mis-handle by
    // assuming a cartridge - a thrown **rock** (seven bash), and the rockets and shells that do **stab** (a 120 mm HEAT's forty, an explosive rocket's five hundred).
    assert_eq!(ammo.iter().filter(|round| round.damage > 0.0).count(), 429, "rounds that do damage");
    assert_eq!(ammo.iter().filter(|round| round.damage_bullet > 0).count(), 391, "of which 391 are bullet");
    let rock = round("rock");
    assert_eq!(rock.damage_bash, 7, "a thrown rock is seven bash, and it is in this table because it is ammunition");
    let rocket = round("explosive_hm_rocket");
    assert_eq!(rocket.damage_stab, 500, "and an explosive rocket is five hundred stab");
}
