//! What chance an attack has of hitting a monster's weak point.
//!
//! Source: `corpus/weakpoints/weakpoint-chance.json` is `--dump-weakpoint-chance`, which writes the reference's own `weakpoint::hit_chance` over a grid of skill, difficulty and coverage, for
//! both halves of its branch. A test that wrote its own bell curve would prove the arithmetic and say nothing about whether the port's `erf` is the reference's.

#![allow(clippy::panic, clippy::expect_used, clippy::indexing_slicing)]

/// The port's normal tail is the reference's, row for row.
///
/// Reference: `weakpoint::hit_chance` (`weakpoint.cpp:536-578`). **All 240 rows must match to a ten-thousandth of a percent**, which is what a `float` return can carry: the reference's own answer
/// is a `float` multiplied by `float` coverage, so a difference in the fifth decimal is the reference's precision rather than the port's error. The tolerance is still a hundred times tighter
/// than a `float` at a hundred, which is what makes this a gate on the *formula*: an `erf` that was a percent out, a difficulty that was not shifted by ten for the bad branch, or a floor that was
/// missed would move whole columns of the grid.
#[test]
fn the_ported_weakpoint_chance_matches_the_reference() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/weakpoints/weakpoint-chance.json");
    let text = std::fs::read_to_string(&path).expect("the weakpoint dump is in the corpus");
    let value = cdda_json::parse(text.as_bytes()).expect("the dump parses");
    let cdda_json::Value::Object(root) = value else {
        panic!("the dump's root is an object");
    };
    let Some(cdda_json::Value::Array(rows)) = root.get("rows") else {
        panic!("the dump has rows");
    };
    let number = |row: &cdda_json::Value, key: &str| -> f64 {
        let cdda_json::Value::Object(object) = row else {
            panic!("a row is an object");
        };
        match object.get(key) {
            Some(cdda_json::Value::Float(found)) => *found,
            Some(cdda_json::Value::Int(found)) => *found as f64,
            _ => panic!("a row states {key}"),
        }
    };
    let mut checked = 0;
    for row in rows {
        let cdda_json::Value::Object(object) = row else {
            panic!("a row is an object");
        };
        let helps = matches!(object.get("good"), Some(cdda_json::Value::Bool(true)));
        let mine = cdda_sim::weakpoints::hit_chance(
            number(row, "coverage"),
            number(row, "difficulty"),
            number(row, "skill"),
            helps,
            1.0,
        );
        let reference = number(row, "chance");
        assert!(
            (mine - reference).abs() < 1e-4,
            "coverage {} difficulty {} skill {} good {helps}: port {mine}, reference {reference}",
            number(row, "coverage"),
            number(row, "difficulty"),
            number(row, "skill")
        );
        checked += 1;
    }
    assert_eq!(checked, 240, "**every row of the reference's grid was checked**");
}

/// A skilful attacker gets several draws at a weak point, and the reweighing favours the hard ones.
///
/// Reference: `select_weakpoint`'s `rolls = std::max( 1.0f, 1.0f + attack.wp_skill / 2.5f )` and `reweigh( base, rolls ) = 1 - ( 1 - base )^rolls` (`weakpoint.cpp:583-596`). **The reference's
/// own comment is the gate**: "a 1% base combined with 5 rolls has a 4.9% weight... a 75% chance of not hitting a weak point becomes a 23.7% chance with 5 rolls".
#[test]
fn a_skilful_attacker_reweighs_the_hard_weak_points_up() {
    use cdda_sim::weakpoints::{reweigh, rolls_for};

    // The reference's own two worked examples, to a thousandth.
    assert!((rolls_for(10.0) - 5.0).abs() < 1e-9, "**ten points of skill is five rolls**");
    assert!((rolls_for(0.0) - 1.0).abs() < 1e-9, "and a beginner gets one");
    assert!((rolls_for(-3.0) - 1.0).abs() < 1e-9, "and never fewer than one");
    assert!((reweigh(0.01, 5.0) - 0.049).abs() < 1e-3, "one percent with five rolls is 4.9 percent");
    // The reference's second example is written the other way round: **a seventy-five percent chance of *not* hitting becomes 23.7** - that is `1 - reweigh( 0.25, 5 )`, not `reweigh( 0.75, 5 )`.
    assert!((1.0 - reweigh(0.25, 5.0) - 0.237).abs() < 1e-3, "**and a quarter's chance of missing falls to 23.7 percent**");
    // A single roll changes nothing, which is why a beginner's distribution is the plain coverage.
    assert!((reweigh(0.42, 1.0) - 0.42).abs() < 1e-12, "one roll is no reweighing at all");
}

