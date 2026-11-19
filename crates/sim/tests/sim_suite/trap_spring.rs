//! **What a trap does when a creature steps on it.** Reference: `trapfunc::eocs` (`trapfunc.cpp:382-400`), reached from `trap::trigger`: the trap's programs run with the creature
//! as the speaker and the trap's own square in the invocation's context as `trap_location`, and the trap is **removed** afterwards.

#![allow(clippy::panic, clippy::expect_used)]

use bevy_ecs::prelude::*;

fn a_trap(action: &str, programs: &[&str]) -> cdda_content::traps::Trap {
    cdda_content::traps::Trap {
        id: "tr_test".to_owned(),
        name: "a test trap".to_owned(),
        symbol: i32::from(b'^'),
        color: cdda_core_types::colors::color_from_string("red").expect("a colour"),
        visibility: 0,
        avoidance: 0,
        difficulty: 0,
        radius: 0,
        benign: false,
        always_invisible: false,
        flags: std::collections::BTreeSet::new(),
        action: Some(action.to_owned()),
        eocs: programs.iter().map(|name| (*name).to_owned()).collect(),
        trigger_u: None,
        trigger_npc: None,
        map_regen: None,
        spell: None,
    }
}

/// A world with ground, a creature on it, and one program registered under `EOC_mark`.
fn a_world() -> (World, Entity) {
    let mut world = World::new();
    let mut level = cdda_core_types::level::Level::new();
    for x in -10..=20 {
        for y in -10..=20 {
            level.set_terrain(x, y, 0, 0);
        }
    }
    cdda_sim::load_level(&mut world, &level);
    let who = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
    let program = cdda_json::parse(
        br#"{"type": "effect_on_condition", "id": "EOC_mark", "effect": [{"u_message": "sprung"}]}"#
            .as_slice(),
    )
    .expect("json");
    let read = cdda_content::dialogue::read_program(&program).expect("the program reads");
    cdda_sim::eoc_registry::set_programs(&mut world, &[read]);
    cdda_sim::PositionIndex::rebuild(&mut world);
    (world, who)
}

/// **A trap whose action is `eocs` runs its programs and is gone afterwards.**
#[test]
fn a_sprung_trap_runs_its_programs_and_is_removed() {
    let (mut world, who) = a_world();
    cdda_sim::trap_registry::set_traps(&mut world, &[a_trap("eocs", &["EOC_mark"])]);
    assert!(cdda_sim::square::set_trap(&mut world, (0, 0, 0), "tr_test"));
    assert!(
        cdda_sim::trap_spring::spring(&mut world, who, (0, 0, 0)),
        "**the trap springs**"
    );
    assert_eq!(
        cdda_sim::messages::lines(&world),
        vec!["sprung".to_owned()],
        "its program ran"
    );
    assert_eq!(
        cdda_sim::square::trap_at(&world, (0, 0, 0)),
        None,
        "**and the trap is gone**, the way a sprung trap is"
    );
    assert!(
        !cdda_sim::trap_spring::spring(&mut world, who, (0, 0, 0)),
        "springing it again finds nothing"
    );
}

/// **The program is handed the trap's square in its context**, under the name `trap_location`, which is how the telepad's own program knows where it is.
#[test]
fn the_program_is_handed_the_traps_square() {
    let (mut world, who) = a_world();
    // A program that teleports to whatever square the trap hands it: if the context were not set, the effect would find nothing and answer `CannotAnswer`.
    let program = cdda_json::parse(
        br#"{"type": "effect_on_condition", "id": "EOC_hop", "effect": [
            {"u_teleport": {"context_val": "trap_location"}, "force": true}
        ]}"#
        .as_slice(),
    )
    .expect("json");
    let read = cdda_content::dialogue::read_program(&program).expect("the program reads");
    cdda_sim::eoc_registry::set_programs(&mut world, &[read]);
    cdda_sim::trap_registry::set_traps(&mut world, &[a_trap("eocs", &["EOC_hop"])]);
    assert!(cdda_sim::square::set_trap(&mut world, (3, 4, 0), "tr_test"));
    assert!(cdda_sim::trap_spring::spring(&mut world, who, (3, 4, 0)));
    assert_eq!(
        world
            .get::<cdda_sim::Position>(who)
            .map(|at| (at.x, at.y, at.z)),
        Some((3, 4, 0)),
        "**the creature is on the trap's square**, teleported there by the context value the trap set"
    );
}

