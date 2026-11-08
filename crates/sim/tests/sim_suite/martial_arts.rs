//! Which martial arts buffs are in play, and what they need in hand.
//!
//! Reference: `ma_buff::is_valid_character` (`martialarts.cpp:713-760`). A static buff states its requirement **first** and its numbers second, and the requirement decides whether the buff
//! applies at all: an unarmed-only buff needs an empty hand, a melee-only buff needs a weapon the style allows, and a buff that names neither applies to **neither** - which is why the reference
//! loads both flags as `false` rather than `true` (`martialarts.cpp:250-251`).

#![allow(clippy::panic, clippy::expect_used, clippy::float_cmp, clippy::indexing_slicing)]

use cdda_sim::martial_arts::{Hands, Requirement, applies};

fn requirement(unarmed: bool, melee: bool) -> Requirement {
    Requirement {
        unarmed_allowed: unarmed,
        melee_allowed: melee,
        strictly_unarmed: false,
        wall_adjacent: false,
        weapon_categories: Vec::new(),
    }
}

/// **A buff that names neither hand applies to neither**, which is the reference's own default.
#[test]
fn a_buff_that_names_no_hand_applies_to_none() {
    let neither = requirement(false, false);
    assert!(!applies(&neither, Hands::Empty, &[]), "an empty hand earns nothing from a buff that does not ask for one");
    assert!(!applies(&neither, Hands::Weapon("a knife"), &["knife"]), "and neither does a weapon");
}

/// An unarmed-only buff is worth something with an empty hand and nothing with a weapon.
#[test]
fn an_unarmed_buff_needs_an_empty_hand() {
    let unarmed = requirement(true, false);
    assert!(applies(&unarmed, Hands::Empty, &[]), "**an empty hand is what it asks for**");
    assert!(
        !applies(&unarmed, Hands::Weapon("a knife"), &["knife"]),
        "and a knife is not, even one the style would allow"
    );
}

/// A melee-only buff needs a weapon **the style allows**, which is the reference's `is_valid_weapon` and the category list beside it.
#[test]
fn a_melee_buff_needs_a_weapon_of_a_category_it_names() {
    let mut melee = requirement(false, true);
    assert!(!applies(&melee, Hands::Empty, &[]), "a melee buff does nothing in an empty hand");
    assert!(
        applies(&melee, Hands::Weapon("a knife"), &["knife"]),
        "**a weapon with no category stated is allowed by any melee buff**"
    );
    melee.weapon_categories = vec!["KNIFE".to_owned()];
    assert!(applies(&melee, Hands::Weapon("a knife"), &["KNIFE"]), "and a stated category needs a weapon of it");
    assert!(
        !applies(&melee, Hands::Weapon("a bat"), &["BLUNT"]),
        "**a bat is not a knife, so the buff is not in play**"
    );
}

/// A buff that allows both is worth something either way.
#[test]
fn a_buff_that_allows_both_is_worth_something_either_way() {
    let both = requirement(true, true);
    assert!(applies(&both, Hands::Empty, &[]), "an empty hand is allowed");
    assert!(applies(&both, Hands::Weapon("a knife"), &["knife"]), "and so is a weapon");
}

/// A bonus's scale is multiplied by whatever it scales with, and a bonus that scales with nothing is its scale alone.
///
/// Reference: `effect_scaling::get` (`bonuses.cpp:318-372`) and `stat_bonus`'s own constructor (`bonuses.cpp:120-160`). The `stat` is the reference's `scaling_stat`: a **character stat**
/// (`str`, `dex`, `int`, `per`), a **skill** by name (`melee`, `dodge`, `unarmed`), or `NONE` - which is not zero but **the scale itself**, and that is the case a port gets wrong by defaulting
/// to nothing.
#[test]
fn a_bonuss_scale_is_multiplied_by_what_it_scales_with() {
    use cdda_sim::martial_arts::{Scaling, scaled};

    let dex_two = Scaling::Stat { name: "dex".to_owned(), value: 12.0 };
    assert_eq!(scaled(1.5, &dex_two), 18.0, "**a scale of one and a half with a dexterity of twelve is eighteen**");
    let melee_three = Scaling::Skill { name: "melee".to_owned(), level: 3.0 };
    assert_eq!(scaled(0.5, &melee_three), 1.5, "and half of a melee skill of three is one and a half");
    assert_eq!(
        scaled(2.0, &Scaling::None),
        2.0,
        "**a bonus that scales with nothing is its scale, not zero**"
    );
    assert_eq!(scaled(2.0, &Scaling::Unknown("speed".to_owned())), 0.0, "a stat the port does not read answers nothing");
}