/// Which weak point a roll lands on, walked the way the reference walks its list.
///
/// Reference: `weakpoints::select_weakpoint` (`weakpoint.cpp:588-631`). Each entry's **raw** chance is added to a running base, the base is **reweighed** through `1 - (1-base)^rolls`, and the roll is
/// compared against the **difference** between consecutive reweighed figures. **The comparison is `idx < chance`, strictly**, so a roll exactly on a boundary belongs to the next entry - and the
/// answer is the *index*, because a caller wants to spend the hit, not hold a reference the store may have moved.
///
/// **A coverage of fifty is not a chance of fifty.** `hit_chance` multiplies by the normal tail, which is one half when the attacker's skill equals the weak point's difficulty - so two entries of
/// fifty coverage at difficulty zero are worth twenty-five each, and the test's boundaries are twenty-five and a hundred rather than fifty and a hundred.
#[test]
fn a_roll_walks_the_weak_points_in_reweighed_order() {
    use cdda_sim::weakpoints::{Weakpoint, reweigh, select_weakpoint};

    let list = [
        Weakpoint {
            id: "eye".to_owned(),
            coverage: 50.0,
            difficulty: 0.0,
            helps_attacker: true,
            coverage_multiplier: 1.0,
        },
        Weakpoint {
            id: "knee".to_owned(),
            coverage: 50.0,
            difficulty: 0.0,
            helps_attacker: true,
            coverage_multiplier: 1.0,
        },
    ];
    // A beginner gets one roll, so each entry's share is its own raw chance: twenty-five, then twenty-five.
    assert_eq!(select_weakpoint(&list, 0.0, 0.0), Some(0), "a low roll takes the first");
    assert_eq!(select_weakpoint(&list, 0.0, 24.999), Some(0), "**and twenty-five is the boundary**: below it is the first");
    assert_eq!(select_weakpoint(&list, 0.0, 25.0), Some(1), "at exactly twenty-five the first has had its share");
    assert_eq!(select_weakpoint(&list, 0.0, 49.999), Some(1), "and the second absorbs the rest");
    assert_eq!(
        select_weakpoint(&list, 0.0, 100.0),
        None,
        "**a roll past every share is the default weak point**, which is the monster's body rather than a failure"
    );

    // With five rolls the distribution is reweighed: the pair's fifty percent becomes 96.875, and the first entry takes most of it.
    assert!((100.0 * reweigh(0.5, 5.0) - 96.875).abs() < 1e-9, "fifty percent through five rolls is 96.875");
    // **An expert nearly always finds the first weak point**: at ten points of skill against a difficulty of zero the tail is almost one, so the pair's raw chance is nearly its whole coverage
    // and the reweighed first share is 96.875 of a hundred.
    assert_eq!(select_weakpoint(&list, 10.0, 50.0), Some(0), "**an expert's fifty is still the first weak point**");
    assert_eq!(select_weakpoint(&list, 10.0, 89.0), Some(0), "and so is eighty-nine");
    assert_eq!(select_weakpoint(&list, 10.0, 96.9), Some(1), "only past 96.875 does the second one open");
    assert_eq!(select_weakpoint(&list, 10.0, 99.99), Some(1), "which absorbs what is left of a hundred");
    assert_eq!(select_weakpoint(&list, 10.0, 100.0), None, "and the roll's own top is the body");
}

/// Hitting a weak point changes the armour it meets and the damage it deals.
///
/// Reference: `weakpoint::apply_to( resistances & )` and `apply_to( damage_instance &, bool is_crit )` (`weakpoint.cpp:496-526`). Two rules, and the second has a **substitution** in it:
///
/// ```text
/// armour:  value *= armor_mult      then   value -= armor_penalty
/// damage:  if is_crit  -> *= crit_mult       (and damage_mult is NOT applied)
///          else        -> *= damage_mult
/// ```
///
/// **A critical hit replaces the weak point's ordinary multiplier rather than stacking with it**, which is the reference's `else if`: a weak point worth twice the damage normally is not worth
/// six times on a crit unless its own `crit_mult` says so. That is the whole reason a critical hit is worth hunting - and the reason a port that multiplied both would quietly double it.
#[test]
fn a_weak_point_changes_the_armour_and_the_damage() {
    use cdda_sim::weakpoints::{WeakpointFactors, apply_to_armour, apply_to_damage};

    let factors = WeakpointFactors {
        armour_multiplier: [("bash".to_owned(), 0.5)].into_iter().collect(),
        armour_penalty: [("bash".to_owned(), 2.0)].into_iter().collect(),
        damage_multiplier: [("cut".to_owned(), 2.0)].into_iter().collect(),
        crit_multiplier: [("cut".to_owned(), 3.0)].into_iter().collect(),
    };

    // Armour: ten bash becomes five by the multiplier and three by the penalty after it - **in that order**, so a penalty is worth more against heavy armour.
    let mut armour = [("bash".to_owned(), 10.0), ("cut".to_owned(), 4.0)].into_iter().collect();
    apply_to_armour(&factors, &mut armour);
    assert!((armour["bash"] - 3.0).abs() < 1e-9, "**half of ten, less two, is three**: {}", armour["bash"]);
    assert!((armour["cut"] - 4.0).abs() < 1e-9, "and a type the weak point says nothing about is untouched");

    // Damage: a cut of ten is twenty normally, and **thirty on a crit rather than sixty**.
    let mut damage = [("cut".to_owned(), 10.0), ("bash".to_owned(), 10.0)];
    let ordinary = apply_to_damage(&factors, &mut damage, false);
    assert!((ordinary[0].1 - 20.0).abs() < 1e-9, "the cut is doubled: {}", ordinary[0].1);
    assert!((ordinary[1].1 - 10.0).abs() < 1e-9, "and the bash is untouched");

    let mut damage = [("cut".to_owned(), 10.0), ("bash".to_owned(), 10.0)];
    let critical = apply_to_damage(&factors, &mut damage, true);
    assert!(
        (critical[0].1 - 30.0).abs() < 1e-9,
        "**a crit triples, and does not also double**: {}",
        critical[0].1
    );
    assert!((critical[1].1 - 10.0).abs() < 1e-9, "and the bash is still untouched");
}

/// How good an attacker is at finding a weak point.
///
/// Reference: `Character::generic_weakpoint_skill` (`weakpoint.cpp:82-90`):
///
/// ```text
/// skill = ( level( skill_1 ) + level( skill_2 ) ) / 2
/// stat  = ( dex - 8 ) / 8 + ( per - 8 ) / 8
/// mul   = ( limb score 1 + limb score 2 ) / 2
/// return ( skill + stat ) * mul
/// ```
///
/// **It is a product, not a sum**: an attacker with no limbs to see or react with finds nothing however skilled it is, which is why a hurt head costs more than a point of skill. A monster's
/// answer is simply its melee skill (`monster.cpp:76-79`).
#[test]
fn a_weak_point_needs_a_skilled_eye_to_find() {
    use cdda_sim::weakpoints::attacker_skill;

    // A survivor with three levels of melee and none of the weapon's skill, an ordinary body's dexterity and perception, and whole eyes and head.
    let ordinary = attacker_skill(3.0, 0.0, 10, 10, 1.0, 1.0);
    assert!((ordinary - 2.0).abs() < 1e-9, "**one and a half of skill plus half of a point of stat**: {ordinary}");
    // **Eight is the centre**: a survivor of exactly eight and eight has no stat bonus at all, and no skill means no chance of finding anything.
    assert!((attacker_skill(0.0, 0.0, 8, 8, 1.0, 1.0) - 0.0).abs() < 1e-9, "a beginner of ordinary build");
    // **The limbs multiply the whole thing**: half the reaction halves the skill.
    let hurt = attacker_skill(3.0, 0.0, 10, 10, 1.0, 0.5);
    assert!((hurt - 1.5).abs() < 1e-9, "**half of the head halves the skill**: {hurt}");
    assert!(
        (attacker_skill(3.0, 0.0, 10, 10, 0.0, 0.0) - 0.0).abs() < 1e-9,
        "and no eyes and no reaction finds nothing at all"
    );
    // A better weapon skill is averaged with melee, not added to it.
    assert!(
        (attacker_skill(4.0, 2.0, 8, 8, 1.0, 1.0) - 3.0).abs() < 1e-9,
        "four and two average to three"
    );
}

