//! Items lying on a square: what is there, and in what order.
//!
//! Reference: `map_stack` (`src/map.cpp`), which keeps a tile's items in the order they were added.
//! The order is not cosmetic: a fire that burns a pile consumes it front to back, so the order is
//! part of what a replay has to reproduce.

#![allow(clippy::panic, clippy::expect_used)]

use bevy_ecs::prelude::*;
use cdda_sim::{GroundItem, ItemTypeId, drop_item, items_at, take_item};

fn kind(n: u32) -> ItemTypeId {
    ItemTypeId(n)
}

#[test]
fn a_square_hands_back_what_was_put_on_it_in_order() {
    let mut world = World::new();
    drop_item(&mut world, (5, 5), kind(1), 1);
    drop_item(&mut world, (5, 5), kind(2), 3);
    let third = drop_item(&mut world, (5, 5), kind(3), 1);
    let items = items_at(&mut world, (5, 5));
    let kinds: Vec<u32> = items
        .iter()
        .map(|entity| world.get::<GroundItem>(*entity).expect("an item").kind.0)
        .collect();
    assert_eq!(kinds, vec![1, 2, 3], "insertion order");
    assert_eq!(items.last(), Some(&third), "the most recent is last");
}

#[test]
fn an_empty_square_holds_nothing() {
    let mut world = World::new();
    assert!(items_at(&mut world, (1, 1)).is_empty());
}

#[test]
fn a_square_only_returns_its_own_items() {
    let mut world = World::new();
    drop_item(&mut world, (5, 5), kind(1), 1);
    drop_item(&mut world, (6, 5), kind(2), 1);
    assert_eq!(
        items_at(&mut world, (5, 5)).len(),
        1,
        "the neighbour's item stays put"
    );
    assert_eq!(items_at(&mut world, (6, 5)).len(), 1);
}

#[test]
fn charges_survive_the_round_trip() {
    let mut world = World::new();
    let entity = drop_item(&mut world, (5, 5), kind(7), 42);
    let item = world.get::<GroundItem>(entity).expect("an item");
    assert_eq!(item.charges, 42);
}

#[test]
fn a_taken_item_is_gone_and_the_rest_stay_in_order() {
    let mut world = World::new();
    drop_item(&mut world, (5, 5), kind(1), 1);
    let middle = drop_item(&mut world, (5, 5), kind(2), 1);
    drop_item(&mut world, (5, 5), kind(3), 1);
    take_item(&mut world, middle);
    let remaining: Vec<u32> = items_at(&mut world, (5, 5))
        .iter()
        .map(|entity| world.get::<GroundItem>(*entity).expect("an item").kind.0)
        .collect();
    assert_eq!(remaining, vec![1, 3], "what is left keeps its order");
}

/// Faults come off one at a time, and the singular in the name is the rule.
///
/// Reference: `item::remove_single_fault_of_type` (`item.cpp:7874-7882`): it walks the item's faults and `return`s on the first whose type matches - **an item with two mechanical faults loses
/// one per call, not both**, which is what a caller clearing a blow's damage would get wrong.
#[test]
fn one_fault_of_a_type_comes_off_per_call() {
    use cdda_sim::{ItemFaults, remove_one_fault_of_type};

    let mut faults = ItemFaults::default();
    for fault in ["dented", "cracked", "wet"] {
        faults.0.insert(fault.to_owned());
    }
    // The fault catalogue says what each id is; this crate does not own it.
    let kinds = |fault: &str| Some(if fault == "wet" { "wet" } else { "mechanical_damage" }.to_owned());

    let first = remove_one_fault_of_type(&mut faults, kinds, "mechanical_damage");
    assert!(first.is_some(), "one came off");
    assert_eq!(faults.0.len(), 2, "**one**, not both mechanical faults");
    let second = remove_one_fault_of_type(&mut faults, kinds, "mechanical_damage");
    assert!(second.is_some() && second != first, "the next call takes the other");
    assert!(remove_one_fault_of_type(&mut faults, kinds, "mechanical_damage").is_none(), "then none is left");
    assert_eq!(faults.0.len(), 1, "and the wet fault was never touched");
    assert_eq!(remove_one_fault_of_type(&mut faults, kinds, "wet").as_deref(), Some("wet"), "which the wet type still finds");
}

