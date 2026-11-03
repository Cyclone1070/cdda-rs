//! Things inside things.
//!
//! Reference: `item::contents` (`item.cpp`), which the reference keeps as a tree inside the item and the port keeps as a link between entities.

use bevy_ecs::prelude::*;


/// A thing inside another thing has no square, and the container weighs what it holds.
///
/// Reference: `item::contents` and `item::weight( include_contents )` (`item.cpp:7229-7305`): the inner item is part of the container rather than standing
/// somewhere, so the container's weight includes it, and a container cannot hold itself.
#[test]
fn a_thing_inside_a_thing_has_no_square_and_adds_its_weight() {
    let mut world = World::new();
    let mut level = cdda_core_types::level::Level::new();
    level.set_terrain(0, 0, 0, 0);
    cdda_sim::load_level(&mut world, &level);
    let names = {
        // A registry with a can and a rag, by the numbers the drop calls below use.
        let mut names = cdda_sim::item_registry::ItemNames {
            names: vec![String::new(), "can".to_owned(), "rag".to_owned()],
            by_name: [
                ("can".to_owned(), cdda_sim::ItemTypeId(1)),
                ("rag".to_owned(), cdda_sim::ItemTypeId(2)),
            ]
            .into_iter()
            .collect(),
            volumes_ml: vec![0, 500, 250],
            weights_g: vec![0, 100, 5],
            holds_ml: vec![0, 500, 0],
            pockets: Vec::new(),
            pocket_ranges: vec![(0, 0), (0, 0), (0, 0)],
            thrown_damage: Vec::new(),
        };
        names.names.truncate(3);
        names
    };
    let can = cdda_sim::drop_item(&mut world, (0, 0), cdda_sim::ItemTypeId(1), 1);
    let rag = cdda_sim::drop_item(&mut world, (0, 0), cdda_sim::ItemTypeId(2), 2);
    assert_eq!(
        cdda_sim::containment::weight_of(&world, can, &names, true),
        100,
        "an empty can weighs its own hundred grams"
    );
    assert!(cdda_sim::containment::put_inside(&mut world, rag, can), "a can holds a rag");
    assert!(
        world.get::<cdda_sim::Position>(rag).is_none(),
        "what is inside a can is not standing on the square"
    );
    assert_eq!(
        cdda_sim::containment::contents_of(&world, can),
        vec![rag],
        "and the can says what it holds"
    );
    assert_eq!(
        cdda_sim::containment::weight_of(&world, can, &names, true),
        110,
        "two five-gram rags are ten grams on top"
    );
    assert_eq!(
        cdda_sim::containment::weight_of(&world, can, &names, false),
        100,
        "and the can alone is still a hundred"
    );
    assert!(
        !cdda_sim::containment::put_inside(&mut world, can, can),
        "a can cannot be inside itself"
    );
    assert!(
        !cdda_sim::containment::put_inside(&mut world, can, rag),
        "and not inside what it already holds"
    );
    assert!(
        cdda_sim::containment::take_out(&mut world, rag, (0, 0, 0)),
        "taking it out puts it back on a square"
    );
    assert!(world.get::<cdda_sim::Position>(rag).is_some());
}