/// A blow that lands on a weak point is changed by it.
///
/// Reference: `Creature::deal_damage` asks `mon->type->weakpoints.select_weakpoint( attack )` (`creature.cpp:1146`) and then spends the answer three ways - the **armour** it meets
/// (`apply_to( resistances )`), the **damage** it deals (`apply_to( damage_instance, is_crit )`), and whether the blow can critical at all (`crit_roll &= ret.wp->is_good &&
/// !ret.wp->id.empty()`).
///
/// **A weak point that hurts the monster cancels the attacker's crit**: a blow that found a bad place is not a clean one, however well it was struck. The gate below is the doubled-damage
/// half: the same seed, the same swing, two worlds - one body with the weak point and one without - and **the weak point's body takes more from the same blow**.
#[test]
fn a_blow_that_lands_on_a_weak_point_is_changed_by_it() {
    use bevy_ecs::prelude::*;
    use cdda_sim::item_registry::{Kind, set_item_types};
    use cdda_sim::weakpoints::{WeakPoint, WeakPoints, Weakpoint, WeakpointFactors};

    /// One world, one swing: a survivor with a rock in hand against a body of nine hundred hit points, with or without a weak point that doubles the bash.
    fn blow(weak: bool) -> i32 {
        let mut world = World::new();
        cdda_sim::singletons::put_rolls(&mut world, cdda_core_types::rng::Rolls::new(9));
        set_item_types(
            &mut world,
            &[Kind {
                id: "rock",
                volume_ml: 50,
                weight_g: 657,
                pockets: &[],
                thrown_damage: 0.0,
                melee_bash: 20.0,
                melee_cut: 0.0,
                melee_stab: 0.0,
                to_hit: 0,
            }],
        );
        let rock = cdda_sim::item_registry::id_of(&mut world, "rock").expect("the rock");
        let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
        let zombie = cdda_sim::creature::spawn_creature(&mut world, "the zombie", (1, 0, 0));
        world.entity_mut(zombie).insert((
            cdda_sim::creature::Health { hp: 9_000, max_hp: 9_000 },
            // **An unarmoured body**, because the fixture is about the weak point rather than about which layer the blow stopped in.
            cdda_sim::creature::Armour { bash: 0, cut: 0, stab: 0 },
        ));
        world.entity_mut(survivor).insert((
            cdda_sim::creature::Dexterity { base: 8 },
            cdda_sim::creature::Wielding(rock),
        ));
        if weak {
            let mut factors = WeakpointFactors::default();
            factors.damage_multiplier.insert("bash".to_owned(), 2.0);
            // **A hundred percent coverage at a difficulty of minus a hundred for every attack type**: it cannot be missed, so the comparison is about the multiplier rather than a roll.
            let doubled = WeakPoint {
                id: "ribs".to_owned(),
                coverage: 100.0,
                helps_attacker: true,
                difficulties: vec![-100.0; 5],
                coverage_multipliers: vec![1.0; 5],
                effects: Vec::new(),
                factors,
            };
            let plain = WeakPoint {
                id: String::new(),
                coverage: 100.0,
                helps_attacker: true,
                difficulties: vec![-100.0; 5],
                coverage_multipliers: vec![1.0; 5],
                effects: Vec::new(),
                factors: WeakpointFactors::default(),
            };
            world.entity_mut(zombie).insert(WeakPoints {
                entries: vec![doubled],
                default: plain,
                families: Vec::new(),
            });
        }
        let dealt = cdda_sim::creature::swing_what_the_character_holds(&mut world, survivor, zombie).unwrap_or(-1);
        let lost = 9_000 - cdda_sim::creature::health_of(&world, zombie).unwrap_or(9_000);
        assert_eq!(dealt, lost, "what the swing says it dealt is what the body lost");
        dealt
    }

    let plain = blow(false);
    let weak = blow(true);
    assert!(plain > 0, "the rock hurts a body with no weak points: {plain}");
    assert_eq!(weak, plain * 2, "**and the same blow into a doubled weak point is worth twice as much**: {weak} against {plain}");
}

/// A monster the store spawns from its type carries the weak points its body has.
///
/// Reference: `monster::monster( const mtype * )` copies the type onto the monster (`monster.cpp:1300-1330`), and the weak points live on the type - so a body placed in the world already has them
/// and `deal_damage` can ask (`creature.cpp:1146`). **A spawned monster with no weak points is a body the port cannot aim at**, whatever the data says.
#[test]
fn a_spawned_monster_carries_its_weak_points() {
    use cdda_json::{Value, parse};
    use std::path::Path;

    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/monsters/monsters.json");
    let text = std::fs::read(path).expect("the monster dump exists");
    let value = parse(&text).expect("the dump is valid JSON");
    let Value::Object(root) = value else {
        panic!("the dump's root is an object");
    };
    let Some(Value::Array(prototypes)) = root.get("prototypes") else {
        panic!("the dump has prototypes");
    };
    let monsters = cdda_content::monsters::load_objects(prototypes).expect("the monsters load");
    let zombie = monsters
        .iter()
        .find(|monster| monster.id == "mon_zombie")
        .expect("a zombie");
    let mut world = bevy_ecs::world::World::new();
    cdda_sim::monster_registry::set_monster_types(
        &mut world,
        &[cdda_sim::monster_registry::MonsterKind {
            id: "mon_zombie",
            hp: zombie.hp,
        }],
    );
    let kind = cdda_sim::monster_registry::id_of(&mut world, "mon_zombie").expect("the kind");
    let spawned = cdda_sim::creature::spawn_monster_of_type(&mut world, kind, zombie, (1, 1, 0), 100);
    let carried = world
        .get::<cdda_sim::weakpoints::WeakPoints>(spawned)
        .expect("**a spawned zombie keeps the weak points its type states**");
    assert_eq!(carried.entries.len(), 5, "a zombie's body has five");
    assert!(
        carried.entries.iter().all(|entry| !entry.id.is_empty()),
        "and every one of them is named"
    );
    // A monster whose type states none gets no component at all, which is the port's way of saying "nothing to find" rather than an empty list to walk.
    let plain = monsters
        .iter()
        .find(|monster| monster.weakpoints.is_none() && monster.id != "mon_null")
        .expect("some type states no weak points");
    let mut world = bevy_ecs::world::World::new();
    cdda_sim::monster_registry::set_monster_types(
        &mut world,
        &[cdda_sim::monster_registry::MonsterKind {
            id: "plain",
            hp: plain.hp,
        }],
    );
    let kind = cdda_sim::monster_registry::id_of(&mut world, "plain").expect("the kind");
    let spawned = cdda_sim::creature::spawn_monster_of_type(&mut world, kind, plain, (1, 1, 0), 100);
    assert!(
        world.get::<cdda_sim::weakpoints::WeakPoints>(spawned).is_none(),
        "a body with nothing to aim at carries no weak point set"
    );
}