/// How much of a worn item can still be repaired - levels when they differ, and a plain yes-or-no when they do not.
///
/// Reference: `item::repairable_levels` (`item.cpp:9472-9479`). **The fallback answers in a different unit from the main answer**: levels when `damage_level( damage ) > damage_level( degradation )`,
/// and otherwise `damage > degradation`, which is one or zero - so a blade a thousandth past its degradation is repairable **once**, and one worn through is not repairable at all.
#[test]
fn repair_levels_fall_back_to_a_yes_or_no() {
    use cdda_sim::repairable_levels;

    // Halfway through the damage is level 3 (`1 + 4 * 2000 / 4000`), and a whole level is one repair.
    assert_eq!(repairable_levels(2_000, 0, 4_000, false), 3, "level 3 against level 0");
    // Damage and degradation in the same level: the fallback says yes, once.
    assert_eq!(repairable_levels(2_000, 1_500, 4_000, false), 1, "same level, but damage is past the degradation");
    // Nothing to repair.
    assert_eq!(repairable_levels(1_500, 1_500, 4_000, false), 0, "identical numbers repair nothing");
    assert_eq!(repairable_levels(0, 0, 4_000, false), 0, "and a new item has nothing to repair");
    // A charge-counted item is **level 5 the moment it takes any damage at all** - the scale has no partials for it - so it always reads as five levels to repair.
    assert_eq!(repairable_levels(100, 0, 0, true), 5, "a leaking bottle is ruined, not scratched");
    assert_eq!(repairable_levels(0, 0, 0, true), 0, "and an untouched one is level zero");
}

/// An item's health is a fraction of its maximum, and a stack has none to lose.
///
/// Reference: `item::get_relative_health` (`item.cpp:9419-9426`). **A charge-counted type answers 1.0 whatever its damage says** - it loses charges, not health - which is also what stops the
/// division by a maximum of zero that every stack would otherwise perform.
#[test]
fn item_health_is_a_fraction_and_a_stack_has_none_to_lose() {
    use cdda_sim::relative_health;

    assert!((relative_health(0, 4_000) - 1.0).abs() < f64::EPSILON, "undamaged is whole");
    assert!((relative_health(1_000, 4_000) - 0.75).abs() < f64::EPSILON, "a quarter worn is three quarters there");
    assert!((relative_health(4_000, 4_000) - 0.0).abs() < f64::EPSILON, "at the maximum there is nothing left");
    assert!((relative_health(3_000, 0) - 1.0).abs() < f64::EPSILON, "a stack is always whole, because its health is not what it loses");
    // A type with no maximum that somehow holds damage still answers whole rather than dividing by zero.
    assert!(relative_health(500, 0).is_finite(), "no infinity, ever");
}

/// The registry can say which faults a kind catches, and says nothing for a kind nobody described.
///
/// Reference: `itype::faults`, the weighted list `item::set_random_fault_of_type` filters by fault type before picking (`item.cpp:7854-7868`). **A kind with no entry answers an empty list**, which
/// is the same "missing is a zero" rule the chip-resistance table follows - and it means the pick sets no fault rather than inventing one.
#[test]
fn the_registry_knows_which_faults_a_kind_catches() {
    use bevy_ecs::prelude::World;

    let mut world = World::new();
    cdda_sim::item_registry::set_item_fault_weights(
        &mut world,
        &[(
            cdda_sim::ItemTypeId(7),
            vec![("dented".to_owned(), 3), ("cracked".to_owned(), 1)],
        )],
    );
    let weights = cdda_sim::item_registry::fault_weights_of(&mut world, cdda_sim::ItemTypeId(7));
    assert_eq!(weights.len(), 2, "two faults, with their weights");
    assert_eq!(weights.first().map(|(fault, weight)| (fault.as_str(), *weight)), Some(("dented", 3)), "in the data's own order");
    assert!(
        cdda_sim::item_registry::fault_weights_of(&mut world, cdda_sim::ItemTypeId(8)).is_empty(),
        "a kind nobody described catches nothing"
    );
}

