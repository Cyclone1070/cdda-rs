//! What a creature has become, and what it may become next.
//!
//! Reference: `Character::can_mutate` and `Character::mutate_towards` (`mutation.cpp:540-700`). The creature holds ids; the four sets that decide a gain live in the data
//! table, which these tests build by hand so the rules are checked without a corpus.

use bevy_ecs::prelude::*;
use cdda_content::mutations::{Mutation, Mutations};

fn table(entries: &[(&str, Mutation)]) -> Mutations {
    use std::fmt::Write as _;
    let mut text = String::from("{\"mutations\":[");
    for (index, (id, one)) in entries.iter().enumerate() {
        if index > 0 {
            text.push(',');
        }
        let list = |ids: &[String]| {
            ids.iter()
                .map(|id| format!("\"{id}\""))
                .collect::<Vec<_>>()
                .join(",")
        };
        let _ = write!(
            text,
            "{{\"id\":\"{id}\",\"points\":{},\"visibility\":{},\"purifiable\":{},\"prereqs\":[{}],\"prereqs2\":[],\"cancels\":[{}],\"category\":[{}]}}",
            one.points,
            one.visibility,
            one.purifiable,
            list(&one.prereqs),
            list(&one.cancels),
            list(&one.categories)
        );
    }
    text.push_str("]}");
    let Ok(root) = cdda_json::parse(text.as_bytes()) else {
        return Mutations::default();
    };
    let Some((table, _)) = cdda_content::mutations::from_dump(&root) else {
        return Mutations::default();
    };
    table
}

fn trait_of(prereqs: &[&str], cancels: &[&str]) -> Mutation {
    Mutation {
        points: 0,
        visibility: 0,
        purifiable: true,
        prereqs: prereqs.iter().map(|one| (*one).to_owned()).collect(),
        prereqs2: Vec::new(),
        cancels: cancels.iter().map(|one| (*one).to_owned()).collect(),
        categories: Vec::new(),
    }
}

/// A mutation needs one of its prerequisites, and a name the data does not define is not a mutation.
///
/// Reference: `Character::can_mutate` (`mutation.cpp:540-575`): `prereqs` is a list of **alternatives**, so holding either is enough.
#[test]
fn a_mutation_needs_one_of_its_prerequisites() {
    let data = table(&[
        ("BEAUTIFUL", trait_of(&[], &["UGLY"])),
        ("UGLY", trait_of(&[], &["BEAUTIFUL"])),
        ("PRETTY", trait_of(&["BEAUTIFUL", "UGLY"], &[])),
    ]);
    let mut world = World::new();
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));

    assert_eq!(
        cdda_sim::mutations::can_gain(&world, survivor, &data, "PRETTY"),
        cdda_sim::mutations::Gain::MissingPrerequisite,
        "with neither prerequisite held, the gain waits"
    );
    cdda_sim::mutations::gain(&mut world, survivor, &data, "BEAUTIFUL");
    assert_eq!(
        cdda_sim::mutations::can_gain(&world, survivor, &data, "PRETTY"),
        cdda_sim::mutations::Gain::Yes,
        "and either one of them is enough"
    );
    assert_eq!(
        cdda_sim::mutations::can_gain(&world, survivor, &data, "NOT_IN_THE_DATA"),
        cdda_sim::mutations::Gain::Unknown
    );
}

/// Gaining a mutation takes away what it conflicts with, so the two are never held together.
///
/// Reference: `Character::mutate_towards` (`mutation.cpp:600-700`): the cancels are removed **before** the new trait is added.
#[test]
fn gaining_a_mutation_replaces_what_it_cancels() {
    let data = table(&[
        ("BEAUTIFUL", trait_of(&[], &["UGLY"])),
        ("UGLY", trait_of(&[], &["BEAUTIFUL"])),
    ]);
    let mut world = World::new();
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
    assert_eq!(
        cdda_sim::mutations::gain(&mut world, survivor, &data, "UGLY"),
        cdda_sim::mutations::Gain::Yes
    );
    assert!(cdda_sim::mutations::has(&world, survivor, "UGLY"));

    assert_eq!(
        cdda_sim::mutations::gain(&mut world, survivor, &data, "BEAUTIFUL"),
        cdda_sim::mutations::Gain::Yes
    );
    assert!(
        cdda_sim::mutations::has(&world, survivor, "BEAUTIFUL"),
        "the new trait is held"
    );
    assert!(
        !cdda_sim::mutations::has(&world, survivor, "UGLY"),
        "and the one it cancels is gone"
    );
    assert_eq!(
        cdda_sim::mutations::mutations_of(&world, survivor),
        vec!["BEAUTIFUL".to_owned()]
    );
}

