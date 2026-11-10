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