/// Which difficulty a blow is read against is decided by its **biggest** damage type.
///
/// Reference: `weakpoint_attack::type_of_melee_attack` (`weakpoint.cpp:354-371`) walks the damage units and takes the type with the **largest amount** - strictly larger, so a tie keeps the first
/// the blow states - and then names it `MELEE_BASH`, `MELEE_CUT` or `MELEE_STAB`. **A blow whose biggest type is none of the three is `NONE`**, which is the index the port was reading for every
/// blow before this round: a knife read a zombie head's bash difficulty instead of its stab one.
///
/// The weak point's own `difficulty` list is in the reference's order - `NONE`, bash, cut, stab, projectile - which is the order the dump writes it in.
#[test]
fn a_blow_is_read_against_the_difficulty_of_its_biggest_damage_type() {
    use cdda_sim::weakpoints::{AttackType, difficulty_for, type_of_melee_attack};

    assert_eq!(type_of_melee_attack((20, 0, 0)), AttackType::MeleeBash, "a crowbar's blow is bash");
    assert_eq!(type_of_melee_attack((0, 20, 0)), AttackType::MeleeCut, "a machete's is cut");
    assert_eq!(type_of_melee_attack((0, 0, 20)), AttackType::MeleeStab, "and a knife's is stab");
    // **The biggest wins, not the first**: a blow of five bash and twelve stab is a stab.
    assert_eq!(type_of_melee_attack((5, 0, 12)), AttackType::MeleeStab, "the largest amount decides");
    // A tie keeps the earlier type, because the reference only replaces on a strictly larger amount.
    assert_eq!(type_of_melee_attack((10, 0, 10)), AttackType::MeleeBash, "a tie keeps the first");
    assert_eq!(type_of_melee_attack((0, 0, 0)), AttackType::None, "a blow worth nothing has no type");

    // The list is read by the index the enum states, which is the reference's own numbering.
    let difficulty = [-100.0, 2.0, 4.0, 6.0, 8.0];
    assert_eq!(difficulty_for(&difficulty, AttackType::None), -100.0, "the default entry");
    assert_eq!(difficulty_for(&difficulty, AttackType::MeleeBash), 2.0);
    assert_eq!(difficulty_for(&difficulty, AttackType::MeleeCut), 4.0);
    assert_eq!(difficulty_for(&difficulty, AttackType::MeleeStab), 6.0);
    assert_eq!(difficulty_for(&difficulty, AttackType::Projectile), 8.0);
    assert_eq!(
        difficulty_for(&[], AttackType::MeleeStab),
        0.0,
        "**a list too short to hold the index answers nothing rather than the wrong figure**"
    );
}

/// A weak point that is struck can trigger something.
///
/// Reference: `weakpoint_effect::apply_to` (`weakpoint.cpp:273-300`). Three gates before anything happens, **in this order**:
///
/// ```text
/// percent_hp = min( 100, 100 * total_damage / hp_max )
/// if percent_hp < damage_required.first or damage_required.second < percent_hp: return
/// if not ( rng_float( 0, 100 ) < chance ): return
/// then add the effect for rng( duration.first, duration.second ) turns at rng( intensity.first, intensity.second )
/// ```
///
/// **The damage gate comes first and it is a share of the body's maximum, capped at a hundred** - so a blow that removes a quarter of a small body's hit points passes a twenty percent gate while the
/// same blow against a huge one does not. The chance is a **strict** `rng < chance`, so a chance of zero never fires and a chance of a hundred always does.
#[test]
fn a_struck_weak_point_can_trigger_an_effect() {
    use cdda_sim::weakpoints::{TriggeredEffect, triggers};

    let stun = TriggeredEffect {
        effect: "stunned".to_owned(),
        chance: 100.0,
        damage_required: (20.0, 100.0),
        duration: (2, 4),
        intensity: (1, 1),
        permanent: false,
    };

    // A body of a hundred and twenty hit points taking thirty: a quarter of it, so **in range**.
    let mut rolls = cdda_core_types::rng::Rolls::new(7);
    assert!(
        triggers(&stun, 30, 120, &mut rolls),
        "**a quarter of the body passes a twenty percent gate**, and a chance of a hundred always fires"
    );
    // A body of a thousand taking the same thirty: three percent, so **out of range** whatever the chance says.
    let mut rolls = cdda_core_types::rng::Rolls::new(7);
    assert!(!triggers(&stun, 30, 1_000, &mut rolls), "three percent of a huge body does not");
    // The cap: a blow that takes everything is a hundred percent, not more, so a gate with a ceiling of a hundred still passes.
    let mut rolls = cdda_core_types::rng::Rolls::new(7);
    assert!(triggers(&stun, 500, 100, &mut rolls), "**the share is capped at a hundred**, so overkill still passes");
    // A chance of zero never fires, because the comparison is strict.
    let never = TriggeredEffect { chance: 0.0, ..stun.clone() };
    let mut rolls = cdda_core_types::rng::Rolls::new(7);
    assert!(!triggers(&never, 30, 120, &mut rolls), "a chance of zero never fires");
    // A body with no maximum has no share to speak of, and the reference would divide by zero; the port answers zero rather than a nonsense share.
    let mut rolls = cdda_core_types::rng::Rolls::new(7);
    assert!(!triggers(&stun, 30, 0, &mut rolls), "a body with no maximum triggers nothing");
}

