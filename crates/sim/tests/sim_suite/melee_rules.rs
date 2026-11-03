#![allow(clippy::expect_used, clippy::float_cmp, clippy::panic)]


/// How well a character aims a blow: their dexterity, their skills, and the weapon's own balance.
///
/// Reference: `Character::get_melee_hit_base` (`melee.cpp:375-386`) is `get_hit_base()` - which is `get_dex() / 4.0f` (`character.cpp:5705-5709`) - plus `get_hit_weapon`, which is
/// `skill / 3 + melee_skill / 2 + weap.get_to_hit()`. The three terms are added in tenths of an accuracy point in the port, which is the unit `MeleeAttack::accuracy_tenths` already
/// uses, so a fixture and a character's own skill are the same kind of number.
#[test]
fn how_well_a_character_aims_a_blow() {
    use cdda_sim::melee::{hit_base_tenths, hit_weapon_tenths};

    // Dexterity eight is two points of accuracy, which is twenty tenths.
    assert_eq!(hit_base_tenths(8), 20, "a dexterous character aims well");
    assert_eq!(hit_base_tenths(0), 0, "and one with no dexterity does not");
    assert_eq!(hit_base_tenths(12), 30, "twelve is three points");

    // Skills are thirds and halves: unarmed three is one point, melee four is two.
    assert_eq!(hit_weapon_tenths(3, 4, 0), 30, "one point of unarmed and two of melee");
    // The weapon's own balance is added last and can be negative - a badly balanced thing aims worse.
    assert_eq!(hit_weapon_tenths(0, 0, -2), -20, "a clumsy weapon takes accuracy away");
    assert_eq!(hit_weapon_tenths(0, 0, 3), 30, "a well balanced one gives it");
}

/// The registry knows what a swung thing is worth, for every type the reference dumped.
///
/// Reference: `itype::melee_damage` (`item_factory.cpp:4109`) and `itype::m_to_hit` (`itype.h:1540`) are what a character's blow carries and how well it aims. The dump
/// `corpus/items/thrown-damage.json` version 2 carries all four columns, and this gate compares **all of them for all 9,912 types** - so a registry that dropped a damage type, or a
/// `to_hit` that quietly became zero, fails here rather than in a swing nobody can see.
#[test]
fn the_registry_carries_what_a_swung_thing_is_worth() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/items/thrown-damage.json");
    let Ok(bytes) = std::fs::read(path) else {
        panic!("the item dump exists")
    };
    let root = cdda_json::parse(&bytes).unwrap_or_else(|_| panic!("the dump is valid JSON"));
    let cdda_json::Value::Object(root) = root else {
        panic!("the dump's root is not an object");
    };
    let Some(cdda_json::Value::Array(types)) = root.get("types") else {
        panic!("the dump has no types");
    };

    let number = |value: Option<&cdda_json::Value>| -> f64 {
        match value {
            Some(cdda_json::Value::Float(value)) => *value,
            Some(cdda_json::Value::Int(value)) => {
                #[allow(clippy::cast_precision_loss)]
                {
                    *value as f64
                }
            }
            _ => 0.0,
        }
    };

    // One row per type: id, the three melee damage types, and how well it aims.
    let mut rows: Vec<(String, f64, f64, f64, i32)> = Vec::new();
    for one in types {
        let cdda_json::Value::Object(fields) = one else {
            continue;
        };
        let Some(cdda_json::Value::Str(id)) = fields.get("id") else {
            continue;
        };
        #[allow(clippy::cast_possible_truncation)]
        let to_hit = number(fields.get("to_hit")) as i32;
        rows.push((
            id.clone(),
            number(fields.get("melee_bash")),
            number(fields.get("melee_cut")),
            number(fields.get("melee_stab")),
            to_hit,
        ));
    }
    assert_eq!(rows.len(), 9_912, "every type in the dump");

    let mut world = bevy_ecs::prelude::World::new();
    let borrowed: Vec<cdda_sim::item_registry::Kind<'_>> = rows
        .iter()
        .map(|(id, bash, cut, stab, to_hit)| cdda_sim::item_registry::Kind {
            id,
            volume_ml: 50,
            weight_g: 100,
            pockets: &[],
            thrown_damage: 0.0,
            melee_bash: *bash,
            melee_cut: *cut,
            melee_stab: *stab,
            to_hit: *to_hit,
        })
        .collect();
    cdda_sim::item_registry::set_item_types(&mut world, &borrowed);
    let names = cdda_sim::item_registry::snapshot(&mut world);

    // Every row compared, and the wrong ones counted so a silent zero shows up as a number rather than as a pass.
    let mut wrong = Vec::new();
    for (id, bash, cut, stab, to_hit) in &rows {
        let Some(kind) = names.id_of(id) else {
            wrong.push(format!("{id}: not in the registry"));
            continue;
        };
        let got = (
            names.melee_bash_of(kind),
            names.melee_cut_of(kind),
            names.melee_stab_of(kind),
            names.to_hit_of(kind),
        );
        if (got.0 - bash).abs() > f64::EPSILON
            || (got.1 - cut).abs() > f64::EPSILON
            || (got.2 - stab).abs() > f64::EPSILON
            || got.3 != *to_hit
        {
            wrong.push(format!("{id}: ({bash}, {cut}, {stab}, {to_hit}) became {got:?}"));
        }
    }
    let first_few: Vec<&String> = wrong.iter().take(5).collect();
    assert!(wrong.is_empty(), "{} of 9,912 wrong: {first_few:?}", wrong.len());

    // Named checks, because a comparison of nine thousand rows says nothing about which rows matter.
    let of = |id: &str| {
        let kind = names.id_of(id).unwrap_or_else(|| panic!("the registry has {id}"));
        (
            names.melee_bash_of(kind),
            names.melee_stab_of(kind),
            names.to_hit_of(kind),
        )
    };
    assert_eq!(of("rock"), (7.0, 0.0, -1), "a rock in the fist");
    assert_eq!(of("knife_combat"), (0.0, 21.0, -1), "a knife stabs and does not bash");
    assert_eq!(of("null"), (0.0, 0.0, -2), "and the null item is worth nothing, and aims worst");
}
