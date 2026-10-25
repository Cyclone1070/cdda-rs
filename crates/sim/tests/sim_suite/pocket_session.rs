//! A container filling up, step by step, against the reference's own record.
//!
//! `corpus/items/pocket-session.json` is `--dump-pocket-session`: a scripted sequence of `item::put_in` calls on one container at a time, and after every step what the
//! container **weighs**, what it **takes up**, how much **room** it has left and how much its contents add to its size. The decision and its consequence are recorded
//! together, which is what makes it a session rather than a table of numbers.
//!
//! This file replays the **backpack** half: three rocks and a plank fit, and the reference then refuses a bottle of water because a backpack cannot hold liquid. The port
//! models the first part - volume against capacity, contents counted in the container's own weight and volume - and not the second, which is a pocket's phase rule and is
//! **declared** rather than quietly passed.

#![allow(clippy::expect_used, clippy::panic)]

use bevy_ecs::prelude::*;
use cdda_sim::{
    Container, GroundItem, ItemTypeId, TypeVolume, TypeWeight, drop_item, insert_into, item_volume,
    item_weight,
};
use std::collections::BTreeMap;

/// One recorded step: what was tried, what the reference answered, and what the container looked like afterwards.
struct Step {
    container: String,
    payload: String,
    ok: bool,
}

/// The backpack's five recorded steps, in the order the reference ran them.
fn steps() -> Vec<Step> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/items/pocket-session.json");
    let bytes = std::fs::read(&path).expect("the pocket session");
    let cdda_json::Value::Object(root) = cdda_json::parse(&bytes).expect("valid JSON") else {
        panic!("the session is an object")
    };
    let Some(cdda_json::Value::Array(all)) = root.get("steps") else {
        panic!("the session holds steps")
    };
    all.iter()
        .map(|step| {
            let cdda_json::Value::Object(step) = step else {
                panic!("a step")
            };
            let text = |key: &str| match step.get(key) {
                Some(cdda_json::Value::Str(value)) => value.clone(),
                other => panic!("{key} is text: {other:?}"),
            };

            Step {
                container: text("container"),
                payload: text("payload"),
                ok: matches!(step.get("ok"), Some(cdda_json::Value::Bool(true))),
            }
        })
        .collect()
}

/// What the prototypes say about each type the session mentions, in the units the sim asks for.
struct Table(BTreeMap<String, (TypeWeight, TypeVolume)>);

impl Table {
    fn load() -> Self {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/prototypes/items.json");
        let bytes = std::fs::read(&path).expect("the prototypes");
        let cdda_json::Value::Object(root) = cdda_json::parse(&bytes).expect("valid JSON") else {
            panic!("the dump is an object")
        };
        let Some(cdda_json::Value::Array(types)) = root.get("prototypes") else {
            panic!("the dump holds prototypes")
        };
        let mut table = BTreeMap::new();
        for entry in types {
            let cdda_json::Value::Object(entry) = entry else {
                panic!("a prototype")
            };
            let whole = |key: &str| match entry.get(key) {
                Some(cdda_json::Value::Int(value)) => *value,
                other => panic!("{key} is a whole number: {other:?}"),
            };
            let Some(cdda_json::Value::Str(id)) = entry.get("id") else {
                panic!("an id")
            };
            let no_drop = match entry.get("flags") {
                Some(cdda_json::Value::Array(flags)) => flags
                    .iter()
                    .any(|flag| matches!(flag, cdda_json::Value::Str(name) if name == "NO_DROP")),
                _ => false,
            };
            table.insert(
                id.clone(),
                (
                    TypeWeight {
                        milligrams: whole("weight_mg"),
                        integral_milligrams: whole("integral_weight_mg"),
                        count_by_charges: matches!(
                            entry.get("count_by_charges"),
                            Some(cdda_json::Value::Bool(true))
                        ),
                        no_drop,
                    },
                    TypeVolume {
                        millilitres: whole("volume"),
                        integral_millilitres: whole("integral_volume"),
                        count_by_charges: matches!(
                            entry.get("count_by_charges"),
                            Some(cdda_json::Value::Bool(true))
                        ),
                        stack_size: whole("stack_size"),
                    },
                ),
            );
        }
        Self(table)
    }