/// A blow into a weak point that triggers an effect leaves it on the body.
///
/// Reference: `wkpt->apply_effects( *this, total_damage, attack )` (`creature.cpp:1453-1455`), called after `apply_damage`, so **the effect lands only when the blow actually landed** and is given the
/// damage the body took rather than the blow's face value. The effect is added for a duration drawn from the data's range and at an intensity from its own.
///
/// **This is the half of a weak point that no number shows**: a struck eye is not worth more damage, it blinds.
#[test]
fn a_blow_into_a_weak_point_leaves_the_effect_it_triggers() {
    use bevy_ecs::prelude::*;
    use cdda_sim::item_registry::{Kind, set_item_types};
    use cdda_sim::weakpoints::{WeakPoint, WeakPoints, WeakpointFactors};

    let mut world = World::new();
    cdda_sim::singletons::put_rolls(&mut world, cdda_core_types::rng::Rolls::new(9));
    set_item_types(
        &mut world,
        &[Kind {
            id: "rock",
            volume_ml: 50,
            weight_g: 657,
            pockets: &[],
            thrown_damage: 0.0,
            melee_bash: 20.0,
            melee_cut: 0.0,
            melee_stab: 0.0,
            to_hit: 0,
        }],
    );
    let rock = cdda_sim::item_registry::id_of(&mut world, "rock").expect("the rock");
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
    // **An effect only lands if the world has been told what it is**: the store holds effect definitions by id, so the fixture registers the one the data names.
    cdda_sim::effect_registry::set_effect_types(
        &mut world,
        &[cdda_content::effects::EffectType {
            id: "stunned".to_owned(),
            max_intensity: 5,
            max_duration: 100,
            dur_add_perc: 100,
            int_add_val: 0,
            int_decay_step: -1,
            int_decay_tick: 0,
            int_dur_factor: 0,
            int_decay_remove: false,
            main_parts_only: false,
            show_in_info: false,
            show_intensity: true,
            pain_sizing: false,
            hurt_sizing: false,
            harmful_cough: false,
            impairs_movement: false,
            pkill_addict_reduces: false,
            kill_chance: Vec::new(),
            red_kill_chance: Vec::new(),
            apply_ratings: Vec::new(),
            flags: std::collections::BTreeSet::new(),
            resist_traits: Vec::new(),
            resist_effects: Vec::new(),
            removes_effects: Vec::new(),
            blocks_effects: Vec::new(),
            mods: cdda_content::effects::ModTable::new(),
        }],
    );
    let zombie = cdda_sim::creature::spawn_creature(&mut world, "the zombie", (1, 0, 0));
    world.entity_mut(zombie).insert((
        cdda_sim::creature::Health { hp: 120, max_hp: 120 },
        cdda_sim::creature::Armour { bash: 0, cut: 0, stab: 0 },
    ));
    world.entity_mut(survivor).insert((
        cdda_sim::creature::Dexterity { base: 8 },
        cdda_sim::creature::Wielding(rock),
    ));
    // A weak point that cannot be missed and always stuns for two turns: the damage gate is met by a sixth of the body, which twenty of a hundred and twenty is.
    world.entity_mut(zombie).insert(WeakPoints {
        entries: vec![WeakPoint {
            id: "the eye".to_owned(),
            coverage: 100.0,
            helps_attacker: true,
            difficulties: vec![-100.0; 5],
            coverage_multipliers: vec![1.0; 5],
            factors: WeakpointFactors::default(),
            effects: vec![cdda_sim::weakpoints::TriggeredEffect {
                effect: "stunned".to_owned(),
                chance: 100.0,
                damage_required: (5.0, 100.0),
                duration: (2, 2),
                intensity: (1, 1),
                permanent: false,
            }],
        }],
        default: WeakPoint {
            id: String::new(),
            coverage: 100.0,
            helps_attacker: true,
            difficulties: vec![-100.0; 5],
            coverage_multipliers: vec![1.0; 5],
            factors: WeakpointFactors::default(),
            effects: Vec::new(),
        },
        families: Vec::new(),
    });

    cdda_sim::creature::swing_what_the_character_holds(&mut world, survivor, zombie);
    let worn = cdda_sim::applied_effects::worn(&world, zombie);
    let stun = worn
        .iter()
        .find(|worn| worn.id == "stunned")
        .expect("**a blow into a weak point that stuns leaves the body stunned**");
    assert_eq!(stun.duration, 2, "for the two turns the data states, from a range of two to two");
}