/// Flat bonuses add, and multiplied bonuses multiply - and a multiplier that comes out negative is **clamped to nothing**.
///
/// Reference: `bonus_container::get_flat` (`bonuses.cpp:221-240`) sums and answers zero when the stat is absent; `get_mult` (`:242-258`) starts at **one**, multiplies, and answers
/// `max( 0.0f, ret )` with the reference's own comment that "all relevant effects require non-negative values".
#[test]
fn flat_bonuses_add_and_multipliers_multiply_then_clamp() {
    use cdda_sim::martial_arts::{Bonus, flat_of, mult_of};
    use std::collections::BTreeMap;

    let mut flat = BTreeMap::new();
    flat.insert(
        "hit".to_owned(),
        vec![
            Bonus { scale: 1.0, scaling: cdda_sim::martial_arts::Scaling::None },
            Bonus { scale: 0.5, scaling: cdda_sim::martial_arts::Scaling::Skill { name: "melee".to_owned(), level: 4.0 } },
        ],
    );
    assert_eq!(flat_of(&flat, "hit"), 3.0, "**flat bonuses add**: one plus half of a melee skill of four");
    assert_eq!(flat_of(&flat, "dodge"), 0.0, "a stat with no bonuses is worth nothing at all");

    let mut mult = BTreeMap::new();
    mult.insert(
        "block".to_owned(),
        vec![
            Bonus { scale: 2.0, scaling: cdda_sim::martial_arts::Scaling::None },
            Bonus { scale: 0.5, scaling: cdda_sim::martial_arts::Scaling::Skill { name: "dex".to_owned(), level: 4.0 } },
        ],
    );
    assert_eq!(mult_of(&mult, "block"), 4.0, "**multipliers multiply**: two times half of four");
    assert_eq!(mult_of(&mult, "speed"), 1.0, "a stat with no multipliers is worth one, not zero");

    // **Two negatives multiply to a positive**, which is why the clamp needs an odd count to show: one bonus of minus two is a product of minus two, and that is what the reference floors.
    let mut negative = BTreeMap::new();
    negative.insert(
        "speed".to_owned(),
        vec![Bonus { scale: -2.0, scaling: cdda_sim::martial_arts::Scaling::None }],
    );
    assert_eq!(mult_of(&negative, "speed"), 0.0, "**and a product below zero is clamped to zero**");
}

/// A move's `weighting` is how many times it appears in the list the pick chooses from, and **every move that qualifies appears at least once**.
///
/// Reference: `Character::pick_technique` (`melee.cpp:1406-1440`): the qualified techniques are pushed once, then `weighting - 1` more times, and `random_entry` picks uniformly from that list.
/// So a weighting of four is four entries out of five, and **a weighting of nothing is still one entry** - the `> 1` guard means the loop never runs, but the first push already happened.
#[test]
fn a_move_s_weighting_is_how_many_entries_it_gets() {
    use cdda_sim::martial_arts::{Candidate, pick};

    let candidates = vec![
        Candidate { id: "tec_a".to_owned(), weighting: 1 },
        Candidate { id: "tec_b".to_owned(), weighting: 3 },
    ];
    // Four entries: a, b, b, b. A roll is `[0, 1)`, so the boundaries are three quarters of the way through.
    assert_eq!(pick(&candidates, 0.0).as_deref(), Some("tec_a"), "the first entry is the first move");
    assert_eq!(pick(&candidates, 0.24).as_deref(), Some("tec_a"), "**a quarter of the range is the one-entry move**");
    assert_eq!(pick(&candidates, 0.25).as_deref(), Some("tec_b"), "and the next quarter is the three-entry one");
    assert_eq!(pick(&candidates, 0.99).as_deref(), Some("tec_b"), "right up to the end");

    // **A weighting of nothing is still offered**: the reference pushes every qualified technique before it duplicates any.
    let zero = vec![
        Candidate { id: "tec_never".to_owned(), weighting: 0 },
        Candidate { id: "tec_once".to_owned(), weighting: 1 },
    ];
    assert_eq!(pick(&zero, 0.0).as_deref(), Some("tec_never"), "a move weighted nothing is still one entry in two");
    assert_eq!(pick(&zero, 0.5).as_deref(), Some("tec_once"), "and the other is the other half");

    // Nothing qualifies, so nothing is picked - which the reference spells as `tec_none`.
    assert_eq!(pick(&[], 0.5), None, "**an empty list picks nothing at all**");
}