/// **A trap whose action is not `eocs` is left alone.** The named actions are their own rules and are not ported; half-triggering one - removing the trap without doing what
/// it does - would be worse than leaving it set.
#[test]
fn a_trap_the_port_cannot_act_on_stays_where_it_is() {
    let (mut world, who) = a_world();
    cdda_sim::trap_registry::set_traps(&mut world, &[a_trap("bubble", &[])]);
    assert!(cdda_sim::square::set_trap(&mut world, (0, 0, 0), "tr_test"));
    assert!(
        !cdda_sim::trap_spring::spring(&mut world, who, (0, 0, 0)),
        "nothing happens"
    );
    assert_eq!(
        cdda_sim::square::trap_at(&world, (0, 0, 0)),
        Some("tr_test".to_owned()),
        "**and the trap is still set**"
    );
    assert!(
        cdda_sim::messages::lines(&world).is_empty(),
        "no program ran"
    );
}

/// **A benign trap does not fire.** Reference: `map::maybe_trigger_trap` returns before triggering a `benign` trap (`map.cpp:9923-9926`) - a cot is a trap in the data and
/// nothing should happen to whoever lies on it.
#[test]
fn a_benign_trap_does_not_fire() {
    let (mut world, who) = a_world();
    let mut cot = a_trap("eocs", &["EOC_mark"]);
    cot.benign = true;
    cdda_sim::trap_registry::set_traps(&mut world, &[cot]);
    assert!(cdda_sim::square::set_trap(&mut world, (0, 0, 0), "tr_test"));
    assert!(
        !cdda_sim::trap_spring::spring(&mut world, who, (0, 0, 0)),
        "**a benign trap is not sprung**"
    );
    assert!(
        cdda_sim::messages::lines(&world).is_empty(),
        "no program ran"
    );
    assert_eq!(
        cdda_sim::square::trap_at(&world, (0, 0, 0)),
        Some("tr_test".to_owned()),
        "and it stays where it is"
    );
}

/// **An `AVATAR_ONLY` trap fires for the avatar and for nobody else.** Reference: `map.cpp:9928-9930` skips the trigger when the flag is set and the creature is not the avatar.
#[test]
fn an_avatar_only_trap_ignores_everyone_but_the_avatar() {
    let (mut world, who) = a_world();
    let mut only = a_trap("eocs", &["EOC_mark"]);
    only.flags.insert("AVATAR_ONLY".to_owned());
    cdda_sim::trap_registry::set_traps(&mut world, &[only]);
    assert!(cdda_sim::square::set_trap(&mut world, (0, 0, 0), "tr_test"));
    assert!(
        !cdda_sim::trap_spring::spring(&mut world, who, (0, 0, 0)),
        "**nobody but the avatar sets it off**, and this creature is not the avatar"
    );
    assert!(cdda_sim::messages::lines(&world).is_empty());
    assert_eq!(
        cdda_sim::square::trap_at(&world, (0, 0, 0)),
        Some("tr_test".to_owned()),
        "the trap is still set"
    );

    // The same creature, now the avatar.
    cdda_sim::missions::set_avatar(&mut world, who);
    assert!(
        cdda_sim::trap_spring::spring(&mut world, who, (0, 0, 0)),
        "**the avatar sets it off**"
    );
    assert_eq!(cdda_sim::messages::lines(&world), vec!["sprung".to_owned()]);
}

