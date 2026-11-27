//! The two numbers a trade's price is moved by: a barter skill's curve, and the balance of wits and words between buyer and seller.
//!
//! Reference: `npc_trading::net_price_adjustment` (`npctrade.cpp:143-160`) and `price_adjustment` (`skill.cpp:655-660`). The pair is the **whole** of the reference's opinion of a
//! trade that is not a faction's price rule: the seller's intelligence against the buyer's, the two barter skills against each other, and a reciprocal when the **buyer** is the
//! NPC rather than the seller.
//!
//! **The numbers are arguments, not fields.** The reference reads `int_cur` and `skill_speech` off two `Character`s; the port has no character stats yet, so the rule takes a
//! [`Trader`] and a [`Trader`] and is testable before the stats exist. When the stats do, they fill this in - the rule does not change.

/// What a trade needs to know about one side of it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Trader {
    /// Their intelligence (`int_cur`).
    pub intelligence: i32,
    /// Their speech skill (`skill_speech`), the reference's barter skill.
    pub barter: i32,
    /// Whether this side is a **person** rather than the player.
    pub is_npc: bool,
}

/// The curve a barter-skill difference makes.
///
/// Reference: `price_adjustment( int barter_skill )` (`skill.cpp:655-660`): `val = 0.045 * skill² - 0.025 * skill + 1` on the skill capped at **five**, and the reciprocal when the
/// difference favours the buyer - so a seller better at barter than the buyer asks more, and a better buyer pays less.
#[must_use]
pub fn price_adjustment(barter_skill: i32) -> f64 {
    let skill = f64::from(barter_skill.abs().min(5));
    let val = 0.045 * skill * skill - 0.025 * skill + 1.0;
    if barter_skill >= 0 { val } else { 1.0 / val }
}

/// How far the price swings from value, given both sides.
///
/// Reference: `npc_trading::net_price_adjustment( buyer, seller )` (`npctrade.cpp:143-160`):
/// `int_adj = 1 + 0.05 * min( 19, |seller.int - buyer.int| )`, `soc_adj = price_adjustment( round( seller.barter - buyer.barter ) )`, and
/// `adjust = int_adj * soc_adj` when the **seller** is the cleverer (or equally clever) side, `soc_adj / int_adj` when they are not. The answer is **negated and inverted** when
/// the buyer is the person, because the reference's caller multiplies by `1 + 0.25 * adjust` and a buyer's advantage must move the price the other way.
#[must_use]
pub fn net_price_adjustment(buyer: Trader, seller: Trader) -> f64 {
    let int_diff = seller.intelligence - buyer.intelligence;
    let int_adj = 1.0 + 0.05 * f64::from(int_diff.abs().min(19));
    let soc_adj = price_adjustment(seller.barter - buyer.barter);
    let adjust = if int_diff >= 0 {
        int_adj * soc_adj
    } else {
        soc_adj / int_adj
    };
    if seller.is_npc { adjust } else { -1.0 / adjust }
}

/// What a creature is worth to a trade: its four numbers' relevant two, its barter skill, and whether it is a person.
///
/// Reference: the reference asks a `Character` for `int_cur` and `get_skill_level( skill_speech )` and an `npc` for its own kind. The port asks the store, which is where both
/// already are - `stats` and `skills_of` - so no rule has to carry a copy of either.
#[must_use]
pub fn trader_for(world: &bevy_ecs::prelude::World, creature: bevy_ecs::prelude::Entity) -> Trader {
    Trader {
        intelligence: crate::stats::stats_of(world, creature).intelligence,
        barter: crate::skills_of::level_of(world, creature, "speech"),
        is_npc: world.get::<crate::npcs::Npc>(creature).is_some(),
    }
}

/// What a shop asks for one of a kind.
///
/// Reference: the non-faction path of `npc_trading::adjusted_price` (`npctrade.cpp:191-202`): the item's **sold** price (`price_post`, in cents) is multiplied by
/// `1 + 0.25 * net_price_adjustment( buyer, seller )` and rounded **up**. That is the whole rule the port has; the faction's price rules, the NPC's own `value()` override and
/// the container recursion are named gaps.
#[must_use]
pub fn price_for(
    world: &bevy_ecs::prelude::World,
    buyer: bevy_ecs::prelude::Entity,
    seller: bevy_ecs::prelude::Entity,
    kind: crate::ItemTypeId,
) -> i64 {
    let base = world
        .try_query_filtered::<&crate::item_registry::ItemTypes, bevy_ecs::prelude::With<crate::item_registry::TheItemTypes>>()
        .and_then(|mut query| query.iter(world).next().map(|types| types.price_post_of(kind)))
        .unwrap_or(0);
    let adjust = net_price_adjustment(trader_for(world, buyer), trader_for(world, seller));
    #[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)]
    let price = (base as f64 * (1.0 + 0.25 * adjust)).ceil() as i64;
    price
}