/// One pass fills every kind table, and all four answer from it.
///
/// Reference: the reference's `item_factory` fills `itype`'s flags, qualities, chip resistance and faults from **one** load, so a kind's four answers cannot disagree. This gate installs two kinds in
/// one call and asks every table about both, plus a third kind nobody described - which must answer the missing-table values throughout.
#[test]
fn one_pass_fills_every_kind_table() {
    use bevy_ecs::prelude::World;
    use cdda_sim::ItemTypeId;
    use cdda_sim::item_registry::{KindFacts, install_kind_facts};

    let mut world = World::new();
    install_kind_facts(
        &mut world,
        &[
            (
                ItemTypeId(7),
                KindFacts {
                    flags: vec!["DURABLE_MELEE".to_owned()],
                    qualities: vec![("CUT".to_owned(), 1)],
                    chip_resistance: 9,
                    faults: vec![("dented".to_owned(), 3)],
                },
            ),
            (ItemTypeId(8), KindFacts { chip_resistance: 20, ..KindFacts::default() }),
        ],
    );

    assert!(cdda_sim::item_registry::kind_has_flag(&world, ItemTypeId(7), "DURABLE_MELEE"), "the flag came through");
    assert_eq!(cdda_sim::item_registry::chip_resistance_of(&mut world, ItemTypeId(7)), 9, "and the chip figure");
    assert_eq!(cdda_sim::item_registry::chip_resistance_of(&mut world, ItemTypeId(8)), 20, "for the second kind too");
    assert_eq!(
        cdda_sim::item_registry::fault_weights_of(&mut world, ItemTypeId(7)).first().map(|(fault, weight)| (fault.as_str(), *weight)),
        Some(("dented", 3)),
        "and the faults"
    );
    // Nobody described kind nine: no flags, the reference's fallback chip figure, no faults.
    assert!(!cdda_sim::item_registry::kind_has_flag(&world, ItemTypeId(9), "DURABLE_MELEE"), "no flags");
    assert_eq!(cdda_sim::item_registry::chip_resistance_of(&mut world, ItemTypeId(9)), 2, "and the reference's fallback of two");
    assert!(cdda_sim::item_registry::fault_weights_of(&mut world, ItemTypeId(9)).is_empty(), "and no faults");
}

/// Loading the game's item data fills the kind tables, not just the kinds.
///
/// Reference: `item_factory` keeps a type's flags, qualities and faults on the `itype` itself, so a game that loaded the data can answer all three. **Before round 1218 the port installed
/// only the types and the qualities**, and a rock's flags, chip resistance and faults were all the missing-table answers however much data had been loaded.
#[test]
fn loading_the_item_data_fills_the_kind_tables() {
    use bevy_ecs::prelude::World;

    let data = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../oracle/src/data/json");
    if !data.is_dir() {
        return;
    }
    let kinds = cdda_content::items::load_paths(&[&data]).expect("the game data loads");
    // The chip figures, from the prototypes dump, exactly as the binary joins them.
    let dump = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/prototypes/items.json");
    let chip = std::fs::read(dump)
        .ok()
        .and_then(|bytes| cdda_json::parse(&bytes).ok())
        .map_or_else(std::collections::BTreeMap::new, |value| cdda_content::items::chip_resistance(&value));
    let mut world = World::new();
    cdda_sim::item_data::install(&mut world, &kinds, &chip);

    // A bionic catches the salvaged fault at full weight, which is data the type records carry and the tables did not.
    let bionic = cdda_sim::item_registry::id_of(&mut world, "bio_adrenaline").expect("the bionic is a kind");
    let faults = cdda_sim::item_registry::fault_weights_of(&mut world, bionic);
    assert!(
        faults.iter().any(|(fault, weight)| fault == "fault_bionic_salvaged" && *weight == 100),
        "the bionic's fault is installed: {faults:?}"
    );
    // And the chip figure came from the dump: a rock resists chipping at twenty, not at the fallback two.
    let rock = cdda_sim::item_registry::id_of(&mut world, "rock").expect("the rock is a kind");
    assert_eq!(
        cdda_sim::item_registry::chip_resistance_of(&mut world, rock),
        20,
        "the dump's figure reached the store"
    );
    // And the flags came with it: a kind whose data names a flag answers it.
    let flagged = kinds
        .iter()
        .find(|kind| !kind.flags.is_empty())
        .and_then(|kind| kind.flags.iter().next().map(|flag| (kind.id.clone(), flag.clone())));
    if let Some((id, flag)) = flagged {
        let number = cdda_sim::item_registry::id_of(&mut world, &id).expect("the kind is installed");
        assert!(cdda_sim::item_registry::kind_has_flag(&world, number, &flag), "{id} answers {flag}");
    }
}

