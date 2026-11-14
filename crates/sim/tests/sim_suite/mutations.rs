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
            "{{\"id\":\"{id}\",\"points\":{},\"visibility\":{},\"valid\":true,\"purifiable\":{},\"startingtrait\":{},\"threshold\":{},\"threshreq\":[{}],\"vitamin_cost\":{},\"enchant_values\":[{}],\"prereqs\":[{}],\"prereqs2\":[],\"cancels\":[{}],\"category\":[{}]}}",
            one.points,
            one.visibility,
            one.purifiable,
            one.startingtrait,
            one.threshold,
            list(&one.threshreq),
            one.vitamin_cost,
            one.enchant_values
                .iter()
                .map(|value| format!(
                    "{{\"value\":\"{}\",\"add\":{},\"multiply\":{}}}",
                    value.value, value.add, value.multiply
                ))
                .collect::<Vec<_>>()
                .join(","),
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
        valid: true,
        startingtrait: false,
        threshold: false,
        threshreq: Vec::new(),
        vitamin_cost: 100,
        enchantments: Vec::new(),
        enchant_values: Vec::new(),
        prereqs: prereqs.iter().map(|one| (*one).to_owned()).collect(),
        prereqs2: Vec::new(),
        cancels: cancels.iter().map(|one| (*one).to_owned()).collect(),
        categories: Vec::new(),
        // Nothing about switching on: these gates are about what a mutation needs to be **gained**, and activation has its own.
        starts_active: false,
        cost: 0,
        cooldown_turns: 0,
        hunger: false,
        thirst: false,
        sleepiness: false,
        mana: false,
        activated_eocs: Vec::new(),
        deactivated_eocs: Vec::new(),
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
    cdda_sim::mutations::gain(
        &mut world,
        survivor,
        &data,
        &cdda_content::mutations::Categories::default(),
        "BEAUTIFUL",
    );
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
        cdda_sim::mutations::gain(
            &mut world,
            survivor,
            &data,
            &cdda_content::mutations::Categories::default(),
            "UGLY"
        ),
        cdda_sim::mutations::Gain::Yes
    );
    assert!(cdda_sim::mutations::has(&world, survivor, "UGLY"));

    assert_eq!(
        cdda_sim::mutations::gain(
            &mut world,
            survivor,
            &data,
            &cdda_content::mutations::Categories::default(),
            "BEAUTIFUL"
        ),
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
    let data = table(&[
        ("GLASSJAW", stuck),
        ("PRETTY", trait_of(&[], &["GLASSJAW"])),
    ]);
    let mut world = World::new();
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
    cdda_sim::mutations::gain(
        &mut world,
        survivor,
        &data,
        &cdda_content::mutations::Categories::default(),
        "GLASSJAW",
    );
    assert_eq!(
        cdda_sim::mutations::can_gain(&world, survivor, &data, "PRETTY"),
        cdda_sim::mutations::Gain::Conflicting
    );
    assert_eq!(
        cdda_sim::mutations::gain(
            &mut world,
            survivor,
            &data,
            &cdda_content::mutations::Categories::default(),
            "PRETTY"
        ),
        cdda_sim::mutations::Gain::Conflicting
    );
    assert!(
        cdda_sim::mutations::has(&world, survivor, "GLASSJAW"),
        "and nothing was taken away"
    );
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
        cdda_sim::mutations::gain(
            &mut world,
            survivor,
            &data,
            &cdda_content::mutations::Categories::default(),
            id,
        );
    }
    assert_eq!(
        cdda_sim::mutations::mutations_of(&world, survivor).len(),
        3,
        "a gain spares its own prerequisite, so CHITIN and ANTENNAE are held together"
    );

    // Gaining CARAPACE prunes the insect trait that is not its prerequisite, spares the one that is, and leaves another category alone.
    cdda_sim::mutations::gain(
        &mut world,
        survivor,
        &data,
        &cdda_content::mutations::Categories::default(),
        "CARAPACE",
    );
    let held = cdda_sim::mutations::mutations_of(&world, survivor);
    assert!(
        held.contains(&"CARAPACE".to_owned()),
        "the new trait is held"
    );
    assert!(
        held.contains(&"CHITIN".to_owned()),
        "the trait it grew out of is spared"
    );
    assert!(
        !held.contains(&"ANTENNAE".to_owned()),
        "another trait of the same category is pruned"
    );
    assert!(
        held.contains(&"TAIL".to_owned()),
        "and a trait of another category is untouched"
    );
}

