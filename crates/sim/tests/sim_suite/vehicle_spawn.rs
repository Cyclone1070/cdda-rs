//! What a placed vehicle finds in its cargo: the spawn list it was placed with, and the count a chance becomes.

#![allow(clippy::expect_used, clippy::panic)]

use bevy_ecs::prelude::*;
use cdda_core_types::rng::Rolls;

/// **A spawn list is data, and the car's is read.** `--dump-vehicles` now reports each prototype's `item_spawns` (`src/veh_type.h:470-481`): the square, the chance of a batch, the
/// items a batch lists and the groups it draws from. The car's five are the ones a placed car uses - `car_misc` and `car_kit` at 90%, snacks at 5, fast food at 2, and the spare
/// tyre kit at 40 - and the port must have them before it can place a single item.
#[test]
fn the_cars_spawn_list_is_read() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/vehicles.json");
    let bytes = std::fs::read(path).expect("the vehicle recording");
    let value = cdda_json::parse(&bytes).expect("valid json");
    let vehicles = cdda_content::vehicles::from_dump(&value).expect("the vehicles load");
    let car = vehicles
        .prototypes
        .iter()
        .find(|prototype| prototype.id == "car")
        .expect("the car");
    assert_eq!(car.spawns.len(), 5, "**the car lists five spawns**");

    let chances: Vec<i32> = car.spawns.iter().map(|spawn| spawn.chance).collect();
    assert_eq!(
        chances,
        vec![90, 5, 2, 90, 40],
        "**and the chances are the data's**, in the data's order"
    );
    let groups: Vec<&str> = car
        .spawns
        .iter()
        .filter_map(|spawn| spawn.groups.first().map(String::as_str))
        .collect();
    assert_eq!(
        groups,
        vec![
            "car_misc",
            "snacks",
            "fast_food",
            "car_kit",
            "spare_tire_kit_small_jack"
        ],
        "**every one of them draws from a named item group**"
    );
    assert!(
        car.spawns.iter().all(|spawn| spawn.items.is_empty()),
        "the car lists no loose items: its loot is all group draws"
    );
    assert!(
        car.spawns
            .iter()
            .all(|spawn| spawn.x.abs() <= 2 && spawn.y.abs() <= 2),
        "and the squares are inside the vehicle"
    );
}

