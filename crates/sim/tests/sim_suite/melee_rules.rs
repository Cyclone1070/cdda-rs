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

/// The registry knows what a swung thing is worth, and the reference's own dump says so.
///
/// Reference: `itype::melee_damage` (`item_factory.cpp:4109`) is what a character's blow carries; the throw path has read `thrown_damage` for rounds and nothing read the melee
/// half, because nothing in the port swung an item. The numbers here come from `corpus/items/thrown-damage.json`, whose `melee_bash` column the oracle now writes beside the thrown
/// damage.
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
    let mut kinds = Vec::new();
    for one in types {
        let cdda_json::Value::Object(fields) = one else {
            continue;
        };
        let Some(cdda_json::Value::Str(id)) = fields.get("id") else {
            continue;
        };
        // The three melee damage types the data uses, taken from the dump's own `damage` array rather than from its bash column: a combat knife's whole worth is stabbing.
        let mut split = (0.0, 0.0, 0.0);
        if let Some(cdda_json::Value::Array(units)) = fields.get("damage") {
            for unit in units {
                let cdda_json::Value::Object(unit) = unit else {
                    continue;
                };
                let Some(cdda_json::Value::Str(kind)) = unit.get("type") else {
                    continue;
                };
                let amount = match unit.get("amount") {
                    Some(cdda_json::Value::Float(value)) => *value,
                    Some(cdda_json::Value::Int(value)) => {
                        #[allow(clippy::cast_precision_loss)]
                        {
                            *value as f64
                        }
                    }
                    _ => 0.0,
                };
                match kind.as_str() {
                    "bash" => split.0 = amount,
                    "cut" => split.1 = amount,
                    "stab" => split.2 = amount,
                    _ => {}
                }
            }
        }
        kinds.push((id.clone(), split));
    }
    assert_eq!(kinds.len(), 9_912, "every type in the dump");

    let mut world = bevy_ecs::prelude::World::new();
    let borrowed: Vec<cdda_sim::item_registry::Kind<'_>> = kinds
        .iter()
        .map(|(id, (bash, cut, stab))| cdda_sim::item_registry::Kind {
            id,
            volume_ml: 50,
            weight_g: 100,
            pockets: &[],
            thrown_damage: 0.0,
            melee_bash: *bash,
            melee_cut: *cut,
            melee_stab: *stab,
        })
        .collect();
    cdda_sim::item_registry::set_item_types(&mut world, &borrowed);
    let names = cdda_sim::item_registry::snapshot(&mut world);

    // Three named checks, and the count: a rock is a bash weapon, a knife cuts, and nothing swings for the `null` item.
    let of = |id: &str| {
        let kind = names.id_of(id).unwrap_or_else(|| panic!("the registry has {id}"));
        (names.melee_bash_of(kind), names.melee_cut_of(kind))
    };
    assert_eq!(of("rock"), (7.0, 0.0), "a rock in the fist");
    // A combat knife does **no** bash at all and fourteen points of stabbing: a registry that kept only bash and cut would call it worthless.
    let knife = names
        .id_of("knife_combat")
        .unwrap_or_else(|| panic!("the registry has the combat knife"));
    assert_eq!(names.melee_stab_of(knife), 14.0, "a combat knife stabs");
    assert_eq!(of("null"), (0.0, 0.0), "and the null item is worth nothing");
}