/// Whether a person will sell something, given what their class's shelf says.
///
/// Reference: `npc::wants_to_sell( const item_location &, int at_price )` (`npc.cpp:1981-2007`):
///
/// 1. an **ally** exchanges freely (`will_exchange_items_freely`);
/// 2. a group the shopkeeper **cannot** sell from acts as a **blacklist**: if the item is in one, it is refused with that group's own message - which is the reference's
///    inverted-looking test and the reason a shop with a "we don't stock guns" group refuses guns rather than merely failing to stock them;
/// 3. otherwise the price decides, and a negative price is a refusal.
///
/// **Named gaps**: the flags (`TRADER_KEEP`, `TRADER_KEEP_EQUIPPED`), whether the item is worn or held by the shopkeeper (`sells_belongings`), and the contents check
/// (`this_or_single_content`). The port's items carry no flags and its carried list is not split into worn and held, so those three refusals cannot be asked.
#[must_use]
pub fn wants_to_sell(
    world: &bevy_ecs::prelude::World,
    npc: bevy_ecs::prelude::Entity,
    item: &str,
) -> bool {
    if world.get::<crate::npcs::Ally>(npc).is_some() {
        return true;
    }
    let Some(class_id) = crate::npcs::class_of(world, npc) else {
        return true;
    };
    let Some(class) = crate::npc_classes::class_named(world, &class_id) else {
        return true;
    };
    let groups = world
        .try_query_filtered::<&crate::item_groups::ItemGroups, bevy_ecs::prelude::With<crate::item_groups::TheItemGroups>>()
        .and_then(|mut query| query.iter(world).next().cloned())
        .unwrap_or_default();
    for group in &class.shop_item_groups {
        // `can_sell` is `!strict && trust`-gated in the reference; the port has no factions, so a **strict** group is one whose trust floor it cannot check and which it therefore
        // treats as "not trusted" - the same answer the reference gives a stranger.
        if !group.strict {
            continue;
        }
        if cdda_content::item_group_draw::contains(&groups.0, &group.group, item) {
            return false;
        }
    }
    true
}

/// Whether a person will buy something.
///
/// Reference: `npc::wants_to_buy( const item &, int at_price )` (`npc.cpp:2014-2043`): an **ally** buys freely, a `DANGEROUS` item or an **armed bomb** is refused outright, and
/// everything else the port can ask falls to the price. The reference's `TRADER_AVOID` flag, the `SQUEAMISH` filthy rule and the class blacklist are named gaps: the port's items
/// carry no flags, and a blacklist is a content rule the loader does not read.
#[must_use]
pub fn wants_to_buy(
    world: &bevy_ecs::prelude::World,
    npc: bevy_ecs::prelude::Entity,
    price: i64,
) -> bool {
    if world.get::<crate::npcs::Ally>(npc).is_some() {
        return true;
    }
    price >= 0
}

/// One thing a shop offers, and what it asks for it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Offer {
    /// The item entity itself.
    pub item: bevy_ecs::prelude::Entity,
    /// What the shop asks, in cents.
    pub price: i64,
}

/// What a shopkeeper is willing to sell, and for how much.
///
/// Reference: `npc_trading::init_selling` (`npctrade.cpp:117-141`): every item the shopkeeper carries is priced (`np.value( it )`), and the ones they are willing to sell become
/// offers with their count. The **wielded** weapon joins the list only when the person exchanges freely (`will_exchange_items_freely`).
///
/// The port's prices come from [`price_for`], which is the reference's non-faction `adjusted_price`, and the willingness from [`wants_to_sell`]. Offers keep the order the
/// carried list has, because that is the order the reference's `items_with` produces and a window shows.
///
/// **Named gaps**: `np.value( it )` is the NPC's own valuation, which the port replaces with the plain adjusted price, and the wielded weapon's free-exchange offer is not in the
/// list.
#[must_use]
pub fn selling(
    world: &mut bevy_ecs::prelude::World,
    buyer: bevy_ecs::prelude::Entity,
    seller: bevy_ecs::prelude::Entity,
) -> Vec<Offer> {
    let carried = {
        let mut pairs: Vec<(bevy_ecs::prelude::Entity, crate::ItemTypeId)> = Vec::new();
        let mut held = world.try_query::<(
            bevy_ecs::prelude::Entity,
            &crate::creature::Carried,
            &crate::GroundItem,
        )>();
        let Some(query) = held.as_mut() else {
            return Vec::new();
        };
        for (entity, carry, item) in query.iter(world) {
            if carry.0 == seller {
                pairs.push((entity, item.kind));
            }
        }
        pairs
    };
    let mut offers = Vec::with_capacity(carried.len());
    for (entity, kind) in carried {
        // **A liquid is never offered**, which is the reference's `!it.made_of( phase_id::LIQUID )` filter on the carried list (`npctrade.cpp:120-122`).
        if crate::item_registry::is_liquid(world, kind) {
            continue;
        }
        let Some(name) = crate::item_registry::name_of(world, kind) else {
            continue;
        };
        if !wants_to_sell(world, seller, &name) {
            continue;
        }
        let price = price_for(world, buyer, seller, kind);
        if price <= 0 {
            continue;
        }
        offers.push(Offer {
            item: entity,
            price,
        });
    }
    offers
}
