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

/// How many rounds a shot spends: one for almost every gun, and a hundred for the two that fire a whole belt.
///
/// Reference: `item::ammo_required()` (`item.cpp:11535-11550`) - the type's `ammo_to_fire` times its gunmods' multiplier plus their modifier, none of which the port has, so it is the
/// type's own figure. The gate's **three-value** distribution is the data's: a port that assumed one round a shot would be wrong for a machine gun by two orders of magnitude.
#[test]
fn a_shot_spends_the_rounds_the_gun_s_own_data_says() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/items/guns.json");
    let bytes = std::fs::read(path).expect("the guns dump exists");
    let value = cdda_json::parse(&bytes).expect("valid JSON");
    let guns = cdda_content::guns::load_dump(&value).expect("the dump loads");

    let count = |rounds: i32| guns.iter().filter(|gun| gun.ammo_to_fire == rounds).count();
    assert_eq!(count(1), 403, "almost every gun spends one round a shot");
    assert_eq!(count(100), 2, "two spend a hundred");
    assert_eq!(count(50), 1, "and one spends fifty");
    assert_eq!(
        403 + 2 + 1,
        guns.len(),
        "which is every gun in the dump, so no gun states a figure this gate has not seen"
    );
    let rifle = guns.iter().find(|gun| gun.id == "ruger_mini").expect("the Mini-14");
    assert_eq!(rifle.ammo_to_fire, 1, "a rifle spends one round");
}

/// Every gun's sights, which is the number the aiming rules were waiting for.
///
/// Reference: `gun_type::sight_dispersion`, read by `item::sight_dispersion` (`item.cpp:11134-11160`) and made worse by the shooter's eyes in `Character::effective_dispersion`
/// (`character.cpp:941-944`). The gate pins the data's own spread rather than a single figure: **a rifle's 40, a bow's 200 or more**, and the floor of the distribution, because the aiming
/// rules divide by these numbers and a zero would be a division by nothing.
#[test]
fn every_gun_states_the_error_its_sights_add() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/items/guns.json");
    let bytes = std::fs::read(path).expect("the guns dump exists");
    let value = cdda_json::parse(&bytes).expect("valid JSON");
    let guns = cdda_content::guns::load_dump(&value).expect("the dump loads");

    let sight = |id: &str| guns.iter().find(|gun| gun.id == id).expect("the gun is in the dump").sight_dispersion;
    assert_eq!(sight("ruger_mini"), 40, "a Mini-14's iron sights are forty");
    assert!(sight("longbow") >= 200, "and a longbow's are at least two hundred: {}", sight("longbow"));

    // The distribution, so a loader that dropped the column would fail even if two guns happened to read zero.
    let distinct: std::collections::BTreeSet<i32> = guns.iter().map(|gun| gun.sight_dispersion).collect();
    assert!(distinct.len() > 5, "the data states many different sights: {} distinct values", distinct.len());
    // The lowest is **ten**, not zero: every gun in the reference's data states some sight error, so the aiming rules' zero case is a hand-built one rather than a data one.
    assert_eq!(distinct.iter().copied().min(), Some(10), "and the lowest is ten - no gun states no sights at all");
    assert!(
        guns.iter().filter(|gun| gun.sight_dispersion > 0).count() > 300,
        "most guns do state one: {}",
        guns.iter().filter(|gun| gun.sight_dispersion > 0).count()
    );
}

/// Every gun states how many shots a trigger pull makes, and most of them are one.
///
/// Reference: `islot_gun::modes` (`itype.h:826`) always holds a `DEFAULT` entry, and its `qty` is `item::burst_size` (`item.cpp:11640-11650`). **This is the column round 1167 named as
/// missing**: a burst is the gun's own property and not a number a scenario should have to invent.
#[test]
fn every_gun_states_how_many_shots_one_trigger_pull_makes() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/items/guns.json");
    let bytes = std::fs::read(path).expect("the guns dump exists");
    let value = cdda_json::parse(&bytes).expect("valid JSON");
    let guns = cdda_content::guns::load_dump(&value).expect("the dump loads");

    let burst = |id: &str| guns.iter().find(|gun| gun.id == id).expect("the gun is in the dump").burst;
    assert_eq!(burst("ruger_mini"), 1, "a Mini-14 is single-shot");
    let automatic = guns.iter().filter(|gun| gun.burst > 1).count();
    assert!(automatic >= 20, "and the data has automatics: {automatic} of {} guns", guns.len());
    assert!(guns.iter().all(|gun| gun.burst >= 1), "no gun fires fewer than one shot a pull");
    let most = guns.iter().map(|gun| gun.burst).max();
    assert_eq!(most, Some(50), "the highest is fifty, which is the data's own ceiling: {most:?}");
    assert!(
        guns.iter().filter(|gun| gun.burst == 1).count() > 300,
        "and single-shot is the common case: {}",
        guns.iter().filter(|gun| gun.burst == 1).count()
    );
}