/// A starting trait is only taken away when its category's chance succeeds, and a category of a hundred always takes it.
///
/// Reference: `Character::mutate_towards` (`mutation.cpp:1666-1674`): a cancelled trait that is a base trait moves to `canceltrait` unless
/// `x_in_y( category.base_removal_chance, 100 )` succeeds - so a category whose chance is zero never loses its starting traits.
#[test]
fn a_starting_trait_is_removed_only_when_its_category_allows() {
    let mut starting = trait_of(&[], &[]);
    starting.startingtrait = true;
    starting.purifiable = true;
    let data = table(&[
        ("BEAUTIFUL", starting),
        ("UGLY", {
            let mut one = trait_of(&[], &["BEAUTIFUL"]);
            one.categories = vec!["NONE_ALLOWED".to_owned()];
            one
        }),
        // A starting trait nobody names a category for, removed by a category that always allows it.
        ("FAST_LEARNER", {
            let mut one = trait_of(&[], &[]);
            one.startingtrait = true;
            one
        }),
        ("SLOW_LEARNER", trait_of(&[], &["FAST_LEARNER"])),
    ]);
    let category_text = "{\"categories\":[{\"id\":\"NONE_ALLOWED\",\"base_removal_chance\":0,\"threshold_min\":1100}]}";
    let Ok(root) = cdda_json::parse(category_text.as_bytes()) else {
        return;
    };
    let Some((categories, _)) = cdda_content::mutations::categories_from_dump(&root) else {
        return;
    };

    let mut world = World::new();
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
    cdda_sim::singletons::put_rolls(&mut world, cdda_core_types::rng::Rolls::new(7));

    cdda_sim::mutations::gain(&mut world, survivor, &data, &categories, "BEAUTIFUL");
    cdda_sim::mutations::gain(&mut world, survivor, &data, &categories, "UGLY");
    assert!(
        cdda_sim::mutations::has(&world, survivor, "BEAUTIFUL"),
        "a starting trait of a category that never allows removal stays"
    );

    // With no category of its own, the chance is the default hundred, and the starting trait goes.
    cdda_sim::mutations::gain(&mut world, survivor, &data, &categories, "FAST_LEARNER");
    cdda_sim::mutations::gain(&mut world, survivor, &data, &categories, "SLOW_LEARNER");
    assert!(
        !cdda_sim::mutations::has(&world, survivor, "FAST_LEARNER"),
        "and with no category to stop it, a starting trait is removed"
    );
}

/// A threshold mutation needs **every** threshold it names, where an ordinary prerequisite needs only one.
///
/// Reference: `mutation_branch::threshreq` (`mutation.h:333`) and its use in `Character::mutate_towards`: `prereqs` is a list of alternatives and `threshreq` is a list of
/// requirements, so two thresholds asked for means two thresholds held.
#[test]
fn a_threshold_mutation_needs_every_threshold_it_names() {
    let mut cross = trait_of(&[], &[]);
    cross.threshold = true;
    let mut beast = trait_of(&[], &[]);
    beast.threshold = true;
    let mut past = trait_of(&[], &[]);
    past.threshold = true;
    past.threshreq = vec!["THRESH_CROSS".to_owned(), "THRESH_BEAST".to_owned()];
    let data = table(&[
        ("THRESH_CROSS", cross),
        ("THRESH_BEAST", beast),
        ("PAST_BOTH", past),
    ]);

    let mut world = World::new();
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
    let none = cdda_content::mutations::Categories::default();
    assert_eq!(
        cdda_sim::mutations::can_gain(&world, survivor, &data, "PAST_BOTH"),
        cdda_sim::mutations::Gain::MissingPrerequisite,
        "with neither threshold held, it waits"
    );
    cdda_sim::mutations::gain(&mut world, survivor, &data, &none, "THRESH_CROSS");
    assert_eq!(
        cdda_sim::mutations::can_gain(&world, survivor, &data, "PAST_BOTH"),
        cdda_sim::mutations::Gain::MissingPrerequisite,
        "and one of two is not enough"
    );
    cdda_sim::mutations::gain(&mut world, survivor, &data, &none, "THRESH_BEAST");
    assert_eq!(
        cdda_sim::mutations::can_gain(&world, survivor, &data, "PAST_BOTH"),
        cdda_sim::mutations::Gain::Yes,
        "with both held it can be gained"
    );
}