/// **A creature riding in a vehicle is above the trap.** Reference: `map::creature_on_trap` returns at once for a character `in_vehicle` or gliding (`map.cpp:9886-9888`), so
/// driving over a trap does not set it off.
#[test]
fn a_rider_does_not_set_off_a_trap() {
    let (mut world, who) = a_world();
    let vehicle = world
        .spawn((
            cdda_sim::vehicles::Vehicle::default(),
            cdda_sim::Position { x: 0, y: 0, z: 0 },
        ))
        .id();
    world
        .entity_mut(who)
        .insert(cdda_sim::vehicles::Riding { vehicle, part: 0 });
    cdda_sim::PositionIndex::rebuild(&mut world);
    // **The trap has to be a real one**, or the test would pass because the store knows no such trap rather than because the rider is above it.
    cdda_sim::trap_registry::set_traps(&mut world, &[a_trap("eocs", &["EOC_mark"])]);
    assert!(cdda_sim::square::set_trap(&mut world, (0, 0, 0), "tr_test"));
    assert!(
        !cdda_sim::trap_spring::spring(&mut world, who, (0, 0, 0)),
        "**a rider is above the trap**"
    );
    assert!(cdda_sim::messages::lines(&world).is_empty());
    assert_eq!(
        cdda_sim::square::trap_at(&world, (0, 0, 0)),
        Some("tr_test".to_owned()),
        "and the trap is still there"
    );
}

/// **A trap says what happened, in the words the data gave it.** Reference: `map::maybe_trigger_trap` announces `tr.get_trigger_message_u()` to the player and
/// `tr.get_trigger_message_npc()` to anyone else - with `<npcname>` replaced by the creature's own name (`Creature::replace_with_npc_name`, `creature.cpp:3408-3412`) - but only when
/// the trap is not `always_invisible` and the data supplied **both** lines (`trap.h:258-266`).
fn a_trap_with_lava_words() -> cdda_content::traps::Trap {
    let mut trap = a_trap("eocs", &["EOC_mark"]);
    trap.trigger_u = Some("You stepped into lava!".to_owned());
    trap.trigger_npc = Some("<npcname> stepped into lava!".to_owned());
    trap
}

#[test]
fn the_avatar_hears_the_traps_own_words() {
    let (mut world, who) = a_world();
    cdda_sim::missions::set_avatar(&mut world, who);
    cdda_sim::trap_registry::set_traps(&mut world, &[a_trap_with_lava_words()]);
    assert!(cdda_sim::square::set_trap(&mut world, (0, 0, 0), "tr_test"));
    assert!(cdda_sim::trap_spring::spring(&mut world, who, (0, 0, 0)));
    assert_eq!(
        cdda_sim::messages::lines(&world),
        vec!["You stepped into lava!".to_owned(), "sprung".to_owned()],
        "**the player's line first, then whatever the trap's program said**"
    );
}

#[test]
fn anyone_else_hears_the_line_with_their_own_name_in_it() {
    let (mut world, who) = a_world();
    cdda_sim::trap_registry::set_traps(&mut world, &[a_trap_with_lava_words()]);
    assert!(cdda_sim::square::set_trap(&mut world, (0, 0, 0), "tr_test"));
    // The creature's name is what the reference substitutes for `<npcname>`; this one is not the avatar, so it hears the third person.
    assert!(cdda_sim::trap_spring::spring(&mut world, who, (0, 0, 0)));
    assert_eq!(
        cdda_sim::messages::lines(&world).first().cloned(),
        Some("the survivor stepped into lava!".to_owned()),
        "**the third-person line, with the creature's name where `<npcname>` was**"
    );
}

#[test]
fn a_trap_without_words_says_nothing() {
    let (mut world, who) = a_world();
    cdda_sim::trap_registry::set_traps(&mut world, &[a_trap("eocs", &["EOC_mark"])]);
    assert!(cdda_sim::square::set_trap(&mut world, (0, 0, 0), "tr_test"));
    assert!(cdda_sim::trap_spring::spring(&mut world, who, (0, 0, 0)));
    assert_eq!(
        cdda_sim::messages::lines(&world),
        vec!["sprung".to_owned()],
        "only the program's line: no trap words were given"
    );
}