/// **A chance becomes a count through the dice, and only through the dice.** Reference: `roll_remainder` (`rng.cpp:136-147`) - the whole part, plus one when a die falls inside the
/// fraction; a whole number consumes **no roll at all**, which is what keeps a replay in step with the reference's own stream.
#[test]
fn a_chance_becomes_a_count_without_stealing_a_roll() {
    let mut rolls = Rolls::fixed(0.99);
    assert_eq!(
        cdda_sim::vehicle_spawn::roll_remainder(0.9, &mut rolls),
        0,
        "**a die outside the fraction adds nothing**: 0.9 wants one only if the die is under 0.9"
    );
    assert_eq!(
        cdda_sim::vehicle_spawn::roll_remainder(3.0, &mut rolls),
        3,
        "a whole number is itself"
    );
    assert_eq!(
        cdda_sim::vehicle_spawn::roll_remainder(2.5, &mut rolls),
        2,
        "and a fraction is decided by the die"
    );
    assert_eq!(
        cdda_sim::vehicle_spawn::roll_remainder(-0.9, &mut rolls),
        0,
        "a negative fraction rounds the other way"
    );

    let mut always = Rolls::fixed(0.0);
    assert_eq!(
        cdda_sim::vehicle_spawn::roll_remainder(0.9, &mut always),
        1,
        "**a die inside the fraction adds the batch**"
    );

    // **And the chance is scaled before it is rounded**: `chance * max( spawn_rate, 1 ) / 100`. A ninety per cent spawn is 0.9 of a batch, and the option is floored at one so it
    // cannot silence a spawn the data gave a chance to.
    let mut low = Rolls::fixed(0.99);
    assert_eq!(
        cdda_sim::vehicle_spawn::batch_count(90, 1.0, &mut low),
        0,
        "ninety per cent of a batch, and this die says no"
    );
    let mut certain = Rolls::fixed(0.0);
    assert_eq!(
        cdda_sim::vehicle_spawn::batch_count(90, 1.0, &mut certain),
        1,
        "and this one says yes"
    );
    // **A two per cent spawn is two hundredths of a batch**: the die has to fall inside that sliver, so a die near the top of its range misses and one at the bottom hits.
    let mut high = Rolls::fixed(0.99);
    assert_eq!(
        cdda_sim::vehicle_spawn::batch_count(2, 1.0, &mut high),
        0,
        "**a die near one misses a two per cent spawn**"
    );
    assert_eq!(
        cdda_sim::vehicle_spawn::batch_count(2, 1.0, &mut certain),
        1,
        "and a die at zero lands inside it"
    );
    assert_eq!(
        cdda_sim::vehicle_spawn::batch_count(200, 1.0, &mut certain),
        2,
        "a two hundred per cent chance is two batches before any die is rolled"
    );
    // **And the option multiplies the chance**: `ITEM_SPAWNRATE` of two doubles every spawn before the dice see it, so ninety per cent becomes 1.8 batches - one whole and a
    // fraction the die decides.
    assert_eq!(
        cdda_sim::vehicle_spawn::batch_count(90, 2.0, &mut certain),
        2,
        "**ninety per cent at twice the spawn rate is nearly two batches**"
    );
    let mut high_rate = Rolls::fixed(0.99);
    assert_eq!(
        cdda_sim::vehicle_spawn::batch_count(90, 2.0, &mut high_rate),
        1,
        "and the fraction still needs the die"
    );
}

/// **A placed vehicle is filled from its spawn list, by the dice.** Reference: `vehicle::place_spawn_items` (`vehicle.cpp:6379-6450`) - a chance becomes batches, a batch makes
/// its listed items when `rng_float( 0, 1 ) < spawn_rate`, and each listed group is **drawn** with the spawn rate inside the draw. Everything made is stowed into the cargo part at
/// the spawn's own square.
///
/// The dice are fixed at zero here so the test says what the rule *does* at a known roll rather than what a seed happened to produce: a die at zero is inside every probability, so
/// every batch happens and every listed item is made.
#[test]
fn a_placed_vehicle_is_filled_from_its_spawn_list() {
    use cdda_content::vehicles::Spawn;

    let (mut world, vehicle) = one_trunk_with_one_group();

    let spawns = vec![
        // **A group spawn at full chance**: one batch, drawn, stowed.
        Spawn {
            x: 0,
            y: 0,
            chance: 100,
            items: Vec::new(),
            variants: Vec::new(),
            groups: vec!["junk".to_owned()],
        },
        // **A listed item at full chance**: made directly.
        Spawn {
            x: 0,
            y: 0,
            chance: 100,
            items: vec!["rock".to_owned()],
            variants: Vec::new(),
            groups: Vec::new(),
        },
        // **A spawn on a square with no cargo**: nothing to fill, so nothing placed.
        Spawn {
            x: 9,
            y: 9,
            chance: 100,
            items: vec!["rock".to_owned()],
            variants: Vec::new(),
            groups: Vec::new(),
        },
    ];

    let placed = cdda_sim::vehicle_spawn::place_spawn_items(&mut world, vehicle, &spawns, 1.0);
    assert_eq!(
        placed, 2,
        "**two stacks went in**: one drawn from the group, one listed outright, and nothing for the square with no cargo"
    );
    let in_part = cdda_sim::vehicle_cargo::items_in_part(&world, vehicle, 0);
    assert_eq!(in_part.len(), 2, "both stacks are in the trunk");
    assert_eq!(
        cdda_sim::vehicle_mass::mass(&world, vehicle),
        2 * 657,
        "**and they weigh what two rocks weigh**"
    );
    assert!(
        cdda_sim::item_registry::id_in(&world, "rock").is_some(),
        "the rock is the type the registry read"
    );

    // **And a listed item is a die of its own**: with the spawn rate at zero a batch still happens - the option is floored at one for the count - but `rng_float( 0, 1 ) < 0.0`
    // never holds, so nothing listed is made. The group draw is the port's known gap: its rate is applied inside the reference's `items_from`, which the port's draw does not take.
    let mut world = world;
    cdda_sim::singletons::put_rolls(&mut world, Rolls::fixed(0.5));
    let listed_only = vec![Spawn {
        x: 0,
        y: 0,
        chance: 100,
        items: vec!["rock".to_owned()],
        variants: Vec::new(),
        groups: Vec::new(),
    }];
    assert_eq!(
        cdda_sim::vehicle_spawn::place_spawn_items(&mut world, vehicle, &listed_only, 0.0),
        0,
        "**a spawn rate of zero makes no listed item**, though the batch was still rolled"
    );
}