/// The data does weight its moves, so the duplication branch is a rule the corpus needs rather than a hypothetical.
#[test]
fn the_corpus_weights_its_moves_evenly_enough_to_need_the_rule() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../corpus/martial-arts/martial-arts.json");
    let text = std::fs::read(path).expect("the martial arts dump exists");
    let value = cdda_json::parse(&text).expect("the dump is valid JSON");
    let cdda_json::Value::Object(root) = value else {
        panic!("the dump's root is an object");
    };
    let Some(cdda_json::Value::Array(rows)) = root.get("styles") else {
        panic!("the dump has styles");
    };
    let mut weightings = Vec::new();
    for row in rows {
        let cdda_json::Value::Object(style) = row else {
            continue;
        };
        let Some(cdda_json::Value::Array(moves)) = style.get("techniques") else {
            continue;
        };
        for one in moves {
            let cdda_json::Value::Object(move_row) = one else {
                continue;
            };
            if let Some(cdda_json::Value::Int(weighting)) = move_row.get("weighting") {
                weightings.push(*weighting);
            }
        }
    }
    assert_eq!(weightings.len(), 171, "**a hundred and seventy-one moves in the corpus**");
    assert!(
        weightings.iter().filter(|weighting| **weighting > 1).count() > 20,
        "and more than twenty of them are weighted above one, so the duplication is not a corner case"
    );
    assert_eq!(
        weightings.iter().copied().max(),
        Some(100),
        "the heaviest weighting is a hundred"
    );
}

/// A move is offered only when the moment is its moment.
///
/// Reference: `Character::evaluate_technique` (`melee.cpp:1446-1550`). Six refusals, and each one is a line a port is tempted to drop because the move "looks fine":
///
/// - a **dummy** is never offered (`:1451-1455`);
/// - a **defensive** move is not an attack (`:1457-1461`);
/// - a move that is **not `crit_ok`** needs the moment's own criticality to match its `crit_tec` (`:1512-1517`);
/// - a move that **needs ammunition** wants a loaded weapon (`:1519-1523`);
/// - a **disarm** against a body with no weapon does nothing (`:1528-1532`);
/// - a **negative weighting** is a *chance*, not a prohibition: it rolls `one_in( |weighting| )` and only then qualifies (`:1544-1549`).
#[test]
fn a_move_is_offered_only_when_the_moment_is_its_moment() {
    use cdda_sim::martial_arts::{Moment, TechniqueFlags, offers};

    let plain = TechniqueFlags {
        weighting: 1,
        dummy: false,
        defensive: false,
        crit_tec: false,
        crit_ok: false,
        reach_tec: false,
        reach_ok: false,
        dodge_counter: false,
        block_counter: false,
        needs_ammo: false,
        disarms: false,
        take_weapon: false,
    };
    let ordinary = Moment {
        crit: false,
        reach_attacking: false,
        dodge_counter: false,
        block_counter: false,
        loaded: false,
        target_armed: true,
        self_armed: false,
        wall_adjacent: true,
    };
    assert!(offers(&plain, &ordinary, 0.5), "an ordinary move is offered in an ordinary moment");

    // A dummy is never offered, whatever else it says.
    let mut dummy = plain.clone();
    dummy.dummy = true;
    assert!(!offers(&dummy, &ordinary, 0.5), "**a dummy is never offered**");

    // A defensive move is a counter, not an attack.
    // **A counter is flagged twice**: `defensive` says it is not an attack, and the counter flag says which moment it answers. One without the other is refused, because the reference checks
    // both `defensive` and that `block_counter == tec.block_counter`.
    let mut defensive = plain.clone();
    defensive.defensive = true;
    defensive.block_counter = true;
    assert!(!offers(&defensive, &ordinary, 0.5), "**a defensive move is not an attack**");
    assert!(
        offers(&defensive, &Moment { block_counter: true, ..ordinary.clone() }, 0.5),
        "but it is offered to a block counter"
    );

    // A critical-only move needs the critical, and a move that is not crit_ok refuses a critical moment.
    let mut critical_only = plain.clone();
    critical_only.crit_tec = true;
    assert!(!offers(&critical_only, &ordinary, 0.5), "**a critical move is not offered on an ordinary blow**");
    assert!(
        offers(&critical_only, &Moment { crit: true, ..ordinary.clone() }, 0.5),
        "and is offered when the blow is critical"
    );
    assert!(
        !offers(&plain, &Moment { crit: true, ..ordinary.clone() }, 0.5),
        "**a move that is not crit_ok refuses the critical moment**, because crit != its crit_tec"
    );

    // Ammunition, reach and the two weapon rules.
    let mut needs_ammo = plain.clone();
    needs_ammo.needs_ammo = true;
    assert!(!offers(&needs_ammo, &ordinary, 0.5), "a move that needs a loaded weapon is not offered empty");
    assert!(
        offers(&needs_ammo, &Moment { loaded: true, ..ordinary.clone() }, 0.5),
        "and is offered when the weapon is loaded"
    );

    let mut disarm = plain.clone();
    disarm.disarms = true;
    assert!(!offers(&disarm, &Moment { target_armed: false, ..ordinary.clone() }, 0.5), "a disarm does nothing against empty hands");

    let mut reach = plain.clone();
    reach.reach_tec = true;
    assert!(!offers(&reach, &ordinary, 0.5), "**a reach move needs a reach attack**");
    assert!(
        offers(&reach, &Moment { reach_attacking: true, ..ordinary.clone() }, 0.5),
        "and is offered when the attack reaches"
    );

    // A negative weighting is a chance rather than a prohibition.
    let mut occasional = plain.clone();
    occasional.weighting = -4;
    assert!(offers(&occasional, &ordinary, 0.0), "**a weighting of minus four is offered on a roll of nothing**");
    assert!(!offers(&occasional, &ordinary, 0.5), "and refused on a roll of a half, because one in four is a quarter of the range");
    assert!(offers(&occasional, &ordinary, 0.1), "but a tenth is inside that quarter");
}

