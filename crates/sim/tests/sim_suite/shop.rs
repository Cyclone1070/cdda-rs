//! What a shopkeeper restocks, and when.
//!
//! Reference: `npc::shop_restock` (`npc.cpp:2100-2189`).

#![allow(clippy::expect_used)]

use bevy_ecs::prelude::World;

/// Builds a world with one item type, one group and one class, so the rule has a shop to stock.
fn a_shop() -> World {
    let mut world = World::new();
    cdda_sim::turn::set_random(&mut world, cdda_core_types::rng::Rolls::new(7));
    cdda_sim::item_registry::set_item_types(
        &mut world,
        &[cdda_sim::item_registry::Kind {
            id: "test_rock",
            volume_ml: 250,
            weight_g: 200,
            pockets: &[],
            thrown_damage: 0.0,
            melee_bash: 0.0,
            melee_cut: 0.0,
            melee_stab: 0.0,
            to_hit: 0,
        }],
    );
    let mut groups = std::collections::BTreeMap::new();
    for id in ["shop_rigid", "shop_other_rigid"] {
        groups.insert(
            id.to_owned(),
            cdda_content::item_group_draw::Definition {
                id: id.to_owned(),
                distribution: false,
                entries: vec![cdda_content::item_group_draw::Entry::Item {
                    id: "test_rock".to_owned(),
                    probability: 100,
                    count: (1, 1),
                    event: None,
                }],
            },
        );
    }
    cdda_sim::item_groups::set_groups(&mut world, groups);
    let group = |id: &str, rigid: bool| cdda_content::npc_classes::ShopkeeperItemGroup {
        group: id.to_owned(),
        trust: 0,
        strict: false,
        rigid,
    };
    let class =
        |id: &str, groups: Vec<cdda_content::npc_classes::ShopkeeperItemGroup>, days: i64| {
            cdda_content::npc_classes::NpcClass {
                id: id.to_owned(),
                name: id.to_owned(),
                job_description: String::new(),
                common: true,
                shop_item_groups: groups,
                restock_interval: days * 24 * 60 * 60,
            }
        };
    cdda_sim::npc_classes::set_classes(
        &mut world,
        vec![
            class("a_shopkeeper", vec![group("shop_rigid", true)], 6),
            class("no_shelf", Vec::new(), 6),
        ],
    );
    world
}

fn a_person(world: &mut World, class: &str) -> bevy_ecs::prelude::Entity {
    let who = cdda_sim::creature::spawn_creature(world, "Vlad", (4, 5, 0));
    cdda_sim::npcs::make_npc(world, who, "vlad");
    world
        .entity_mut(who)
        .insert(cdda_sim::npcs::NpcClass(class.to_owned()));
    who
}

/// **A shopkeeper restocks their shelf, and then waits.** Reference: `npc::shop_restock` (`npc.cpp:2100-2189`): the first visit always restocks (`restock` starts at
/// `calendar::turn_zero`), a later one waits `restock_interval`, an **ally** never restocks, and a class with no shelf is not a shopkeeper at all.
#[test]
fn a_shopkeeper_restocks_their_shelf_and_then_waits() {
    let mut world = a_shop();
    let vlad = a_person(&mut world, "a_shopkeeper");
    cdda_sim::calendar::set_turn(&mut world, 1000);
    assert!(
        cdda_sim::shop::shop_restock(&mut world, vlad),
        "**the first visit restocks**"
    );
    let stock = |world: &mut World| {
        let mut query = world.query::<&cdda_sim::creature::Carried>();
        query
            .iter(world)
            .filter(|carried| carried.0 == vlad)
            .count()
    };
    assert_eq!(
        stock(&mut world),
        1,
        "**and the rigid group's item is stock**"
    );
    // **The same turn does not restock again**, and neither does the day after: the interval is six days.
    assert!(
        !cdda_sim::shop::shop_restock(&mut world, vlad),
        "**the same turn waits**"
    );
    cdda_sim::calendar::set_turn(&mut world, 1000 + 6 * 24 * 60 * 60 - 1);
    assert!(
        !cdda_sim::shop::shop_restock(&mut world, vlad),
        "**one turn short of the interval still waits**"
    );
    cdda_sim::calendar::set_turn(&mut world, 1000 + 6 * 24 * 60 * 60);
    assert!(
        cdda_sim::shop::shop_restock(&mut world, vlad),
        "**and the interval's last turn restocks**"
    );
    assert_eq!(stock(&mut world), 2, "**with a second item**");
    // **An ally does not shop, and a class with no shelf is not a shopkeeper.**
    let ally = a_person(&mut world, "a_shopkeeper");
    cdda_sim::npcs::make_ally(&mut world, ally);
    cdda_sim::calendar::set_turn(&mut world, 99_999);
    assert!(
        !cdda_sim::shop::shop_restock(&mut world, ally),
        "**an ally never restocks**"
    );
    let nobody = a_person(&mut world, "no_shelf");
    cdda_sim::calendar::set_turn(&mut world, 99_999);
    assert!(
        !cdda_sim::shop::shop_restock(&mut world, nobody),
        "**a class with no shelf is not a shopkeeper**"
    );
}