    /// Hands the sim the facts it asks for by number, from what the data says by name.
    fn numbered(
        &self,
        order: &[String],
    ) -> (
        BTreeMap<String, ItemTypeId>,
        impl Fn(ItemTypeId) -> TypeWeight + use<>,
        impl Fn(ItemTypeId) -> TypeVolume + use<>,
    ) {
        let kinds: BTreeMap<String, ItemTypeId> = order
            .iter()
            .enumerate()
            .map(|(index, id)| (id.clone(), ItemTypeId(u32::try_from(index).expect("an index"))))
            .collect();
        let by_kind: BTreeMap<u32, (TypeWeight, TypeVolume)> = order
            .iter()
            .enumerate()
            .map(|(index, id)| {
                let facts = self.0.get(id).copied().expect("a measured type");
                (u32::try_from(index).expect("an index"), facts)
            })
            .collect();
        let weights = by_kind.clone();
        (
            kinds,
            move |kind| weights.get(&kind.0).expect("a measured type").0,
            move |kind| by_kind.get(&kind.0).expect("a measured type").1,
        )
    }
}


/// What the data says a container's pockets hold, and what they will accept.
struct Pockets(BTreeMap<String, Vec<cdda_sim::Pocket>>);

impl Pockets {
    fn load() -> Self {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/items/pockets.json");
        let bytes = std::fs::read(&path).expect("the pockets");
        let cdda_json::Value::Object(root) = cdda_json::parse(&bytes).expect("valid JSON") else {
            panic!("the dump is an object")
        };
        let Some(cdda_json::Value::Array(types)) = root.get("types") else {
            panic!("the dump holds types")
        };
        let mut table = BTreeMap::new();
        for entry in types {
            let cdda_json::Value::Object(entry) = entry else {
                panic!("a type")
            };
            let Some(cdda_json::Value::Str(id)) = entry.get("id") else {
                panic!("an id")
            };
            let Some(cdda_json::Value::Array(pockets)) = entry.get("pockets") else {
                panic!("pockets")
            };
            let mut kept: Vec<cdda_sim::Pocket> = Vec::new();
            for pocket in pockets {
                let cdda_json::Value::Object(pocket) = pocket else {
                    panic!("a pocket")
                };
                if !matches!(pocket.get("kind"), Some(cdda_json::Value::Str(kind)) if kind == "CONTAINER") {
                    continue;
                }
                // **A container with several pockets holds what they all hold**, and accepts a liquid if any one of them does - which is how the reference's
                // `item_contents::insert_item` behaves when it offers the payload to each pocket in turn.
                kept.push(cdda_sim::Pocket {
                    capacity_ml: match pocket.get("max_volume") {
                        Some(cdda_json::Value::Int(volume)) => *volume,
                        other => panic!("max_volume is a whole number: {other:?}"),
                    },
                    weight_capacity_grams: match pocket.get("max_weight") {
                        Some(cdda_json::Value::Int(weight)) => *weight,
                        other => panic!("max_weight is a whole number: {other:?}"),
                    },
                    watertight: matches!(
                        pocket.get("watertight"),
                        Some(cdda_json::Value::Bool(true))
                    ),
                    airtight: matches!(pocket.get("airtight"), Some(cdda_json::Value::Bool(true))),
                    // The reference's compatibility limits, recorded with the rest of the pocket, so the port asks the questions the data actually states:
                    // 2,502 of the corpus's pockets carry at least one of them.
                    max_item_length_mm: match pocket.get("max_length") {
                        Some(cdda_json::Value::Int(length)) => *length,
                        other => panic!("max_length is a whole number: {other:?}"),
                    },
                    min_item_length_mm: match pocket.get("min_length") {
                        Some(cdda_json::Value::Int(length)) => *length,
                        other => panic!("min_length is a whole number: {other:?}"),
                    },
                    min_item_volume_ml: match pocket.get("min_volume") {
                        Some(cdda_json::Value::Int(volume)) => *volume,
                        other => panic!("min_volume is a whole number: {other:?}"),
                    },
                    items: Vec::new(),
                    flags: Vec::new(),
                    materials: Vec::new(),
                    holster: false,
                    max_item_volume_ml: None,
                });
            }
            table.insert(id.clone(), kept);
        }
        Self(table)
    }

    fn of(&self, id: &str) -> Vec<cdda_sim::Pocket> {
        self.0
            .get(id)
            .unwrap_or_else(|| panic!("{id} has no pockets entry"))
            .clone()
    }
}

/// One step of the recording, replayed: what the reference answered, and what the port answered.
struct Played {
    payload: String,
    recorded_ok: bool,
    placed: Result<(), cdda_sim::InsertError>,
    weight: i64,
    volume: i64,
    room: i64,
}

