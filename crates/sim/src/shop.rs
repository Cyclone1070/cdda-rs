//! What a shopkeeper restocks, and when.
//!
//! Reference: `npc::shop_restock` (`npc.cpp:2100-2189`). The rule is the **interval gate** and the **shelf**: a shopkeeper waits `restock_interval` between restocks, an ally or a
//! person with no shelf never restocks at all, and every group their class names is drawn and becomes stock.

use bevy_ecs::prelude::{Component, Entity, World};

/// When a shopkeeper last restocked, in turns.
///
/// Reference: the reference's `npc::restock` (`npc.h`), initialised to `calendar::turn_zero` - which is why a shopkeeper who has **never** restocked restocks on the first
/// visit, while one who has waits out their interval.
#[derive(Component, Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Shop {
    /// The turn of the last restock; zero means never.
    pub last_restock: i64,
}

/// Restocks a shopkeeper, and says whether they did.
///
/// Reference: `npc::shop_restock` (`npc.cpp:2100-2189`), in the reference's order:
///
/// 1. **the interval** - nothing happens until `restock_interval` has passed since the last restock, unless there has never been one (`restock != calendar::turn_zero`,
///    `:2103-2107`);
/// 2. **who may restock at all** - an **ally** never shops, and neither does a person whose class has no shelf (`is_player_ally() || !is_shopkeeper()`, `:2109-2111`);
/// 3. the last-restock time is written **before** the goods are made, so a restock that fails to place anything still counts (`:2112`);
/// 4. every group whose `can_restock` holds is drawn - **rigid** groups once, whatever the shop can afford (`:2137-2147`), and value groups until the shop's value is spent
///    (`:2149-2172`).
///
/// **Named gaps, and they are the value half of the rule**: the **valued groups' loop** needs an item's `price( true )`, which is `price_post` **in cents** with damage and
/// charge scaling (`item.cpp:7178-7200`) - the port's item type carries `price` and not `price_post`, so subtracting a real price here would be inventing a number; the
/// faction's **wealth and currency** (`:2128-2135`); and the **zones** the goods are distributed into (`:2187-2188`). The port puts rigid stock into the shopkeeper's hands,
/// which is the one place it can be seen without zones.
pub fn shop_restock(world: &mut World, npc: Entity) -> bool {
    let Some(class_id) = crate::npcs::class_of(world, npc) else {
        return false;
    };
    let Some(class) = crate::npc_classes::class_named(world, &class_id) else {
        return false;
    };
    let turn = crate::calendar::of(world).map_or(0, |calendar| calendar.turn);
    let last = world.get::<Shop>(npc).copied().unwrap_or_default();
    // 1. **The interval**: a shopkeeper who has restocked before waits it out.
    if last.last_restock != 0 && turn - last.last_restock < class.restock_interval {
        return false;
    }
    // 2. **Who may restock**: an ally does not shop, and a class with no shelf is not a shopkeeper.
    if world.get::<crate::npcs::Ally>(npc).is_some() || class.shop_item_groups.is_empty() {
        return false;
    }
    // 3. The time is written before the goods, as the reference does.
    world.entity_mut(npc).insert(Shop {
        last_restock: if turn == 0 { 1 } else { turn },
    });
    // 4. **The shelf**: every group that may restock is drawn.
    for group in &class.shop_item_groups {
        // `can_restock` (`npc_class.cpp:204-207`) is `!strict || can_sell`, and `can_sell` gates on the faction's trust, which the port has no factions for - so a **strict**
        // group is exactly the case the port cannot answer and draws nothing for.
        if group.strict {
            continue;
        }
        // **A group is drawn once when it is rigid**, as the reference's `items_from` does (`:2137-2147`); the value groups' loop is the named gap, so a non-rigid group is drawn
        // the same way here rather than looped over an invented price.
        let Some(drawn) = crate::item_groups::draw(world, &group.group) else {
            continue;
        };
        let here = world.get::<crate::Position>(npc).copied();
        for (id, count) in drawn {
            if count <= 0 {
                continue;
            }
            let Some(kind) = crate::item_registry::id_of(world, &id) else {
                continue;
            };
            // The stock lands where the shopkeeper stands, because the port has no zones to distribute it into.
            let square = here.map_or((0, 0), |at| (at.x, at.y));
            let item = crate::drop_item(world, square, kind, count);
            world.entity_mut(item).insert(crate::creature::Carried(npc));
        }
    }
    true
}
