//! What a placed vehicle finds in its cargo: the spawn list it was placed with, and the count a chance becomes.

#![allow(clippy::expect_used, clippy::panic)]

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