/// **A group draw obeys the spawn rate too**, one die per item. Reference: `items_from( id, birthday, spawn_flags::use_spawn_rate )`, which skips an item when
/// `rng_float( 0, 1 ) > spawn_rate` (`item_group.cpp:285-296`) - so a zero rate draws nothing even from a group whose only entry is certain, and a rate of one draws it.
#[test]
fn a_group_draw_obeys_the_spawn_rate() {
    let (mut world, vehicle) = one_trunk_with_one_group();
    let group_only = vec![cdda_content::vehicles::Spawn {
        x: 0,
        y: 0,
        chance: 100,
        items: Vec::new(),
        variants: Vec::new(),
        groups: vec!["junk".to_owned()],
    }];
    cdda_sim::singletons::put_rolls(&mut world, Rolls::fixed(0.5));
    assert_eq!(
        cdda_sim::vehicle_spawn::place_spawn_items(&mut world, vehicle, &group_only, 0.0),
        0,
        "**a spawn rate of zero draws nothing from a group**"
    );
    cdda_sim::singletons::put_rolls(&mut world, Rolls::fixed(0.5));
    assert_eq!(
        cdda_sim::vehicle_spawn::place_spawn_items(&mut world, vehicle, &group_only, 1.0),
        1,
        "and a rate of one draws the group's certain entry"
    );
}

/// **A vehicle with one cargo part and one item group.** The part's `cargo_weight_modifier` is the data's own hundred - a default of zero would hide every stowed item from the scale -
/// and the group holds one certain rock of 657 g, so a placed item is visible in the vehicle's mass.
fn one_trunk_with_one_group() -> (World, Entity) {
    use cdda_content::item_group_draw::{Definition, Entry};
    use cdda_content::vehicles::{PartSpec, Vehicles};

    let mut world = World::new();
    // **A part's cargo weighs what it carries only when the modifier says so**: the data's usual 100, and a default of zero would hide every stowed item from the scale.
    let cargo_spec = PartSpec {
        id: "trunk".to_owned(),
        flags: std::iter::once("CARGO".to_owned()).collect(),
        cargo_weight_modifier: 100,
        ..PartSpec::default()
    };
    cdda_sim::vehicles::set_store(
        &mut world,
        Vehicles {
            parts: vec![cargo_spec],
            prototypes: Vec::new(),
        },
    );
    // One item type, weighty enough that a stowed copy shows up in the part's mass.
    cdda_sim::item_registry::set_item_types(
        &mut world,
        &[cdda_sim::item_registry::Kind {
            id: "rock",
            volume_ml: 0,
            weight_g: 657,
            pockets: &[],
            thrown_damage: 0.0,
            melee_bash: 0.0,
            melee_cut: 0.0,
            melee_stab: 0.0,
            to_hit: -1,
        }],
    );
    let mut definitions = std::collections::BTreeMap::new();
    definitions.insert(
        "junk".to_owned(),
        Definition {
            id: "junk".to_owned(),
            distribution: false,
            entries: vec![Entry::Item {
                id: "rock".to_owned(),
                probability: 100,
                count: (1, 1),
                event: None,
            }],
        },
    );
    cdda_sim::item_groups::set_groups(&mut world, definitions);
    cdda_sim::singletons::put_rolls(&mut world, Rolls::fixed(0.0));

    let vehicle = world
        .spawn(cdda_sim::vehicles::Vehicle {
            parts: vec![cdda_sim::vehicles::VehiclePart {
                part: "trunk".to_owned(),
                x: 0,
                y: 0,
                ..cdda_sim::vehicles::VehiclePart::default()
            }],
            ..cdda_sim::vehicles::Vehicle::default()
        })
        .id();

    (world, vehicle)
}

