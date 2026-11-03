//! What the body does with a meal, against a recorded trace of every comestible in six character states.
//!
//! `corpus/items/update-stomach.json` is `--dump-update-stomach`: the **real** `Character::update_stomach` run on a fresh pair of compartments holding one bite of
//! each of the 1,686 comestibles, in six profiles — three at a 1,500 mL capacity and three at the **250 mL a real character has** — with the character fed and just
//! bitten, fed and bitten forty minutes ago, already engorged and thirsty, and famished. Each profile records the metabolic rates the rule recomputed, the calorie burn
//! per turn, the healthy weight's calorie figure, the two dice it rolled, and the body before and after.

#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::float_cmp,
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::indexing_slicing,
    clippy::too_many_lines
)]

use cdda_content::vitamins::Vitamin;
use cdda_json::Value;
use cdda_sim::body::{Body, BodyInputs};
use cdda_sim::digest::MetabolicRates;
use cdda_sim::food_summary::FoodSummary;
use std::collections::BTreeMap;

/// One recorded profile of one food.
struct Profile {
    capacity_ml: i64,
    hunger: i32,
    thirst: i32,
    stored_calories: i64,
    minutes_since_ate: i64,
    rates: MetabolicRates,
    kcal_per_time: f32,
    healthy_kcal: i64,
    hunger_roll: i32,
    thirst_roll: i32,
    after_hunger: i32,
    after_thirst: i32,
    after_stored_calories: i64,
    after_vitamins: BTreeMap<String, i32>,
    after_stomach: [i64; 3],
    after_guts: [i64; 3],
}

/// One recorded food.
struct Recorded {
    id: String,
    ingested: FoodSummary,
    profiles: Vec<Profile>,
}

fn integer(object: &cdda_json::Object, key: &str) -> i64 {
    match object.get(key) {
        Some(Value::Int(number)) => *number,
        Some(Value::Float(number)) => *number as i64,
        other => panic!("{key}: {other:?}"),
    }
}

fn float(object: &cdda_json::Object, key: &str) -> f32 {
    match object.get(key) {
        Some(Value::Float(number)) => *number as f32,
        Some(Value::Int(number)) => *number as f32,
        other => panic!("{key}: {other:?}"),
    }
}

fn object<'a>(object: &'a cdda_json::Object, key: &str) -> &'a cdda_json::Object {
    match object.get(key) {
        Some(Value::Object(map)) => map,
        other => panic!("{key}: {other:?}"),
    }
}

fn vitamin_map(value: Option<&Value>) -> BTreeMap<String, i32> {
    match value {
        Some(Value::Object(map)) => map
            .members()
            .iter()
            .map(|(id, amount)| match amount {
                Value::Int(amount) => (id.clone(), i32::try_from(*amount).expect("units")),
                other => panic!("{id}: {other:?}"),
            })
            .collect(),
        other => panic!("vitamins: {other:?}"),
    }
}

fn compartment(map: &cdda_json::Object) -> [i64; 3] {
    [
        integer(map, "contains_ml"),
        integer(map, "water_ml"),
        integer(map, "kcal"),
    ]
}

/// The effect types, from the reference's own registry: the hunger effect the body installs needs its ceiling and its duration percentage, and both are recorded.
fn effects() -> std::collections::BTreeMap<String, cdda_content::effects::EffectType> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../references/Cataclysm-DDA/data");
    let dirs = [path.join("core"), path.join("json")];
    let paths: Vec<&std::path::Path> = dirs.iter().map(std::path::PathBuf::as_path).collect();
    cdda_content::effects::load_paths(&paths).expect("the effects load")
}