/// Replays every step the port models and hands back what each one did, in order.
///
/// The recording is one script: the backpack, then a bottle (not modelled yet), then the backpack **again from empty** for the two pocket-limit steps — so the replay
/// starts a new container wherever the recording does.
fn replay() -> Vec<Played> {
    let steps = steps();
    assert_eq!(steps.len(), 14, "the recording has fourteen steps");
    let table = Table::load();
    let pockets = Pockets::load();
    let order = vec![
        "backpack".to_owned(),
        "rock".to_owned(),
        "2x4".to_owned(),
        "water_clean".to_owned(),
        "log".to_owned(),
        "bundle_branch".to_owned(),
        "art_crystal".to_owned(),
        "manhole_cover".to_owned(),
        "huge_atomic_battery_cell".to_owned(),
    ];
    let (kinds, weight_of, volume_of) = table.numbered(&order);
    let kind_of = |name: &str| *kinds.get(name).unwrap_or_else(|| panic!("{name} is not in the session"));

    let mut world = World::new();
    let mut container = Entity::PLACEHOLDER;
    let mut current = String::new();
    let mut played = Vec::new();
    for step in &steps {
        if step.container != current {
            current.clone_from(&step.container);
            // A container the replay does not model yet still breaks the sequence, so the next backpack starts empty.
            if step.container == "backpack" {
                container = drop_item(&mut world, (5, 5), kind_of("backpack"), 1);
                world
                    .entity_mut(container)
                    .insert(Container::with_pockets(pockets.of("backpack")));
            }
        }
        if step.container != "backpack" {
            // The bottle's steps are the next round's work: a liquid is stored as charges and the port does not model that yet.
            continue;
        }
        let payload = drop_item(&mut world, (5, 5), kind_of(&step.payload), 1);
        // **The volumes are measured before the question is asked**, because a container's own volume depends on what is already inside it, and the sim asks about items
        // it cannot hand back to the caller.
        let mut volumes: BTreeMap<Entity, i64> = BTreeMap::new();
        let mut weights: BTreeMap<Entity, i64> = BTreeMap::new();
        let entities: Vec<Entity> = world
            .query::<(Entity, &GroundItem)>()
            .iter(&world)
            .map(|(entity, _)| entity)
            .collect();
        for entity in entities {
            let measured = item_volume(&mut world, entity, &volume_of);
            volumes.insert(entity, measured);
            let weighed = item_weight(&mut world, entity, &weight_of);
            weights.insert(entity, weighed);
        }
        let outcome = insert_into(&mut world, container, payload, |entity| {
            let room = volumes.get(&entity).copied().unwrap_or(0);
            let weigh = weights.get(&entity).copied().unwrap_or(0);
            if step.payload == "water_clean" {
                cdda_sim::ItemFacts::liquid(room)
            } else {
                cdda_sim::ItemFacts::solid_weighing(room, weigh)
            }
        });
        let used: i64 = cdda_sim::contents(&mut world, container)
            .into_iter()
            .map(|inside| item_volume(&mut world, inside, &volume_of))
            .sum();
        let capacity: i64 = pockets
            .of("backpack")
            .iter()
            .map(|pocket| pocket.capacity_ml)
            .sum();
        played.push(Played {
            payload: step.payload.clone(),
            recorded_ok: step.ok,
            placed: outcome,
            weight: item_weight(&mut world, container, &weight_of),
            volume: item_volume(&mut world, container, &volume_of),
            room: capacity - used,
        });
    }
    played
}

#[test]
fn the_backpack_loads_the_way_the_reference_recorded_it() {
    let played = replay();
    let accepted: Vec<&Played> = played.iter().filter(|step| step.recorded_ok).collect();
    assert_eq!(accepted.len(), 5, "five steps were accepted, all of them by the port too");
    for step in &accepted {
        assert!(
            step.placed.is_ok(),
            "the port takes the {} as the reference did: {:?}",
            step.payload,
            step.placed
        );
    }
    // The recording's own numbers, step by step: three rocks at 657 g and 250 ml, then a plank at 2440 g and 4130 ml.
    let weights: Vec<i64> = accepted.iter().map(|step| step.weight).collect();
    assert_eq!(weights, vec![1290, 1947, 2604, 5044, 1033], "the container's own weight at each step");
    let volumes: Vec<i64> = accepted.iter().map(|step| step.volume).collect();
    assert_eq!(volumes, vec![2250, 2500, 2750, 6880, 2500], "and its own volume");
    let rooms: Vec<i64> = accepted.iter().map(|step| step.room).collect();
    assert_eq!(rooms, vec![35260, 35010, 34760, 30630, 35010], "and the room it had left");
}

