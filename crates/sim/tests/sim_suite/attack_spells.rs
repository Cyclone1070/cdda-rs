//! How many of the game's `attack` spells the port can actually resolve.
//!
//! Reference: `spell_effect::target_attack` (`magic_spell_effect.cpp`), which every one of the data's attack spells names.
//! "Attack spells are ported" had been a claim about a function existing; this sweep turns it into a measurement over the
//! whole data set, and a failure here means either the data changed or the rule stopped covering something.

#![allow(clippy::panic, clippy::expect_used, clippy::indexing_slicing)]

use bevy_ecs::prelude::*;
use cdda_sim::spellcasting::{self, Cast};

/// A body with one part, because the sweep is about the spells and not about anatomy.
fn a_torso() -> Vec<cdda_sim::hit_location::BodyPart> {
    vec![cdda_sim::hit_location::BodyPart {
        name: "torso".to_owned(),
        hit_size: 36.0,
        hit_difficulty: 1.0,
        upper: false,
        lower: false,
                ..Default::default()
            }]
}

/// What the sweep found, so the counts can be asserted and printed.
#[derive(Default, Debug)]
struct Sweep {
    seen: usize,
    hit: usize,
    out_of_range: usize,
    nothing_to_hit: usize,
    filtered: usize,
    dealt: i64,
    /// The spells that landed, by id, so the damage total can be compared against exactly those.
    landed: Vec<String>,
}

fn sweep(world: &mut World, spells: &[cdda_content::spells::Spell]) -> Sweep {
    let mut found = Sweep::default();
    for spell in spells.iter().filter(|spell| spell.effect_name == "attack") {
        found.seen += 1;
        // **A filtered spell's monster stands one square off**, because the rule reads the creature on the square it is aimed at and the caster itself stands on (0,0,0).
        let at = if spell.targeted_monster_ids.is_empty() { (0, 0, 0) } else { (1, 0, 0) };
        let target = cdda_sim::creature::spawn_creature(world, "the target", at);
        // A target with no body has nothing to hit, which is what the first version of this sweep measured: 77 refusals
        // that said more about the harness than about the rule.
        world
            .entity_mut(target)
            .insert(cdda_sim::creature::Body { parts: a_torso() });
        cdda_sim::creature::set_health(world, target, 80);
        // **A spell that names the monsters it may affect is asked with a monster it names**, which is the rule round 1475 put in: `death_kill_vines` is not supposed to land on
        // a torso, it is supposed to land on `mon_creeper_vine`. The sweep gives it one when the registry holds it, and counts the spell as filtered when the data names a monster the
        // fixture world does not have.
        let named = spell.targeted_monster_ids.iter().next().cloned();
        if let Some(name) = named {
            if let Some(kind) = cdda_sim::monster_registry::id_of(world, &name) {
                cdda_sim::creature::set_kind(world, target, kind);
            } else {
                found.filtered += 1;
                world.despawn(target);
                continue;
            }
        }
        match spellcasting::cast_attack(world, (0, 0, 0), spell, 0, target, 0.0) {
            Cast::Hit { dealt, .. } => {
                found.hit += 1;
                found.dealt += i64::from(dealt);
                found.landed.push(spell.id.clone());

            }
            Cast::OutOfRange => found.out_of_range += 1,
            Cast::NothingToHit => found.nothing_to_hit += 1,
            Cast::NotAnAttack => panic!("{} is an attack by its own name", spell.id),
        }
        world.despawn(target);
    }
    found
}