/// A shot spends the weak point it lands on, read against the projectile difficulty.
///
/// Reference: `Character::ranged_weakpoint_skill` (`weakpoint.cpp:97-102`) is `generic_weakpoint_skill` with the **gun's own skill twice** and **vision twice** - not the melee figures - and the
/// attack is `attack_type::PROJECTILE`, so the walk reads the fifth entry of the difficulty list rather than the bash one.
///
/// **A bullet is not a crowbar**: a weak point whose stab difficulty is six and whose projectile difficulty is a hundred is one a knife finds and a rifle does not, which is exactly the shape
/// the data gives a zombie's eye.
#[test]
fn a_shot_spends_the_weak_point_it_lands_on() {
    use bevy_ecs::prelude::*;
    use cdda_sim::weakpoints::{WeakPoint, WeakPoints, WeakpointFactors};

    let read = |name: &str| {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("../../corpus/{name}"));
        let bytes = std::fs::read(path).expect("the corpus file exists");
        cdda_json::parse(&bytes).expect("valid JSON")
    };
    let guns = cdda_content::guns::load_dump(&read("items/guns.json")).expect("the guns load");
    let lines = cdda_content::skills::load_dump(&read("skills.json")).expect("the skills load");
    let gun = guns.iter().find(|gun| gun.id == "ruger_mini").expect("the corpus has the Mini-14").clone();
    let rounds = cdda_content::ammo::load_dump(&read("items/ammo.json")).expect("the ammo loads");
    // **A cartridge carries the damage, not the gun**: `.223` is thirty-nine points of stab, which is what the weak point's multiplier has something to double.
    let round = rounds.iter().find(|round| round.id == "223").expect("the corpus has a .223").clone();

    // One world, one shot: the same seed, the same gun, the same distance - with or without a weak point that doubles the bullet.
    let shot = |weak: bool| -> i32 {
        let mut world = World::new();
        cdda_sim::singletons::put_rolls(&mut world, cdda_core_types::rng::Rolls::new(4_242));
        let shooter = cdda_sim::creature::spawn_creature(&mut world, "the shooter", (0, 0, 0));
        world
            .entity_mut(shooter)
            .insert(cdda_sim::creature::Dexterity { base: 8 })
            .insert(cdda_sim::creature::Perception { base: 8 })
            .insert(cdda_sim::creature::Moves { left: 200 });
        // **A big thing at point blank**, so the shot lands and the armour is the only thing between the bullet and the body.
        let beast = cdda_sim::creature::spawn_creature(&mut world, "a big thing", (1, 0, 0));
        world.entity_mut(beast).insert((
            cdda_sim::creature::Creature {
                name: "a big thing".to_owned(),
                size: cdda_sim::CreatureSize::Huge,
            },
            cdda_sim::creature::Health { hp: 900, max_hp: 900 },
            cdda_sim::creature::Armour { bash: 0, cut: 0, stab: 0 },
        ));
        if weak {
            let mut factors = WeakpointFactors::default();
            factors.damage_multiplier.insert("stab".to_owned(), 2.0);
            world.entity_mut(beast).insert(WeakPoints {
                entries: vec![WeakPoint {
                    id: "the heart".to_owned(),
                    coverage: 100.0,
                    helps_attacker: true,
                    // **A difficulty of minus a hundred for every type**: the point of this gate is the multiplier, not the walk.
                    difficulties: vec![-100.0; 5],
                    coverage_multipliers: vec![1.0; 5],
                    effects: Vec::new(),
                    factors,
                }],
                default: WeakPoint {
                    id: String::new(),
                    coverage: 100.0,
                    helps_attacker: true,
                    difficulties: vec![-100.0; 5],
                    coverage_multipliers: vec![1.0; 5],
                    effects: Vec::new(),
                    factors: WeakpointFactors::default(),
                },
                families: Vec::new(),
            });
        }
        let fired = cdda_sim::shot::fire_at(&mut world, shooter, &gun, &lines, (1, 0, 0), 18.0, Some(&round), None, 0.0, false);
        assert_eq!(fired.hurt, 900 - cdda_sim::creature::health_of(&world, beast).unwrap_or(900), "what the shot says it dealt is what the body lost");
        fired.hurt
    };

    let plain = shot(false);
    let weak = shot(true);
    assert!(plain > 0, "the rifle hurts a body with no weak points: {plain}");
    assert!(
        weak > plain,
        "**the same bullet into a doubled weak point is worth more**: {weak} against {plain}"
    );
    // **The doubling is not exact, because the blow is a cut and a stab at once**: the multiplier names the stab only, so the gate reads the direction and the sum.
    assert_eq!(weak, plain * 2, "the Mini-14's round is all stab, so the doubling is exact: {weak} against {plain}");
}

/// A thrown thing spends the weak point it lands on, with the thrower's own figures.
///
/// Reference: `Character::throw_weakpoint_skill` (`weakpoint.cpp:104-107`) is `generic_weakpoint_skill` with **`throw` twice** and **vision twice**, and the attack is `attack_type::PROJECTILE`
/// because a throw is not a melee blow however it lands. **The three paths differ only in which skill and which limbs they read** - the walk, the multipliers and the effects are one rule.
#[test]
fn a_thrown_thing_spends_the_weak_point_it_lands_on() {
    use bevy_ecs::prelude::*;
    use cdda_sim::weakpoints::{WeakPoint, WeakPoints, WeakpointFactors};

    // One world, one throw: the same seed, the same rock, the same distance - with or without a weak point that doubles the bash.
    let toss = |weak: bool| -> i32 {
        let mut world = World::new();
        cdda_sim::singletons::put_rolls(&mut world, cdda_core_types::rng::Rolls::new(31));
        let thrower = cdda_sim::creature::spawn_creature(&mut world, "the thrower", (0, 0, 0));
        world
            .entity_mut(thrower)
            .insert(cdda_sim::creature::Dexterity { base: 8 })
            .insert(cdda_sim::creature::Perception { base: 8 })
            .insert(cdda_sim::creature::Moves { left: 200 });
        let beast = cdda_sim::creature::spawn_creature(&mut world, "a big thing", (2, 0, 0));
        world.entity_mut(beast).insert((
            cdda_sim::creature::Creature {
                name: "a big thing".to_owned(),
                size: cdda_sim::CreatureSize::Huge,
            },
            cdda_sim::creature::Health { hp: 900, max_hp: 900 },
            cdda_sim::creature::Armour { bash: 0, cut: 0, stab: 0 },
        ));
        if weak {
            let mut factors = WeakpointFactors::default();
            factors.damage_multiplier.insert("bash".to_owned(), 2.0);
            world.entity_mut(beast).insert(WeakPoints {
                entries: vec![WeakPoint {
                    id: "the temple".to_owned(),
                    coverage: 100.0,
                    helps_attacker: true,
                    difficulties: vec![-100.0; 5],
                    coverage_multipliers: vec![1.0; 5],
                    effects: Vec::new(),
                    factors,
                }],
                default: WeakPoint {
                    id: String::new(),
                    coverage: 100.0,
                    helps_attacker: true,
                    difficulties: vec![-100.0; 5],
                    coverage_multipliers: vec![1.0; 5],
                    effects: Vec::new(),
                    factors: WeakpointFactors::default(),
                },
                families: Vec::new(),
            });
        }
        // A rock in the hand: the item registry needs the kind before the thing can exist.
        cdda_sim::item_registry::set_item_types(
            &mut world,
            &[cdda_sim::item_registry::Kind {
                id: "rock",
                volume_ml: 50,
                weight_g: 657,
                pockets: &[],
                thrown_damage: 10.0,
                melee_bash: 7.0,
                melee_cut: 0.0,
                melee_stab: 0.0,
                to_hit: -1,
            }],
        );
        let kind = cdda_sim::item_registry::id_of(&mut world, "rock").expect("the rock");
        let rock = cdda_sim::drop_item(&mut world, (0, 0), kind, 1);
        // **The rock is in hand without the carry rules**: this gate is about the throw's weak point, and a body with no strength to spare would refuse to lift it for a reason of its own.
        world
            .entity_mut(rock)
            .insert(cdda_sim::creature::Carried(thrower));
        let before = cdda_sim::creature::health_of(&world, beast).unwrap_or(900);
        cdda_sim::throwing::throw_at(&mut world, thrower, rock, (2, 0, 0));
        let after = cdda_sim::creature::health_of(&world, beast).unwrap_or(900);
        before - after
    };

    let plain = toss(false);
    let weak = toss(true);
    assert!(plain > 0, "the rock hurts a body with no weak points: {plain}");
    assert_eq!(
        weak,
        plain * 2,
        "**the same throw into a doubled weak point is worth twice as much**: {weak} against {plain}"
    );
}

