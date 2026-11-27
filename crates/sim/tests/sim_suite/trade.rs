//! The numbers a trade's price is moved by.
//!
//! Reference: `npc_trading::net_price_adjustment` (`npctrade.cpp:143-160`) and `price_adjustment` (`skill.cpp:655-660`).

#![allow(clippy::expect_used)]

use bevy_ecs::prelude::World;

/// The barter curve, at the values a player can reach in one character: `0.045 * skill² - 0.025 * skill + 1` on the skill capped at five, reciprocal when it favours the buyer.
///
/// The cap is the whole subtlety: a difference of **five** and a difference of **fifty** answer the same number, and the sign of the difference decides whether the value is
/// used or inverted.
#[test]
fn the_barter_curve_rises_then_caps_and_inverts() {
    let close = |a: f64, b: f64| (a - b).abs() < 1e-9;
    assert!(
        close(cdda_sim::trade::price_adjustment(0), 1.0),
        "**no difference, no change**"
    );
    assert!(
        close(cdda_sim::trade::price_adjustment(1), 0.045 - 0.025 + 1.0),
        "**one point of barter is worth 1.02**"
    );
    assert!(
        close(
            cdda_sim::trade::price_adjustment(5),
            0.045 * 25.0 - 0.125 + 1.0
        ),
        "**five points cap the curve at 1.0**"
    );
    assert!(
        close(
            cdda_sim::trade::price_adjustment(50),
            cdda_sim::trade::price_adjustment(5)
        ),
        "**fifty is the same as five, because the skill is capped**"
    );
    assert!(
        close(
            cdda_sim::trade::price_adjustment(-3),
            1.0 / cdda_sim::trade::price_adjustment(3)
        ),
        "**a buyer three better inverts the curve**"
    );
}

/// The whole adjustment: a clever seller asks more, a buyer who is the person gets the inverse sign.
#[test]
fn the_clever_side_moves_the_price_and_the_person_side_inverts_it() {
    let close = |a: f64, b: f64| (a - b).abs() < 1e-9;
    let seller = cdda_sim::trade::Trader {
        intelligence: 8,
        barter: 0,
        is_npc: true,
    };
    let buyer = cdda_sim::trade::Trader {
        intelligence: 8,
        barter: 0,
        is_npc: false,
    };
    assert!(
        close(cdda_sim::trade::net_price_adjustment(buyer, seller), 1.0),
        "**evenly matched, the price is value**"
    );
    // **Four points of intelligence is 1.20**, the reference's `1 + 0.05 * min( 19, |diff| )`.
    let dull_buyer = cdda_sim::trade::Trader {
        intelligence: 4,
        ..buyer
    };
    assert!(
        close(
            cdda_sim::trade::net_price_adjustment(dull_buyer, seller),
            1.2
        ),
        "**a seller four cleverer asks 1.2 of value**"
    );
    // **And when the buyer is the person, the sign inverts**: `-1 / adjust`.
    let npc_buyer = cdda_sim::trade::Trader {
        is_npc: true,
        ..buyer
    };
    let person_seller = cdda_sim::trade::Trader {
        is_npc: false,
        ..seller
    };
    assert!(
        close(
            cdda_sim::trade::net_price_adjustment(npc_buyer, person_seller),
            -1.0
        ),
        "**an evenly matched NPC buyer pays the same, negated**"
    );
    // **Barter rides on top of intelligence**: three points of seller barter is `1.33` before the intelligence term.
    let chatty_seller = cdda_sim::trade::Trader {
        barter: 3,
        ..seller
    };
    assert!(
        close(
            cdda_sim::trade::net_price_adjustment(buyer, chatty_seller),
            cdda_sim::trade::price_adjustment(3)
        ),
        "**a seller three better at barter asks the curve's worth more**"
    );
}