/// The catalogue says what kind of fault an id is, and says nothing about an id it has never seen.
///
/// Reference: `Fault::type()`, which `item::set_random_fault_of_type` compares against `"mechanical_damage"` (`item.cpp:7856-7860`). **An uninstalled catalogue answers nothing for every id**, which
/// makes every fault ineligible - the pick sets no fault rather than guessing one's type.
#[test]
fn the_catalogue_says_what_kind_of_fault_an_id_is() {
    use bevy_ecs::prelude::World;

    let mut world = World::new();
    assert_eq!(cdda_sim::item_registry::fault_type_of(&mut world, "dented"), None, "nothing installed, nothing known");
    cdda_sim::item_registry::set_fault_types(
        &mut world,
        &[
            ("dented".to_owned(), "mechanical_damage".to_owned()),
            ("wet".to_owned(), "wet".to_owned()),
        ],
    );
    assert_eq!(
        cdda_sim::item_registry::fault_type_of(&mut world, "dented").as_deref(),
        Some("mechanical_damage"),
        "a dent is mechanical damage"
    );
    assert_eq!(cdda_sim::item_registry::fault_type_of(&mut world, "wet").as_deref(), Some("wet"), "and wetness is its own kind");
    assert_eq!(cdda_sim::item_registry::fault_type_of(&mut world, "cracked"), None, "an id the catalogue has never seen has no type");
}

/// A fault the item's own faults block is never picked, and the catalogue supplies the blocks.
///
/// Reference: `item::set_random_fault_of_type` keeps only faults that `can_have_fault( f.obj )` allows (`item.cpp:7857`), and `can_have_fault` refuses anything one of the item's existing faults
/// **blocks** - "a thing cannot have a chipped blade when the blade is gone". Round 1222 answered this callback with an empty set; this gate holds the real one.
#[test]
fn a_blocked_fault_is_never_picked() {
    use bevy_ecs::prelude::World;

    let mut world = World::new();
    cdda_sim::item_registry::set_fault_blocks(
        &mut world,
        &[(
            "broken".to_owned(),
            ["cracked".to_owned(), "dented".to_owned()].into_iter().collect(),
        )],
    );
    let blocks = cdda_sim::item_registry::blocks_of(&mut world, "broken");
    assert_eq!(blocks.len(), 2, "a broken blade blocks two faults");
    assert!(blocks.contains("cracked") && blocks.contains("dented"), "and names both");

    // A fault that blocks nothing answers an empty set, and a fault nobody described does too.
    assert!(cdda_sim::item_registry::blocks_of(&mut world, "wet").is_empty(), "wetness blocks nothing");
    assert!(cdda_sim::item_registry::blocks_of(&mut world, "unknown").is_empty(), "and an unknown id blocks nothing");

    // With "broken" among the item's own faults, neither blocked fault can be caught - the pick has no eligible candidate and sets nothing.
    let existing: std::collections::BTreeSet<String> = ["broken".to_owned()].into_iter().collect();
    let candidates = vec![("cracked".to_owned(), 100), ("dented".to_owned(), 100)];
    let mut rolls = cdda_core_types::rng::Rolls::new(5);
    // The blocks are read before the pick, because the pick's callback cannot borrow the world.
    // **Keyed by the faults the item has, not by the candidates**: `can_have_fault` asks each *present* fault what it prevents.
    let inline: std::collections::BTreeMap<String, std::collections::BTreeSet<String>> = existing
        .iter()
        .map(|fault| (fault.clone(), cdda_sim::item_registry::blocks_of(&mut world, fault)))
        .collect();
    let picked = cdda_sim::pick_fault(
        &candidates,
        &existing,
        "mechanical_damage",
        |_| Some("mechanical_damage".to_owned()),
        |fault| inline.get(fault).cloned().unwrap_or_default(),
        &mut rolls,
    );
    assert_eq!(picked, None, "every mechanical fault is blocked by the broken blade");
}