/// A body's weak-point families reward the proficiency an attacker has learned.
///
/// Reference: `weakpoint_families::modifier` (`weakpoint.cpp:171-178`) and `weakpoint_family::modifier` (`:110-115`): each family adds a **bonus** when the attacker knows its proficiency and a
/// **penalty** when it does not, and the total is their **sum**.
///
/// **The two figures are not a trade**: a monster with two families whose bonuses are one and a half and a half gives an attacker who knows both **two** points of skill, and an attacker who knows
/// neither is one and a half points **worse** - which is why a butcher's training is worth more against some bodies than others.
#[test]
fn a_body_s_families_reward_the_proficiency_an_attacker_has() {
    use cdda_sim::weakpoints::{WeakpointFamily, families_modifier};

    let zombie = vec![
        WeakpointFamily {
            id: "gross_anatomy".to_owned(),
            proficiency: "prof_gross_anatomy".to_owned(),
            bonus: 1.5,
            penalty: -0.5,
        },
        WeakpointFamily {
            id: "physiology".to_owned(),
            proficiency: "prof_physiology".to_owned(),
            bonus: 0.5,
            penalty: -1.0,
        },
    ];

    // An attacker who knows both: a point and a half plus a half.
    assert!(
        (families_modifier(&zombie, &|_| true) - 2.0).abs() < 1e-9,
        "**knowing both is worth two points**: {}",
        families_modifier(&zombie, &|_| true)
    );
    // One of the two: a point and a half for the first, and the second's penalty of minus one.
    assert!(
        (families_modifier(&zombie, &|name| name == "prof_gross_anatomy") - 0.5).abs() < 1e-9,
        "knowing one is worth its bonus and the other's penalty"
    );
    // Neither: minus a half and minus one.
    assert!(
        (families_modifier(&zombie, &|_| false) + 1.5).abs() < 1e-9,
        "**knowing neither is worth the penalties, which are already negative in the data**: {}",
        families_modifier(&zombie, &|_| false)
    );
    // **A body with no families asks for nothing**, which is most of the corpus and every body a port invents.
    assert!(
        (families_modifier(&[], &|_| true) - 0.0).abs() < 1e-9,
        "a body with no families changes no skill"
    );
}

/// The body an attacker aims at is what decides which training is worth something.
///
/// Reference: `compute_wp_skill`'s last two lines (`weakpoint.cpp:401-409`) - the attacker's own figure plus `mon_tar->type->families.modifier( *chr_att )`. **The families are the target's**: it
/// is the body being struck that says which proficiency matters, which is why a butcher learns a dozen anatomies rather than one.
///
/// The gate asks the walk directly rather than swinging, so the two outcomes differ for one reason: a body whose one weak point can never be missed is found by an attacker the family **pays**, and
/// missed by one it **penalises**, because the penalty is subtracted from the very skill the walk is handed.
#[test]
fn the_body_s_families_reach_the_walk_s_skill() {
    use bevy_ecs::prelude::*;
    use cdda_sim::weakpoints::{
        AttackType, WeakPoint, WeakPoints, WeakpointFactors, WeakpointFamily, stored_weak_point,
    };

    let found = |known: bool| -> String {
        let mut world = World::new();
        cdda_sim::singletons::put_rolls(&mut world, cdda_core_types::rng::Rolls::new(5));
        let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
        let zombie = cdda_sim::creature::spawn_creature(&mut world, "a zombie", (1, 0, 0));
        let mut doubled = WeakpointFactors::default();
        doubled.damage_multiplier.insert("bash".to_owned(), 2.0);
        world.entity_mut(zombie).insert(WeakPoints {
            entries: vec![WeakPoint {
                id: "the skull".to_owned(),
                coverage: 100.0,
                helps_attacker: true,
                // **A point that cannot be missed until the family takes the skill away**: the walk is the only thing under test.
                difficulties: vec![-100.0; 5],
                coverage_multipliers: vec![1.0; 5],
                effects: Vec::new(),
                factors: doubled,
            }],
            default: WeakPoint {
                id: String::new(),
                coverage: 100.0,
                helps_attacker: true,
                difficulties: vec![-100.0; 5],
                coverage_multipliers: vec![1.0; 5],
                effects: Vec::new(),
                factors: WeakpointFactors::default(),
            },
            families: vec![WeakpointFamily {
                id: "prof_wp_zombie".to_owned(),
                proficiency: "prof_wp_zombie".to_owned(),
                bonus: 20.0,
                // **The data's penalties are already negative** - four points under for an armoured synthetic - so the rule adds a signed figure rather than subtracting an unsigned one.
                penalty: -200.0,
            }],
        });
        let base = cdda_sim::weakpoints::attacker_skill(
            0.0,
            0.0,
            12,
            12,
            cdda_sim::creature::limb_score(&world, survivor, "vision"),
            cdda_sim::creature::limb_score(&world, survivor, "reaction"),
        );
        let families = world
            .get::<WeakPoints>(zombie)
            .map(|weak| weak.families.clone())
            .unwrap_or_default();
        let skill = cdda_sim::weakpoints::skill_with_families(base, &families, &|_| known);
        let mut dice = cdda_core_types::rng::Rolls::new(5);
        stored_weak_point(&mut world, zombie, AttackType::MeleeBash, skill, &mut dice)
            .map_or_else(String::new, |weak| weak.id)
    };

    assert_eq!(
        found(true),
        "the skull",
        "**the bonus pays for the point, so the walk finds it**"
    );
    assert_eq!(
        found(false),
        "",
        "**the penalty takes the skill below the point, so the plain body answers instead**"
    );
}