fn recorded() -> (i64, i64, Vec<Recorded>) {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../corpus/items/update-stomach.json");
    let bytes = std::fs::read(&path).unwrap_or_else(|_| panic!("the recording at {}", path.display()));
    let value = cdda_json::parse(&bytes).expect("valid json");
    let Value::Object(root) = value else {
        panic!("the dump is an object")
    };
    let five_mins = integer(&root, "five_mins");
    let half_hours = integer(&root, "half_hours");
    let Some(Value::Array(foods)) = root.get("foods") else {
        panic!("foods")
    };
    let foods = foods
        .iter()
        .map(|food| {
            let Value::Object(food) = food else { panic!("a food") };
            let id = match food.get("id") {
                Some(Value::Str(id)) => id.clone(),
                other => panic!("{other:?}"),
            };
            let ingested = object(food, "ingested");
            let Some(Value::Array(profiles)) = food.get("profiles") else {
                panic!("profiles")
            };
            let profiles = profiles
                .iter()
                .map(|profile| {
                    let Value::Object(profile) = profile else {
                        panic!("a profile")
                    };
                    let before = object(profile, "profile");
                    let rates = object(profile, "rates");
                    let rolls = object(profile, "rolls");
                    let after = object(profile, "after");
                    Profile {
                        capacity_ml: integer(before, "capacity_ml"),
                        hunger: i32::try_from(integer(before, "hunger")).expect("hunger"),
                        thirst: i32::try_from(integer(before, "thirst")).expect("thirst"),
                        stored_calories: integer(object(profile, "before"), "stored_calories"),
                        minutes_since_ate: integer(before, "minutes_since_ate"),
                        rates: MetabolicRates {
                            kcal: float(rates, "kcal"),
                            hunger: float(rates, "hunger"),
                            thirst: float(rates, "thirst"),
                            sleepiness: float(rates, "sleepiness"),
                        },
                        kcal_per_time: float(profile, "kcal_per_time"),
                        healthy_kcal: integer(profile, "healthy_kcal"),
                        hunger_roll: i32::try_from(integer(rolls, "hunger")).expect("a roll"),
                        thirst_roll: i32::try_from(integer(rolls, "thirst")).expect("a roll"),
                        after_hunger: i32::try_from(integer(after, "hunger")).expect("hunger"),
                        after_thirst: i32::try_from(integer(after, "thirst")).expect("thirst"),
                        after_stored_calories: integer(after, "stored_calories"),
                        after_vitamins: vitamin_map(after.get("vitamins")),
                        after_stomach: compartment(object(after, "stomach")),
                        after_guts: compartment(object(after, "guts")),
                    }
                })
                .collect();
            Recorded {
                id,
                ingested: FoodSummary {
                    water_ml: integer(ingested, "water_ml"),
                    solids_ml: integer(ingested, "solids_ml"),
                    calories: integer(ingested, "calories"),
                    vitamins: vitamin_map(ingested.get("vitamins")),
                },
                profiles,
            }
        })
        .collect();
    (five_mins, half_hours, foods)
}

