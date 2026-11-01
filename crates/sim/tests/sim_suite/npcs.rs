//! People are a component, and the store says so.

use bevy_ecs::prelude::*;

/// A person is a creature carrying the marker, and a monster is one that is not.
///
/// Reference: the reference tells people from monsters by **type** - `Character`/`npc` beside `monster` - and asks `is_npc()`. The port keeps one store, so the
/// difference is a component: `Npc` means a person, `CreatureKind` means a monster, and a rule asks which without knowing either class existed.
#[test]
fn a_person_is_a_component_not_a_kind() {
    let mut world = World::new();
    let survivor = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
    let shopkeeper = cdda_sim::creature::spawn_creature(&mut world, "the shopkeeper", (3, 0, 0));
    let zombie = cdda_sim::creature::spawn_creature(&mut world, "a zombie", (4, 0, 0));
    cdda_sim::creature::set_kind(&mut world, zombie, cdda_sim::MonsterKindId(0));

    // Nothing is a person until the world says so, which keeps every creature the existing tests make a plain creature.
    assert!(!cdda_sim::npcs::is_npc(&world, shopkeeper));
    cdda_sim::npcs::make_npc(&mut world, shopkeeper, "SHOPKEEPER");

    assert!(cdda_sim::npcs::is_npc(&world, shopkeeper));
    assert_eq!(
        cdda_sim::npcs::unique_id_of(&world, shopkeeper),
        Some("SHOPKEEPER")
    );
    assert!(!cdda_sim::npcs::is_npc(&world, zombie), "a monster is not a person");
    assert_eq!(cdda_sim::npcs::unique_id_of(&world, zombie), None);

    // The list holds every person with the square they stand on, and nobody else.
    let listed = cdda_sim::npcs::npcs(&world);
    assert_eq!(listed, vec![(shopkeeper, (3, 0, 0))]);

    cdda_sim::npcs::make_npc(&mut world, survivor, "AVATAR");
    assert_eq!(cdda_sim::npcs::npcs(&world).len(), 2);

    // A person who is destroyed leaves the list, which is the store being the only source of truth.
    world.despawn(shopkeeper);
    let listed = cdda_sim::npcs::npcs(&world);
    assert_eq!(listed.len(), 1);
    assert_eq!(listed.first().map(|(entity, _)| *entity), Some(survivor));
}