/// **What a shop asks, from the store.** Reference: the non-faction path of `npc_trading::adjusted_price` (`npctrade.cpp:191-202`): the item's sold price times
/// `1 + 0.25 * net_price_adjustment( buyer, seller )`, rounded up.
///
/// The numbers are the test's: an item worth **100 cents** sold by a shopkeeper of intelligence **8** to a survivor of intelligence **4** is `1.2` of value from the adjustment,
/// so `100 * 1.3 = 130`. Then the same shopkeeper with three points of barter asks the curve on top (`1.33`), and the shop's price goes to `100 * (1 + 0.25 * 1.596)` = **140**.
/// A port that read `price` instead of `price_post`, or forgot the quarter, or truncated the ceil, lands on a different number in every one of those.
#[test]
fn a_shops_price_comes_from_the_store() {
    let mut world = World::new();
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
    cdda_sim::item_registry::set_prices_post(&mut world, &[("test_rock".to_owned(), 100)]);
    let kind = cdda_sim::item_registry::id_of(&mut world, "test_rock").expect("the kind");
    let shopper = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
    cdda_sim::stats::set_stats(
        &mut world,
        shopper,
        cdda_sim::stats::Stats {
            intelligence: 4,
            ..cdda_sim::stats::Stats::default()
        },
    );
    let keeper = cdda_sim::creature::spawn_creature(&mut world, "Vlad", (1, 0, 0));
    cdda_sim::npcs::make_npc(&mut world, keeper, "vlad");
    assert_eq!(
        cdda_sim::trade::price_for(&world, shopper, keeper, kind),
        130,
        "**100 cents of value, a 1.2 adjustment, and the quarter: 130**"
    );
    cdda_sim::skills_of::set_level(&mut world, keeper, "speech", 3);
    assert_eq!(
        cdda_sim::trade::price_for(&world, shopper, keeper, kind),
        140,
        "**three points of barter on top: 1.596 times value, so 140 after the quarter and the ceil**"
    );
    // **The buyer's side inverts when the person is the seller**: the same adjustment, negated, so a person selling to an NPC asks less than value would suggest.
    let seller = cdda_sim::creature::spawn_creature(&mut world, "the survivor selling", (2, 0, 0));
    cdda_sim::stats::set_stats(
        &mut world,
        seller,
        cdda_sim::stats::Stats {
            intelligence: 4,
            ..cdda_sim::stats::Stats::default()
        },
    );
    // **Both of the person's disadvantages compound**: four points duller is `1.2` to the buyer's side, three points less barter inverts the curve to `0.7518`, and the person as
    // seller negates and inverts the product - `-1 / (0.7518 / 1.2) = -1.596` - so `100 * (1 - 0.399)` rounds up to **61**. A port that only looked at intelligence would say
    // 61 is impossible and one that only looked at barter would say 75; only the whole rule answers 61.
    assert_eq!(
        cdda_sim::trade::price_for(&world, keeper, seller, kind),
        61,
        "**an NPC buying from a duller, worse-spoken person pays 61**"
    );
}