/// **A `map_regen` trap rebuilds the tile it stands on.** Reference: `trapfunc::map_regen` (`trapfunc.cpp:1522-1547`): only a **character** sets it off, it says "Your
/// surroundings shift!", the trap is removed **before** the mapgen runs (so a mapgen that moves the character leaves no trap behind), and the update runs at the character's
/// own overmap tile.
///
/// The trap ranks third in the data by action - seventeen traps name it - and it is the one that reaches the mapgen layer the census kept blaming.
#[test]
fn a_map_regen_trap_rebuilds_the_tile_and_is_removed() {
    let (mut world, who) = a_world();
    // A level large enough to hold two tiles, and codes for the terrains the update draws.
    cdda_sim::mapgen_update::set_terrain_codes(
        &mut world,
        [("t_floor".to_owned(), 1u32), ("t_wall".to_owned(), 2u32)]
            .into_iter()
            .collect(),
    );
    let mut update = a_mapgen("godco_update");
    update.terrain.insert('#', "t_wall".to_owned());
    update.rows = vec!["##".to_owned(), "##".to_owned()];
    cdda_sim::mapgen_update::set_mapgens(&mut world, [("godco_update".to_owned(), update)]);

    let mut trap = a_trap("map_regen", &[]);
    trap.map_regen = Some("godco_update".to_owned());
    cdda_sim::trap_registry::set_traps(&mut world, &[trap]);
    // The trap stands on the tile's first square, which is where the update lands.
    assert!(cdda_sim::square::set_trap(&mut world, (0, 0, 0), "tr_test"));
    // A person, which is what the reference's `dynamic_cast<Character *>` accepts - the avatar or an NPC, not a monster.
    cdda_sim::npcs::make_npc(&mut world, who, "GODCO_WORKER");
    assert!(
        cdda_sim::trap_spring::spring(&mut world, who, (0, 0, 0)),
        "**the trap springs**"
    );
    assert_eq!(
        cdda_sim::square::terrain_at(&world, (0, 0, 0)),
        Some(2),
        "the update's first square is walled"
    );
    assert_eq!(
        cdda_sim::square::terrain_at(&world, (1, 1, 0)),
        Some(2),
        "and its last, so the whole mapgen landed at the tile's corner"
    );
    assert_eq!(
        cdda_sim::square::trap_at(&world, (0, 0, 0)),
        None,
        "and the trap is gone"
    );
    assert!(
        cdda_sim::messages::lines(&world)
            .iter()
            .any(|line| line == "Your surroundings shift!"),
        "**the character is told the map changed**: {:#?}",
        cdda_sim::messages::lines(&world)
    );
}

/// **A `map_regen` trap does nothing to a monster.** Reference: `trapfunc::map_regen` returns false for anything that is not a character (`trapfunc.cpp:1524-1546`).
#[test]
fn a_map_regen_trap_ignores_a_monster() {
    let (mut world, who) = a_world();
    cdda_sim::mapgen_update::set_terrain_codes(
        &mut world,
        [("t_floor".to_owned(), 1u32), ("t_wall".to_owned(), 2u32)]
            .into_iter()
            .collect(),
    );
    let mut update = a_mapgen("godco_update");
    update.terrain.insert('#', "t_wall".to_owned());
    update.rows = vec!["##".to_owned()];
    cdda_sim::mapgen_update::set_mapgens(&mut world, [("godco_update".to_owned(), update)]);
    let mut trap = a_trap("map_regen", &[]);
    trap.map_regen = Some("godco_update".to_owned());
    cdda_sim::trap_registry::set_traps(&mut world, &[trap]);
    assert!(cdda_sim::square::set_trap(&mut world, (0, 0, 0), "tr_test"));
    // What the floor was before anything happened: the claim is that it does **not** change, not that it is any particular terrain.
    let before = cdda_sim::square::terrain_at(&world, (0, 0, 0));
    // A monster is a creature the port marks as one, which is what the reference's `dynamic_cast<Character *>` refuses.
    cdda_sim::creature::set_kind(&mut world, who, cdda_sim::MonsterKindId(1));
    assert!(
        !cdda_sim::trap_spring::spring(&mut world, who, (0, 0, 0)),
        "**nothing happens to a monster**"
    );
    assert_eq!(
        cdda_sim::square::terrain_at(&world, (0, 0, 0)),
        before,
        "and the floor is untouched"
    );
}

