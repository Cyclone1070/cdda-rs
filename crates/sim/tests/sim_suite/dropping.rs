//! **Putting down what is held**, and the two dialogue effects that do it.
//!
//! Reference: `Character::remove_weapon` (`character.cpp`) and `talk_function::drop_weapon` (`npctalk_funcs.cpp:1059-1066`) / `talk_function::player_weapon_drop`
//! (`:1082-1090`). The interesting half is that a dropped thing goes back **into the index**: the index is the only place that says what is on a square, so a thing that lost
//! its `WieldedBy` without gaining a position would be in nobody's hands and on no floor.

#![allow(clippy::expect_used)]

use bevy_ecs::prelude::*;
use cdda_sim::ItemTypeId;
use cdda_sim::creature::{Carried, WieldedBy, spawn_creature};

/// A creature at `square` holding one thing of `kind`.
fn holding(world: &mut World, square: (i32, i32), kind: ItemTypeId) -> (Entity, Entity) {
    let creature = spawn_creature(world, "the survivor", (square.0, square.1, 0));
    let item = cdda_sim::drop_item(world, square, kind, 1);
    world.entity_mut(item).remove::<cdda_sim::Position>();
    world.entity_mut(item).insert(Carried(creature));
    world.entity_mut(item).insert(WieldedBy(creature));
    (creature, item)
}

#[test]
fn dropping_puts_the_thing_on_the_square_and_in_the_index() {
    let mut world = World::new();
    let (creature, item) = holding(&mut world, (3, 4), ItemTypeId(7));
    assert!(
        world.get::<cdda_sim::Position>(item).is_none(),
        "**a held thing has no square**"
    );
    let dropped = cdda_sim::dropping::put_down_what_is_held(&mut world, creature);
    assert_eq!(
        dropped,
        Some(item),
        "**the thing that was held comes down**"
    );
    assert!(
        world.get::<WieldedBy>(item).is_none(),
        "**and is no longer in a hand**"
    );
    assert_eq!(
        world.get::<cdda_sim::Position>(item).map(|at| (at.x, at.y)),
        Some((3, 4)),
        "**it lands where the creature stands**"
    );
    let indexed = cdda_sim::singletons::index(&world)
        .is_some_and(|index| index.at_level(3, 4, 0).contains(&item));
    assert!(
        indexed,
        "**and the index says so, because the index is the only place that says what is on a square**"
    );
    // **An empty hand drops nothing**, which is what the reference's `remove_weapon` on an empty hand gives back.
    assert_eq!(
        cdda_sim::dropping::put_down_what_is_held(&mut world, creature),
        None,
        "**and a second call has nothing left to drop**"
    );
}

/// **`drop_weapon` drops the person's weapon; `player_weapon_drop` drops the player's.** Reference: `talk_function::drop_weapon` (`npctalk_funcs.cpp:1059-1066`) and
/// `talk_function::player_weapon_drop` (`:1082-1090`) - the first acts on the person being talked to, the second on the player, whoever the conversation is with.
#[test]
fn the_two_drop_weapon_effects_act_on_different_people() {
    let mut world = World::new();
    let avatar = spawn_creature(&mut world, "the survivor", (0, 0, 0));
    cdda_sim::missions::set_avatar(&mut world, avatar);
    let vlad = spawn_creature(&mut world, "Vlad", (5, 5, 0));
    cdda_sim::npcs::make_npc(&mut world, vlad, "vlad");
    for (who, square) in [(avatar, (0, 0)), (vlad, (5, 5))] {
        let item = cdda_sim::drop_item(&mut world, square, ItemTypeId(7), 1);
        world.entity_mut(item).remove::<cdda_sim::Position>();
        world.entity_mut(item).insert(Carried(who));
        world.entity_mut(item).insert(WieldedBy(who));
    }
    let effect = |name: &str| cdda_content::dialogue::Effect::LegacyStringEffect {
        name: name.to_owned(),
    };
    // **The person drops theirs first.**
    assert_eq!(
        cdda_sim::dialogue_effects::run_effect(
            &mut world,
            avatar,
            Some(vlad),
            &effect("drop_weapon")
        ),
        cdda_sim::dialogue_effects::Applied::Done,
        "**drop_weapon runs**"
    );
    assert_eq!(
        cdda_sim::creature::wielding(&mut world, vlad),
        None,
        "**and the person's hands are empty**"
    );
    assert_eq!(
        cdda_sim::creature::wielding(&mut world, avatar),
        Some(cdda_sim::creature::wielding(&mut world, avatar).expect("the player still holds one")),
        "**while the player still holds theirs**"
    );
    // **Then the player's own.**
    assert_eq!(
        cdda_sim::dialogue_effects::run_effect(
            &mut world,
            avatar,
            Some(vlad),
            &effect("player_weapon_drop")
        ),
        cdda_sim::dialogue_effects::Applied::Done,
        "**player_weapon_drop runs**"
    );
    assert_eq!(
        cdda_sim::creature::wielding(&mut world, avatar),
        None,
        "**and now the player's hands are empty too**"
    );
}
