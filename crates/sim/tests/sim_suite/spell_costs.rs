//! What a spell costs a caster who carries nothing: the reference's own numbers for every spell at five levels.
//!
//! Reference: `spell::energy_cost` (`magic.cpp:1163-1195`) and the recording `corpus/spells-costs.json`, written by
//! `--dump-spell-costs` from `spell::energy_cost` itself. The rule is not `Scaling::at`'s clamp: the reference branches on where
//! the base sits against the final value, rounds `base + increment * level`, and then adds a somatic term for encumbered hands.

#![allow(clippy::panic, clippy::expect_used, clippy::float_cmp, clippy::indexing_slicing)]

use cdda_sim::spellcasting::{Somatic, energy_cost};

fn corpus(file: &str) -> cdda_json::Value {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../corpus")
        .join(file);
    let text = std::fs::read_to_string(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()));
    cdda_json::parse(text.as_bytes()).expect("the corpus is json")
}

/// One member of an object, as a string.
fn text(value: &cdda_json::Value, key: &str) -> String {
    let cdda_json::Value::Object(fields) = value else {
        panic!("not an object");
    };
    match fields.get(key) {
        Some(cdda_json::Value::Str(found)) => found.clone(),
        _ => panic!("no {key}"),
    }
}

/// One member of an object, as a number.
fn number(value: &cdda_json::Value, key: &str) -> f64 {
    let cdda_json::Value::Object(fields) = value else {
        panic!("not an object");
    };
    match fields.get(key) {
        Some(cdda_json::Value::Float(found)) => *found,
        Some(cdda_json::Value::Int(found)) => {
            #[allow(clippy::cast_precision_loss)]
            let as_float = *found as f64;
            as_float
        }
        _ => panic!("no {key}"),
    }
}

/// One member of an object, as a boolean.
fn truth(value: &cdda_json::Value, key: &str) -> bool {
    let cdda_json::Value::Object(fields) = value else {
        panic!("not an object");
    };
    matches!(fields.get(key), Some(cdda_json::Value::Bool(true)))
}

/// One member of an object, as a list.
fn list<'a>(value: &'a cdda_json::Value, key: &str) -> &'a [cdda_json::Value] {
    let cdda_json::Value::Object(fields) = value else {
        panic!("not an object");
    };
    match fields.get(key) {
        Some(cdda_json::Value::Array(found)) => found,
        _ => panic!("no {key}"),
    }
}

#[test]
fn every_recorded_spell_cost_is_reproduced() {
    let spells = corpus("spells.json");
    let costs = corpus("spells-costs.json");
    let by_id: std::collections::BTreeMap<String, cdda_content::spells::Spell> = list(&spells, "spells")
        .iter()
        .map(|entry| {
            let cdda_json::Value::Object(object) = entry else {
                panic!("a spell is an object");
            };
            let spell = cdda_content::spells::read_spell(object).expect("a spell");
            (spell.id.clone(), spell)
        })
        .collect();
    let mut compared = 0;
    let mut clamped_to_the_ceiling = 0;
    let mut differed = Vec::new();
    for case in list(&costs, "cases") {
        let id = &text(case, "id");
        let Some(spell) = by_id.get(id) else {
            panic!("the cost dump names a spell the spell dump does not: {id}");
        };
        #[allow(clippy::cast_possible_truncation)]
        let level = number(case, "level") as i32;
        // Reference: `set_level` climbs one level at a time and stops at the spell's own ceiling (`magic.cpp:1127-1141`), so a
        // request past the ceiling is the ceiling.
        let ceiling = spell.max_level.map_or(level, |max| {
            #[allow(clippy::cast_possible_truncation)]
            let whole = max as i32;
            level.min(whole)
        });
        let somatic = Somatic {
            #[allow(clippy::cast_possible_truncation)]
            hand_encumbrance: number(case, "somatic_encumbrance") as i32,
            subtle_spell: false,
            no_hands: truth(case, "no_hands"),
        };
        let Some(ours) = energy_cost(spell, ceiling, somatic) else {
            panic!("{id} at level {level}: the port has no price for a spell the reference priced");
        };
        #[allow(clippy::cast_possible_truncation)]
        let recorded = number(case, "energy_cost") as i64;
        compared += 1;
        if recorded != i64::from(ours) {
            differed.push(format!("{id}@{level}: reference {recorded}, port {ours}"));
        }
        if ceiling != level {
            clamped_to_the_ceiling += 1;
        }
    }
    assert!(
        differed.is_empty(),
        "{} of {compared} recorded prices disagree: {differed:?}",
        differed.len()
    );
    assert_eq!(compared, 1_225, "the recording holds five levels for each of 245 spells");
    assert!(
        clamped_to_the_ceiling >= 500,
        "and {clamped_to_the_ceiling} of the requests were past the spell's ceiling, so `set_level`'s clamp is exercised rather than assumed"
    );
    // **A measured gap, not a guess**: every spell in this content states a base equal to its final value and an increment of zero, so the
    // corpus can prove the port reads the price and the ceiling but **cannot** exercise the reference's three-branch climb at all. The next
    // test is what covers that, and it says so.
    let divergent = by_id
        .values()
        .filter(|spell| match (spell.energy.start, spell.energy.end) {
            (Some(start), Some(end)) => start != end,
            _ => false,
        })
        .count();
    assert_eq!(
        divergent, 0,
        "**no spell in the content states a base different from its final value** - {divergent} would change that, and this gate would need a new recording"
    );
    println!("spell costs: {compared} compared, {clamped_to_the_ceiling} clamped to a ceiling, {divergent} spells across the branch");
}

