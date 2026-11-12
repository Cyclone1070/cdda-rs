//! That a **garment** protects by damage type too, which is what the materials say.
//!
//! Reference: `item::_resist` asks each material `material_type::resist( damage_type )` (`material.cpp:213-216`) over `resistances::type_resist` (`damage.cpp:719-723`),
//! which answers **zero for a type the material does not name**. A coat of leather that states bash and cut but no stab therefore stops a club and a blade and nothing at all
//! against a point.

#![allow(clippy::panic, clippy::expect_used, clippy::indexing_slicing, clippy::float_cmp)]

use bevy_ecs::prelude::*;

fn a_layer(damage_type: &str, resist: f64) -> cdda_content::armour::Layer {
    cdda_content::armour::Layer {
        damage_type: damage_type.to_owned(),
        resist,
        thickness: 1.0,
        cover: 100,
    }
}

fn a_world() -> World {
    let mut world = World::new();
    cdda_sim::turn::set_random(&mut world, cdda_core_types::rng::Rolls::new(1));
    world
}

/// **Four per millimetre against bash, one against cut, and nothing at all against a stab.**
#[test]
fn a_garment_writes_a_figure_per_damage_type() {
    let mut world = a_world();
    let wearer = cdda_sim::creature::spawn_creature(&mut world, "the wearer", (0, 0, 0));
    world.spawn((
        cdda_sim::creature::WornBy(wearer),
        cdda_sim::creature::Garment {
            layers: vec![
                ("torso".to_owned(), a_layer("bash", 4.0)),
                ("torso".to_owned(), a_layer("cut", 1.0)),
            ],
            vitals: Vec::new(),
        },
    ));
    cdda_sim::creature::dress(&mut world);
    let worn = world
        .get::<cdda_sim::creature::PartArmour>(wearer)
        .expect("the torso is covered");
    let torso = worn.0.get("torso").expect("and only the torso");
    assert_eq!(torso.get("bash"), Some(&4), "**four of bash** is the figure the layer states");
    assert_eq!(torso.get("cut"), Some(&1), "**one of cut** is its own, much smaller, figure");
    assert_eq!(
        torso.get("stab"),
        None,
        "**and nothing at all against a stab**, because no layer states that type - the reference's zero rather than a borrowed figure"
    );
}

/// **A blow reads the figure for the type it deals**, so the same coat stops a club and barely touches a blade.
#[test]
fn armour_on_answers_from_the_garment_for_the_type_asked() {
    let mut world = a_world();
    let wearer = cdda_sim::creature::spawn_creature(&mut world, "the wearer", (0, 0, 0));
    world.spawn((
        cdda_sim::creature::WornBy(wearer),
        cdda_sim::creature::Garment {
            layers: vec![
                ("torso".to_owned(), a_layer("bash", 4.0)),
                ("torso".to_owned(), a_layer("cut", 1.0)),
            ],
            vitals: Vec::new(),
        },
    ));
    cdda_sim::creature::dress(&mut world);
    assert_eq!(
        cdda_sim::creature::armour_on(&world, wearer, "torso", Some("bash")),
        4,
        "a club meets four"
    );
    assert_eq!(
        cdda_sim::creature::armour_on(&world, wearer, "torso", Some("cut")),
        1,
        "**a blade meets one**, not the bash figure"
    );
    assert_eq!(
        cdda_sim::creature::armour_on(&world, wearer, "torso", Some("stab")),
        0,
        "and a point meets nothing, which is what the data says"
    );
    assert_eq!(
        cdda_sim::creature::armour_on(&world, wearer, "torso", None),
        4,
        "a mixed blow still asks the bash figure, which the port documents as its limit"
    );
}