/// A conflict that cannot be taken away stops the gain, because removing it is what gaining would otherwise do.
///
/// Reference: `Character::mutate_towards` (`mutation.cpp:1705-1712`): a cancelled trait that is not purifiable makes the function return false - "We can't cancel
/// unpurifiable mutations".
#[test]
fn a_conflict_that_cannot_be_removed_stops_the_gain() {
    let mut stuck = trait_of(&[], &[]);
    stuck.purifiable = false;
    let data = table(&[("GLASSJAW", stuck), ("PRETTY", trait_of(&[], &["GLASSJAW"]))]);
    let mut world = World::new();
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
    cdda_sim::mutations::gain(&mut world, survivor, &data, "GLASSJAW");
    assert_eq!(
        cdda_sim::mutations::can_gain(&world, survivor, &data, "PRETTY"),
        cdda_sim::mutations::Gain::Conflicting
    );
    assert_eq!(
        cdda_sim::mutations::gain(&mut world, survivor, &data, "PRETTY"),
        cdda_sim::mutations::Gain::Conflicting
    );
    assert!(cdda_sim::mutations::has(&world, survivor, "GLASSJAW"), "and nothing was taken away");
}

/// A mutation prunes the other traits of its category, but never the ones it grew out of.
///
/// Reference: `Character::mutate_towards` (`mutation.cpp:1643-1652`): `get_mutations_in_types` fills the cancel list with every held trait of the same category, and
/// the traits in `get_all_mutation_prereqs` are skipped - *"Check mutations of the same type - except for the ones we might need for pre-reqs"*.
#[test]
fn a_mutation_prunes_its_own_category_and_spares_its_prerequisites() {
    // ANTENNAE needs CHITIN, which is what lets the two of them be held at once: a gain spares its own prerequisites.
    let mut first = trait_of(&["CHITIN"], &[]);
    first.categories = vec!["INSECT".to_owned()];
    let mut second = trait_of(&[], &[]);
    second.categories = vec!["INSECT".to_owned()];
    let mut grown = trait_of(&["CHITIN"], &[]);
    grown.categories = vec!["INSECT".to_owned()];
    let mut chitin = trait_of(&[], &[]);
    chitin.categories = vec!["INSECT".to_owned()];
    let mut elsewhere = trait_of(&[], &[]);
    elsewhere.categories = vec!["FELINE".to_owned()];
    let data = table(&[
        ("ANTENNAE", first),
        ("WINGS_BUG", second),
        ("CARAPACE", grown),
        ("CHITIN", chitin),
        ("TAIL", elsewhere),
    ]);

    let mut world = World::new();
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
    for id in ["CHITIN", "ANTENNAE", "TAIL"] {
        cdda_sim::mutations::gain(&mut world, survivor, &data, id);
    }
    assert_eq!(
        cdda_sim::mutations::mutations_of(&world, survivor).len(),
        3,
        "a gain spares its own prerequisite, so CHITIN and ANTENNAE are held together"
    );

    // Gaining CARAPACE prunes the insect trait that is not its prerequisite, spares the one that is, and leaves another category alone.
    cdda_sim::mutations::gain(&mut world, survivor, &data, "CARAPACE");
    let held = cdda_sim::mutations::mutations_of(&world, survivor);
    assert!(held.contains(&"CARAPACE".to_owned()), "the new trait is held");
    assert!(held.contains(&"CHITIN".to_owned()), "the trait it grew out of is spared");
    assert!(!held.contains(&"ANTENNAE".to_owned()), "another trait of the same category is pruned");
    assert!(held.contains(&"TAIL".to_owned()), "and a trait of another category is untouched");
}
