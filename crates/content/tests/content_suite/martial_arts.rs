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

/// A style is a set of **moves** as well as a set of buffs, and every move states the same kind of requirement.
///
/// Reference: `martialart::techniques` (`martialarts.h:409`) and `ma_technique` (`:162-247`). **171 techniques across the 38 styles**, which is the other half of what a fight reads: a buff
/// changes the striker, a technique changes the blow.
#[test]
fn a_style_offers_the_moves_the_data_gives_it() {
    let found = styles();
    let techniques: usize = found.all.iter().map(|style| style.techniques.len()).sum();
    assert_eq!(techniques, 171, "**the corpus gives the styles a hundred and seventy-one moves**");

    let aikido = found.all.iter().find(|style| style.id == "style_aikido").expect("aikido");
    let disarm = aikido
        .techniques
        .iter()
        .find(|technique| technique.id == "tec_aikido_dodgedisarm")
        .expect("aikido's dodge disarm");
    assert_eq!(disarm.weighting, 1, "the data's weighting is kept");
    assert!(!disarm.defensive, "a disarm is an attack rather than a counter");
    assert!(
        disarm.requirement.strictly_unarmed || disarm.requirement.unarmed_allowed,
        "**and it says what it needs in hand**, like any buff"
    );

    // A counter is marked as one, which is the flag that decides whether it fires on a hit or on a dodge.
    assert!(
        aikido.techniques.iter().any(|technique| technique.defensive),
        "aikido offers at least one counter"
    );
}

/// A technique that only fires on a critical says so, which is the flag the fight has to honour.
#[test]
fn a_technique_can_require_a_critical() {
    let found = styles();
    let critical = found
        .all
        .iter()
        .flat_map(|style| style.techniques.iter())
        .filter(|technique| technique.crit_tec)
        .count();
    assert!(critical > 0, "**some moves only fire on a critical hit**");
}

/// The flags that decide whether a move is offered at all arrive with the move.
///
/// Reference: `evaluate_technique` (`melee.cpp:1446-1550`) reads nine of them, and **the corpus uses most**: dummies that are never offered, counters that answer a dodge or a block, moves that
/// only fire on a critical, and reach moves that only work at reach.
#[test]
fn every_flag_that_decides_whether_a_move_is_offered_is_read() {
    let found = styles();
    let moves: Vec<&cdda_content::martial_arts::Technique> =
        found.all.iter().flat_map(|style| style.techniques.iter()).collect();
    assert_eq!(moves.len(), 171, "a hundred and seventy-one moves");
    let count = |check: &dyn Fn(&cdda_content::martial_arts::Technique) -> bool| moves.iter().filter(|movement| check(movement)).count();

    // **The numbers the data actually carries**, not the ones a reader would guess: forty-nine moves care about a critical, thirty are counters, thirteen disarm, and the two dodge-counter
    // branches - with the dummy and ammunition ones - are **empty in this corpus**. Naming the empties is the point: a branch the data never reaches is untested behaviour, not proven behaviour.
    assert_eq!(count(&|movement| movement.crit_tec), 49, "**forty-nine moves only fire on a critical**");
    assert_eq!(count(&|movement| movement.crit_ok), 38, "and thirty-eight are offered either way");
    assert_eq!(count(&|movement| movement.defensive), 30, "thirty are counters rather than attacks");
    assert_eq!(count(&|movement| movement.disarms), 13, "thirteen take a weapon away");
    assert_eq!(count(&|movement| movement.block_counter), 2, "two answer a block");
    assert_eq!(count(&|movement| movement.reach_tec), 1, "one only works at reach");
    assert_eq!(count(&|movement| movement.reach_ok), 4, "and four work at reach as well as close");
    assert_eq!(count(&|movement| movement.take_weapon), 1, "one takes the weapon for the striker");
    assert_eq!(count(&|movement| movement.dummy), 0, "**no style offers a dummy in this corpus**");
    assert_eq!(count(&|movement| movement.dodge_counter), 0, "and none answers a dodge");
    assert_eq!(count(&|movement| movement.needs_ammo), 0, "and none wants a loaded weapon");
}