/// The store answers which move a striker makes, from the style it carries and what is in its hands.
///
/// Reference: `Character::pick_technique` (`melee.cpp:1406-1440`) over `martial_arts_data->get_all_techniques`, which is the selected style's own set filtered by
/// `ma_technique::is_valid_character`. **The port keeps the selected style as a component**, because a style is what a creature has learned rather than a table to look up.
#[test]
fn the_store_answers_which_move_a_striker_makes() {
    use bevy_ecs::prelude::*;
    use cdda_sim::martial_arts::{Moment, Style, Technique, TechniqueFlags, pick_for};

    let mut world = World::new();
    let fighter = cdda_sim::creature::spawn_creature(&mut world, "the fighter", (0, 0, 0));
    let zombie = cdda_sim::creature::spawn_creature(&mut world, "a zombie", (1, 0, 0));

    let plain = TechniqueFlags {
        weighting: 1,
        dummy: false,
        defensive: false,
        crit_tec: false,
        crit_ok: false,
        reach_tec: false,
        reach_ok: false,
        dodge_counter: false,
        block_counter: false,
        needs_ammo: false,
        disarms: false,
        take_weapon: false,
    };
    let mut critical = plain.clone();
    critical.crit_tec = true;
    let ordinary = plain.clone();
    let style = Style {
        id: "style_test".to_owned(),
        techniques: vec![
            Technique {
                id: "tec_ordinary".to_owned(),
                requirement: cdda_sim::martial_arts::Requirement {
                    unarmed_allowed: true,
                    melee_allowed: true,
                    strictly_unarmed: false,
                    wall_adjacent: false,
                    weapon_categories: Vec::new(),
                },
                flags: ordinary,
                movement: cdda_sim::martial_arts::AppliedMove::default(),
            },
            Technique {
                id: "tec_critical".to_owned(),
                requirement: cdda_sim::martial_arts::Requirement {
                    unarmed_allowed: true,
                    melee_allowed: true,
                    strictly_unarmed: false,
                    wall_adjacent: false,
                    weapon_categories: Vec::new(),
                },
                flags: critical,
                movement: cdda_sim::martial_arts::AppliedMove::default(),
            },
        ],
    };

    // No style, no move - a fighter who has learned nothing swings the plain blow.
    assert_eq!(
        pick_for(&mut world, fighter, zombie, &Moment::ordinary(), 0.5),
        None,
        "**a fighter with no style offers no move**"
    );

    world
        .entity_mut(fighter)
        .insert(cdda_sim::martial_arts::Practising { style });
    // In an ordinary moment the critical-only move is refused, so the ordinary one is all there is.
    assert_eq!(
        pick_for(&mut world, fighter, zombie, &Moment::ordinary(), 0.5).as_deref(),
        Some("tec_ordinary"),
        "an ordinary moment offers the ordinary move"
    );
    // In a critical moment the ordinary move is refused and the critical one is offered.
    let moment = Moment {
        crit: true,
        ..Moment::ordinary()
    };
    assert_eq!(
        pick_for(&mut world, fighter, zombie, &moment, 0.5).as_deref(),
        Some("tec_critical"),
        "**and a critical moment offers only the move that wants one**"
    );
}

