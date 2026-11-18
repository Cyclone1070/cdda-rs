//! What a step costs and where the price comes from: the square's combined cost, the body's two factors, and the charge the reference actually makes.

#![allow(clippy::expect_used, clippy::panic)]

use bevy_ecs::prelude::*;
use cdda_json::Value;

/// **A walking session, priced end to end.** `corpus/walk.json` is `--dump-walk`: three steps through `game::walk_move`, with both squares' costs, the map's combined cost, and the
/// charge - plus the survivor's limb scores and flat-ground multiplier.
///
/// Three layers are checked together, because the reference reaches the charge through all three: the port's own combined cost must equal the recording's, the blend built from the
/// recording's scores must equal the reference's, and `run_cost` of that must be the charge.
#[test]
fn a_walk_step_is_priced_by_the_square_and_the_body() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/walk.json");
    let bytes = std::fs::read(path).expect("the walking recording");
    let value = cdda_json::parse(&bytes).expect("valid json");
    let Value::Object(root) = value else {
        panic!("the recording is an object");
    };

    let number = |field: &str| -> f64 {
        match root.get(field) {
            Some(Value::Float(value)) => *value,
            Some(Value::Int(value)) => f64::from(i32::try_from(*value).expect("an int")),
            other => panic!("{field}: {other:?}"),
        }
    };
    let narrow = |value: f64| -> f32 {
        #[expect(
            clippy::cast_possible_truncation,
            reason = "a limb score is single precision in the reference too"
        )]
        let narrowed = value as f32;
        narrowed
    };
    let move_speed = narrow(number("move_speed"));
    let footing = narrow(number("footing"));
    let flat_ground = narrow(number("flat_ground"));

    let Some(Value::Array(cases)) = root.get("cases") else {
        panic!("cases");
    };

    let mut seen = 0;
    for case in cases {
        let Value::Object(row) = case else {
            panic!("a case is an object");
        };
        let name = match row.get("case") {
            Some(Value::Str(text)) => text.clone(),
            _ => panic!("a name"),
        };
        let int = |field: &str| -> i32 {
            match row.get(field) {
                Some(Value::Int(value)) => i32::try_from(*value).expect("an int"),
                other => panic!("{name}.{field}: {other:?}"),
            }
        };
        let combined = int("combined");
        let charge = int("charge");
        assert_eq!(int("cost_from"), 2, "{name}: both squares are floor");
        assert_eq!(int("cost_to"), 2, "{name}: both squares are floor");

        let blend = cdda_sim::run_cost::limb_run_cost_modifier(
            cdda_sim::run_cost::limb_cost_modifier(footing),
            cdda_sim::run_cost::limb_cost_modifier(move_speed),
        );
        #[expect(
            clippy::cast_precision_loss,
            reason = "a combined cost is a small count of movement points"
        )]
        let base = combined as f32;
        let diagonal = row.get("dx").is_some() && int("dx") != 0 && int("dy") != 0;
        // **Stamina, from the recording, as it stood when the step was taken**: the price is asked of the creature before it pays, and the burn takes the breath out afterwards. It
        // falls with every step, which is why the tired case charges 291 where a rested one charges 163.
        let before = match row.get("stamina_before") {
            Some(Value::Int(value)) => *value,
            other => panic!("{name}.stamina_before: {other:?}"),
        };
        let after = match row.get("stamina") {
            Some(Value::Int(value)) => *value,
            other => panic!("{name}.stamina: {other:?}"),
        };
        let max = match row.get("stamina_max") {
            Some(Value::Int(value)) => *value,
            other => panic!("{name}.stamina_max: {other:?}"),
        };
        #[expect(
            clippy::cast_precision_loss,
            reason = "stamina is a count of points in the thousands"
        )]
        let narrow_int = |value: &i64| -> f32 {
            let points = *value as f64;
            let ceiling = f64::from(i32::try_from(max).expect("its maximum"));
            narrow(points / ceiling)
        };
        let stamina = narrow_int(&before);
        let ours = cdda_sim::run_cost::run_cost(base, blend, flat_ground, diagonal, stamina);
        assert_eq!(
            ours, charge,
            "{name}: **the charge**, from the combined cost {combined}, the body's {blend}, flat ground {flat_ground} and {stamina} stamina"
        );
        // **And the breath it costs**: `burn_move_stamina` with the data's own fifteen per hundred moves
        // (`data/core/external_options.json:39-43`), through the same modifier the price divides by.
        let burn = cdda_sim::run_cost::stamina_burn(
            charge,
            cdda_sim::walk::BASE_STAMINA_BURN_RATE,
            stamina,
        );
        assert_eq!(
            before - i64::from(burn),
            after,
            "{name}: **the breath the step costs**: {burn} out of {before}"
        );
        seen += 1;
    }
    assert_eq!(
        seen, 4,
        "**all four recorded steps were priced**, the last one by a wrecked survivor"
    );
}

/// **What a body does to a price, with no map involved.** A creature with no price factors pays the square's cost; one carrying the recorded survivor's scores pays 163 for the same
/// hundred. The map's own half - a floor step being a hundred - is pinned in `vehicles::a_walk_step_lands_refuses_and_boards`, where the suite's terrain fixtures live.
#[test]
fn a_body_prices_its_own_step() {
    let mut world = World::new();
    let plain = world
        .spawn((
            cdda_sim::Position { x: 4, y: 4, z: 0 },
            cdda_sim::creature::Moves { left: 1000 },
        ))
        .id();
    assert_eq!(
        cdda_sim::walk::price_of_step(&world, plain, 100),
        100,
        "a body with no price factors pays the square's cost"
    );
    let priced = world
        .spawn((
            cdda_sim::Position { x: 6, y: 4, z: 0 },
            cdda_sim::creature::Moves { left: 1000 },
            cdda_sim::walk::LimbCost {
                move_speed: 0.553_607,
                footing: 0.467_213,
                flat_ground: 0.85,
            },
            cdda_sim::creature::Stamina {
                cur: 8_500.0,
                max: 8_500.0,
            },
        ))
        .id();
    let paid = cdda_sim::walk::price_of_step(&world, priced, 100);
    assert_eq!(
        paid, 163,
        "**and the survivor's body makes a floor step cost 163**"
    );
}