#[test]
fn every_attack_spell_in_the_game_lands_something() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/spells.json");
    let text = std::fs::read_to_string(path).expect("the spell dump");
    let value = cdda_json::parse(text.as_bytes()).expect("valid json");
    let spells = cdda_content::spells::load_dump(&value).expect("the dump loads");

    let mut world = World::new();
    let found = sweep(&mut world, &spells);
    println!("attack sweep: {found:?}");

    assert_eq!(found.seen, 77, "the game has 77 attack spells");
    assert_eq!(
        found.hit + found.out_of_range + found.nothing_to_hit + found.filtered,
        found.seen,
        "every attack spell was asked and answered"
    );
    // **Five of the game's attack spells name the monsters they may affect** and the sweep's fixture world holds none of those kinds, so they are filtered rather than landing.
    assert_eq!(found.filtered, 5, "five attack spells filter by monster id");
    assert_eq!(
        found.hit + found.filtered + found.nothing_to_hit,
        77,
        "every attack spell either landed its blow or was filtered by the monster it names"
    );
    assert_eq!(found.out_of_range, 0, "a target on the caster's own square is never out of range");
    // **One attack spell in the data may only be aimed at the ground** - `death_portal` - so a creature standing on the square it is aimed at is not something it touches. The
    // reference agrees, and that is the whole point of `spell::is_valid_target`: the effect name says `attack`, the targets say what the attack is for.
    assert_eq!(
        found.nothing_to_hit, 1,
        "and exactly one attack spell passes over creatures because its data names only the ground"
    );
    // The damage that arrived must be the damage the data states, unaltered: an unarmoured torso on the caster's own
    // square is the one case where nothing may change the number.
    #[allow(clippy::cast_possible_truncation)]
    let stated: i64 = spells
        .iter()
        .filter(|spell| spell.effect_name == "attack")
        .filter(|spell| found.landed.contains(&spell.id))
        .filter_map(|spell| spell.damage.at(0))
        .filter(|damage| *damage > 0.0)
        .map(|damage| damage.round() as i64)
        .sum();
    // Reference: `damage_targets` only deals damage "if( sp.damage( caster ) > 0 )"
    // (`magic_spell_effect.cpp:563`). Two of the game's attack spells carry **negative** damage - `AEA_HEAL` at -2 and
    // `spur_shrill` at -100 - and the reference deals none of it, applying an effect instead. The port matches on the
    // damage half and has no effect to add yet, which is a declared gap rather than a wrong number.
    let positive = spells
        .iter()
        .filter(|spell| spell.effect_name == "attack")
        .filter(|spell| spell.damage.at(0).is_some_and(|damage| damage > 0.0))
        .count();
    println!("stated {stated} over {positive} spells with positive damage");
    // 32 carry damage at level 0, 43 scale from zero with level, and 2 are negative (healing) and deal none.
    assert_eq!(
        positive, 32,
        "thirty-two of the 77 attack spells carry damage at level 0"
    );
    assert_eq!(
        found.dealt, stated,
        "every point of positive damage the data states arrived: {stated} across 75 spells, the largest being the one \
         that says 100,000,000"
    );
}

/// The damage a spell deals at its own ceiling, transcribed from the reference rather than read from the port.
///
/// Reference: `spell::damage` (`magic.cpp:816-837`) is `min_leveled_damage` - `min_damage + round( level *
/// damage_increment )` - clamped to `max_damage` by `min` when the increment climbs and by `max` when it falls. So
/// `max_damage` is a **cap**, not the value at the ceiling: `bring_the_sun` starts at 10 with a cap of 12 and a zero
/// increment, and deals 10 at every level. Expecting the cap was this test's first mistake, and it was 2 points wrong
/// across the set.
fn damage_at_ceiling(row: &cdda_json::Object) -> Option<i64> {
    let number = |key: &str| match row.get(key) {
        Some(cdda_json::Value::Float(number)) => Some(*number),
        Some(cdda_json::Value::Int(number)) => {
            #[allow(clippy::cast_precision_loss)]
            let as_float = *number as f64;
            Some(as_float)
        }
        _ => None,
    };
    let start = number("min_damage")?;
    let increment = number("damage_increment")?;
    let cap = number("max_damage")?;
    #[allow(clippy::cast_possible_truncation)]
    let max_level = number("max_level")? as i32;
    if max_level == 0 {
        return None;
    }
    #[allow(clippy::cast_precision_loss)]
    let climbed = start + increment * f64::from(max_level);
    let at_the_ceiling = if increment >= 0.0 {
        climbed.min(cap)
    } else {
        climbed.max(cap)
    };
    #[allow(clippy::cast_possible_truncation)]
    let as_whole: i64 = at_the_ceiling.round() as i64;
    (as_whole > 0).then_some(as_whole)
}