/// A style learned from the data can be picked from, and one the data does not define cannot.
///
/// Reference: `Character::martial_arts_data` is set from the style's own id (`character_martial_arts::set_style`, `martialarts.cpp`), and a style id the data never defined is refused rather than
/// installed as an empty style - **a fighter with an empty style and a fighter with none are the same fighter**, so the port must not confuse them at the point of learning.
#[test]
fn a_style_is_learned_from_the_data() {
    use cdda_sim::martial_arts::{Moment, learn_style, pick_for};

    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/martial-arts/martial-arts.json");
    let text = std::fs::read(path).expect("the martial arts dump exists");
    let value = cdda_json::parse(&text).expect("the dump is valid JSON");
    let cdda_json::Value::Object(root) = value else {
        panic!("the dump's root is an object");
    };
    let Some(cdda_json::Value::Array(rows)) = root.get("styles") else {
        panic!("the dump has styles");
    };
    let styles = cdda_content::martial_arts::Styles {
        all: rows
            .iter()
            .filter_map(|row| match row {
                cdda_json::Value::Object(object) => cdda_content::martial_arts::read_style(object),
                _ => None,
            })
            .collect(),
    };

    let mut world = bevy_ecs::prelude::World::new();
    cdda_sim::martial_arts::install(&mut world, &styles);
    let fighter = cdda_sim::creature::spawn_creature(&mut world, "the fighter", (0, 0, 0));
    let zombie = cdda_sim::creature::spawn_creature(&mut world, "a zombie", (1, 0, 0));

    assert!(
        learn_style(&mut world, fighter, "style_aikido"),
        "**a style the data defines is learned**"
    );
    let chosen = pick_for(&mut world, fighter, zombie, &Moment::ordinary(), 0.5);
    assert!(
        chosen.is_some(),
        "**and a fighter who has learned aikido has moves to make**: {chosen:?}"
    );
    let explained = chosen.expect("a move");
    assert!(
        explained.starts_with("tec_"),
        "the answer is one of the data's own move ids: {explained}"
    );

    // A style the data never defined is refused, and the fighter keeps the one it had.
    assert!(
        !learn_style(&mut world, fighter, "style_from_a_mod_nobody_installed"),
        "a style the data does not define is not learned"
    );
    assert_eq!(
        pick_for(&mut world, fighter, zombie, &Moment::ordinary(), 0.5),
        Some(explained),
        "and the fighter still knows the style it had"
    );
}

/// The swing asks the style, and the move it made is left where a turn can see it.
///
/// Reference: `Character::melee_attack` picks a technique before it strikes (`melee.cpp:754`: `attack = pick_technique( t, cur_weapon, critical_hit, false, false )`) and the chosen move travels
/// with that blow. The port's blow is a function call rather than a struct, so the move it chose is **left on the striker** as a component - a store fact a turn, a test or a UI can read, which is
/// the same reason every other derived answer in this port is a component rather than a return value nobody keeps.
#[test]
fn the_swing_asks_the_style_and_leaves_the_move_it_made() {
    use bevy_ecs::prelude::*;
    use cdda_sim::martial_arts::{Practising, Style, Technique, TechniqueFlags, Requirement, LastMove};

    let mut world = World::new();
    cdda_sim::singletons::put_rolls(&mut world, cdda_core_types::rng::Rolls::new(5));
    cdda_sim::item_registry::set_item_types(
        &mut world,
        &[cdda_sim::item_registry::Kind {
            id: "rock",
            volume_ml: 50,
            weight_g: 657,
            pockets: &[],
            thrown_damage: 0.0,
            melee_bash: 20.0,
            melee_cut: 0.0,
            melee_stab: 0.0,
            to_hit: 40,
        }],
    );
    let kind = cdda_sim::item_registry::id_of(&mut world, "rock").expect("the rock");
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
    world.entity_mut(survivor).insert((
        cdda_sim::creature::Dexterity { base: 12 },
        cdda_sim::creature::Perception { base: 12 },
        cdda_sim::creature::Moves { left: 400 },
        cdda_sim::creature::Wielding(kind),
    ));
    let zombie = cdda_sim::creature::spawn_creature(&mut world, "a zombie", (1, 0, 0));
    world.entity_mut(zombie).insert((
        cdda_sim::creature::Health { hp: 9_000, max_hp: 9_000 },
        cdda_sim::creature::Armour { bash: 0, cut: 0, stab: 0 },
    ));

    // A fighter with no style leaves no move behind, which is the plain blow.
    cdda_sim::creature::swing_what_the_character_holds(&mut world, survivor, zombie);
    assert_eq!(
        world.get::<LastMove>(survivor).and_then(|last| last.0.clone()),
        None,
        "**a fighter with no style makes no move**"
    );

    // A style with one move that can never be refused: the swing should leave it behind.
    let flags = TechniqueFlags {
        weighting: 1,
        dummy: false,
        defensive: false,
        crit_tec: false,
        crit_ok: true,
        reach_tec: false,
        reach_ok: false,
        dodge_counter: false,
        block_counter: false,
        needs_ammo: false,
        disarms: false,
        take_weapon: false,
    };
    world.entity_mut(survivor).insert(Practising {
        style: Style {
            id: "style_test".to_owned(),
            techniques: vec![Technique {
                id: "tec_always".to_owned(),
                requirement: Requirement {
                    unarmed_allowed: true,
                    melee_allowed: true,
                    strictly_unarmed: false,
                    wall_adjacent: false,
                    weapon_categories: Vec::new(),
                },
                flags,
                movement: cdda_sim::martial_arts::AppliedMove::default(),
            }],
        },
    });
    cdda_sim::creature::swing_what_the_character_holds(&mut world, survivor, zombie);
    assert_eq!(
        world.get::<LastMove>(survivor).and_then(|last| last.0.clone()).as_deref(),
        Some("tec_always"),
        "**and a fighter with a style leaves the move it made**"
    );
}