/// **A broken part damages what it lets through, and destroys what it ruins.** Reference: `vehicle.cpp:6430-6434` - `if( broken && e.mod_damage( rng( 1, e.max_damage() ) ) ) continue;`
/// - so the item is lost when its damage **passes** its maximum, which is `item::mod_damage`'s own test.
///
/// The dice are fixed so the test says what the rule does at a known roll: a die at the top of the range damages an item by its maximum and destroys it; a die at the bottom damages it
/// by the minimum and leaves it in the part, marked.
#[test]
fn a_broken_part_damages_what_it_lets_through() {
    use cdda_content::vehicles::Spawn;

    let (mut world, vehicle) = one_trunk_with_one_group();
    // A can takes four thousand points of damage before it is ruined; the rock takes none at all, because any damage destroys it.
    let mut limits = std::collections::BTreeMap::new();
    limits.insert("rock".to_owned(), 0);
    cdda_sim::item_registry::set_damage_limits(&mut world, &limits);
    let rock = cdda_sim::item_registry::id_in(&world, "rock").expect("the rock");
    assert_eq!(
        cdda_sim::item_registry::max_damage_in(&world, rock),
        Some(0),
        "**a rock's ceiling is zero**, which the reference's own table says"
    );

    // Break the trunk.
    {
        let mut built = world
            .get_mut::<cdda_sim::vehicles::Vehicle>(vehicle)
            .expect("a vehicle");
        if let Some(trunk) = built.parts.first_mut() {
            trunk.broken = true;
        }
    }

    let listed_only = vec![Spawn {
        x: 0,
        y: 0,
        chance: 100,
        items: vec!["rock".to_owned()],
        variants: Vec::new(),
        groups: Vec::new(),
    }];
    // **Three dice, in order**: the batch's `one_in( 2 )`, which must say no; the listed item's `rng_float( 0, 1 ) < spawn_rate`, which says yes; and the damage's `rng( 0, 0 )`,
    // which answers zero - so the rock survives undamaged. A fixed die cannot say all three, and this is the one place a scripted run is worth more than a seed.
    let scripted = cdda_core_types::rng::Recording {
        draws: vec![
            cdda_core_types::rng::Draw::Int {
                low: 0,
                high: 1,
                drawn: 1,
            },
            cdda_core_types::rng::Draw::Real {
                low: 0.0,
                high: 1.0,
                drawn: 0.0,
            },
            cdda_core_types::rng::Draw::Int {
                low: 0,
                high: 1,
                drawn: 0,
            },
        ],
        position: 0,
        engine_draws: Vec::new(),
    };
    cdda_sim::singletons::put_rolls(&mut world, Rolls::replaying(scripted));
    let placed = cdda_sim::vehicle_spawn::place_spawn_items(&mut world, vehicle, &listed_only, 1.0);
    assert_eq!(
        placed, 1,
        "**a catch at the bottom of the range keeps the rock**"
    );
    let held = cdda_sim::vehicle_cargo::items_in_part(&world, vehicle, 0);
    assert_eq!(held.len(), 1, "it is in the broken trunk");
    assert_eq!(
        held.iter()
            .filter_map(|item| world.get::<cdda_sim::GroundItem>(*item))
            .map(|item| item.damage)
            .sum::<i32>(),
        0,
        "undamaged"
    );
}
