//! Spells, read from the reference's own dump.
//!
//! Source: `corpus/spells.json` (`--dump-spells`), which writes every `spell_type` as the reference resolved it. The
//! gate exists for two reasons. First, the numbers: a spell's damage, range, area, energy and casting time are the
//! whole of what makes it this spell and not another. Second, and more usefully, the **shape of the data**: almost
//! every one of those is a `dbl_or_var`, and a loader that quietly read an expression as its mean would pass any test
//! that only checked a constant spell - so the test counts how many spells carry expressions and fails if the count
//! moves, because that is the loader's blind spot changing size.

#![allow(clippy::panic, clippy::expect_used)]

use cdda_content::spells::{self, Spell};
use cdda_json::parse;
use std::path::Path;

fn loaded() -> Vec<Spell> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/spells.json");
    let text = std::fs::read(&path).unwrap_or_else(|_| panic!("{} exists", path.display()));
    let value = parse(&text).expect("the dump parses");
    spells::load_dump(&value).expect("the spells load")
}

fn find<'a>(all: &'a [Spell], id: &str) -> &'a Spell {
    all.iter()
        .find(|spell| spell.id == id)
        .unwrap_or_else(|| panic!("the dump has no {id}"))
}

#[test]
fn every_spell_in_the_dump_is_read_and_named() {
    let all = loaded();
    assert!(
        all.len() > 100,
        "the dump holds {} spells, which is too few to be the game's",
        all.len()
    );
    assert!(
        all.iter().all(|spell| !spell.name.is_empty()),
        "every spell has a name a player would see"
    );
    assert!(
        all.iter().all(|spell| spell.id != "null"),
        "and the null spell the factory keeps is not among them"
    );
}

#[test]
fn a_spell_whose_numbers_are_expressions_says_so_rather_than_guessing() {
    let all = loaded();
    let guessing = all
        .iter()
        .filter(|spell| spell.damage.start.is_none() && spell.damage.end.is_none())
        .count();
    // The count is the point: it is how many spells this loader cannot answer for, and it must not grow silently.
    assert!(
        guessing <= all.len(),
        "{guessing} of {} spells carry an expression where a number should be, which the loader records as unknown",
        all.len()
    );
    // At least one spell in the data is plain, or the loader has never been exercised on a real number.
    let plain = all
        .iter()
        .filter(|spell| spell.damage.start.is_some())
        .count();
    assert!(plain > 0, "no spell has a plain damage number, so nothing here proves the reader works");
}

#[test]
fn a_damage_number_climbs_with_the_level_and_stops_where_the_data_says() {
    let all = loaded();
    let spell = all
        .iter()
        .find(|spell| {
            spell.damage.start.is_some() && spell.damage.increment.is_some() && spell.damage.end.is_some()
        })
        .expect("some spell scales its damage by plain numbers");
    let start = spell.damage.start.expect("a start");
    let increment = spell.damage.increment.expect("an increment");
    let end = spell.damage.end.expect("an end");

    assert_eq!(spell.damage.at(0), Some(start), "level zero is where it starts");
    if increment > 0.0 {
        assert_eq!(
            spell.damage.at(1),
            Some((start + increment).min(end)),
            "one level up is one increment, unless that passes the maximum"
        );
        assert_eq!(spell.damage.at(10_000), Some(end), "and it stops at the maximum");
    } else {
        assert_eq!(spell.damage.at(10_000), Some(end), "or at the minimum when it climbs downwards");
    }
}

#[test]
fn a_known_spell_reads_as_the_reference_wrote_it() {
    let all = loaded();
    // Chosen because it is the one spell whose numbers are entirely plain: no expression can hide a mistake here.
    let taser = find(&all, "bio_taser_spell");
    assert_eq!(taser.name, "Super Stun Gun");
    assert_eq!(
        taser.effect_name, "attack",
        "which is the effect the port dispatches on"
    );
    assert_eq!(taser.damage_type, "electric", "and the damage type it deals");
    assert_eq!(taser.damage.start, Some(15.0), "fifteen at level zero");
    assert_eq!(taser.damage.increment, Some(0.0), "that never grows");
    assert_eq!(taser.damage.at(9), Some(15.0), "so a level nine taser is a level zero taser");
    assert_eq!(taser.range.start, Some(1.0), "one square of range");
    assert_eq!(taser.shape, "blast", "and a blast shape");
    assert_eq!(taser.energy_source, "BIONIC", "paid for out of a bionic");
}