/// **A blade meets the garment's cut figure, not its bash figure.**
///
/// Reference: `Character::absorb_hit` walks the blow's `damage_units` and asks `get_armor_type( dt, bp )` for **each unit's own type** (`character_armor.cpp:70-86`), and
/// `Creature::deal_melee_hit` hands it the whole blow. A coat that states four against a club and five against a blade therefore stops five from the blade - a port that filled
/// one figure for all three types would stop either four or five from everything.
#[test]
fn a_cut_blow_meets_the_garments_cut_figure() {
    use cdda_sim::creature::MeleeAttack;

    /// What a cutting blow takes off a survivor wearing the coat, on one seed.
    fn taken_from_a_cut_blow() -> i32 {
        let mut world = World::new();
        let mut level = cdda_core_types::level::Level::new();
        for x in 0..4 {
            level.set_terrain(x, 0, 1, 1);
        }
        cdda_sim::load_level(&mut world, &level);
        let mut terrain = cdda_sim::terrain_table::TerrainTable::new();
        terrain.insert(1, true);
        terrain.insert_move_cost(1, 2);
        cdda_sim::set_prototypes(
            &mut world,
            cdda_sim::Prototypes {
                terrain,
                furniture: cdda_sim::terrain_table::TerrainTable::new(),
            },
        );
        let striker = cdda_sim::creature::spawn_creature(&mut world, "a knife", (0, 0, 0));
        cdda_sim::creature::set_health(&mut world, striker, 20);
        cdda_sim::creature::set_speed(&mut world, striker, 100, 0);
        world.entity_mut(striker).insert(MeleeAttack(cdda_sim::melee::Attack {
            accuracy_tenths: 400,
            fixed: cdda_sim::melee::Damage {
                cut: 10,
                ..Default::default()
            },
            dice: 1,
            sides: 1,
        }));
        let wearer = cdda_sim::creature::spawn_creature(&mut world, "the wearer", (1, 0, 0));
        cdda_sim::creature::set_health(&mut world, wearer, 100);
        world.entity_mut(wearer).insert(cdda_sim::creature::Body {
            parts: vec![cdda_sim::hit_location::BodyPart {
                name: "torso".to_owned(),
                hit_size: 36.0,
                hit_difficulty: 1.0,
                upper: false,
                lower: false,
                ..Default::default()
            }],
        });
        world.spawn((
            cdda_sim::creature::WornBy(wearer),
            cdda_sim::creature::Garment {
                layers: vec![
                    ("torso".to_owned(), a_layer("bash", 4.0)),
                    ("torso".to_owned(), a_layer("cut", 5.0)),
                ],
                vitals: Vec::new(),
            },
        ));
        cdda_sim::creature::dress(&mut world);
        cdda_sim::creature::set_goal(&mut world, striker, (1, 0, 0));
        cdda_sim::singletons::put_rolls(&mut world, cdda_core_types::rng::Rolls::new(4));
        cdda_sim::turn::run_turn(&mut world);
        100 - cdda_sim::creature::health_of(&world, wearer).expect("still alive")
    }

    // Ten of cutting and one die of one face, and **the die lands as bash** (`monster_melee_damage` adds the dice to the blunt part, `melee.cpp:2143-2146`). So the blow is
    // ten of cut against five and one of bash against four: five gets through, where a port that filled one figure for all three types lets ten through.
    assert_eq!(
        taken_from_a_cut_blow(),
        5,
        "**ten of cutting against five of armour is five**, which is the coat's cut figure and not its bash one"
    );
}

/// **Each face of a blow wears the garment that met it, and only that one.**
///
/// Reference: `Character::absorb_hit` calls `damage_armor_durability( elem, ... )` once per damage unit (`character_armor.cpp:88-104`). Two garments, one stating only bash and one
/// stating only cut, are both in the way of a blow that carries both - and a port that settled the blow once, under the first figure it found, would wear one of them and not the
/// other.
#[test]
fn a_blow_wears_every_face_that_met_it() {
    let mut world = a_world();
    let wearer = cdda_sim::creature::spawn_creature(&mut world, "the wearer", (0, 0, 0));
    let plates = world
        .spawn((
            cdda_sim::creature::WornBy(wearer),
            cdda_sim::creature::Garment {
                layers: vec![("torso".to_owned(), a_layer("bash", 1.0))],
                vitals: Vec::new(),
            },
        ))
        .id();
    let cloth = world
        .spawn((
            cdda_sim::creature::WornBy(wearer),
            cdda_sim::creature::Garment {
                layers: vec![("torso".to_owned(), a_layer("cut", 1.0))],
                vitals: Vec::new(),
            },
        ))
        .id();
    cdda_sim::creature::settle_armour_blow(&mut world, wearer, "torso", 20.0, 20.0, 0.0, 0.0);
    assert!(
        world.get::<cdda_sim::creature::Wear>(plates).is_some(),
        "**the plate that met the blunt half is worn**"
    );
    assert!(
        world.get::<cdda_sim::creature::Wear>(cloth).is_some(),
        "**and so is the cloth that met the cutting half**, which one settlement under one figure could not do"
    );
}
