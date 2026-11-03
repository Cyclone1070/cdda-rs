//! How fast a monster mends, from the data's own number.
//!
//! Reference: `mtype::regenerates` (`monster.cpp:4094-4103`), which is the hit points a monster regains per hour - and the two fallbacks the rule uses when a type states
//! none. The dump carries it as a float because the data writes fractions of a hit point.

#![allow(clippy::expect_used, clippy::panic)]

// The rates are the data's own and compared exactly: a rate that arrives a hair off is a loader that rounded, which is the thing this gate is for.
#![allow(clippy::float_cmp)]

use cdda_json::{Value, parse};

fn monsters() -> Vec<cdda_content::monsters::MonsterType> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/monsters/monsters.json");
    let bytes = std::fs::read(path).expect("the monster dump exists");
    let root = parse(&bytes).expect("the dump is valid JSON");
    let Value::Object(root) = root else {
        panic!("the dump's root is not an object");
    };
    let Some(Value::Array(prototypes)) = root.get("prototypes") else {
        panic!("the dump has no prototypes");
    };
    cdda_content::monsters::load_objects(prototypes).expect("the dump loads")
}

#[test]
fn a_monster_that_states_its_mending_carries_the_number_the_data_gives_it() {
    let all = monsters();
    // The dump holds 1,154 prototypes and the loader answers 1,155: it keeps the entry an id is resolved against as well as the entries the data states, which is the
    // behaviour the monster registry relies on. The number is pinned so a loader that starts dropping prototypes says so here rather than in a later test.
    assert_eq!(all.len(), 1_155, "every type the dump yields");
    let stated: Vec<&cdda_content::monsters::MonsterType> =
        all.iter().filter(|one| one.regenerates != 0.0).collect();
    // A count, not a name: if the loader stops reading the field, every rate collapses to zero and this says so.
    assert_eq!(stated.len(), 41, "types the data gives a mending rate");

    // A zombie states none, so its mending comes from the fallbacks - which is the half a loader that invented a default would get wrong.
    let zombie = all
        .iter()
        .find(|one| one.id == "mon_zombie")
        .expect("the corpus has a zombie");
    assert_eq!(zombie.regenerates, 0.0, "a zombie states no rate of its own");
    assert!(
        zombie.materials.keys().any(|material| material == "flesh"),
        "and it is made of flesh, which is what its fallback reads"
    );

    // The ends of the range, by name: a shoggoth mends fifty hit points an hour and a mutant a single one. Two named values are what says the field is read at all;
    // the count above says it is read **everywhere**.
    let rate_of = |id: &str| {
        all.iter()
            .find(|one| one.id == id)
            .map_or_else(|| panic!("the dump has {id}"), |one| one.regenerates)
    };
    assert_eq!(rate_of("mon_shoggoth"), 50.0, "a shoggoth's own rate");
    assert_eq!(rate_of("mon_mutant_alpha"), 1.0, "a mutant's single hit point an hour");
}