/// One mapgen object with the two symbols the tests draw with.
fn a_mapgen(id: &str) -> cdda_content::mapgen::MapgenObject {
    cdda_content::mapgen::MapgenObject {
        om_terrain: id.to_owned(),
        rows: vec![".".to_owned()],
        terrain: [('.', "t_floor".to_owned())].into_iter().collect(),
        furniture: std::collections::BTreeMap::new(),
        weight: 100,
        fields: Vec::new(),
    }
}

/// **A `spell` trap casts its spell where it stands and is gone afterwards** - the action thirty-four traps in the data name, the most common of all.
///
/// Reference: `trapfunc::cast_spell` (`trapfunc.cpp:1568-1606`): the trap is removed **before** the cast, because a teleport would leave `p` pointing at a square the
/// creature has left; the spell is cast with the creature as its caster and the trap's own square as the centre; and `UNCONSUMED` keeps the trap set.
#[test]
fn a_spell_trap_casts_where_it_stands_and_is_removed() {
    let (mut world, who) = a_world();
    // A spell the port can perform: `target_attack`'s sibling for a square is the summon, which places a monster at the centre.
    install_a_spell(&mut world, "summon_trapspider");
    let mut trap = a_trap("spell", &[]);
    trap.spell = Some(cdda_content::traps::TrapSpell {
        id: "summon_trapspider".to_owned(),
        level: 1,
        max_level: None,
    });
    cdda_sim::trap_registry::set_traps(&mut world, &[trap]);
    assert!(cdda_sim::square::set_trap(&mut world, (0, 0, 0), "tr_test"));
    assert!(
        cdda_sim::trap_spring::spring(&mut world, who, (0, 0, 0)),
        "**the trap springs**"
    );
    assert_eq!(
        cdda_sim::square::trap_at(&world, (0, 0, 0)),
        None,
        "and the trap is gone, removed before the cast"
    );
}

/// **An `UNCONSUMED` spell trap stays set**, so a trap that keeps casting does.
#[test]
fn an_unconsumed_spell_trap_stays_set() {
    let (mut world, who) = a_world();
    install_a_spell(&mut world, "summon_trapspider");
    let mut trap = a_trap("spell", &[]);
    trap.flags.insert("UNCONSUMED".to_owned());
    trap.spell = Some(cdda_content::traps::TrapSpell {
        id: "summon_trapspider".to_owned(),
        level: 0,
        max_level: None,
    });
    cdda_sim::trap_registry::set_traps(&mut world, &[trap]);
    assert!(cdda_sim::square::set_trap(&mut world, (0, 0, 0), "tr_test"));
    assert!(cdda_sim::trap_spring::spring(&mut world, who, (0, 0, 0)));
    assert_eq!(
        cdda_sim::square::trap_at(&world, (0, 0, 0)),
        Some("tr_test".to_owned()),
        "**the trap is still there**, because the data said it is not consumed"
    );
}

use cdda_content::spells::Scaling;

/// A spell the port can perform, installed under a name: `summon` is one of the effects it has a rule for.
fn install_a_spell(world: &mut World, id: &str) {
    let spell = cdda_content::spells::Spell {
        id: id.to_owned(),
        name: id.to_owned(),
        effect_name: "summon".to_owned(),
        effect_str: "mon_zombie".to_owned(),
        damage_type: String::new(),
        shape: "point".to_owned(),
        energy_source: "MANA".to_owned(),
        damage: Scaling::default(),
        range: Scaling::default(),
        area: Scaling::default(),
        energy: Scaling::default(),
        duration: Scaling::default(),
        casting_time: Scaling::default(),
        difficulty: None,
        max_level: None,
        valid_targets: std::collections::BTreeSet::new(),
        targeted_monster_ids: std::collections::BTreeSet::new(),
        targeted_species_ids: std::collections::BTreeSet::new(),
        ignored_species_ids: std::collections::BTreeSet::new(),
        flags: std::collections::BTreeSet::new(),
    };
    cdda_sim::spell_types::set_types(world, vec![spell]);
}