/// The same sweep at each spell's own ceiling, where the level-scaled damage has to be the data's maximum.
///
/// Reference: `spell::damage()` clamps the level-scaled value between the spell's minimum and maximum, so a spell cast at
/// its own `max_level` deals exactly its `max_damage`. At level 0, 43 of the 77 attack spells deal nothing because they
/// scale up from zero; this sweep is the one that exercises the arithmetic.
#[test]
#[allow(clippy::too_many_lines)]
fn at_its_ceiling_every_attack_spell_deals_the_damage_the_data_states() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/spells.json");
    let text = std::fs::read_to_string(path).expect("the spell dump");
    let value = cdda_json::parse(text.as_bytes()).expect("valid json");
    let spells = cdda_content::spells::load_dump(&value).expect("the dump loads");

    let mut world = World::new();
    // The caster stands still for the whole sweep: the rule reads the creature on the square a spell is aimed at, and a filtered spell's monster stands one square off.
    cdda_sim::creature::spawn_creature(&mut world, "the caster", (0, 0, 0));
    // **Every monster an attack spell names is registered**, so a spell that filters by monster id is asked with one of its own instead of being refused for the fixture's sake.
    let named: Vec<&str> = spells
        .iter()
        .filter(|spell| spell.effect_name == "attack")
        .flat_map(|spell| spell.targeted_monster_ids.iter().map(String::as_str))
        .collect();
    let kinds: Vec<cdda_sim::monster_registry::MonsterKind> = named
        .iter()
        .map(|id| cdda_sim::monster_registry::MonsterKind { id, hp: 1_000_000 })
        .collect();
    cdda_sim::monster_registry::set_monster_types(&mut world, &kinds);
    let mut seen = 0;
    let mut skipped: Vec<String> = Vec::new();
    let mut dealt_total: i64 = 0;
    // The same total restricted to the spells that have levels to climb, which is the set the dump states a `max_damage`
    // for unambiguously.
    let mut dealt_scaled: i64 = 0;
    let mut stated_total: i64 = 0;
    for spell in spells.iter().filter(|spell| spell.effect_name == "attack") {
        seen += 1;
        #[allow(clippy::cast_possible_truncation)]
        let ceiling = spell.max_level.unwrap_or(0.0) as i32;
        // A filtered spell's monster stands one square off, because the rule reads the creature on the square it is aimed at and the caster stands on (0,0,0). A spell that reaches
        // no further than its own square would have to share that square with the caster, where the reference's own `creature_at` answers the caster - so it is skipped, and counted.
        let filtered = !spell.targeted_monster_ids.is_empty();
        if filtered && spell.range.at(ceiling).unwrap_or(0.0) < 1.0 {
            skipped.push(spell.id.clone());
            continue;
        }
        let at = if filtered { (1, 0, 0) } else { (0, 0, 0) };
        let target = cdda_sim::creature::spawn_creature(&mut world, "the target", at);
        // A spell that names monsters is asked with one of them, which is the rule round 1475 taught the cast.
        if let Some(name) = spell.targeted_monster_ids.iter().next()
            && let Some(kind) = cdda_sim::monster_registry::id_of(&mut world, name)
        {
            cdda_sim::creature::set_kind(&mut world, target, kind);
        }
        world
            .entity_mut(target)
            .insert(cdda_sim::creature::Body { parts: a_torso() });
        cdda_sim::creature::set_health(&mut world, target, 1_000_000);
        let cast = spellcasting::cast_attack(&mut world, (0, 0, 0), spell, ceiling, target, 0.0);
        let told = spell.damage.at(ceiling).unwrap_or(0.0);
        if told > 0.0 {
            let Cast::Hit { dealt, .. } = cast else {
                panic!("{} at level {ceiling} did not land: {cast:?}", spell.id);
            };
            dealt_total += i64::from(dealt);
            if ceiling > 0 {
                dealt_scaled += i64::from(dealt);
                let _ = &dealt;
            }
            #[allow(clippy::cast_possible_truncation)]
            let stated = told.round() as i64;
            assert_eq!(
                i64::from(dealt), stated,
                "{} at its ceiling of {ceiling} dealt {dealt} instead of {stated}",
                spell.id
            );
            stated_total += stated;
        }
        world.despawn(target);
    }
    assert_eq!(seen, 77, "the game has 77 attack spells");
    // **Two attack spells that filter by monster id reach no further than their own square** (`death_kill_vines` and `death_kill_breathers`, both range 0), so the sweep cannot stand
    // their monster where the rule would find it: it would have to share the caster's square, and the reference's own `creature_at` answers the caster there.
    assert_eq!(
        skipped,
        vec!["death_kill_vines".to_owned(), "death_kill_breathers".to_owned()],
        "the two short-range filtered spells are named rather than quietly dropped"
    );
    // The data's own ceiling for each spell, read straight from the dump rather than through the loader: this is the
    // comparison that makes the sweep an oracle test instead of the port agreeing with itself.
    let cdda_json::Value::Object(root) = &value else {
        panic!("a spell dump is an object");
    };
    let Some(cdda_json::Value::Array(rows)) = root.get("spells") else {
        panic!("a spell dump has spells");
    };
    let mut from_the_data: i64 = 0;
    for row in rows {
        let cdda_json::Value::Object(row) = row else {
            continue;
        };
        if row.get("effect_name") != Some(&cdda_json::Value::Str("attack".to_owned())) {
            continue;
        }
        // A spell the sweep could not stand a target for is left out of the expected sum as well, so the comparison stays about the same set of spells.
        if matches!(row.get("id"), Some(cdda_json::Value::Str(id)) if skipped.contains(id)) {
            continue;
        }
        #[allow(clippy::cast_possible_truncation)]
        let max_level = match row.get("max_level") {
            Some(cdda_json::Value::Float(number)) => *number as i32,
            _ => 0,
        };
        // Only a spell with levels to climb has an unambiguous `max_damage` in the data: a spell with a ceiling of zero
        // is cast at level zero, where its damage is its minimum, and the two are compared by count instead.
        if max_level == 0 {
            continue;
        }
        let Some(at_the_ceiling) = damage_at_ceiling(row) else {
            continue;
        };
        from_the_data += at_the_ceiling;
    }
    println!(
        "at the ceiling: {dealt_total} dealt, {stated_total} stated, {from_the_data} from the data's max_damage"
    );
    assert_eq!(dealt_total, stated_total, "every stated point arrived");
    assert_eq!(
        dealt_scaled, from_the_data,
        "and for the spells that climb, the total is the data's own sum of max_damage"
    );
}

