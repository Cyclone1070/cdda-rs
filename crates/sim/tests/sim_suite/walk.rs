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
        // **Stamina, from the recording**: it falls with every step, so each case carries its own ratio and the charge moves with it.
        let stamina = match (row.get("stamina"), row.get("stamina_max")) {
            (Some(Value::Int(have)), Some(Value::Int(cap))) => {
                let have = f64::from(i32::try_from(*have).expect("stamina"));
                let cap = f64::from(i32::try_from(*cap).expect("its maximum"));
                narrow(have / cap)
            }
            _ => panic!("{name}: stamina"),
        };
        let ours = cdda_sim::run_cost::run_cost(base, blend, flat_ground, diagonal, stamina);
        assert_eq!(
            ours, charge,
            "{name}: **the charge**, from the combined cost {combined}, the body's {blend}, flat ground {flat_ground} and {stamina} stamina"
        );
        seen += 1;
    }
    assert_eq!(seen, 3, "all three recorded steps were priced");

    // **A creature with the recorded body pays the recorded price**, and one without pays the square's own. The map's own half - a floor step being a hundred - is pinned in
    // `vehicles::a_walk_step_lands_refuses_and_boards`, where the suite's terrain fixtures live.
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
                move_speed,
                footing,
                flat_ground,
                stamina: 1.0,
            },
        ))
        .id();
    let paid = cdda_sim::walk::price_of_step(&world, priced, 100);
    assert_eq!(
        paid, 163,
        "**and the survivor's body makes a floor step cost 163**"
    );
}
