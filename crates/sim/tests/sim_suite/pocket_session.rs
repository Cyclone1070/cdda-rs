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
    payload: String,
    ok: bool,
    weight: i64,
    volume: i64,
    room: i64,
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
        .filter_map(|step| {
            let cdda_json::Value::Object(step) = step else {
                panic!("a step")
            };
            let text = |key: &str| match step.get(key) {
                Some(cdda_json::Value::Str(value)) => value.clone(),
                other => panic!("{key} is text: {other:?}"),
            };
            let whole = |key: &str| match step.get(key) {
                Some(cdda_json::Value::Int(value)) => *value,
                other => panic!("{key} is a whole number: {other:?}"),
            };
            if text("container") != "backpack" {
                return None;
            }
            Some(Step {
                payload: text("payload"),
                ok: matches!(step.get("ok"), Some(cdda_json::Value::Bool(true))),
                weight: whole("container_weight"),
                volume: whole("container_volume"),
                room: whole("room"),
            })
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

#[test]
fn a_backpack_fills_up_the_way_the_reference_recorded_it() {
    let steps = steps();
    assert_eq!(steps.len(), 6, "the backpack half of the session has six steps");
    let table = Table::load();
    // Every id the session names, numbered so the sim can ask about them.
    let order = vec![
        "backpack".to_owned(),
        "rock".to_owned(),
        "2x4".to_owned(),
        "water_clean".to_owned(),
        "log".to_owned(),
    ];
    let (kinds, weight_of, volume_of) = table.numbered(&order);

    let mut world = World::new();
    let kind_of = |name: &str| *kinds.get(name).unwrap_or_else(|| panic!("{name} is not in the session"));
    let container = drop_item(&mut world, (5, 5), kind_of("backpack"), 1);
    // The backpack's capacity is what the reference's own room numbers add up to: 250 ml per rock, then 4130 for the plank.
    world.entity_mut(container).insert(Container {
        capacity_ml: 35_510,
    });
    let mut compared = 0_usize;
    for (index, step) in steps.iter().enumerate() {
        let kind = kind_of(&step.payload);
        let payload = drop_item(&mut world, (5, 5), kind, 1);
        // **The volumes are measured before the question is asked**, because the sim asks about items it cannot hand back to the caller - and a container's volume
        // depends on what is already inside it, so it is measured fresh at every step.
        let mut volumes: BTreeMap<Entity, i64> = BTreeMap::new();
        let entities: Vec<Entity> = world
            .query::<(Entity, &GroundItem)>()
            .iter(&world)
            .map(|(entity, _)| entity)
            .collect();
        for entity in entities {
            let measured = item_volume(&mut world, entity, &volume_of);
            volumes.insert(entity, measured);
        }
        let placed = insert_into(&mut world, container, payload, |entity| {
            volumes.get(&entity).copied().unwrap_or(0)
        });
        if step.payload == "log" {
            // **A log will not fit**, and the port's capacity rule says so on its own: 50,000 ml into the 30,630 ml the backpack has left. This is the step that makes
            // the capacity load-bearing - without it the container would pass whatever number it was given.
            assert!(!step.ok, "the reference refuses the log");
            let error = placed.expect_err("and so does the port");
            assert!(
                matches!(
                    error,
                    cdda_sim::InsertError::TooBig {
                        remaining_ml: 30_630,
                        wanted_ml: 50_000
                    }
                ),
                "with the room and the want the reference implies: {error:?}"
            );
            continue;
        }
        if step.payload == "water_clean" {
            // **The declared difference.** The reference refuses a liquid in a backpack because a pocket of that kind may not hold one
            // (`item_pocket::can_contain`, `src/item_pocket.cpp`), and the port has no phase rule yet, so it takes the water. The gate says so out loud rather than
            // skipping the step.
            assert!(!step.ok, "the reference refuses the water");
            assert!(
                placed.is_ok(),
                "and the port takes it, which is the declared gap: no phase rule on a pocket"
            );
            continue;
        }
        assert!(step.ok, "the reference accepts step {index} ({})", step.payload);
        placed.unwrap_or_else(|error| panic!("step {index} ({}) failed: {error:?}", step.payload));
        assert_eq!(
            item_weight(&mut world, container, &weight_of),
            step.weight,
            "step {index} ({}): the container weighs what the reference says",
            step.payload
        );
        assert_eq!(
            item_volume(&mut world, container, &volume_of),
            step.volume,
            "step {index} ({}): and takes up what the reference says",
            step.payload
        );
        let accepted_so_far = steps.iter().take(index + 1);
        let used: i64 = accepted_so_far
                .filter(|earlier| earlier.ok)
                .map(|earlier| match earlier.payload.as_str() {
                    "rock" => 250,
                    "2x4" => 4_130,
                    _ => 0,
                })
                .sum::<i64>();
        assert_eq!(35_510 - used, step.room, "step {index}: and has the room the reference says");
        compared += 1;
    }
    assert_eq!(
        compared, 4,
        "four accepted steps were compared, one was refused on capacity and one was declared"
    );
}