/// A creature's strength in a category is the sum of its mutations' heights, and a threshold is worth ten on its own.
///
/// Reference: `Character::calc_mutation_levels` (`character.cpp:7641-7672`) with `mutation_height` (`character.cpp:7626-7639`): a root trait counts three, its child four,
/// its grandchild five, and a threshold ten for each category it names.
#[test]
fn a_category_strength_counts_heights_and_thresholds() {
    let mut root = trait_of(&[], &[]);
    root.categories = vec!["BEAST".to_owned()];
    let mut child = trait_of(&["ROOT"], &[]);
    child.categories = vec!["BEAST".to_owned()];
    let mut grandchild = trait_of(&["CHILD"], &[]);
    grandchild.categories = vec!["BEAST".to_owned()];
    let mut threshold = trait_of(&[], &[]);
    threshold.threshold = true;
    threshold.categories = vec!["BEAST".to_owned()];
    let mut other = trait_of(&[], &[]);
    other.categories = vec!["FELINE".to_owned()];
    let data = table(&[
        ("ROOT", root),
        ("CHILD", child),
        ("GRANDCHILD", grandchild),
        ("THRESH_BEAST", threshold),
        ("OTHER", other),
    ]);

    assert_eq!(cdda_sim::mutations::height_of(&data, "ROOT"), 1);
    assert_eq!(cdda_sim::mutations::height_of(&data, "CHILD"), 2);
    assert_eq!(cdda_sim::mutations::height_of(&data, "GRANDCHILD"), 3);

    let mut world = World::new();
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
    let none = cdda_content::mutations::Categories::default();
    for id in ["ROOT", "CHILD", "GRANDCHILD", "OTHER"] {
        cdda_sim::mutations::gain(&mut world, survivor, &data, &none, id);
    }
    assert_eq!(
        cdda_sim::mutations::category_level(&world, survivor, &data, "BEAST"),
        12,
        "three, four and five, and the other category does not count"
    );
    // Crossing the threshold is a **replacement**: the same-category pruning takes the branch away, which leaves the threshold's ten standing alone.
    cdda_sim::mutations::gain(&mut world, survivor, &data, &none, "THRESH_BEAST");
    let held = cdda_sim::mutations::mutations_of(&world, survivor);
    assert!(
        held.contains(&"THRESH_BEAST".to_owned()) && held.contains(&"OTHER".to_owned()),
        "the threshold prunes its own branch and leaves the other category alone: {held:?}"
    );
    assert!(
        !held.contains(&"ROOT".to_owned()) && !held.contains(&"GRANDCHILD".to_owned()),
        "the beast branch is gone: {held:?}"
    );
    assert_eq!(
        cdda_sim::mutations::category_level(&world, survivor, &data, "BEAST"),
        10,
        "and the strength is then the threshold's own ten"
    );
}