/// A move's effects land on the body it hit, by the chance the data gives them.
///
/// Reference: `melee.cpp:1759-1785`: for each repeat, each of the technique's effects is added when **`x_in_y( chance, 100 )`** holds **and** either the blow drew blood or the effect does not
/// need damage (`di.total_damage() != 0 || !eff.on_damage`). A move's `stun_dur` adds `stunned` for a random duration from one turn to that many, and `down_dur` does the same for `downed`.
#[test]
fn a_move_s_effects_land_by_their_own_chance() {
    use cdda_sim::martial_arts::{AppliedMove, MoveEffect, apply_move};

    let mut world = bevy_ecs::prelude::World::new();
    cdda_sim::singletons::put_rolls(&mut world, cdda_core_types::rng::Rolls::new(11));
    cdda_sim::effect_registry::set_effect_types(
        &mut world,
        &["stunned", "downed", "bleed"]
            .iter()
            .map(|id| cdda_sim::effect_registry::bare(*id))
            .collect::<Vec<_>>(),
    );
    let zombie = cdda_sim::creature::spawn_creature(&mut world, "a zombie", (1, 0, 0));

    let always = MoveEffect {
        id: "bleed".to_owned(),
        duration: 5,
        permanent: false,
        on_damage: false,
        chance: 100,
    };
    let needs_damage = MoveEffect {
        id: "bleed".to_owned(),
        duration: 5,
        permanent: false,
        on_damage: true,
        chance: 100,
    };
    let half = MoveEffect {
        id: "bleed".to_owned(),
        duration: 5,
        permanent: false,
        on_damage: false,
        chance: 50,
    };
    let move_with = |effects: Vec<MoveEffect>| AppliedMove {
        effects,
        stun_dur: 0,
        down_dur: 0,
    };

    // An effect that does not need damage lands on a blow that drew nothing.
    let landed = apply_move(&mut world, zombie, &move_with(vec![always.clone()]), 0);
    assert_eq!(landed, vec!["bleed".to_owned()], "**an effect that needs no damage lands on a harmless blow**");
    assert!(cdda_sim::applied_effects::has(&world, zombie, "bleed"), "and is on the body");

    // One that needs damage does not.
    cdda_sim::applied_effects::remove(&mut world, zombie, "bleed");
    let landed = apply_move(&mut world, zombie, &move_with(vec![needs_damage.clone()]), 0);
    assert!(landed.is_empty(), "**an effect that needs damage does not land on a blow that drew none**");
    let landed = apply_move(&mut world, zombie, &move_with(vec![needs_damage]), 4);
    assert_eq!(landed, vec!["bleed".to_owned()], "and does land when the blow drew blood");

    // The chance is a roll out of a hundred, so half is the first half of the range.
    cdda_sim::applied_effects::remove(&mut world, zombie, "bleed");
    // **The comparison is inclusive**: `x_in_y( 50, 100 )` is `rng_float( 0, 1 ) <= 0.5` (`rng.cpp:120-123`), so a roll of exactly a half lands and the refusal starts just above it.
    cdda_sim::singletons::put_rolls(&mut world, cdda_core_types::rng::Rolls::fixed(0.51));
    assert!(
        apply_move(&mut world, zombie, &move_with(vec![half.clone()]), 0).is_empty(),
        "**a fifty-in-a-hundred effect refuses a roll of fifty-one**"
    );
    cdda_sim::singletons::put_rolls(&mut world, cdda_core_types::rng::Rolls::fixed(0.1));
    assert_eq!(
        apply_move(&mut world, zombie, &move_with(vec![half]), 0),
        vec!["bleed".to_owned()],
        "and lands on a roll of ten"
    );

    // A stun lands on the body as `stunned`, for at least the one turn the reference never goes below.
    let mut stunned = move_with(Vec::new());
    stunned.stun_dur = 3;
    let landed = apply_move(&mut world, zombie, &stunned, 0);
    assert_eq!(landed, vec!["stunned".to_owned()], "a stunning move lands a stun");
    let duration = cdda_sim::applied_effects::worn(&world, zombie)
        .into_iter()
        .find(|worn| worn.id == "stunned")
        .map(|worn| worn.duration)
        .expect("a stun with a duration");
    assert!(
        (1..=3).contains(&duration),
        "**the stun lasts from one turn to three, never zero**: {duration}"
    );
}