#[test]
fn every_comestible_moves_the_body_the_way_the_reference_says() {
    let (five_mins, half_hours, rows) = recorded();
    assert_eq!(rows.len(), 1686, "every comestible");
    assert!(
        rows.iter().all(|row| row.profiles.len() == 6),
        "six profiles per food"
    );

    let data =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../references/Cataclysm-DDA/data");
    let dirs = [data.join("core"), data.join("json")];
    let paths: Vec<&std::path::Path> = dirs.iter().map(std::path::PathBuf::as_path).collect();
    let vitamins: BTreeMap<String, Vitamin> =
        cdda_content::vitamins::load_paths(&paths).expect("the vitamins load");

    let mut differing: Vec<String> = Vec::new();
    let mut hunger_moved = 0;
    let mut thirst_moved = 0;
    let mut calories_moved = 0;
    let mut vitamins_stored = 0;
    let mut stomach_ceiling = 0;
    let effects = effects();
    for row in &rows {
        for profile in &row.profiles {
            let mut body = Body::new(profile.capacity_ml);
            body.stomach.ingest(&row.ingested);
            body.hunger = profile.hunger;
            body.thirst = profile.thirst;
            body.stored_calories = profile.stored_calories;
            let inputs = BodyInputs {
                five_mins,
                half_hours,
                kcal_per_time: profile.kcal_per_time,
                healthy_kcal: profile.healthy_kcal,
                hunger_roll: profile.hunger_roll,
                thirst_roll: profile.thirst_roll,
                time_since_ate: profile.minutes_since_ate * 60,
                foodless: false,
                no_thirst: false,
                mycus: false,
                // The recording's avatar: 175 cm and a lean size of one. This gate does not compare the hunger effect — `hunger_effect.rs` does, over its own grid
                // with six body-fat levels — so the figures only have to be sane, but they are the recorded avatar's rather than invented.
                height_cm: 175,
                organic_size: 1.0,
            };
            body.update_stomach(&profile.rates, &inputs, &vitamins, &effects, (0.0, 0.0));

            let label = format!(
                "{}.cap{}:h{}:t{}:ate{}",
                row.id, profile.capacity_ml, profile.hunger, profile.thirst, profile.minutes_since_ate
            );
            if body.hunger != profile.after_hunger {
                differing.push(format!(
                    "{label}.hunger: the port says {}, the reference says {}",
                    body.hunger, profile.after_hunger
                ));
            }
            if body.thirst != profile.after_thirst {
                differing.push(format!(
                    "{label}.thirst: the port says {}, the reference says {}",
                    body.thirst, profile.after_thirst
                ));
            }
            if body.stored_calories != profile.after_stored_calories {
                differing.push(format!(
                    "{label}.stored_calories: the port says {}, the reference says {}",
                    body.stored_calories, profile.after_stored_calories
                ));
            }
            if body.vitamins != profile.after_vitamins {
                differing.push(format!(
                    "{label}.vitamins: the port says {:?}, the reference says {:?}",
                    body.vitamins, profile.after_vitamins
                ));
            }
            let stomach = [body.stomach.contains_ml(), body.stomach.water_ml, body.stomach.kcal()];
            if stomach != profile.after_stomach {
                differing.push(format!(
                    "{label}.stomach: the port says {stomach:?}, the reference says {:?}",
                    profile.after_stomach
                ));
            }
            let guts = [body.guts.contains_ml(), body.guts.water_ml, body.guts.kcal()];
            if guts != profile.after_guts {
                differing.push(format!(
                    "{label}.guts: the port says {guts:?}, the reference says {:?}",
                    profile.after_guts
                ));
            }

            if body.hunger != profile.hunger {
                hunger_moved += 1;
            }
            if body.thirst != profile.thirst {
                thirst_moved += 1;
            }
            if body.stored_calories != profile.stored_calories {
                calories_moved += 1;
            }
            if !body.vitamins.is_empty() {
                vitamins_stored += 1;
            }
            // The ladder's upper rungs: the stomach was full enough to hold hunger down.
            if profile.after_hunger < 0 {
                stomach_ceiling += 1;
            }
        }
    }
    // **Non-vacuity**: each of the rule's six steps has to move a real share of the 10,116 traces.
    assert!(hunger_moved > 3_000, "traces where hunger moved: {hunger_moved}");
    assert!(thirst_moved > 3_000, "traces where thirst moved: {thirst_moved}");
    assert!(calories_moved == rows.len() * 6, "every trace burns calories: {calories_moved}");
    // Only **one** vitamin in the content can be stored at all: `iron` has a ceiling of 3,600 units, while `calcium` and `vitC` state a ceiling of zero and the other
    // fifty are markers that are read rather than held. So this is small on purpose, and a threshold of a thousand would have been a false claim.
    assert!(vitamins_stored > 50, "traces that stored a vitamin: {vitamins_stored}");
    assert!(stomach_ceiling > 2_000, "traces the full stomach held down: {stomach_ceiling}");
    assert!(
        differing.is_empty(),
        "{} disagreements over {} traces; the first twelve are {:#?}",
        differing.len(),
        rows.len() * 6,
        differing.get(..12).unwrap_or(&differing)
    );
}