/// The dialogue effect that asks for a change now gives one, from the category the creature has gone furthest into.
///
/// Reference: `f_mutate` (`npctalk.cpp:3560-3570`) takes the creature's **highest** mutation category and calls `mutate_category`, which draws from that category's valid
/// list and tries twice (`mutation.cpp:1354-1404`). Until this test the effect answered `CannotAnswer` for every depth above zero.
#[test]
fn the_mutate_effect_gives_a_mutation_from_the_strongest_category() {
    use bevy_ecs::prelude::*;

    let mut worm = trait_of(&[], &[]);
    worm.categories = vec!["BEAST".to_owned()];
    let mut wings = trait_of(&[], &[]);
    wings.categories = vec!["BEAST".to_owned()];
    let mut cat = trait_of(&[], &[]);
    cat.categories = vec!["FELINE".to_owned()];
    let data = table(&[("WORM", worm), ("WINGS", wings), ("CAT_EYES", cat)]);

    let mut world = World::new();
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
    let categories = cdda_content::mutations::Categories::default();
    cdda_sim::mutation_data::install(&mut world, data.clone(), categories.clone());
    cdda_sim::singletons::put_rolls(&mut world, cdda_core_types::rng::Rolls::new(11));

    // The creature is already a beast, so the beast category is the one it has gone furthest into.
    let mut root = trait_of(&[], &[]);
    root.categories = vec!["BEAST".to_owned()];
    cdda_sim::mutations::gain(&mut world, survivor, &data, &categories, "WORM");

    let effect = cdda_content::dialogue::Effect::Mutation {
        highest_category: cdda_content::dialogue::Expression::Number(1.0),
        category: None,
        npc: false,
        use_vitamins: true,
        true_random: false,
    };
    let applied = cdda_sim::dialogue_effects::run_effects(&mut world, survivor, None, &[effect]);
    assert_ne!(
        applied,
        cdda_sim::dialogue_effects::Applied::CannotAnswer,
        "a depth above zero is answered now that there is a pool to draw from"
    );
    let held = cdda_sim::mutations::mutations_of(&world, survivor);
    assert!(
        held.iter().any(|id| id == "WINGS"),
        "and it gained the other beast trait: {held:?}"
    );
    assert!(
        !held.iter().any(|id| id == "CAT_EYES"),
        "not the feline one, which is another category: {held:?}"
    );
}

/// A mutation that carries a strength enchantment makes its carrier stronger, and carries more with it.
///
/// Reference: `Character::get_str` (`character.cpp:3353`) applies the enchantment cache to the base, and `Character::weight_capacity` (`character.cpp:3353-3361`) adds four
/// kilograms a point of the **result** - so a `STRENGTH +1` mutation is four kilograms in the hands, not four on the character sheet.
#[test]
fn a_strength_enchantment_reaches_the_carrying_capacity() {
    use bevy_ecs::prelude::*;

    let mut strong = trait_of(&[], &[]);
    strong.enchantments = vec!["INLINE_ENCH_STR_UP_0".to_owned()];
    strong.enchant_values = vec![cdda_content::mutations::EnchantmentValue {
        value: "STRENGTH".to_owned(),
        add: 1.0,
        multiply: 0.0,
    }];
    let data = table(&[("STR_UP", strong)]);
    let categories = cdda_content::mutations::Categories::default();

    let mut world = World::new();
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
    world
        .entity_mut(survivor)
        .insert(cdda_sim::creature::Strength { base: 8 });
    cdda_sim::mutation_data::install(&mut world, data.clone(), categories.clone());

    assert_eq!(
        cdda_sim::creature::effective_strength(&world, survivor),
        8,
        "with no mutation the base is the answer"
    );
    let without = cdda_sim::creature::carrying_capacity(&world, survivor);
    cdda_sim::mutations::gain(&mut world, survivor, &data, &categories, "STR_UP");
    assert_eq!(
        cdda_sim::creature::effective_strength(&world, survivor),
        9,
        "and the enchantment is worth one point of strength"
    );
    assert_eq!(
        cdda_sim::creature::carrying_capacity(&world, survivor),
        without + 4_000,
        "which is four kilograms more it can carry"
    );
}