/// **What a shop will sell, and to whom.** Reference: `npc::wants_to_sell` (`npc.cpp:1981-2007`): an **ally** exchanges freely; a group the shopkeeper cannot sell from acts as a
/// **blacklist** for the items it names - which is the reference's inverted-looking test, and the reason a shop with a "we do not stock guns" group refuses guns instead of merely
/// failing to stock them; otherwise the price decides.
///
/// The blacklist is the case worth pinning: a shop whose `guns` group is **strict** (its trust floor unmet) refuses a pistol that is inside it, and sells a rock that is not.
#[test]
fn a_shop_refuses_what_its_untrusted_shelf_names() {
    let mut world = World::new();
    let mut definitions = std::collections::BTreeMap::new();
    let item = |id: &str| cdda_content::item_group_draw::Entry::Item {
        id: id.to_owned(),
        probability: 100,
        count: (1, 1),
        event: None,
    };
    definitions.insert(
        "guns".to_owned(),
        cdda_content::item_group_draw::Definition {
            id: "guns".to_owned(),
            distribution: false,
            entries: vec![item("pistol")],
        },
    );
    definitions.insert(
        "goods".to_owned(),
        cdda_content::item_group_draw::Definition {
            id: "goods".to_owned(),
            distribution: false,
            entries: vec![item("rock")],
        },
    );
    cdda_sim::item_groups::set_groups(&mut world, definitions);
    cdda_sim::npc_classes::set_classes(
        &mut world,
        vec![cdda_content::npc_classes::NpcClass {
            id: "a_shopkeeper".to_owned(),
            name: "shopkeeper".to_owned(),
            job_description: String::new(),
            common: true,
            shop_item_groups: vec![
                cdda_content::npc_classes::ShopkeeperItemGroup {
                    group: "guns".to_owned(),
                    trust: 0,
                    strict: true,
                    rigid: true,
                },
                cdda_content::npc_classes::ShopkeeperItemGroup {
                    group: "goods".to_owned(),
                    trust: 0,
                    strict: false,
                    rigid: true,
                },
            ],
            restock_interval: 6 * 24 * 60 * 60,
        }],
    );
    let keeper = cdda_sim::creature::spawn_creature(&mut world, "Vlad", (1, 0, 0));
    cdda_sim::npcs::make_npc(&mut world, keeper, "vlad");
    world
        .entity_mut(keeper)
        .insert(cdda_sim::npcs::NpcClass("a_shopkeeper".to_owned()));
    assert!(
        !cdda_sim::trade::wants_to_sell(&world, keeper, "pistol"),
        "**an untrusted shelf's pistol is refused**"
    );
    assert!(
        cdda_sim::trade::wants_to_sell(&world, keeper, "rock"),
        "**a trusted shelf's rock is sold**"
    );
    // **An ally gives freely**, whatever its class says.
    cdda_sim::npcs::make_ally(&mut world, keeper);
    assert!(
        cdda_sim::trade::wants_to_sell(&world, keeper, "pistol"),
        "**an ally hands over the pistol**"
    );
    // **And buying falls to the price** for anyone who is **not** an ally - which the same person is by now, so the price rules need somebody else to be asked about.
    let other = cdda_sim::creature::spawn_creature(&mut world, "Vlad's brother", (2, 0, 0));
    cdda_sim::npcs::make_npc(&mut world, other, "vlads_brother");
    assert!(
        cdda_sim::trade::wants_to_buy(&world, other, 100),
        "**a price above nothing is a yes**"
    );
    assert!(
        !cdda_sim::trade::wants_to_buy(&world, other, -1),
        "**and a negative price is a no**"
    );
    assert!(
        cdda_sim::trade::wants_to_buy(&world, keeper, -1),
        "**but an ally buys even at a negative price, because an ally exchanges freely**"
    );
}

/// A world with three kinds, a shelf that trusts one group and not another, and a shopkeeper.
fn a_shop_world() -> World {
    let mut world = World::new();
    let kind = |id: &'static str, volume: i64, weight: i64| cdda_sim::item_registry::Kind {
        id,
        volume_ml: volume,
        weight_g: weight,
        pockets: &[],
        thrown_damage: 0.0,
        melee_bash: 0.0,
        melee_cut: 0.0,
        melee_stab: 0.0,
        to_hit: 0,
    };
    cdda_sim::item_registry::set_item_types(
        &mut world,
        &[
            kind("test_rock", 250, 200),
            kind("test_pistol", 500, 900),
            kind("test_gem", 50, 10),
            kind("test_water", 250, 250),
        ],
    );
    cdda_sim::item_registry::set_prices_post(
        &mut world,
        &[
            ("test_rock".to_owned(), 100),
            ("test_pistol".to_owned(), 1000),
            ("test_gem".to_owned(), 500),
            ("test_water".to_owned(), 1),
        ],
    );
    cdda_sim::item_registry::set_liquids(&mut world, &[("test_water".to_owned(), true)]);
    let item = |id: &str| cdda_content::item_group_draw::Entry::Item {
        id: id.to_owned(),
        probability: 100,
        count: (1, 1),
        event: None,
    };
    let mut definitions = std::collections::BTreeMap::new();
    for (id, contents) in [("guns", item("test_pistol")), ("goods", item("test_rock"))] {
        definitions.insert(
            id.to_owned(),
            cdda_content::item_group_draw::Definition {
                id: id.to_owned(),
                distribution: false,
                entries: vec![contents],
            },
        );
    }
    cdda_sim::item_groups::set_groups(&mut world, definitions);
    let group = |id: &str, strict: bool| cdda_content::npc_classes::ShopkeeperItemGroup {
        group: id.to_owned(),
        trust: 0,
        strict,
        rigid: true,
    };
    cdda_sim::npc_classes::set_classes(
        &mut world,
        vec![cdda_content::npc_classes::NpcClass {
            id: "a_shopkeeper".to_owned(),
            name: "shopkeeper".to_owned(),
            job_description: String::new(),
            common: true,
            shop_item_groups: vec![group("guns", true), group("goods", false)],
            restock_interval: 6 * 24 * 60 * 60,
        }],
    );
    world
}