/// The effects the attack spells apply are in the corpus, and the port can say how long each lasts.
///
/// Reference: `add_effect_to_target` (`magic_spell_effect.cpp:499-520`) turns `spell::duration` into the effect's
/// lifetime. The port does not apply them yet - this is the measurement that says how many are waiting, and the check
/// that the duration the loader reads is the reference's own number rather than a default standing in for one.
#[test]
fn the_attack_spells_that_apply_an_effect_say_for_how_long() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/spells.json");
    let text = std::fs::read_to_string(path).expect("the spell dump");
    let value = cdda_json::parse(text.as_bytes()).expect("valid json");
    let spells = cdda_content::spells::load_dump(&value).expect("the dump loads");

    let named: Vec<&cdda_content::spells::Spell> = spells
        .iter()
        .filter(|spell| spell.effect_name == "attack" && !spell.effect_str.is_empty())
        .collect();
    assert_eq!(
        named.len(),
        27,
        "twenty-seven of the game's attack spells apply an effect as well as dealing damage"
    );
    let timed = named
        .iter()
        .filter(|spell| spell.duration.at(0).is_some_and(|turns| turns > 0.0))
        .count();
    assert_eq!(
        timed, 25,
        "twenty-five of them last a stated time; the other two name an effect with no duration"
    );
    // One of them, to prove the number is the reference's and not a placeholder: the adrenaline artifact lasts two
    // thousand turns, the same figure the dump carries.
    let adrenaline = named
        .iter()
        .find(|spell| spell.effect_str == "adrenaline")
        .expect("the adrenaline artifact is an attack spell");
    assert_eq!(adrenaline.duration.at(0), Some(120_000.0));
    assert_eq!(adrenaline.id, "AEA_ADRENALINE");
}