#[test]
fn a_pocket_is_its_own_limit_and_a_liquid_needs_a_watertight_one() {
    let played = replay();
    let of = |payload: &str| {
        played
            .iter()
            .find(|step| step.payload == payload)
            .unwrap_or_else(|| panic!("the recording has a {payload} step"))
    };
    // **A log will not fit**: 50,000 ml against the 20,120 ml the biggest pocket has left after three rocks and a plank. The number named is the largest pocket's
    // room, not the 30,630 ml the whole backpack has left.
    let log = of("log");
    assert!(!log.recorded_ok, "the reference refuses the log");
    assert_eq!(
        log.placed,
        Err(cdda_sim::InsertError::TooBig {
            remaining_ml: 20_120,
            wanted_ml: 50_000
        })
    );
    // **And a branch bigger than any one pocket is refused too**, though it would fit the backpack's pockets added together: 30,000 ml against 25,000, 510, 5,000 and
    // 5,000. A pooled capacity takes it; a pocket is its own limit.
    let branch = of("bundle_branch");
    assert!(!branch.recorded_ok, "the reference refuses the 30,000 ml branch");
    assert_eq!(
        branch.placed,
        Err(cdda_sim::InsertError::TooBig {
            remaining_ml: 25_000,
            wanted_ml: 30_000
        }),
        "the largest pocket's room, not the sum of the four"
    );
    // **The phase rule**: no pocket of the backpack is watertight, so the water is refused for its phase and not for room.
    let water = of("water_clean");
    assert!(!water.recorded_ok, "the reference refuses the water");
    assert_eq!(
        water.placed,
        Err(cdda_sim::InsertError::WrongPhase {
            phase: cdda_sim::Phase::Liquid
        })
    );
    // **Heavy, not big.** A manhole cover is 20,000 ml - inside the 25,000 ml pocket - and 115 kg, which the pocket's 30 kg will not take; an atomic battery cell is
    // 20,000 ml and 34 kg, which it will not either. The reference's own words are "item is too heavy, item is too long, or item is too short" - it names three limits in
    // one message and cannot say which one blocked - so the gate asserts the refusal and the port's reason, and the length and shortness limits are **declared** below.
    for payload in ["manhole_cover", "huge_atomic_battery_cell"] {
        let step = of(payload);
        assert!(!step.recorded_ok, "the reference refuses the {payload}");
        assert!(
            matches!(step.placed, Err(cdda_sim::InsertError::TooHeavy { .. })),
            "and the port refuses it for its weight: {:?}",
            step.placed
        );
    }
    // And one that fits a single pocket **exactly** (a 500 ml crystal in the 510 ml pocket) is accepted.
    let crystal = of("art_crystal");
    assert!(crystal.recorded_ok, "the reference takes the crystal");
    assert!(crystal.placed.is_ok(), "and so does the port: {:?}", crystal.placed);
}

#[test]
fn a_liquid_needs_a_watertight_pocket() {
    // **The phase rule** (`src/item_pocket.cpp:1396-1401`): a liquid may not go into a pocket that is not `watertight`, and a gas needs `airtight`. The reference's own
    // words for the refusal are "can't contain liquid" and "can't contain gas".
    //
    // The data is what makes this more than a flag: the backpack in `corpus/items/pockets.json` has four container pockets and **none of them is watertight**, while the
    // plastic bottle has one that is - so the same water is refused by one container and accepted by the other, with no rule differing between the two calls.
    let mut world = World::new();
    let backpack = drop_item(&mut world, (5, 5), ItemTypeId(40), 1);
    world.entity_mut(backpack).insert(Container::new(35_510));
    let bottle = drop_item(&mut world, (5, 5), ItemTypeId(41), 1);
    world.entity_mut(bottle).insert(Container::new(500).watertight());
    let water = drop_item(&mut world, (5, 5), ItemTypeId(42), 1);
    let facts = |entity: Entity| {
        if entity == water {
            cdda_sim::ItemFacts::liquid(250)
        } else {
            cdda_sim::ItemFacts::solid(1_000)
        }
    };
    let refused = insert_into(&mut world, backpack, water, facts);
    assert!(
        matches!(refused, Err(cdda_sim::InsertError::WrongPhase { .. })),
        "a backpack may not hold water: {refused:?}"
    );
    insert_into(&mut world, bottle, water, facts).expect("but a bottle may, because it is watertight");
}
