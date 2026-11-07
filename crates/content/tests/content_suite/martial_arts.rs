//! What the martial arts data says each style asks for.

#![allow(clippy::panic, clippy::expect_used, clippy::indexing_slicing)]

use cdda_json::{Value, parse};
use std::path::Path;

fn styles() -> cdda_content::martial_arts::Styles {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/martial-arts/martial-arts.json");
    let text = std::fs::read(path).expect("the martial arts dump exists");
    let value = parse(&text).expect("the dump is valid JSON");
    let Value::Object(root) = value else {
        panic!("the dump's root is an object");
    };
    let Some(Value::Array(rows)) = root.get("styles") else {
        panic!("the dump has styles");
    };
    let all = rows
        .iter()
        .filter_map(|row| match row {
            Value::Object(object) => cdda_content::martial_arts::read_style(object),
            _ => None,
        })
        .collect();
    cdda_content::martial_arts::Styles { all }
}

/// **Thirty-eight styles and thirty-four of them with static buffs**, which is the count the data states and the reason the requirements matter: a style's buffs are what a fight reads.
#[test]
fn every_style_in_the_data_is_read() {
    let found = styles();
    assert_eq!(found.all.len(), 38, "the corpus holds thirty-eight styles");
    assert_eq!(
        found.all.iter().filter(|style| !style.static_buffs.is_empty()).count(),
        34,
        "and thirty-four of them state at least one static buff"
    );
}

/// Aikido's first static buff allows both an empty hand and a weapon, and states no categories - so it is in play for any weapon.
#[test]
fn a_styles_buff_states_what_it_allows() {
    let found = styles();
    let aikido = found.all.iter().find(|style| style.id == "style_aikido").expect("aikido");
    assert_eq!(aikido.leg_block, 99, "aikido blocks with a leg ninety-nine times in a hundred");
    assert_eq!(aikido.arm_block, 0, "and never with an arm");
    let first = &aikido.static_buffs[0];
    assert_eq!(first.id, "buff_aikido_static1", "the data's own order is kept");
    assert!(first.unarmed_allowed, "**the stance is worth something in an empty hand**");
    assert!(first.melee_allowed, "and with a weapon");
    assert!(first.weapon_categories.is_empty(), "with no category stated, any weapon is allowed");
}

/// A style that allows no weapon at all says so, and the port reads that rather than assuming it.
#[test]
fn a_style_can_refuse_every_weapon() {
    let found = styles();
    let unarmed = found
        .all
        .iter()
        .filter(|style| style.force_unarmed)
        .count();
    assert!(unarmed > 0, "**some styles refuse every weapon outright**");
    let strictly = found.all.iter().filter(|style| style.strictly_melee).count();
    assert!(strictly > 0, "and some insist on one");
}