/// A creature that has taken no mutation never gains a bad one, and one that has wandered from its tree can.
///
/// Reference: `Character::roll_bad_mutation` (`mutation.cpp:1109-1139`): the chance of a bad roll is `0.5 * instability / good traits`, capped at **0.67**, and **a creature
/// with no instability never rolls bad at all** - so a first mutation is always good or neutral. The port drew from the whole category until this test.
#[test]
fn a_first_mutation_is_never_bad() {
    let mut good = trait_of(&[], &[]);
    good.points = 1;
    good.categories = vec!["BEAST".to_owned()];
    let mut bad = trait_of(&[], &[]);
    bad.points = -3;
    bad.categories = vec!["BEAST".to_owned()];
    let data = table(&[("GOOD_ONE", good), ("BAD_ONE", bad)]);

    assert_eq!(
        cdda_sim::mutations::total_in_category(
            &data,
            "BEAST",
            cdda_sim::mutations::Count::Positive
        ),
        1,
        "the category holds one good mutation"
    );
    assert_eq!(
        cdda_sim::mutations::total_in_category(
            &data,
            "BEAST",
            cdda_sim::mutations::Count::Negative
        ),
        1,
        "and one bad"
    );

    let categories = cdda_content::mutations::Categories::default();
    let mut world = bevy_ecs::prelude::World::new();
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
    cdda_sim::mutation_data::install(&mut world, data.clone(), categories.clone());
    assert_eq!(
        cdda_sim::mutations::instability_in_category(&world, survivor, &data, "BEAST"),
        0,
        "a creature with nothing yet is not unstable"
    );
    let mut rolls = cdda_core_types::rng::Rolls::new(5);
    assert!(
        !cdda_sim::mutations::roll_is_bad(&world, survivor, &data, "BEAST", &mut rolls),
        "and never rolls a bad mutation, whatever the dice say"
    );

    // Take the good one, then let the creature wander: a mutation from another tree counts double.
    cdda_sim::mutations::gain(&mut world, survivor, &data, &categories, "GOOD_ONE");
    assert!(
        cdda_sim::mutations::instability_in_category(&world, survivor, &data, "BEAST") > 0,
        "a mutation of its own tree is instability"
    );
}

/// A mutation that steadies a creature lowers the instability its other mutations give it.
///
/// Reference: `get_instability_per_category` multiplies the count by `MUT_INSTABILITY_MOD` before flooring at zero (`mutation.cpp:1082-1085`). The mutation that does the
/// steadying is **not itself instability** - it is a bad mutation, which the sum skips - so the only thing it changes is how unstable the rest of the body is.
#[test]
fn a_steadying_mutation_lowers_instability() {
    let mut good = trait_of(&[], &[]);
    good.points = 1;
    good.categories = vec!["BEAST".to_owned()];
    let mut steady = trait_of(&[], &[]);
    steady.points = -2;
    steady.enchant_values = vec![cdda_content::mutations::EnchantmentValue {
        value: "MUT_INSTABILITY_MOD".to_owned(),
        add: -1.0,
        multiply: 0.0,
    }];
    let data = table(&[("GOOD_ONE", good), ("STEADY", steady)]);
    let categories = cdda_content::mutations::Categories::default();

    let mut world = bevy_ecs::prelude::World::new();
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
    cdda_sim::mutation_data::install(&mut world, data.clone(), categories.clone());
    cdda_sim::mutations::gain(&mut world, survivor, &data, &categories, "GOOD_ONE");
    assert_eq!(
        cdda_sim::mutations::instability_in_category(&world, survivor, &data, "BEAST"),
        1,
        "one mutation of height one is one point of instability"
    );

    cdda_sim::mutations::gain(&mut world, survivor, &data, &categories, "STEADY");
    assert_eq!(
        cdda_sim::mutations::instability_in_category(&world, survivor, &data, "BEAST"),
        0,
        "and a steadying mutation takes it away"
    );
}