/// The move the swing picked is the move that lands: its stun and its effects reach the body it hit.
///
/// Reference: `melee.cpp:1759-1785` runs inside the blow, after the damage, for the technique `melee_attack` chose. The port picked the move before striking (round 1385) and **applies it after** -
/// the same order, and the same reason: the reference's `on_damage` rule needs to know whether the blow drew blood, which is only true once the blow has landed.
#[test]
fn the_move_the_swing_picked_is_the_move_that_lands() {
    use bevy_ecs::prelude::*;
    use cdda_sim::martial_arts::{AppliedMove, MoveEffect, Practising, Requirement, Style, Technique, TechniqueFlags};

    let mut world = World::new();
    cdda_sim::singletons::put_rolls(&mut world, cdda_core_types::rng::Rolls::new(3));
    cdda_sim::effect_registry::set_effect_types(
        &mut world,
        &["stunned", "downed", "bleed"]
            .iter()
            .map(|id| cdda_sim::effect_registry::bare(id))
            .collect::<Vec<_>>(),
    );
    cdda_sim::item_registry::set_item_types(
        &mut world,
        &[cdda_sim::item_registry::Kind {
            id: "rock",
            volume_ml: 50,
            weight_g: 657,
            pockets: &[],
            thrown_damage: 0.0,
            melee_bash: 20.0,
            melee_cut: 0.0,
            melee_stab: 0.0,
            to_hit: 40,
        }],
    );
    let kind = cdda_sim::item_registry::id_of(&mut world, "rock").expect("the rock");
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
    world.entity_mut(survivor).insert((
        cdda_sim::creature::Dexterity { base: 12 },
        cdda_sim::creature::Perception { base: 12 },
        cdda_sim::creature::Moves { left: 400 },
        cdda_sim::creature::Wielding(kind),
    ));
    let zombie = cdda_sim::creature::spawn_creature(&mut world, "a zombie", (1, 0, 0));
    world.entity_mut(zombie).insert((
        cdda_sim::creature::Health { hp: 9_000, max_hp: 9_000 },
        cdda_sim::creature::Armour { bash: 0, cut: 0, stab: 0 },
    ));

    let flags = TechniqueFlags {
        weighting: 1,
        dummy: false,
        defensive: false,
        crit_tec: false,
        crit_ok: true,
        reach_tec: false,
        reach_ok: false,
        dodge_counter: false,
        block_counter: false,
        needs_ammo: false,
        disarms: false,
        take_weapon: false,
    };
    world.entity_mut(survivor).insert(Practising {
        style: Style {
            id: "style_test".to_owned(),
            techniques: vec![Technique {
                id: "tec_stunner".to_owned(),
                requirement: Requirement {
                    unarmed_allowed: true,
                    melee_allowed: true,
                    strictly_unarmed: false,
                    wall_adjacent: false,
                    weapon_categories: Vec::new(),
                },
                flags,
                movement: AppliedMove {
                    effects: vec![MoveEffect {
                        id: "bleed".to_owned(),
                        duration: 5,
                        permanent: false,
                        on_damage: true,
                        chance: 100,
                    }],
                    down_dur: 0,
                    stun_dur: 3,
                },
            }],
        },
    });

    cdda_sim::creature::swing_what_the_character_holds(&mut world, survivor, zombie);
    assert!(cdda_sim::applied_effects::has(&world, zombie, "stunned"), "**the move's stun lands on the body it hit**");
    assert!(cdda_sim::applied_effects::has(&world, zombie, "bleed"), "and so does its effect, because the blow drew blood");
}