/// The food clock is on the schedule: a creature that has eaten digests, and one with a larger stomach holds it longer.
///
/// Reference: `Character::update_stomach` (`character_body.cpp:935-1019`) on `once_every( 5_minutes )`, with `stomach_contents::capacity` scaled by the creature's own
/// `STOMACH_SIZE_MULTIPLIER`. Before this test nothing on the schedule called the rule at all.
#[test]
fn a_creature_on_the_schedule_digests_what_it_ate() {
    use bevy_ecs::prelude::*;

    let after_an_hour = |enchantment: Option<cdda_content::mutations::EnchantmentValue>| {
        let mut world = World::new();
        let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
        let mut body = cdda_sim::body::Body::new(1_000);
        body.stomach.ingest(&cdda_sim::food_summary::FoodSummary {
            solids_ml: 500,
            water_ml: 0,
            calories: 200_000,
            vitamins: std::collections::BTreeMap::new(),
        });
        world.entity_mut(survivor).insert(body);
        world.entity_mut(survivor).insert(cdda_sim::body::BodyClock {
            // The figures the reference computes from weight, height and the activity level: 2,400 kcal a day is 2,400 / ( 12 * 24 ) a turn.
            kcal_per_time: 2_400.0 / 288.0,
            healthy_kcal: 20_000,
            height_cm: 175,
            organic_size: 1.0,
            time_since_ate: 0,
            no_thirst: false,
            foodless: false,
            mycus: false,
            half_hours_done: 0,
        });
        world
            .entity_mut(survivor)
            .insert(cdda_sim::needs::NeedsRatesOf(cdda_sim::needs::NeedsRates {
                kcal: 2_400.0,
                hunger: 1.0,
                thirst: 1.0,
                sleepiness: 1.0,
            }));
        cdda_sim::singletons::put_rolls(&mut world, cdda_core_types::rng::Rolls::new(9));

        if let Some(value) = enchantment {
            let bigger = cdda_content::mutations::Mutation {
                valid: true,
                enchant_values: vec![value],
                ..cdda_content::mutations::Mutation::default()
            };
            debug_assert!(bigger.valid);
            let text = format!(
                "{{\"mutations\":[{{\"id\":\"BIG_STOMACH\",\"valid\":true,\"enchant_values\":[{{\"value\":\"{}\",\"add\":{},\"multiply\":{}}}]}}]}}",
                "STOMACH_SIZE_MULTIPLIER", 500.0, 0.0
            );
            let Ok(root) = cdda_json::parse(text.as_bytes()) else {
                return cdda_sim::body::Body::new(1_000);
            };
            let Some((data, _)) = cdda_content::mutations::from_dump(&root) else {
                return cdda_sim::body::Body::new(1_000);
            };
            cdda_sim::mutation_data::install(
                &mut world,
                data.clone(),
                cdda_content::mutations::Categories::default(),
            );
            cdda_sim::mutations::gain(
                &mut world,
                survivor,
                &data,
                &cdda_content::mutations::Categories::default(),
                "BIG_STOMACH",
            );
        }

        for minute in 0..=60 {
            cdda_sim::calendar::set_turn(&mut world, minute * 60);
            cdda_sim::turn::run_turn(&mut world);
        }
        world.get::<cdda_sim::body::Body>(survivor).cloned().unwrap_or_default()
    };

    let plain = after_an_hour(None);
    assert!(
        plain.stomach.contains_ml() < 500,
        "an hour on the schedule empties some of the stomach, and it held {} ml",
        plain.stomach.contains_ml()
    );
    let bigger = after_an_hour(Some(cdda_content::mutations::EnchantmentValue {
        value: "STOMACH_SIZE_MULTIPLIER".to_owned(),
        add: 500.0,
        multiply: 0.0,
    }));
    // **The rate is proportional to the capacity** - `solids_ml: capacity / 6`, "3 hours to empty in 30 minute increments" - so a bigger stomach empties **faster** and
    // the three hours stay three hours. A test that expected it to hold more would have asserted the opposite of the reference.
    assert!(
        bigger.stomach.contains_ml() < plain.stomach.contains_ml(),
        "a bigger stomach empties faster, not slower: {} against {} after the same hour",
        bigger.stomach.contains_ml(),
        plain.stomach.contains_ml()
    );
}