/// **The climb between base and final**, which the content never states.
///
/// Reference: `spell::energy_cost` (`magic.cpp:1163-1177`) has three branches - base below final takes the smaller of the final value and the
/// rounded climb, above final the larger, equal to final the base alone with the increment ignored. Every figure below is that formula worked
/// out by hand, because the corpus cannot supply one: all 245 spells in the content write `base == final`.
#[test]
fn the_price_climbs_to_its_final_value_from_either_side() {
    let spell = |start: f64, increment: f64, end: f64| cdda_content::spells::Spell {
        id: "climbing".to_owned(),
        name: String::new(),
        effect_name: String::new(),
        effect_str: String::new(),
        damage_type: String::new(),
        shape: String::new(),
        energy_source: "MANA".to_owned(),
        damage: cdda_content::spells::Scaling::default(),
        range: cdda_content::spells::Scaling::default(),
        area: cdda_content::spells::Scaling::default(),
        energy: cdda_content::spells::Scaling {
            start: Some(start),
            increment: Some(increment),
            end: Some(end),
        },
        duration: cdda_content::spells::Scaling::default(),
        casting_time: cdda_content::spells::Scaling::default(),
        difficulty: None,
        max_level: None,
    };
    let bare = Somatic::default();
    let rising = spell(10.0, 5.0, 100.0);
    assert_eq!(energy_cost(&rising, 0, bare), Some(10), "ten at level zero");
    assert_eq!(energy_cost(&rising, 3, bare), Some(25), "five a level, so twenty-five at three");
    assert_eq!(
        energy_cost(&rising, 20, bare),
        Some(100),
        "**and the final value is a ceiling: the climb would be 110**"
    );
    let falling = spell(100.0, -5.0, 10.0);
    assert_eq!(energy_cost(&falling, 0, bare), Some(100), "a hundred at level zero");
    assert_eq!(energy_cost(&falling, 3, bare), Some(85), "five cheaper a level");
    assert_eq!(
        energy_cost(&falling, 30, bare),
        Some(10),
        "and ten is the floor it stops at"
    );
    let level_price = spell(10.0, 5.0, 10.0);
    assert_eq!(
        energy_cost(&level_price, 7, bare),
        Some(10),
        "a base equal to its final value ignores a rising increment"
    );
    // **And a falling one**, which is the case that tells the equal branch from the rising one: with `base <= final` the climb would be five,
    // and the reference's third branch answers the base itself (`magic.cpp:1175-1177`).
    let level_price_down = spell(10.0, -5.0, 10.0);
    assert_eq!(
        energy_cost(&level_price_down, 3, bare),
        Some(10),
        "**a base equal to its final value ignores a falling increment too**"
    );
    let halving = spell(10.0, 0.5, 100.0);
    assert_eq!(
        energy_cost(&halving, 1, bare),
        Some(11),
        "and the climb is **rounded**, half away from zero, so 10.5 is eleven"
    );
}

/// **Encumbered hands cost extra, and the rate depends on what the spell is paid with.**
///
/// Reference: `magic.cpp:1178-1192` - the first ten points of combined hand encumbrance are free (`avg - 5`, not `avg`),
/// a mana spell adds ten times what is left, an hp spell adds it once, a stamina spell a hundred times, and a subtle
/// spell or one that needs no hands adds nothing. The recording cannot show this: its caster carries nothing, so every
/// one of its 1,225 cases has an encumbrance of zero.
#[test]
fn a_heavier_hand_costs_more_at_the_rate_its_energy_source_states() {
    let mana = cdda_content::spells::Spell {
        id: "fixture".to_owned(),
        name: String::new(),
        effect_name: String::new(),
        effect_str: String::new(),
        damage_type: String::new(),
        shape: String::new(),
        energy_source: "MANA".to_owned(),
        damage: cdda_content::spells::Scaling::default(),
        range: cdda_content::spells::Scaling::default(),
        area: cdda_content::spells::Scaling::default(),
        energy: cdda_content::spells::Scaling {
            start: Some(10.0),
            increment: Some(0.0),
            end: Some(10.0),
        },
        duration: cdda_content::spells::Scaling::default(),
        casting_time: cdda_content::spells::Scaling::default(),
        difficulty: None,
        max_level: None,
    };
    let encumbered = Somatic {
        hand_encumbrance: 20,
        ..Somatic::default()
    };
    assert_eq!(
        energy_cost(&mana, 0, Somatic::default()),
        Some(10),
        "bare hands cost the data's price"
    );
    assert_eq!(
        energy_cost(&mana, 0, encumbered),
        Some(160),
        "**and twenty points of encumbrance is fifteen over the free ten, at ten a point for mana**"
    );
    assert_eq!(
        energy_cost(
            &mana,
            0,
            Somatic {
                subtle_spell: true,
                ..encumbered
            }
        ),
        Some(10),
        "a subtle spell is paid as if the hands were free"
    );
    let hp = cdda_content::spells::Spell {
        energy_source: "HP".to_owned(),
        ..mana.clone()
    };
    assert_eq!(
        energy_cost(&hp, 0, encumbered),
        Some(25),
        "an hp spell pays the fifteen once"
    );
    let stamina = cdda_content::spells::Spell {
        energy_source: "STAMINA".to_owned(),
        ..mana
    };
    assert_eq!(
        energy_cost(&stamina, 0, encumbered),
        Some(1_510),
        "and a stamina spell pays a hundred times as much: fifteen hundred on top of ten"
    );
}
