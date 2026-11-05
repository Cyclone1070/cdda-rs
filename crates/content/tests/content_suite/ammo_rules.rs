#![allow(clippy::expect_used)]


/// Every round states the ammunition type it belongs to, and that type is **not** its own id for most of them.
///
/// Reference: `islot_ammo::type` (`itype.h:1063`). The distinction is the one that broke the CLI's `shoot` step the moment a shotgun was tried: a gun names its ammunition type (`"shot"`), the
/// rounds table is keyed by round id (`shot_bird`), and the two agree only where the data happens to make them equal - `223` is one such accident.
#[test]
fn every_round_states_the_ammunition_type_it_belongs_to() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/items/ammo.json");
    let bytes = std::fs::read(path).expect("the ammo dump exists");
    let value = cdda_json::parse(&bytes).expect("valid JSON");
    let rounds = cdda_content::ammo::load_dump(&value).expect("the dump loads");

    let of = |id: &str| rounds.iter().find(|round| round.id == id).expect("the round is in the dump").ammo_type.clone();
    assert_eq!(of("shot_bird"), "shot", "birdshot is a `shot` round");
    assert_eq!(of("shot_00"), "shot", "and so is buckshot");
    assert_eq!(of("223"), "223", "while a .223's type happens to share its id");

    // Counts prove the gate is not looking at a handful: every round states a type, and most of those types carry more than one round.
    let mut by_type: std::collections::BTreeMap<String, usize> = std::collections::BTreeMap::new();
    for round in &rounds {
        assert!(!round.ammo_type.is_empty(), "round `{}` states no ammunition type", round.id);
        *by_type.entry(round.ammo_type.clone()).or_default() += 1;
    }
    assert_eq!(by_type.values().sum::<usize>(), rounds.len(), "every round counted once");
    let shared = by_type.values().filter(|count| **count > 1).count();
    assert!(shared > 50, "most ammunition types have several rounds: {shared} types of {}", by_type.len());
    assert_eq!(by_type.get("shot").copied(), Some(rounds.iter().filter(|round| round.ammo_type == "shot").count()).filter(|count| *count > 1), "and `shot` is one of the crowded ones");
}