/// A fault that names a line says it when it is caught, and a silent fault says nothing.
///
/// Reference: `item::set_fault` prints `f_id.obj().message()` when told to speak (`item.cpp:7840-7842`). **Most faults in this data version name no line**, so the gate proves the speaking path with a
/// fault that does and the silent path with one that does not - a port that printed the empty string would put a blank line in the log, which is visible to a player and caught here.
#[test]
fn a_caught_fault_says_its_own_line() {
    use bevy_ecs::prelude::World;

    let mut world = World::new();
    cdda_sim::item_registry::set_fault_messages(
        &mut world,
        &[("loud".to_owned(), "Your blade has a bad case of the dents.".to_owned())],
    );
    assert_eq!(
        cdda_sim::item_registry::message_of(&mut world, "loud").as_deref(),
        Some("Your blade has a bad case of the dents."),
        "a fault that names a line answers it"
    );
    assert_eq!(cdda_sim::item_registry::message_of(&mut world, "quiet"), None, "a fault nobody described says nothing");
    // And it lands in the log when pushed, which is what the swing does - **before** the table is replaced below, because installing a new table replaces the old one.
    if let Some(line) = cdda_sim::item_registry::message_of(&mut world, "loud") {
        cdda_sim::messages::push(&mut world, &line);
    }

    // An empty string is silence, not a blank line.
    cdda_sim::item_registry::set_fault_messages(&mut world, &[("quiet".to_owned(), String::new())]);
    assert_eq!(cdda_sim::item_registry::message_of(&mut world, "quiet"), None, "an empty line is no line");
    assert_eq!(cdda_sim::messages::lines(&world).len(), 1, "one line in the log");
}

/// A thing made of other things knows its parts, and a part that is gone is not in the list.
///
/// Reference: `item::components` is a `std::map<itype_id, std::vector<item>>` inside every item (`item.h`), and every rule that takes a thing apart walks it - the `FRAGILE_MELEE` breakdown
/// (`melee.cpp:301-330`), disassembly, and repair. **The port does not nest containers**: a part is an entity, and "made of" is a link, so the same part can be asked for by whoever needs it and
/// **a destroyed part leaves no ghost in its parent's list**.
#[test]
fn a_thing_knows_the_parts_it_is_made_of() {
    use bevy_ecs::prelude::*;
    use cdda_sim::item_parts::{PartOf, add_part, parts_of};

    let mut world = World::new();
    let blade = world.spawn_empty().id();
    let handle = world.spawn_empty().id();
    let string = world.spawn_empty().id();
    let weapon = world.spawn_empty().id();
    add_part(&mut world, weapon, blade);
    add_part(&mut world, weapon, handle);
    add_part(&mut world, weapon, string);
    assert_eq!(parts_of(&world, weapon), vec![blade, handle, string], "the parts, in the order they were added");
    assert!(parts_of(&world, blade).is_empty(), "and a part of a part is nothing until someone makes it one");
    assert_eq!(world.get::<PartOf>(blade).map(|link| link.parent), Some(weapon), "the link points at the whole");

    world.entity_mut(handle).despawn();
    assert_eq!(parts_of(&world, weapon), vec![blade, string], "**a destroyed part leaves no ghost**: the list is a query, not a copy");
}

/// Emptying a container puts what it held on the square it stands on, and nothing is left inside it.
///
/// Reference: `item_contents::spill_contents( pos )` (`item_contents.cpp:1228-1237`), called when a weapon is destroyed so its contents are not destroyed with it. The port's container holds
/// **links**, so spilling is taking each link out - and **the container's own contents become empty as a consequence**, not by a second cleanup step that could forget one.
#[test]
fn emptying_a_container_puts_what_it_held_on_the_floor() {
    use bevy_ecs::prelude::*;

    let mut world = World::new();
    let square = (3, 4, 0);
    let bag = world
        .spawn((
            cdda_sim::GroundItem { kind: cdda_sim::ItemTypeId(1), charges: 1, age: 0, burnt: 0, damage: 0, burn: None },
            cdda_sim::Position { x: square.0, y: square.1, z: square.2 },
        ))
        .id();
    let mut held = Vec::new();
    for kind in 2..4 {
        let thing = world
            .spawn(cdda_sim::GroundItem { kind: cdda_sim::ItemTypeId(kind), charges: 1, age: 0, burnt: 0, damage: 0, burn: None })
            .id();
        assert!(cdda_sim::containment::put_inside(&mut world, thing, bag), "the fixture's thing goes in");
        held.push(thing);
    }
    assert_eq!(cdda_sim::containment::contents_of(&world, bag).len(), 2, "two things are inside");

    assert_eq!(cdda_sim::containment::spill_contents(&mut world, bag, square), 2, "**both spill**, and the count says so");
    assert!(cdda_sim::containment::contents_of(&world, bag).is_empty(), "and nothing is left inside");
    for thing in held {
        assert!(world.get::<cdda_sim::containment::ContainedIn>(thing).is_none(), "**the link is gone**, not hidden");
        assert_eq!(
            world.get::<cdda_sim::Position>(thing).map(|at| (at.x, at.y, at.z)),
            Some(square),
            "and it is on the square the container stood on"
        );
    }
}
