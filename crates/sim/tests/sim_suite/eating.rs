//! A bite put into a stomach is absorbed by the turn, which is the wire the food rules were missing.
//!
//! Reference: `Character::consume_effects` (`consumption.cpp:1689-1714`) hands the summary to `stomach.ingest`; `Character::update_stomach`
//! (`character_body.cpp:935-1019`) then absorbs it, and here that is `digest_stage` (`turn.rs`) every five minutes.

use std::collections::{BTreeMap, BTreeSet};

use bevy_ecs::prelude::*;

/// One creature with a body, a clock and the rate table the stage reads.
fn a_creature_who_eats(world: &mut World) -> Entity {
    cdda_sim::calendar::set_turn(world, 0);
    let creature = cdda_sim::creature::spawn_creature(world, "the survivor", (0, 0, 0));
    world
        .entity_mut(creature)
        .insert(cdda_sim::body::Body::default());
    world
        .entity_mut(creature)
        .insert(cdda_sim::body::BodyClock::default());
    // A living body's rates: the guts release vitamins at `5% * hunger` with a floor of `100/24 * hunger`, so **a zero rate releases nothing at all** - which is what
    // an unconfigured creature would answer and what a test must not accidentally model.
    world
        .entity_mut(creature)
        .insert(cdda_sim::needs::NeedsRatesOf(cdda_sim::needs::NeedsRates {
            kcal: 2_000.0,
            hunger: 1.0,
            thirst: 1.0,
            sleepiness: 1.0,
        }));
    creature
}

/// A food whose only content is one vitamin, which is what a drug looks like to the stomach.
fn a_food_with_a_vitamin(kcal: i64, vitamin: &str) -> (BTreeMap<String, i32>, BTreeSet<String>) {
    let mut vitamins = BTreeMap::new();
    vitamins.insert(vitamin.to_owned(), 100);
    let _ = kcal;
    (vitamins, BTreeSet::new())
}

#[test]
fn a_bite_lands_in_the_stomach_and_the_turn_absorbs_it() {
    let mut world = World::new();
    let creature = a_creature_who_eats(&mut world);

    // The vitamin the bite carries has to be a type the store knows, or the reference's own rule skips it.
    let mut table = BTreeMap::new();
    table.insert(
        "vitC".to_owned(),
        cdda_content::vitamins::Vitamin {
            id: "vitC".to_owned(),
            weight_per_unit: Some(1),
            vit_type: "vitamin".to_owned(),
            min: 0,
            max: 1200,
            rate: 1,
        },
    );
    cdda_sim::vitamin_registry::set_vitamin_types(&mut world, &table);

    let (vitamins, flags) = a_food_with_a_vitamin(120, "vitC");
    let facts = cdda_sim::food_summary::ItemFacts {
        weight_g: 100,
        volume_ml: 250,
        count: 1,
        quench: 0,
        stated_kcal: 120,
        stated_vitamins: &vitamins,
    };
    let summary = cdda_sim::eating::eat_a_bite(
        &mut world,
        creature,
        &facts,
        &flags,
        0.0,
        cdda_sim::food_summary::Diet::default(),
        &table,
    )
    .unwrap_or_default();
    let _ = &summary;
    let carried = summary.vitamins.get("vitC").copied().unwrap_or(0);
    assert!(carried > 0, "the bite carries the vitamin: {carried}");
    let stomach = world
        .get::<cdda_sim::body::Body>(creature)
        .map_or(0, |body| body.stomach.contains_ml());
    assert!(stomach > 0, "and it is in the stomach: {stomach} ml");

    // The stage runs every five minutes, but **solids and vitamins move in whole half hours** (`Character::update_stomach` takes `half_hours`), so this runs the
    // half hour the reference waits for rather than the tick that asks.
    // A stomach hands its release to the **guts**, and the guts hand theirs to the body - two half hours, not one.
    for minute in 0..=61 {
        cdda_sim::calendar::set_turn(&mut world, minute * 60);
        cdda_sim::turn::run_turn(&mut world);
    }
    let (stomach, guts, level) = world
        .get::<cdda_sim::body::Body>(creature)
        .map(|body| {
            (
                body.stomach.vitamins.clone(),
                body.guts.vitamins.clone(),
                body.vitamins.get("vitC").copied().unwrap_or(0),
            )
        })
        .unwrap_or_default();
    assert!(
        level > 0,
        "and the turn absorbed it into the body: {level} (stomach {stomach:?}, guts {guts:?})"
    );
}