/// A move that needs limbs needs **enough of them still standing**.
///
/// Reference: `character_martial_arts::choose_attack_vector` (`martialarts.cpp:1495-1515`): for each `limb_req` the reference counts the user's parts of that type whose hit points are **above
/// their health limit**, and discards the vector when the count falls short. A vector needing two arms is refused by a fighter with one arm left, which is the whole point of stating it.
#[test]
fn a_vector_that_needs_limbs_needs_enough_of_them() {
    use cdda_sim::martial_arts_vectors::{LimbState, VectorNeed, limbs_enough};

    let two_arms = VectorNeed {
        kind: "arm".to_owned(),
        count: 2,
    };
    let one_arm = VectorNeed {
        kind: "arm".to_owned(),
        count: 1,
    };
    let both = vec![LimbState { kind: "arm".to_owned(), healthy: true }, LimbState { kind: "arm".to_owned(), healthy: true }];
    assert!(limbs_enough(&both, &two_arms), "two healthy arms satisfy a need for two");
    assert!(limbs_enough(&both, &one_arm), "and a need for one");

    let one_left = vec![LimbState { kind: "arm".to_owned(), healthy: true }, LimbState { kind: "arm".to_owned(), healthy: false }];
    assert!(
        !limbs_enough(&one_left, &two_arms),
        "**a ruined arm does not count towards a need for two**"
    );
    assert!(limbs_enough(&one_left, &one_arm), "but it satisfies a need for one");

    let legs = vec![LimbState { kind: "leg".to_owned(), healthy: true }, LimbState { kind: "leg".to_owned(), healthy: true }];
    assert!(
        !limbs_enough(&legs, &one_arm),
        "**a leg is not an arm**: the need names the kind of limb"
    );
}

/// A move's aims are picked **by weight**, and the weight is a share of the total rather than a rank.
///
/// Reference: `weighted_float_list::pick_ent` (`weighted_list.h:284-295`): the draw is `randi / UINT_MAX * total_weight`, the entries are walked accumulating weight, and the walk stops at the
/// first whose accumulated weight is **`>=` the draw** - so a draw that lands exactly on a boundary goes to the **earlier** entry, not the later one, which is the opposite of what a boundary test
/// written from habit would assert. And `pick` answers nothing at all when the total weight is zero (`:137-143`), so a list of zero-weight aims aims nowhere.
#[test]
fn aims_are_picked_by_their_share_of_the_weight() {
    use cdda_sim::martial_arts_vectors::{WeightedAim, weighted_pick};

    let aims = vec![
        WeightedAim { id: "vector_head".to_owned(), weight: 1.0 },
        WeightedAim { id: "vector_arm".to_owned(), weight: 3.0 },
    ];
    assert_eq!(weighted_pick(&aims, 0.0).as_deref(), Some("vector_head"), "a draw of nothing takes the first aim");
    assert_eq!(
        weighted_pick(&aims, 0.24).as_deref(),
        Some("vector_head"),
        "**a quarter of the weight is still the first aim**, because one of four is its whole share"
    );
    assert_eq!(weighted_pick(&aims, 0.26).as_deref(), Some("vector_arm"), "just past it, the second");
    assert_eq!(weighted_pick(&aims, 0.99).as_deref(), Some("vector_arm"), "and the rest of the range is the second");

    // **The boundary leans earlier**: a draw of exactly one quarter accumulates to exactly the first aim's weight, and `>=` keeps it there.
    assert_eq!(
        weighted_pick(&aims, 0.25).as_deref(),
        Some("vector_head"),
        "**a draw exactly on the boundary stays with the earlier aim**"
    );

    // Nothing weighs anything, so nothing is aimed at - which the reference spells as a null pointer.
    let weightless = vec![
        WeightedAim { id: "vector_a".to_owned(), weight: 0.0 },
        WeightedAim { id: "vector_b".to_owned(), weight: 0.0 },
    ];
    assert_eq!(weighted_pick(&weightless, 0.5), None, "**a list of weightless aims aims at nothing**");
    assert_eq!(weighted_pick(&[], 0.5), None, "and neither does an empty one");
}