/// **What the shop is offering.** Reference: `npc_trading::init_selling` (`npctrade.cpp:117-141`): every item the shopkeeper carries is priced and offered when they are willing
/// to sell it.
///
/// Three items are carried: a **rock** on the trusted shelf, a **pistol** on the untrusted one, and a **gem** nobody's shelf names. The offer list must be the rock and the gem -
/// the pistol refused by the blacklist, the gem sold because a shelf that does not name an item does not refuse it; only an **untrusted** one does - and the rock's price must be
/// the store's.
#[test]
fn the_shop_offers_what_it_will_sell_and_prices_it() {
    let mut world = a_shop_world();
    let keeper = cdda_sim::creature::spawn_creature(&mut world, "Vlad", (1, 0, 0));
    cdda_sim::npcs::make_npc(&mut world, keeper, "vlad");
    world
        .entity_mut(keeper)
        .insert(cdda_sim::npcs::NpcClass("a_shopkeeper".to_owned()));
    let shopper = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
    let mut stock = |name: &str| {
        let kind = cdda_sim::item_registry::id_of(&mut world, name).expect("the kind");
        let entity = cdda_sim::drop_item(&mut world, (1, 0), kind, 1);
        world
            .entity_mut(entity)
            .insert(cdda_sim::creature::Carried(keeper));
        entity
    };
    let rock = stock("test_rock");
    let pistol = stock("test_pistol");
    let gem = stock("test_gem");
    let water = stock("test_water");
    let offers = cdda_sim::trade::selling(&mut world, shopper, keeper);
    let offered = offers.iter().map(|offer| offer.item).collect::<Vec<_>>();
    assert!(
        !offered.contains(&pistol),
        "**the blacklisted pistol is not offered**"
    );
    assert!(offered.contains(&gem), "**the unheard-of gem is**");
    assert!(
        !offered.contains(&water),
        "**and a liquid is never offered, whatever the shelf says**"
    );
    let rock_offer = offers
        .iter()
        .find(|offer| offer.item == rock)
        .expect("**the rock is offered**");
    // **Evenly matched is 125, not 100**: `price_post` is what a shop **pays**, and the reference's `1 + 0.25 * adjust` with an adjustment of one is a shop's quarter on top of
    // it. A port that gave away the margin at even odds would sell every item at cost.
    assert_eq!(
        rock_offer.price, 125,
        "**a quarter over the sold price, because that is the shop's margin**"
    );
}

/// **What the shop will take off the player's hands.** Reference: `npc_trading::init_buying` (`npctrade.cpp:88-115`): every item the player carries is priced at what the person
/// will **pay**, which is `adjusted_price` with the person as the buyer - and that is the same quarter as the selling list, on the other side of the counter.
///
/// With both sides evenly matched a shop asks **125** for a 100-cent item and pays **75** for one: the margin is the shop's, and it is symmetric. A port that used one price for
/// both directions would sell at cost and buy at cost, and never notice until the money ran out.
#[test]
fn the_shop_pays_the_other_side_of_the_quarter() {
    let mut world = a_shop_world();
    let keeper = cdda_sim::creature::spawn_creature(&mut world, "Vlad", (1, 0, 0));
    cdda_sim::npcs::make_npc(&mut world, keeper, "vlad");
    world
        .entity_mut(keeper)
        .insert(cdda_sim::npcs::NpcClass("a_shopkeeper".to_owned()));
    let shopper = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
    let kind = cdda_sim::item_registry::id_of(&mut world, "test_rock").expect("the kind");
    let rock = cdda_sim::drop_item(&mut world, (0, 0), kind, 1);
    world
        .entity_mut(rock)
        .insert(cdda_sim::creature::Carried(shopper));
    let offers = cdda_sim::trade::buying(&mut world, shopper, keeper);
    assert_eq!(offers.len(), 1, "**the rock is offered to the shop**");
    let offered = offers.first().map(|offer| offer.price);
    assert_eq!(
        offered,
        Some(75),
        "**at 75 for a 100-cent item: the quarter, paid instead of charged**"
    );
}