/// The moves carry what they do to a body that is not damage: effects, a stun and a knockdown.
///
/// Reference: `melee.cpp:1759-1785` applies them, and `tech_effect_data` (`martialarts.h:147-160`) states them. **The corpus's own counts**: fourteen moves leave an effect, twenty-eight stun and
/// thirty-seven knock down - and the chances are not all a hundred, which is what makes the roll worth porting.
#[test]
fn a_move_carries_what_it_does_beyond_damage() {
    let found = styles();
    let moves: Vec<&cdda_content::martial_arts::Technique> =
        found.all.iter().flat_map(|style| style.techniques.iter()).collect();

    assert_eq!(
        moves.iter().filter(|movement| !movement.effects.is_empty()).count(),
        14,
        "**fourteen moves leave an effect**"
    );
    assert_eq!(moves.iter().filter(|movement| movement.stun_dur > 0).count(), 28, "twenty-eight stun");
    assert_eq!(moves.iter().filter(|movement| movement.down_dur > 0).count(), 37, "and thirty-seven knock down");

    let effects: Vec<&cdda_content::martial_arts::MoveEffect> =
        moves.iter().flat_map(|movement| movement.effects.iter()).collect();
    // **The corpus states no chance below a hundred and no effect that survives a harmless blow**, so the two branches the reference's roll guards are empty here. That is named rather
    // than glossed: the sim's own gate exercises both with a fixture, and the data proves only the certain, damage-requiring case.
    assert!(
        effects.iter().all(|effect| effect.chance == 100),
        "**every effect in the corpus is a certainty**"
    );
    assert!(
        effects.iter().all(|effect| effect.on_damage),
        "and every one of them needs the blow to draw blood"
    );
    assert!(
        effects.iter().all(|effect| !effect.permanent),
        "and none of them is permanent"
    );
    let longest = effects.iter().map(|effect| effect.duration).max().unwrap_or(0);
    assert!(longest > 0, "the durations are read: the longest is {longest} turns");
}

/// A move says what it aims at, and **some of its aims need limbs the striker must still have**.
///
/// Reference: `attack_vector` (`martialarts.h:70-97`) and the `limb_req` check in `choose_attack_vector` (`martialarts.cpp:1495-1515`). The corpus's own counts: 141 moves carry at least one vector,
/// and a vector that needs two arms is a move a one-armed fighter cannot make.
#[test]
fn a_move_says_what_it_aims_at() {
    let found = styles();
    let moves: Vec<&cdda_content::martial_arts::Technique> =
        found.all.iter().flat_map(|style| style.techniques.iter()).collect();
    let vectors: Vec<&cdda_content::martial_arts::AttackVector> =
        moves.iter().flat_map(|movement| movement.attack_vectors.iter()).collect();

    assert_eq!(
        moves.iter().filter(|movement| !movement.attack_vectors.is_empty()).count(),
        141,
        "**a hundred and forty-one moves say what they aim at**"
    );
    assert!(!vectors.is_empty(), "and the vectors carry their own data");

    let grasp = vectors.iter().find(|vector| vector.id == "vector_grasp").expect("the grasp vector");
    assert_eq!(grasp.limbs, vec!["hand_l".to_owned(), "hand_r".to_owned()], "a grasp aims at both hands");
    assert_eq!(
        grasp.limb_req,
        vec![cdda_content::martial_arts::VectorNeed { kind: "arm".to_owned(), count: 2 }],
        "**and needs two arms to do it**"
    );

    // The kinds the data names are the body's own kinds, which is what makes the requirement readable.
    let kinds: std::collections::BTreeSet<&str> = vectors
        .iter()
        .flat_map(|vector| vector.limb_req.iter().map(|need| need.kind.as_str()))
        .collect();
    assert!(
        kinds.iter().all(|kind| ["head", "torso", "sensor", "mouth", "arm", "hand", "leg", "foot", "wing", "tail", "other"].contains(kind)),
        "**every kind is one of the body's own**: {kinds:?}"
    );
}