/// Striking a body teaches what that body rewards.
///
/// Reference: `monster::deal_damage`'s `type->families.practice_hit( *attacker )` (`monster.cpp:3888-3892`), which is `weakpoint_families::practice` with **two seconds**
/// (`weakpoint.cpp:145-148`) - the same rule as a finished craft, pointed at the proficiencies the struck body names. **Two seconds is small and the corpus's proficiencies take hours**, so the
/// practice shows up as *practised time* rather than as a learned proficiency, which is exactly what the reference's own message-less path does.
#[test]
fn striking_a_body_practises_what_that_body_rewards() {
    use bevy_ecs::prelude::*;
    use cdda_sim::weakpoints::{WeakPoint, WeakPoints, WeakpointFactors, WeakpointFamily};

    let mut world = World::new();
    // The data's table, with one proficiency that takes four seconds so the gate can watch it fill.
    let mut dump = cdda_content::proficiencies::Proficiencies::default();
    dump.by_id.insert(
        "prof_wp_zombie".to_owned(),
        cdda_content::proficiencies::Proficiency {
            id: "prof_wp_zombie".to_owned(),
            seconds: 4,
            can_learn: true,
            required: Vec::new(),
            bonuses: Vec::new(),
        },
    );
    cdda_sim::proficiencies::install(&mut world, &dump);

    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
    let zombie = cdda_sim::creature::spawn_creature(&mut world, "a zombie", (1, 0, 0));
    world.entity_mut(zombie).insert(( 
        cdda_sim::creature::Health { hp: 9_000, max_hp: 9_000 },
        cdda_sim::creature::Armour { bash: 0, cut: 0, stab: 0 },
        WeakPoints {
            entries: Vec::new(),
            default: WeakPoint {
                id: String::new(),
                coverage: 100.0,
                helps_attacker: true,
                difficulties: vec![-100.0; 5],
                coverage_multipliers: vec![1.0; 5],
                effects: Vec::new(),
                factors: WeakpointFactors::default(),
            },
            families: vec![WeakpointFamily {
                id: "prof_wp_zombie".to_owned(),
                proficiency: "prof_wp_zombie".to_owned(),
                bonus: 2.0,
                penalty: -2.0,
            }],
        },
    ));

    let families = vec![WeakpointFamily {
        id: "prof_wp_zombie".to_owned(),
        proficiency: "prof_wp_zombie".to_owned(),
        bonus: 2.0,
        penalty: -2.0,
    }];
    // **At first nothing**, because nothing has struck it.
    assert_eq!(
        cdda_sim::proficiencies::practiced_seconds(&world, survivor, "prof_wp_zombie"),
        0,
        "a survivor who has not struck the body has practised nothing"
    );
    cdda_sim::weakpoints::practise_a_hit(&mut world, survivor, &families);
    assert_eq!(
        cdda_sim::proficiencies::practiced_seconds(&world, survivor, "prof_wp_zombie"),
        2,
        "**one hit is two seconds of practice**: {}",
        cdda_sim::proficiencies::practiced_seconds(&world, survivor, "prof_wp_zombie")
    );
    cdda_sim::weakpoints::practise_a_hit(&mut world, survivor, &families);
    assert!(
        cdda_sim::proficiencies::knows(&world, survivor, "prof_wp_zombie"),
        "**the second hit reaches the four seconds the data asks for, so the proficiency is learned**"
    );
    // A proficiency already known is not practised again, which is the reference's own guard (`proficiency.cpp:307`).
    cdda_sim::weakpoints::practise_a_hit(&mut world, survivor, &families);
    assert_eq!(
        cdda_sim::proficiencies::practiced_seconds(&world, survivor, "prof_wp_zombie"),
        0,
        "a learned proficiency reports no practised time"
    );
}

/// A real swing practises what it struck, not only the rule in isolation.
///
/// Reference: `monster::deal_damage` calls `type->families.practice_hit( *attacker )` on the way out (`monster.cpp:3888-3892`), so the practice rides on **the hit** rather than on a button.
#[test]
fn a_swing_practises_the_body_it_struck() {
    use bevy_ecs::prelude::*;
    use cdda_sim::weakpoints::{WeakPoint, WeakPoints, WeakpointFactors, WeakpointFamily};

    let mut world = World::new();
    let mut dump = cdda_content::proficiencies::Proficiencies::default();
    dump.by_id.insert(
        "prof_wp_zombie".to_owned(),
        cdda_content::proficiencies::Proficiency {
            id: "prof_wp_zombie".to_owned(),
            seconds: 600,
            can_learn: true,
            required: Vec::new(),
            bonuses: Vec::new(),
        },
    );
    cdda_sim::proficiencies::install(&mut world, &dump);
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
        WeakPoints {
            entries: Vec::new(),
            default: WeakPoint {
                id: String::new(),
                coverage: 100.0,
                helps_attacker: true,
                difficulties: vec![-100.0; 5],
                coverage_multipliers: vec![1.0; 5],
                effects: Vec::new(),
                factors: WeakpointFactors::default(),
            },
            families: vec![WeakpointFamily {
                id: "prof_wp_zombie".to_owned(),
                proficiency: "prof_wp_zombie".to_owned(),
                bonus: 2.0,
                penalty: -2.0,
            }],
        },
    ));

    cdda_sim::creature::swing_what_the_character_holds(&mut world, survivor, zombie);
    assert_eq!(
        cdda_sim::proficiencies::practiced_seconds(&world, survivor, "prof_wp_zombie"),
        2,
        "**one landed swing is two seconds toward what the body rewards**"
    );
}