/// A creature with no mutagen in its blood gains no mutation from the dialogue effect, and one that has paid does.
///
/// Reference: `Character::mutation_ok`'s `mut_vit` check (`mutation.cpp:1052-1057`) refuses a trait the creature cannot pay for, and `mutate_category` hands it the
/// **category's** vitamin (`mutation.cpp:1380-1382`). `u_mutate` defaults `use_vitamins` to true (`npctalk.cpp:3565-3568`), so the dialogue effect pays too.
#[test]
fn a_mutation_costs_mutagen() {
    use bevy_ecs::prelude::*;

    // A free trait the creature already has, so `BEAST` is the category it has gone furthest into, and a paid one it might reach.
    let mut free = trait_of(&[], &[]);
    free.categories = vec!["BEAST".to_owned()];
    let mut paid = trait_of(&[], &[]);
    paid.vitamin_cost = 100;
    paid.categories = vec!["BEAST".to_owned()];
    let data = table(&[("FREE_TRAIT", free), ("PAID_TRAIT", paid)]);
    let text = "{\"categories\":[{\"id\":\"BEAST\",\"vitamin\":\"mutagen_beast\",\
        \"base_removal_chance\":0,\"threshold_min\":2200}]}";
    let Ok(root) = cdda_json::parse(text.as_bytes()) else {
        return;
    };
    let Some((categories, _)) = cdda_content::mutations::categories_from_dump(&root) else {
        return;
    };

    let applied = |vitamin_level: i32| {
        let mut world = World::new();
        let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
        let mut body = cdda_sim::body::Body::default();
        body.vitamins
            .insert("mutagen_beast".to_owned(), vitamin_level);
        world.entity_mut(survivor).insert(body);
        cdda_sim::mutation_data::install(&mut world, data.clone(), categories.clone());
        cdda_sim::singletons::put_rolls(&mut world, cdda_core_types::rng::Rolls::new(23));
        let effect = cdda_content::dialogue::Effect::Mutation {
            highest_category: cdda_content::dialogue::Expression::Number(1.0),
            category: None,
            npc: false,
            use_vitamins: true,
            true_random: false,
        };
        cdda_sim::mutations::gain(&mut world, survivor, &data, &categories, "FREE_TRAIT");
        cdda_sim::dialogue_effects::run_effects(&mut world, survivor, None, &[effect]);
        cdda_sim::mutations::mutations_of(&world, survivor)
            .iter()
            .any(|id| id == "PAID_TRAIT")
    };

    assert!(!applied(0), "with no mutagen there is nothing to pay with");
    assert!(!applied(99), "and one unit short is still short");
    assert!(applied(100), "and a hundred units buys the trait");
}

/// A dexterity enchantment reaches the creature's own dexterity, which is what aiming and dodging read.
///
/// Reference: `Character::get_dex` (`character.cpp`) is `get_dex_base()` through the enchantment cache, the same shape `get_str` has - so a mutation that says `DEXTERITY +2` makes its
/// carrier aim two points better and dodge two points better, without either rule knowing where the points came from.
#[test]
fn a_dexterity_enchantment_reaches_the_creatures_own_dexterity() {
    use bevy_ecs::prelude::*;

    let mut nimble = trait_of(&[], &[]);
    nimble.enchantments = vec!["INLINE_ENCH_DEX_UP_0".to_owned()];
    nimble.enchant_values = vec![cdda_content::mutations::EnchantmentValue {
        value: "DEXTERITY".to_owned(),
        add: 2.0,
        multiply: 0.0,
    }];
    let data = table(&[("DEX_UP", nimble)]);
    let categories = cdda_content::mutations::Categories::default();

    let mut world = World::new();
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
    world
        .entity_mut(survivor)
        .insert(cdda_sim::creature::Dexterity { base: 8 });
    cdda_sim::mutation_data::install(&mut world, data.clone(), categories.clone());

    assert_eq!(
        cdda_sim::creature::effective_dexterity(&world, survivor),
        8,
        "with no mutation the base is the answer"
    );
    cdda_sim::mutations::gain(&mut world, survivor, &data, &categories, "DEX_UP");
    assert_eq!(
        cdda_sim::creature::effective_dexterity(&world, survivor),
        10,
        "and the enchantment is worth two points of dexterity"
    );
}
