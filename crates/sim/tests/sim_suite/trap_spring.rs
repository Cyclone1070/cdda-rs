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

/// **A trap whose action has no rule is left alone.** The named actions are their own rules, and one the port has not ported yet - `lava` here - is not half-triggered: removing
/// the trap without doing what it does would be worse than leaving it set. This test's action moves as the frontier moves; when nothing is left it has nothing to say.
#[test]
fn a_trap_the_port_cannot_act_on_stays_where_it_is() {
    let (mut world, who) = a_world();
    cdda_sim::trap_registry::set_traps(&mut world, &[a_trap("shadow", &[])]);
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
        traps: std::collections::BTreeMap::new(),
        items: std::collections::BTreeMap::new(),
        monsters: Vec::new(),
        weight: 100,
        fields: Vec::new(),
        toilets: std::collections::BTreeMap::new(),
        liquids: std::collections::BTreeMap::new(),
        place_items: Vec::new(),
        place_loot: Vec::new(),
        sealed_items: std::collections::BTreeMap::new(),
        vending_machines: std::collections::BTreeMap::new(),
        signs: std::collections::BTreeMap::new(),
        gas_pumps: std::collections::BTreeMap::new(),
        nested: std::collections::BTreeMap::new(),
        fill_ter: None,
        predecessor_mapgen: None,
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

/// **A `glass` trap cuts a foot and slows the walker** - the reference's third family of trap behaviour, and the one that reads the body: which part took the glass, how
/// much of the cut the part's own armour stops, and how much of a turn the flinch costs.
///
/// Reference: `trapfunc::glass` (`trapfunc.cpp:151-175`): only a creature sets it off; whether it steps on the glass at all is `x_in_y(occupied_tile_fraction(size), 1)` -
/// a tiny creature mostly walks between the shards; the cut is `max(0, rng(-10, 5))`, so four times in sixteen there is none; a cut that lands costs `speed * 0.8` moves
/// and is dealt to a random **ground-contact** part; and the trap says "glass cracking!" and is gone.
///
/// **The counts are the test.** A rule that always fires and a rule that never does are both wrong, so a hundred seeds are run and the tally is what is pinned: half the
/// seeds meet the glass for a medium creature, a tenth for a tiny one, and some of the meetings cut - which is the only way to prove a distribution-ruled trap without
/// pretending one lucky seed is the rule.
#[test]
fn a_glass_trap_is_met_half_the_time_by_a_medium_creature() {
    let (mut world, who) = a_world();
    cdda_sim::trap_registry::set_traps(&mut world, &[a_trap("glass", &[])]);
    let (meetings, cuts) = glass_runs(&mut world, who, 100);
    assert!(
        (35..=65).contains(&meetings),
        "**half of a hundred medium creatures meet the glass**: {meetings}"
    );
    assert!(
        cuts > 0 && cuts <= meetings,
        "and some of those meetings cut: {cuts} of {meetings}"
    );
}

/// **A tiny creature walks between the shards most of the time.** Reference: `occupied_tile_fraction( tiny )` is a tenth, so the same hundred seeds meet the glass about a
/// tenth as often - and the test pins both ends, because "never" would pass a one-sided assertion just as happily while proving nothing.
#[test]
fn a_tiny_creature_mostly_walks_between_the_shards() {
    let (mut world, who) = a_world();
    cdda_sim::trap_registry::set_traps(&mut world, &[a_trap("glass", &[])]);
    if let Some(mut creature) = world.get_mut::<cdda_sim::creature::Creature>(who) {
        creature.size = cdda_sim::CreatureSize::Tiny;
    }
    let (meetings, _cuts) = glass_runs(&mut world, who, 100);
    assert!(
        (2..=25).contains(&meetings),
        "**about a tenth of a hundred tiny creatures meet the glass**: {meetings}"
    );
}

/// **The meeting is announced, costs moves and sweeps the glass away.** The seed is found rather than guessed: the test runs seeds until one meets the glass, which is
/// what the tally above already says happens half the time.
#[test]
fn a_glass_trap_tells_the_walker_and_costs_moves() {
    let (mut world, who) = a_world();
    cdda_sim::missions::set_avatar(&mut world, who);
    cdda_sim::trap_registry::set_traps(&mut world, &[a_trap("glass", &[])]);
    give_the_walker_a_turn(&mut world, who);
    let mut seed = 1u32;
    let mut met = false;
    while seed <= 200 {
        cdda_sim::singletons::put_rolls(&mut world, cdda_core_types::rng::Rolls::new(seed));
        cdda_sim::messages::clear(&mut world);
        let before = cdda_sim::creature::moves_left(&world, who);
        cdda_sim::square::remove_trap(&mut world, (0, 0, 0));
        assert!(cdda_sim::square::set_trap(&mut world, (0, 0, 0), "tr_test"));
        if cdda_sim::trap_spring::spring(&mut world, who, (0, 0, 0)) {
            met = true;
            assert!(
                cdda_sim::messages::lines(&world)
                    .iter()
                    .any(|line| line == "You step on some glass!"),
                "**the walker is told**: {:#?}",
                cdda_sim::messages::lines(&world)
            );
            assert_eq!(
                cdda_sim::square::trap_at(&world, (0, 0, 0)),
                None,
                "and the glass is swept away"
            );
            let now = cdda_sim::creature::moves_left(&world, who);
            assert!(
                now < before,
                "**the flinch costs moves**: {before} then {now}"
            );
            break;
        }
        assert_eq!(
            cdda_sim::square::trap_at(&world, (0, 0, 0)),
            Some("tr_test".to_owned()),
            "a meeting that does not happen leaves the glass set"
        );
        seed += 1;
    }
    assert!(
        met,
        "**some seed meets the glass**, or the rule never fires"
    );
}

/// Runs `seeds` springs and answers how many met the glass and how many of those cut, putting the trap back between runs.
fn glass_runs(world: &mut World, who: Entity, seeds: u64) -> (usize, usize) {
    let mut meetings = 0;
    let mut cuts = 0;
    // **Moves to spend**, or the flinch costs nothing and the cut cannot be seen: a creature with an empty bank spends nothing whatever it is charged.
    give_the_walker_a_turn(world, who);
    for seed in 1..=u32::try_from(seeds).unwrap_or(1) {
        cdda_sim::singletons::put_rolls(world, cdda_core_types::rng::Rolls::new(seed));
        // A run that does not meet the glass leaves it set, which is right; the next seed wants a fresh one.
        cdda_sim::square::remove_trap(world, (0, 0, 0));
        assert!(cdda_sim::square::set_trap(world, (0, 0, 0), "tr_test"));
        let before = cdda_sim::creature::moves_left(world, who);
        let hp_before = cdda_sim::creature::health_of(world, who).unwrap_or(0);
        if cdda_sim::trap_spring::spring(world, who, (0, 0, 0)) {
            meetings += 1;
            // **A cut that got through**, which is what armour can stop: moves are spent whether or not anything passed the part's own covering.
            if cdda_sim::creature::health_of(world, who).unwrap_or(0) < hp_before {
                cuts += 1;
            }
            let _ = before;
        }
        // Heal and restore what a run spends, so the next seed starts from the same place: the tally is about the roll, not about a creature being worn down.
        cdda_sim::creature::set_health(world, who, 100);
        give_the_walker_a_turn(world, who);
    }
    (meetings, cuts)
}

/// A plain spawned creature carries neither a speed nor a banked turn, and both are what a trap's flinch is measured against.
fn give_the_walker_a_turn(world: &mut World, who: Entity) {
    cdda_sim::creature::set_speed(world, who, 100, 0);
    cdda_sim::creature::set_health(world, who, 100);
    // **A body that can dodge**: `rng(5, 50) > get_dodge()` is true for every roll when the dodge is zero, so a fixture without one is hit by nothing - and a shotgun test
    // would walk every seed without seeing a shell land.
    world
        .entity_mut(who)
        .insert(cdda_sim::creature::Dodge { points: 6 });
    world
        .entity_mut(who)
        .insert(cdda_sim::creature::Moves { left: 100 });
}

/// **The glass cuts a part that touches the ground, and armour on the parts that do not is worth nothing.** Reference: `trapfunc::glass` deals its cut to
/// `random_entry( c->get_ground_contact_bodyparts() )` (`trapfunc.cpp:168`), so a character in a steel vest still cuts a bare foot.
///
/// **The test is the cover.** Every part that is *not* a leg or a foot is wrapped in armour a cut cannot pass; the ground-contact parts are left bare. Under that cover the
/// only way a cut can still be dealt is for the rule to pick a part that touches the ground - and the tally says it does, over and over, while a rule that picks any part
/// would spend most of its meetings on a covered torso and come back with nothing.
#[test]
fn the_glass_cuts_a_part_that_touches_the_ground() {
    let (mut world, who) = a_world();
    cdda_sim::trap_registry::set_traps(&mut world, &[a_trap("glass", &[])]);
    // A body the fixture gives it: a torso and a head that cannot meet the ground, a leg and a foot that can.
    world.entity_mut(who).insert(cdda_sim::creature::Body {
        parts: vec![
            a_part("torso", "torso"),
            a_part("head", "head"),
            a_part("arm_l", "arm"),
            a_part("arm_r", "arm"),
            a_part("leg_l", "leg"),
            a_part("foot_l", "foot"),
        ],
    });
    let parts = world
        .get::<cdda_sim::creature::Body>(who)
        .map(|body| body.parts.clone())
        .unwrap_or_default();
    let mut wrapped = Vec::new();
    for part in &parts {
        let ground = part
            .primary_limb_type
            .as_deref()
            .is_some_and(|kind| kind == "leg" || kind == "foot");
        if ground {
            continue;
        }
        wrapped.push(part.name.clone());
    }
    assert!(
        !wrapped.is_empty(),
        "the body has parts that do not touch the ground"
    );
    let cover = cdda_sim::creature::Garment {
        layers: wrapped
            .iter()
            .map(|name| {
                (
                    name.clone(),
                    cdda_content::armour::Layer {
                        damage_type: "cut".to_owned(),
                        resist: 100.0,
                        thickness: 1.0,
                        cover: 100,
                    },
                )
            })
            .collect(),
        vitals: Vec::new(),
    };
    // **The turn comes before the garment**: wearing costs moves, and a wearer with an empty bank is too slow to be dressed - which is a trap for the test, not a rule.
    give_the_walker_a_turn(&mut world, who);
    let garment = world.spawn(cover).id();
    assert_eq!(
        cdda_sim::creature::wear(&mut world, who, garment),
        cdda_sim::creature::Wore::Worn,
        "the garment goes on"
    );
    cdda_sim::creature::dress(&mut world);
    let (meetings, cuts) = glass_runs(&mut world, who, 100);
    assert!(
        meetings >= 35,
        "**half the seeds still meet the glass**: {meetings}"
    );
    assert!(
        cuts >= 8,
        "**and the ones that cut land on a bare foot**: {cuts} of {meetings}"
    );
    assert!(
        cuts < meetings,
        "some meetings draw no cut at all: {cuts} of {meetings}"
    );
}

/// One part of the fixture's body, with the kind a rule asks about.
fn a_part(name: &str, kind: &str) -> cdda_sim::hit_location::BodyPart {
    cdda_sim::hit_location::BodyPart {
        name: name.to_owned(),
        hit_size: 1.0,
        hit_difficulty: 1.0,
        primary_limb_type: Some(kind.to_owned()),
        ..cdda_sim::hit_location::BodyPart::default()
    }
}

/// **Armour on the part the glass reaches stops it**, which is what makes the test above about the *choice* of part mean anything: if the rule never read the chosen part's
/// armour, covering the bare feet would change nothing and every cover assertion would pass for the wrong reason.
#[test]
fn armour_on_the_foot_the_glass_reaches_stops_the_cut() {
    let (mut world, who) = a_world();
    world.entity_mut(who).insert(cdda_sim::creature::Body {
        parts: vec![
            a_part("torso", "torso"),
            a_part("leg_l", "leg"),
            a_part("foot_l", "foot"),
        ],
    });
    cdda_sim::trap_registry::set_traps(&mut world, &[a_trap("glass", &[])]);
    // Every part that touches the ground is wrapped; the torso is bare.
    let cover = cdda_sim::creature::Garment {
        layers: ["leg_l", "foot_l"]
            .iter()
            .map(|name| {
                (
                    (*name).to_owned(),
                    cdda_content::armour::Layer {
                        damage_type: "cut".to_owned(),
                        resist: 100.0,
                        thickness: 1.0,
                        cover: 100,
                    },
                )
            })
            .collect(),
        vitals: Vec::new(),
    };
    // **The turn comes before the garment**: wearing costs moves, and a wearer with an empty bank is too slow to be dressed - which is a trap for the test, not a rule.
    give_the_walker_a_turn(&mut world, who);
    let garment = world.spawn(cover).id();
    assert_eq!(
        cdda_sim::creature::wear(&mut world, who, garment),
        cdda_sim::creature::Wore::Worn,
        "the garment goes on"
    );
    cdda_sim::creature::dress(&mut world);
    assert!(
        cdda_sim::creature::armour_on(&world, who, "foot_l", Some("cut")) >= 100,
        "**the foot is covered**, or the test is asking the wrong question: {}",
        cdda_sim::creature::armour_on(&world, who, "foot_l", Some("cut"))
    );
    let (meetings, cuts) = glass_runs(&mut world, who, 100);
    assert!(meetings >= 35, "**the glass is still met**: {meetings}");
    assert_eq!(
        cuts, 0,
        "**and never cuts through it**: {cuts} of {meetings}"
    );
}

/// **A shotgun trap fires, and the trap is a load of buckshot on a string afterwards.** Reference: `trapfunc::shotgun` (`trapfunc.cpp:566-667`), the largest of the actions
/// the port has no rule for - three traps in the data, and the only one that shoots.
///
/// The character's half: it says "Kerblam!" loud enough to be heard across the map (`MAX_VIEW_DISTANCE`), tells the walker what happened, fires **two** shells when the roll
/// says so and the trap is the double-barrelled one (`tr_shotgun_2`), and a shell that is not dodged hits a part chosen by a ten-sided roll - a foot once in twenty, a leg
/// three times, the torso five, the head once - for `rng(40, 60)` bullet damage per shell. Then the trap is gone and its gun and its string of shells are on the ground.
#[test]
fn a_shotgun_trap_fires_and_leaves_its_gun_behind() {
    let (mut world, who) = a_world();
    install_the_shotgun_if_the_data_has_it(&mut world);
    cdda_sim::trap_registry::set_traps(&mut world, &[a_trap("shotgun", &[])]);
    cdda_sim::missions::set_avatar(&mut world, who);
    give_the_walker_a_turn(&mut world, who);
    let (lost, lines) = fire_a_shotgun(&mut world, who, "tr_test");
    assert!(
        lines
            .iter()
            .any(|line| line == "You trigger a shotgun trap!"),
        "**the walker is told**: {lines:#?}"
    );
    assert!(
        lines.iter().any(|line| line.ends_with("is hit!")),
        "**and which part took it**: {lines:#?}"
    );
    assert!(
        (1..=60).contains(&lost),
        "**one shell, forty to sixty points**: {lost}"
    );
    assert_eq!(
        cdda_sim::square::trap_at(&world, (0, 0, 0)),
        None,
        "the trap is gone"
    );
    assert!(
        !cdda_sim::items_at(&mut world, (0, 0)).is_empty(),
        "**the gun and its shells are on the ground**"
    );
}

/// **Only the double-barrelled trap fires twice**, and only when the roll says so. Reference: `trapfunc::shotgun` (`trapfunc.cpp:582-585`) - a character's strength raises the
/// chance of two shells, and `if( here.tr_at( p ) != tr_shotgun_2 ) shots = 1` caps it: a single-barrelled trap fires one whatever the roll. The damage is per shell, so two
/// shells are eighty to a hundred and twenty points and one is forty to sixty - which is how the count is visible at all.
#[test]
fn a_double_barrelled_trap_can_fire_two_shells() {
    let (mut world, who) = a_world();
    install_the_shotgun_if_the_data_has_it(&mut world);
    // The trap's own id is what the cap reads, so the trap is registered - and set - as the double one.
    let mut trap = a_trap("shotgun", &[]);
    trap.id = "tr_shotgun_2".to_owned();
    cdda_sim::trap_registry::set_traps(&mut world, &[trap]);
    cdda_sim::missions::set_avatar(&mut world, who);
    give_the_walker_a_turn(&mut world, who);
    let (lost, _lines) = fire_a_shotgun(&mut world, who, "tr_shotgun_2");
    assert!(
        (80..=120).contains(&lost),
        "**two shells, eighty to a hundred and twenty points**: {lost}"
    );
}

/// **A shell that is dodged takes nothing**, and the trap is spent all the same. Reference: `trapfunc::shotgun` (`trapfunc.cpp:586-628`) - `rng( 5, 50 ) > get_dodge()` and the
/// shot is dodged, which the walker is told; the gun and the shells still hit the ground.
#[test]
fn a_dodged_shell_takes_no_health() {
    let (mut world, who) = a_world();
    install_the_shotgun_if_the_data_has_it(&mut world);
    cdda_sim::trap_registry::set_traps(&mut world, &[a_trap("shotgun", &[])]);
    cdda_sim::missions::set_avatar(&mut world, who);
    give_the_walker_a_turn(&mut world, who);
    // A walker with no dodge at all: `rng(5, 50)` is never below five, so every shell is dodged.
    world
        .entity_mut(who)
        .insert(cdda_sim::creature::Dodge { points: 0 });
    cdda_sim::singletons::put_rolls(&mut world, cdda_core_types::rng::Rolls::new(3));
    assert!(cdda_sim::square::set_trap(&mut world, (0, 0, 0), "tr_test"));
    assert!(cdda_sim::trap_spring::spring(&mut world, who, (0, 0, 0)));
    assert_eq!(
        cdda_sim::creature::health_of(&world, who),
        Some(100),
        "**nothing got through**"
    );
    assert!(
        cdda_sim::messages::lines(&world)
            .iter()
            .any(|line| line == "You dodge the shot!"),
        "and the walker is told: {:#?}",
        cdda_sim::messages::lines(&world)
    );
    assert_eq!(
        cdda_sim::square::trap_at(&world, (0, 0, 0)),
        None,
        "the trap is spent even though the shot missed"
    );
}

/// Runs the trap until a shell lands, and answers how much the walker lost and what was said.
fn fire_a_shotgun(world: &mut World, who: Entity, trap_id: &str) -> (i32, Vec<String>) {
    let mut seed = 1u32;
    while seed <= 400 {
        cdda_sim::singletons::put_rolls(world, cdda_core_types::rng::Rolls::new(seed));
        cdda_sim::messages::clear(world);
        cdda_sim::creature::set_health(world, who, 100);
        cdda_sim::square::remove_trap(world, (0, 0, 0));
        assert!(cdda_sim::square::set_trap(world, (0, 0, 0), trap_id));
        if cdda_sim::trap_spring::spring(world, who, (0, 0, 0)) {
            let lines = cdda_sim::messages::lines(world);
            let lost = 100 - cdda_sim::creature::health_of(world, who).unwrap_or(100);
            if lost > 0 {
                return (lost, lines);
            }
        }
        seed += 1;
    }
    panic!("**no seed of four hundred lands a shell**, so the rule never fires")
}

/// The two item types the trap leaves/// The two item types the trap leaves: a shotgun and its string of shells.
fn install_the_shotgun_if_the_data_has_it(world: &mut World) {
    // The trap spawns `shotgun_s` (a sawn-off) or `shotgun_d` and a `string_36`. The port's catalogue names them, so the test registers them under those names.
    cdda_sim::item_registry::set_item_types(world, &[a_kind("shotgun_d"), a_kind("string_36")]);
}

/// An item type with nothing but a name, which is all a drop needs.
fn a_kind(id: &str) -> cdda_sim::item_registry::Kind<'_> {
    cdda_sim::item_registry::Kind {
        id,
        volume_ml: 0,
        weight_g: 0,
        pockets: &[],
        thrown_damage: 0.0,
        melee_bash: 0.0,
        melee_cut: 0.0,
        melee_stab: 0.0,
        to_hit: 0,
    }
}

/// **A bear trap snaps on a leg.** Reference: `trapfunc::beartrap` (`trapfunc.cpp:189-231`) - two traps in the data, and the most physical rule of the lot:
///
/// - a **tiny** animal does not set it off at all: there is nothing of it in the jaws;
/// - "SNAP!" at volume eight, then the trap is removed **before** the jaws are read;
/// - the jaws close on a part that touches the ground, and are announced ("A bear trap closes on your <part>!") - and a rider hears about its mount first;
/// - the `beartrap` effect goes on the creature, and the jaws deal twelve bash and eighteen cut to the part they closed on;
/// - a cut that drew blood may infect the wound: one in a hundred and twenty-eight, one in five hundred and twelve for a creature that resists infection.
///
/// The numbers are what the test pins: thirty points through a body wearing nothing is twelve plus eighteen, and the same body with four points of cut armour
/// takes twenty-six, which is the only way to see that the two damage types were answered separately.
#[test]
fn a_bear_trap_closes_on_a_leg() {
    let (mut world, who) = a_world();
    world.entity_mut(who).insert(cdda_sim::creature::Body {
        parts: vec![
            a_part("torso", "torso"),
            a_part("leg_l", "leg"),
            a_part("foot_l", "foot"),
        ],
    });
    cdda_sim::creature::set_health(&mut world, who, 100);
    cdda_sim::missions::set_avatar(&mut world, who);
    cdda_sim::singletons::put_rolls(&mut world, cdda_core_types::rng::Rolls::new(5));
    // The effect the jaws leave behind is data, so the test installs the two ids it names.
    cdda_sim::effect_registry::set_effect_types(
        &mut world,
        &[
            cdda_sim::effect_registry::bare("beartrap"),
            cdda_sim::effect_registry::bare("tetanus"),
        ],
    );
    cdda_sim::trap_registry::set_traps(&mut world, &[a_trap("beartrap", &[])]);
    assert!(cdda_sim::square::set_trap(&mut world, (0, 0, 0), "tr_test"));
    assert!(
        cdda_sim::trap_spring::spring(&mut world, who, (0, 0, 0)),
        "**the jaws close**"
    );
    assert_eq!(
        cdda_sim::square::trap_at(&world, (0, 0, 0)),
        None,
        "and the trap is gone"
    );
    assert_eq!(
        cdda_sim::creature::health_of(&world, who),
        Some(70),
        "**twelve bash and eighteen cut, and the body wore nothing**"
    );
    assert!(
        cdda_sim::applied_effects::has(&world, who, "beartrap"),
        "**the jaws are still on it**: {:?}",
        cdda_sim::applied_effects::worn(&world, who)
    );
    assert!(
        cdda_sim::messages::lines(&world)
            .iter()
            .any(|line| line.starts_with("A bear trap closes on your ")),
        "and the walker is told where: {:#?}",
        cdda_sim::messages::lines(&world)
    );
}

/// **Armour on the leg takes the cut and leaves the bash.** `damage_instance` carries both types and each meets its own figure.
#[test]
fn a_bear_trap_takes_the_cut_armour_off_its_bite() {
    let (mut world, who) = a_world();
    world.entity_mut(who).insert(cdda_sim::creature::Body {
        parts: vec![a_part("leg_l", "leg"), a_part("foot_l", "foot")],
    });
    cdda_sim::creature::set_health(&mut world, who, 100);
    cdda_sim::trap_registry::set_traps(&mut world, &[a_trap("beartrap", &[])]);
    let cover = cdda_sim::creature::Garment {
        layers: ["leg_l", "foot_l"]
            .iter()
            .map(|name| {
                (
                    (*name).to_owned(),
                    cdda_content::armour::Layer {
                        damage_type: "cut".to_owned(),
                        resist: 4.0,
                        thickness: 1.0,
                        cover: 100,
                    },
                )
            })
            .collect(),
        vitals: Vec::new(),
    };
    give_the_walker_a_turn(&mut world, who);
    let garment = world.spawn(cover).id();
    assert_eq!(
        cdda_sim::creature::wear(&mut world, who, garment),
        cdda_sim::creature::Wore::Worn,
        "the garment goes on"
    );
    cdda_sim::creature::dress(&mut world);
    cdda_sim::singletons::put_rolls(&mut world, cdda_core_types::rng::Rolls::new(5));
    assert!(cdda_sim::square::set_trap(&mut world, (0, 0, 0), "tr_test"));
    assert!(cdda_sim::trap_spring::spring(&mut world, who, (0, 0, 0)));
    assert_eq!(
        cdda_sim::creature::health_of(&world, who),
        Some(74),
        "**twelve bash plus fourteen cut**: eighteen less the four points the leg wears"
    );
}

/// **A tiny animal does not set a bear trap off.** Reference: `trapfunc.cpp:191-194` - the jaws are not for something that small, and nothing happens at all.
#[test]
fn a_bear_trap_ignores_a_tiny_animal() {
    let (mut world, who) = a_world();
    cdda_sim::creature::set_health(&mut world, who, 100);
    if let Some(mut creature) = world.get_mut::<cdda_sim::creature::Creature>(who) {
        creature.size = cdda_sim::CreatureSize::Tiny;
    }
    cdda_sim::trap_registry::set_traps(&mut world, &[a_trap("beartrap", &[])]);
    assert!(cdda_sim::square::set_trap(&mut world, (0, 0, 0), "tr_test"));
    assert!(
        !cdda_sim::trap_spring::spring(&mut world, who, (0, 0, 0)),
        "**nothing happens**"
    );
    assert_eq!(
        cdda_sim::creature::health_of(&world, who),
        Some(100),
        "and nothing was hurt"
    );
    assert_eq!(
        cdda_sim::square::trap_at(&world, (0, 0, 0)),
        Some("tr_test".to_owned()),
        "and the trap is still set"
    );
}

/// **A wound the jaws drew blood from may fester**, and one they did not cannot. Reference: `trapfunc.cpp:218-224` - the tetanus roll happens only when the character is
/// not `INFECTION_IMMUNE` and the **cut** actually got through, at one in a hundred and twenty-eight (one in five hundred and twelve with `INFRESIST`).
///
/// **The dice are driven, not hoped for.** `Rolls::fixed` answers every draw with the same number, so the low run is a bite that festered and the high run is a bite that
/// did not - which pins both sides of the chance instead of counting a rate over seeds and calling a drifting one right.
#[test]
fn the_wound_may_fester() {
    let (mut world, who) = a_world();
    world.entity_mut(who).insert(cdda_sim::creature::Body {
        parts: vec![a_part("leg_l", "leg"), a_part("foot_l", "foot")],
    });
    cdda_sim::effect_registry::set_effect_types(
        &mut world,
        &[
            cdda_sim::effect_registry::bare("beartrap"),
            cdda_sim::effect_registry::bare("tetanus"),
        ],
    );
    cdda_sim::trap_registry::set_traps(&mut world, &[a_trap("beartrap", &[])]);

    // A low draw is one in a hundred and twenty-eight: the bite festers.
    cdda_sim::creature::set_health(&mut world, who, 100);
    cdda_sim::singletons::put_rolls(&mut world, cdda_core_types::rng::Rolls::fixed(0.0));
    assert!(cdda_sim::square::set_trap(&mut world, (0, 0, 0), "tr_test"));
    assert!(cdda_sim::trap_spring::spring(&mut world, who, (0, 0, 0)));
    assert!(
        cdda_sim::applied_effects::has(&world, who, "tetanus"),
        "**the wound festered**: {:?}",
        cdda_sim::applied_effects::worn(&world, who)
    );

    // A high draw is not: the same bite leaves nothing behind.
    cdda_sim::creature::set_health(&mut world, who, 100);
    cdda_sim::applied_effects::remove(&mut world, who, "tetanus");
    cdda_sim::singletons::put_rolls(&mut world, cdda_core_types::rng::Rolls::fixed(0.999));
    assert!(cdda_sim::square::set_trap(&mut world, (0, 0, 0), "tr_test"));
    assert!(cdda_sim::trap_spring::spring(&mut world, who, (0, 0, 0)));
    assert!(
        !cdda_sim::applied_effects::has(&world, who, "tetanus"),
        "**and the high draw does not fester**"
    );
}

/// **A land mine blows up whoever set it off.** Reference: `trapfunc::landmine` (`trapfunc.cpp:819-833`): the trigger's own square is the centre of an explosion of force
/// eighteen with a factor of a half, the creature that triggered it is the source, and the trap is gone afterwards. A **tiny** animal is too light to set one off.
#[test]
fn a_land_mine_explodes() {
    let (mut world, who) = a_world();
    cdda_sim::creature::set_health(&mut world, who, 100);
    cdda_sim::missions::set_avatar(&mut world, who);
    cdda_sim::trap_registry::set_traps(&mut world, &[a_trap("landmine", &[])]);
    cdda_sim::singletons::put_rolls(&mut world, cdda_core_types::rng::Rolls::new(5));
    assert!(cdda_sim::square::set_trap(&mut world, (0, 0, 0), "tr_test"));
    assert!(
        cdda_sim::trap_spring::spring(&mut world, who, (0, 0, 0)),
        "**the mine goes off**"
    );
    assert!(
        cdda_sim::messages::lines(&world)
            .iter()
            .any(|line| line == "You trigger a land mine!"),
        "and the walker is told: {:#?}",
        cdda_sim::messages::lines(&world)
    );
    assert!(
        cdda_sim::creature::health_of(&world, who).is_some_and(|hp| hp < 100),
        "**and the blast hurts**: {:?}",
        cdda_sim::creature::health_of(&world, who)
    );
    assert_eq!(
        cdda_sim::square::trap_at(&world, (0, 0, 0)),
        None,
        "and the mine is spent"
    );
}

/// **A tiny animal does not set a land mine off.** Reference: `trapfunc.cpp:821-824` - it is too light, and nothing happens at all.
#[test]
fn a_land_mine_ignores_a_tiny_animal() {
    let (mut world, who) = a_world();
    cdda_sim::creature::set_health(&mut world, who, 100);
    if let Some(mut creature) = world.get_mut::<cdda_sim::creature::Creature>(who) {
        creature.size = cdda_sim::CreatureSize::Tiny;
    }
    cdda_sim::trap_registry::set_traps(&mut world, &[a_trap("landmine", &[])]);
    cdda_sim::singletons::put_rolls(&mut world, cdda_core_types::rng::Rolls::new(5));
    assert!(cdda_sim::square::set_trap(&mut world, (0, 0, 0), "tr_test"));
    assert!(!cdda_sim::trap_spring::spring(&mut world, who, (0, 0, 0)));
    assert_eq!(cdda_sim::creature::health_of(&world, who), Some(100));
    assert_eq!(
        cdda_sim::square::trap_at(&world, (0, 0, 0)),
        Some("tr_test".to_owned()),
        "**and the mine is still armed**"
    );
}

/// **Glass caltrops cut a foot that steps on them, and cost the step as well.** Reference: `trapfunc::caltrops_glass` (`trapfunc.cpp:353-379`):
///
/// - only a creature sets them off, and whether it steps on them at all is `x_in_y( occupied_tile_fraction( size ), 1 )`, the chance the plain glass uses;
/// - "You step on a sharp {trap name}!" - the trap's **name**, where the plain glass says only "some glass";
/// - the flinch is `speed * 0.8`, spent **whether or not** the cut gets through;
/// - a cut of `rng( 3, 10 )` on a random **ground-contact** part;
/// - "The shards shatter!", a sound at volume eight, and the trap is removed.
#[test]
fn glass_caltrops_cut_a_foot_and_cost_the_step() {
    let (mut world, who) = a_world();
    world.entity_mut(who).insert(cdda_sim::creature::Body {
        parts: vec![
            a_part("torso", "torso"),
            a_part("leg_l", "leg"),
            a_part("foot_l", "foot"),
        ],
    });
    give_the_walker_a_turn(&mut world, who);
    let mut trap = a_trap("caltrops_glass", &[]);
    trap.name = "glass caltrops".to_owned();
    cdda_sim::trap_registry::set_traps(&mut world, &[trap]);
    assert!(cdda_sim::square::set_trap(&mut world, (0, 0, 0), "tr_test"));
    // **The dice are driven to the low end**: every draw answers zero, so the shards are stepped on (a medium creature fills half its tile) and the cut is the smallest of the range.
    // A seed would make the test say "something happened"; this says what.
    cdda_sim::singletons::put_rolls(&mut world, cdda_core_types::rng::Rolls::fixed(0.0));
    let hp_before = cdda_sim::creature::health_of(&world, who).unwrap_or(0);
    let moves_before = cdda_sim::creature::moves_left(&world, who);
    // **No dodge**: the avoidance roll is skipped, as it is for a monster (`spring_with( .., false )`), so the fixed dice reach the rule under test rather than being spent on
    // whether the creature noticed the shards.
    assert!(
        cdda_sim::trap_spring::spring_with(&mut world, who, (0, 0, 0), false),
        "**a medium creature steps on the shards**"
    );
    assert_eq!(
        cdda_sim::creature::moves_left(&world, who),
        moves_before - 80,
        "**the flinch costs eight tenths of a speed of a hundred**"
    );
    let hurt = hp_before - cdda_sim::creature::health_of(&world, who).unwrap_or(0);
    assert_eq!(
        hurt, 3,
        "**and the glass cuts for the smallest of three to ten**"
    );
    assert_eq!(
        cdda_sim::square::trap_at(&world, (0, 0, 0)),
        None,
        "**the shards are gone**"
    );
    assert!(
        cdda_sim::messages::lines(&world)
            .iter()
            .any(|line| line.contains("steps on a sharp glass caltrops")),
        "**and the trap is named**: {:?}",
        cdda_sim::messages::lines(&world)
    );
}

/// **A tiny creature walks between the shards.** The chance is `occupied_tile_fraction( size )` in one - a tenth of the tile for a tiny thing - so some seeds leave the caltrops
/// standing, and a refusal must spend nothing: not a move, not a hit point.
#[test]
fn a_tiny_creature_walks_between_the_shards() {
    let mut refusals = 0;
    for seed in 1..=40u32 {
        let (mut world, who) = a_world();
        give_the_walker_a_turn(&mut world, who);
        if let Some(mut creature) = world.get_mut::<cdda_sim::creature::Creature>(who) {
            creature.size = cdda_sim::CreatureSize::Tiny;
        }
        let mut trap = a_trap("caltrops_glass", &[]);
        trap.name = "glass caltrops".to_owned();
        cdda_sim::trap_registry::set_traps(&mut world, &[trap]);
        assert!(cdda_sim::square::set_trap(&mut world, (0, 0, 0), "tr_test"));
        cdda_sim::singletons::put_rolls(&mut world, cdda_core_types::rng::Rolls::new(seed));
        let moves_before = cdda_sim::creature::moves_left(&world, who);
        let hp_before = cdda_sim::creature::health_of(&world, who).unwrap_or(0);
        if cdda_sim::trap_spring::spring(&mut world, who, (0, 0, 0)) {
            continue;
        }
        refusals += 1;
        assert_eq!(
            cdda_sim::creature::moves_left(&world, who),
            moves_before,
            "**a refusal costs no move**"
        );
        assert_eq!(
            cdda_sim::creature::health_of(&world, who).unwrap_or(0),
            hp_before,
            "**and no hit point**"
        );
        assert!(
            cdda_sim::square::trap_at(&world, (0, 0, 0)).is_some(),
            "**and the shards stay where they are**"
        );
    }
    assert!(
        refusals > 0,
        "**a tiny creature refused the shards at least once in forty**: {refusals}"
    );
}

/// **A monster stumbles over a cot and a person does not.** Reference: `trapfunc::cot` (`trapfunc.cpp:177-187`): "Haha, only monsters stumble over a cot, humans are smart" -
/// the stumble costs the monster **a whole turn's moves** and is announced; a character walks over it free.
#[test]
fn a_monster_stumbles_over_a_cot_but_a_person_does_not() {
    let (mut world, who) = a_world();
    cdda_sim::trap_registry::set_traps(&mut world, &[a_trap("cot", &[])]);
    assert!(cdda_sim::square::set_trap(&mut world, (0, 0, 0), "tr_test"));
    give_the_walker_a_turn(&mut world, who);
    // The walker is a **monster** here: a creature with a kind, not a person.
    cdda_sim::creature::set_kind(&mut world, who, cdda_sim::MonsterKindId(1));
    let moves_before = cdda_sim::creature::moves_left(&world, who);
    assert!(
        cdda_sim::trap_spring::spring(&mut world, who, (0, 0, 0)),
        "**a monster trips on the cot**"
    );
    assert_eq!(
        cdda_sim::creature::moves_left(&world, who),
        moves_before - 100,
        "**and loses the whole turn**"
    );
    assert!(
        cdda_sim::messages::lines(&world)
            .iter()
            .any(|line| line.contains("stumbles over the cot")),
        "**and is told why**: {:?}",
        cdda_sim::messages::lines(&world)
    );
    // The same cot under a person's feet: nothing happens, and the cot is left standing because a cot is not consumed.
    let (mut world, who) = a_world();
    cdda_sim::trap_registry::set_traps(&mut world, &[a_trap("cot", &[])]);
    assert!(cdda_sim::square::set_trap(&mut world, (0, 0, 0), "tr_test"));
    give_the_walker_a_turn(&mut world, who);
    let moves_before = cdda_sim::creature::moves_left(&world, who);
    assert!(
        !cdda_sim::trap_spring::spring(&mut world, who, (0, 0, 0)),
        "**a person steps over it**"
    );
    assert_eq!(
        cdda_sim::creature::moves_left(&world, who),
        moves_before,
        "**at no cost**"
    );
}

/// **A humming trap says how loud it is by what it says.** Reference: `trapfunc::hum` (`trapfunc.cpp:1472-1491`): a volume of `rng( 1, 200 )` picks one of four sounds -
/// "hrm" up to ten, "hrmmm" to fifty, "HRMMM" to a hundred, "VRMMMMMM" above - and the sound is of the **activity** kind. It answers true whatever is standing on it.
#[test]
fn a_humming_trap_says_how_loud_it_is() {
    let mut heard = Vec::new();
    // **The seeds are spread across the generator's first draw**, which is `16807 * seed`: below the modulus that fraction is `seed / 127773`, so small seeds all land near
    // one and a spread of three hundred covers the whole range - quiet bands, loud ones, and every word between.
    for step in 1..=400u32 {
        let seed = step * 300;
        let (mut world, who) = a_world();
        cdda_sim::trap_registry::set_traps(&mut world, &[a_trap("hum", &[])]);
        assert!(cdda_sim::square::set_trap(&mut world, (0, 0, 0), "tr_test"));
        cdda_sim::singletons::put_rolls(&mut world, cdda_core_types::rng::Rolls::new(seed));
        assert!(
            cdda_sim::trap_spring::spring(&mut world, who, (0, 0, 0)),
            "**a hum always sounds**"
        );
        let line = cdda_sim::sounds::heard_this_turn(&mut world)
            .last()
            .cloned()
            .unwrap_or(cdda_sim::sounds::Sound {
                position: (0, 0, 0),
                volume: 0,
                category: cdda_sim::sounds::SoundCategory::Activity,
                description: String::new(),
            });
        heard.push(line);
    }
    assert!(
        heard.iter().any(|sound| sound.description == "VRMMMMMM"),
        "**the loudest band appeared**: {heard:?}"
    );
    assert!(
        heard.iter().any(|sound| sound.description == "hrm"),
        "**and the quietest too**"
    );
    let mut bands: Vec<&str> = heard
        .iter()
        .map(|sound| sound.description.as_str())
        .collect();
    bands.sort_unstable();
    bands.dedup();
    assert_eq!(
        bands,
        vec!["HRMMM", "VRMMMMMM", "hrm", "hrmmm"],
        "**all four bands appeared over four hundred seeds**"
    );
    // **The volume and the word agree**: every "VRMMMMMM" is above a hundred, every "hrmmm" between eleven and fifty, and every hum is an activity sound.
    for sound in &heard {
        assert_eq!(
            sound.category,
            cdda_sim::sounds::SoundCategory::Activity,
            "**a hum is activity, not violence**"
        );
        let fits = match sound.description.as_str() {
            "hrm" => sound.volume <= 10,
            "hrmmm" => (11..=50).contains(&sound.volume),
            "HRMMM" => (51..=100).contains(&sound.volume),
            "VRMMMMMM" => sound.volume > 100,
            other => panic!("an unknown hum: {other}"),
        };
        assert!(
            fits,
            "**the word fits the volume**: {:?} at {}",
            sound.description, sound.volume
        );
    }
}

/// **Bubble wrap pops and is gone, and a tiny animal walks round it.** Reference: `trapfunc::bubble` (`trapfunc.cpp:133-148`): a tiny creature sets nothing off; anyone else is told
/// ("You step on some bubble wrap!"), the square pops at volume **eighteen** as an **alarm**, and the wrap is consumed. It hurts nobody.
#[test]
fn bubble_wrap_pops_and_is_gone() {
    let (mut world, who) = a_world();
    give_the_walker_a_turn(&mut world, who);
    let mut trap = a_trap("bubble", &[]);
    trap.name = "bubble wrap".to_owned();
    cdda_sim::trap_registry::set_traps(&mut world, &[trap]);
    assert!(cdda_sim::square::set_trap(&mut world, (0, 0, 0), "tr_test"));
    let hp_before = cdda_sim::creature::health_of(&world, who).unwrap_or(0);
    assert!(
        cdda_sim::trap_spring::spring(&mut world, who, (0, 0, 0)),
        "**a person pops it**"
    );
    let sounds = cdda_sim::sounds::heard_this_turn(&mut world);
    let pop = sounds.last().cloned().expect("a pop was heard");
    assert_eq!(pop.description, "Pop!", "**it says what it sounds like**");
    assert_eq!(pop.volume, 18, "**at eighteen**");
    assert_eq!(
        pop.category,
        cdda_sim::sounds::SoundCategory::Alarm,
        "**as an alarm** - it is meant to be noticed"
    );
    assert_eq!(
        cdda_sim::creature::health_of(&world, who).unwrap_or(0),
        hp_before,
        "**and it hurts nobody**"
    );
    assert_eq!(
        cdda_sim::square::trap_at(&world, (0, 0, 0)),
        None,
        "**the wrap is consumed**"
    );
    // A tiny animal walks round it: nothing pops and the wrap stays for the next foot.
    let (mut world, who) = a_world();
    give_the_walker_a_turn(&mut world, who);
    if let Some(mut creature) = world.get_mut::<cdda_sim::creature::Creature>(who) {
        creature.size = cdda_sim::CreatureSize::Tiny;
    }
    cdda_sim::trap_registry::set_traps(&mut world, &[a_trap("bubble", &[])]);
    assert!(cdda_sim::square::set_trap(&mut world, (0, 0, 0), "tr_test"));
    assert!(
        !cdda_sim::trap_spring::spring(&mut world, who, (0, 0, 0)),
        "**a tiny animal does not pop the wrap**"
    );
    assert!(
        cdda_sim::square::trap_at(&world, (0, 0, 0)).is_some(),
        "**and the wrap is still there**"
    );
}

/// **A draining trap takes a point of life from every part of a person, and never moves.** Reference: `trapfunc::drain` (`trapfunc.cpp:1549-1564`): "You feel your life force sapping
/// away.", `hurtall( 1 )` for a character - one harm to **each** body part - or one point of pure damage to a monster's torso, and the trap stays where it is.
#[test]
fn a_drain_takes_a_point_from_every_part() {
    let (mut world, who) = a_world();
    cdda_sim::creature::set_health(&mut world, who, 100);
    world.entity_mut(who).insert(cdda_sim::creature::Body {
        parts: vec![
            a_part("torso", "torso"),
            a_part("head", "head"),
            a_part("leg_l", "leg"),
        ],
    });
    cdda_sim::missions::set_avatar(&mut world, who);
    cdda_sim::trap_registry::set_traps(&mut world, &[a_trap("drain", &[])]);
    assert!(cdda_sim::square::set_trap(&mut world, (0, 0, 0), "tr_test"));
    assert!(
        cdda_sim::trap_spring::spring(&mut world, who, (0, 0, 0)),
        "**the drain runs**"
    );
    // **A point per part**: the body has three, so three points go, not one. A drain that took a single point would be a different rule - `hurtall` is a point to *each* part.
    assert_eq!(
        cdda_sim::creature::health_of(&world, who),
        Some(97),
        "**three parts, three points**"
    );
    assert!(
        cdda_sim::messages::lines(&world)
            .iter()
            .any(|line| line.contains("life force sapping away")),
        "**and the walker feels it**: {:?}",
        cdda_sim::messages::lines(&world)
    );
    assert!(
        cdda_sim::square::trap_at(&world, (0, 0, 0)).is_some(),
        "**the trap does not move** - a drain is a place, not a one-shot"
    );
}

/// **Caltrops cut the foot and may be destroyed, disarmed, or neither.** Reference: `trapfunc::caltrops` (`trapfunc.cpp:311-345`):
///
/// - the step chance is `occupied_tile_fraction`, as for glass;
/// - "You step on a sharp {trap name}!", then the `speed * 0.8` flinch, then a cut of `rng( 9, 15 )` on a ground-contact part;
/// - a wound that drew blood may fester (`try_apply_tetanus`);
/// - a creature of `weight / 1_kilogram` in two hundred breaks the trap outright ("You destroy the ... as you step on it!"), and otherwise a **twenty in a hundred** chance
///   disarms it.
#[test]
fn caltrops_cut_and_leave_the_trap_standing() {
    let (mut world, who) = a_world();
    cdda_sim::missions::set_avatar(&mut world, who);
    world.entity_mut(who).insert(cdda_sim::creature::Body {
        parts: vec![a_part("torso", "torso"), a_part("foot_l", "foot")],
    });
    give_the_walker_a_turn(&mut world, who);
    let mut trap = a_trap("caltrops", &[]);
    trap.name = "caltrops".to_owned();
    cdda_sim::trap_registry::set_traps(&mut world, &[trap]);
    assert!(cdda_sim::square::set_trap(&mut world, (0, 0, 0), "tr_test"));
    // **The dice at the top of their range**: the step is taken (the chance is certain at any draw below a half's complement), the cut is the largest, and neither the break
    // nor the disarm comes up.
    cdda_sim::singletons::put_rolls(&mut world, cdda_core_types::rng::Rolls::fixed(0.25));
    let hp_before = cdda_sim::creature::health_of(&world, who).unwrap_or(0);
    let moves_before = cdda_sim::creature::moves_left(&world, who);
    assert!(
        cdda_sim::trap_spring::spring_with(&mut world, who, (0, 0, 0), false),
        "**the caltrops are stepped on**"
    );
    assert_eq!(
        cdda_sim::creature::moves_left(&world, who),
        moves_before - 80,
        "**the step costs eight tenths of a speed of a hundred**"
    );
    let cut = hp_before - cdda_sim::creature::health_of(&world, who).unwrap_or(0);
    assert_eq!(
        cut, 11,
        "**a quarter of the way through nine to fifteen, rounded**"
    );
    assert!(
        cdda_sim::messages::lines(&world)
            .iter()
            .any(|line| line.contains("step on a sharp caltrops")),
        "**with the word sharp in the line**: {:?}",
        cdda_sim::messages::lines(&world)
    );
    assert!(
        cdda_sim::square::trap_at(&world, (0, 0, 0)).is_some(),
        "**and the caltrops are still there**"
    );
}

/// **A heavy creature breaks the caltrops outright.** Reference: `:333-336` - `x_in_y( weight / 1_kilogram, 200 )`, so two hundred kilograms is certain and the line says what
/// happened.
#[test]
fn heavy_caltrops_break_under_two_hundred_kilograms() {
    let (mut world, who) = a_world();
    cdda_sim::missions::set_avatar(&mut world, who);
    world
        .entity_mut(who)
        .insert(cdda_sim::creature::Weight { grams: 200_000 });
    give_the_walker_a_turn(&mut world, who);
    let mut trap = a_trap("caltrops", &[]);
    trap.name = "caltrops".to_owned();
    cdda_sim::trap_registry::set_traps(&mut world, &[trap]);
    assert!(cdda_sim::square::set_trap(&mut world, (0, 0, 0), "tr_test"));
    cdda_sim::singletons::put_rolls(&mut world, cdda_core_types::rng::Rolls::fixed(0.25));
    assert!(cdda_sim::trap_spring::spring_with(
        &mut world,
        who,
        (0, 0, 0),
        false
    ));
    assert!(
        cdda_sim::messages::lines(&world)
            .iter()
            .any(|line| line.contains("You destroy the caltrops as you step on it!")),
        "**two hundred kilograms break them**: {:?}",
        cdda_sim::messages::lines(&world)
    );
    assert_eq!(
        cdda_sim::square::trap_at(&world, (0, 0, 0)),
        None,
        "**and the caltrops are gone**"
    );
}

/// **A step that neither breaks nor disarms disarms nothing, and one that disarms takes the trap away quietly.** Over sixty seeds a light creature does both: some steps leave
/// the caltrops standing, and some take them away - and the ones that take them away say nothing about destroying them, which is what tells the two branches apart.
#[test]
fn a_light_step_either_disarms_the_caltrops_or_leaves_them() {
    let mut left = 0;
    let mut disarmed = 0;
    for seed in 1..=60u32 {
        let (mut world, who) = a_world();
        cdda_sim::missions::set_avatar(&mut world, who);
        give_the_walker_a_turn(&mut world, who);
        let mut trap = a_trap("caltrops", &[]);
        trap.name = "caltrops".to_owned();
        cdda_sim::trap_registry::set_traps(&mut world, &[trap]);
        assert!(cdda_sim::square::set_trap(&mut world, (0, 0, 0), "tr_test"));
        cdda_sim::singletons::put_rolls(&mut world, cdda_core_types::rng::Rolls::new(seed));
        if !cdda_sim::trap_spring::spring_with(&mut world, who, (0, 0, 0), false) {
            continue;
        }
        if cdda_sim::square::trap_at(&world, (0, 0, 0)).is_some() {
            left += 1;
        } else {
            disarmed += 1;
            assert!(
                !cdda_sim::messages::lines(&world)
                    .iter()
                    .any(|line| line.contains("destroy")),
                "**a disarm is not a destruction**: {:?}",
                cdda_sim::messages::lines(&world)
            );
        }
    }
    assert!(
        left > 0,
        "**some steps leave the caltrops standing**: {left}"
    );
    assert!(
        disarmed > 0,
        "**and some disarm them**: {disarmed} of sixty"
    );
}

/// **A board is a nail in it.** Reference: `trapfunc::board` (`trapfunc.cpp:271-306`): the same shape as caltrops with the numbers of a plank - a cut of `rng( 3, 5 )`, and
/// **forty in a hundred** chance to be disarmed instead of twenty. The line does not call it sharp.
#[test]
fn a_board_cuts_less_and_disarms_more() {
    let (mut world, who) = a_world();
    cdda_sim::missions::set_avatar(&mut world, who);
    world.entity_mut(who).insert(cdda_sim::creature::Body {
        parts: vec![a_part("torso", "torso"), a_part("foot_l", "foot")],
    });
    give_the_walker_a_turn(&mut world, who);
    let mut trap = a_trap("board", &[]);
    trap.name = "nailed board".to_owned();
    cdda_sim::trap_registry::set_traps(&mut world, &[trap]);
    assert!(cdda_sim::square::set_trap(&mut world, (0, 0, 0), "tr_test"));
    cdda_sim::singletons::put_rolls(&mut world, cdda_core_types::rng::Rolls::fixed(0.25));
    let hp_before = cdda_sim::creature::health_of(&world, who).unwrap_or(0);
    assert!(
        cdda_sim::trap_spring::spring_with(&mut world, who, (0, 0, 0), false),
        "**the board is stepped on**"
    );
    assert_eq!(
        hp_before - cdda_sim::creature::health_of(&world, who).unwrap_or(0),
        4,
        "**and a quarter of the way through three to five**"
    );
    assert!(
        cdda_sim::messages::lines(&world)
            .iter()
            .any(|line| line.contains("step on a nailed board")
                || line.contains("steps on a nailed board")),
        "**and the line does not call a board sharp**: {:?}",
        cdda_sim::messages::lines(&world)
    );
}

/// A pit's own effectiveness, which is one unless corpses have filled it. Reference: `pit_effectiveness` (`trapfunc.cpp:113-126`) - the corpses on the square, each measured at its
/// own volume, against ten zombies' worth (625,000 ml).
fn a_pit_world() -> (World, Entity) {
    let (mut world, who) = a_world();
    cdda_sim::missions::set_avatar(&mut world, who);
    world.entity_mut(who).insert(cdda_sim::creature::Body {
        parts: vec![
            a_part("torso", "torso"),
            a_part("leg_l", "leg"),
            a_part("leg_r", "leg"),
        ],
    });
    give_the_walker_a_turn(&mut world, who);
    world
        .entity_mut(who)
        .insert(cdda_sim::creature::Dodge { points: 0 });
    cdda_sim::trap_registry::set_traps(&mut world, &[a_trap("pit", &[])]);
    assert!(cdda_sim::square::set_trap(&mut world, (0, 0, 0), "tr_test"));
    (world, who)
}

/// A world with a pit of the named kind, the walker the avatar of it, a body with the parts a pit can reach, and a whole turn banked.
fn a_pit_for(kind: &str) -> (World, Entity) {
    let (mut world, who) = a_world();
    cdda_sim::missions::set_avatar(&mut world, who);
    world.entity_mut(who).insert(cdda_sim::creature::Body {
        parts: vec![
            a_part("torso", "torso"),
            a_part("leg_l", "leg"),
            a_part("leg_r", "leg"),
            a_part("arm_l", "arm"),
            a_part("arm_r", "arm"),
            a_part("foot_l", "foot"),
            a_part("foot_r", "foot"),
        ],
    });
    give_the_walker_a_turn(&mut world, who);
    world
        .entity_mut(who)
        .insert(cdda_sim::creature::Dodge { points: 0 });
    cdda_sim::effect_registry::set_effect_types(
        &mut world,
        &[cdda_sim::effect_registry::bare("in_pit")],
    );
    cdda_sim::trap_registry::set_traps(&mut world, &[a_trap(kind, &[])]);
    assert!(cdda_sim::square::set_trap(&mut world, (0, 0, 0), "tr_test"));
    (world, who)
}

/// **A person falls in a pit and hurts themself.** Reference: `trapfunc::pit` (`trapfunc.cpp:960-1004`): a tiny creature is too small to be hurt by the fall; anyone else is told
/// ("You fall in a pit!"), takes the `in_pit` effect, and then - unless they dodge it - `eff * rng( 10, 20 ) - rng( dodge, dodge * 5 )` damage: `hurtall` for a share of it and
/// a bash hit on **each** leg.
#[test]
fn a_person_falls_in_a_pit_and_hurts_themselves() {
    let (mut world, who) = a_pit_world();
    cdda_sim::singletons::put_rolls(&mut world, cdda_core_types::rng::Rolls::fixed(0.0));
    let hp_before = cdda_sim::creature::health_of(&world, who).unwrap_or(0);
    assert!(
        cdda_sim::trap_spring::spring_with(&mut world, who, (0, 0, 0), false),
        "**the pit is fallen into**"
    );
    // **The whole fall, counted**: a ten-point fall, five of it dealt to the body (`rng( damage / 2, damage )` at the bottom of its range) and ten to each leg, so twenty-five
    // points are gone and the arithmetic says which rule did it.
    assert_eq!(
        hp_before - cdda_sim::creature::health_of(&world, who).unwrap_or(0),
        25,
        "**five to the body and ten to each leg**"
    );
    let lines = cdda_sim::messages::lines(&world);
    assert!(
        lines.iter().any(|line| line == "You fall in a pit!"),
        "**the fall is announced**: {lines:?}"
    );
    assert!(
        lines.iter().any(|line| line == "You hurt yourself!"),
        "**and the pain with it**: {lines:?}"
    );
}

/// **A nimble person lands unhurt.** Reference: `:986-996` - the damage is reduced by `rng( dodge, dodge * 5 )`, so a dodge of thirty takes the ten-to-twenty fall to nothing and
/// the line becomes "You land nimbly."
#[test]
fn a_nimble_person_lands_unhurt() {
    let (mut world, who) = a_pit_world();
    world
        .entity_mut(who)
        .insert(cdda_sim::creature::Dodge { points: 30 });
    cdda_sim::singletons::put_rolls(&mut world, cdda_core_types::rng::Rolls::fixed(0.0));
    let hp_before = cdda_sim::creature::health_of(&world, who).unwrap_or(0);
    assert!(cdda_sim::trap_spring::spring_with(
        &mut world,
        who,
        (0, 0, 0),
        false
    ));
    assert_eq!(
        cdda_sim::creature::health_of(&world, who).unwrap_or(0),
        hp_before,
        "**a dodge of thirty takes a ten-point fall**"
    );
    assert!(
        cdda_sim::messages::lines(&world)
            .iter()
            .any(|line| line == "You land nimbly."),
        "**and the walker says so**: {:?}",
        cdda_sim::messages::lines(&world)
    );
}

/// **A pit full of corpses does nothing.** Reference: `pit_effectiveness` (`:113-126`) - ten zombies' worth of corpse fills the pit, so a creature that falls in takes **no**
/// damage at all: the effectiveness multiplies the fall to zero.
#[test]
fn a_pit_full_of_corpses_does_nothing() {
    let (mut world, who) = a_pit_world();
    let corpse = world.spawn(()).id();
    world.entity_mut(corpse).insert((
        cdda_sim::Position { x: 0, y: 0, z: 0 },
        cdda_sim::GroundItem {
            kind: cdda_sim::ItemTypeId(1),
            charges: 1,
            age: 0,
            burnt: 0,
            damage: 0,
            burn: None,
        },
        cdda_sim::CorpseOf {
            kind: cdda_sim::MonsterKindId(1),
            died_on: 0,
            revives: false,
            monster_volume_ml: 625_000,
        },
    ));
    cdda_sim::PositionIndex::rebuild(&mut world);
    cdda_sim::singletons::put_rolls(&mut world, cdda_core_types::rng::Rolls::fixed(0.0));
    let hp_before = cdda_sim::creature::health_of(&world, who).unwrap_or(0);
    assert!(cdda_sim::trap_spring::spring_with(
        &mut world,
        who,
        (0, 0, 0),
        false
    ));
    assert_eq!(
        cdda_sim::creature::health_of(&world, who).unwrap_or(0),
        hp_before,
        "**a filled pit is not a pit**"
    );
    assert!(
        cdda_sim::messages::lines(&world)
            .iter()
            .any(|line| line == "You land nimbly."),
        "**and the fall is a soft one**: {:?}",
        cdda_sim::messages::lines(&world)
    );
}

/// **A tiny animal is not hurt by a pit.** Reference: `:967-970` - "tiny animals aren't hurt by falling into pits", so nothing happens at all.
#[test]
fn a_tiny_animal_is_not_hurt_by_a_pit() {
    let (mut world, who) = a_pit_world();
    if let Some(mut creature) = world.get_mut::<cdda_sim::creature::Creature>(who) {
        creature.size = cdda_sim::CreatureSize::Tiny;
    }
    cdda_sim::singletons::put_rolls(&mut world, cdda_core_types::rng::Rolls::fixed(0.0));
    let hp_before = cdda_sim::creature::health_of(&world, who).unwrap_or(0);
    assert!(
        !cdda_sim::trap_spring::spring_with(&mut world, who, (0, 0, 0), false),
        "**the pit does not answer a tiny animal**"
    );
    assert_eq!(
        cdda_sim::creature::health_of(&world, who).unwrap_or(0),
        hp_before
    );
}

/// **A light snare snaps shut on a small animal's leg.** Reference: `trapfunc::snare_light` (`trapfunc.cpp:687-711`): "Snap!" at volume four of the **combat** kind, the trap is
/// destroyed whatever happens, a tiny or **small** creature takes the `lightsnare` effect for a hundred turns and is told, and the trap's kit falls on the square.
#[test]
fn a_light_snare_snaps_on_a_small_animal() {
    let (mut world, who) = a_world();
    give_the_walker_a_turn(&mut world, who);
    cdda_sim::missions::set_avatar(&mut world, who);
    if let Some(mut creature) = world.get_mut::<cdda_sim::creature::Creature>(who) {
        creature.size = cdda_sim::CreatureSize::Small;
    }
    cdda_sim::effect_registry::set_effect_types(
        &mut world,
        &[cdda_sim::effect_registry::bare("lightsnare")],
    );
    // The kit the reference spawns on the square is a **kind**, so the test installs it by name.
    cdda_sim::item_registry::set_item_types(&mut world, &[a_kind("light_snare_kit")]);
    cdda_sim::trap_registry::set_traps(&mut world, &[a_trap("snare_light", &[])]);
    assert!(cdda_sim::square::set_trap(&mut world, (0, 0, 0), "tr_test"));
    cdda_sim::singletons::put_rolls(&mut world, cdda_core_types::rng::Rolls::new(5));
    assert!(
        cdda_sim::trap_spring::spring_with(&mut world, who, (0, 0, 0), false),
        "**the snare springs**"
    );
    let sounds = cdda_sim::sounds::heard_this_turn(&mut world);
    let snap = sounds.last().cloned().expect("a snap was heard");
    assert_eq!(snap.description, "Snap!");
    assert_eq!(snap.volume, 4, "**at volume four**");
    assert_eq!(snap.category, cdda_sim::sounds::SoundCategory::Combat);
    assert!(
        cdda_sim::applied_effects::has(&world, who, "lightsnare"),
        "**a small creature is caught**: {:?}",
        cdda_sim::applied_effects::worn(&world, who)
    );
    assert!(
        cdda_sim::messages::lines(&world)
            .iter()
            .any(|line| line.contains("A snare closes on your leg")),
        "**and told so**: {:?}",
        cdda_sim::messages::lines(&world)
    );
    assert_eq!(
        cdda_sim::square::trap_at(&world, (0, 0, 0)),
        None,
        "**the snare is destroyed by springing**"
    );
    // **The kit comes back**: "Always get trap components back on triggering tile" - the snare's own kit lies where the snare was.
    let kit = cdda_sim::item_registry::id_of(&mut world, "light_snare_kit");
    let lying: Vec<_> = cdda_sim::items_at(&mut world, (0, 0))
        .into_iter()
        .filter(|item| {
            world
                .get::<cdda_sim::GroundItem>(*item)
                .is_some_and(|ground| Some(ground.kind) == kit)
        })
        .collect();
    assert_eq!(
        lying.len(),
        1,
        "**the light snare kit is on the square the snare was on**"
    );
}

/// **A big creature only sets the snare off.** Reference: `:697` - the effect needs `size == tiny || size == small`, so a medium creature hears the snap, loses the trap and walks
/// on with nothing on its leg.
#[test]
fn a_light_snare_does_not_catch_a_medium_creature() {
    let (mut world, who) = a_world();
    give_the_walker_a_turn(&mut world, who);
    cdda_sim::missions::set_avatar(&mut world, who);
    cdda_sim::effect_registry::set_effect_types(
        &mut world,
        &[cdda_sim::effect_registry::bare("lightsnare")],
    );
    cdda_sim::trap_registry::set_traps(&mut world, &[a_trap("snare_light", &[])]);
    assert!(cdda_sim::square::set_trap(&mut world, (0, 0, 0), "tr_test"));
    assert!(cdda_sim::trap_spring::spring_with(
        &mut world,
        who,
        (0, 0, 0),
        false
    ));
    assert!(
        !cdda_sim::applied_effects::has(&world, who, "lightsnare"),
        "**a medium creature is not caught**"
    );
    assert_eq!(cdda_sim::square::trap_at(&world, (0, 0, 0)), None);
}

/// **A tripwire moves the walker and can hurt them.** Reference: `trapfunc::tripwire` (`trapfunc.cpp:405-462`): a tiny creature squeezes under it; anyone else is told ("You trip
/// over a tripwire!"), is put down on a **random empty square next to it**, spends `speed * 1.5` moves, and - if `rng( 5, 20 ) > dexterity` - loses `rng( 1, 4 )` from every part.
#[test]
fn a_tripwire_moves_the_walker_and_hurts_them() {
    let (mut world, who) = a_world();
    give_the_walker_a_turn(&mut world, who);
    cdda_sim::missions::set_avatar(&mut world, who);
    // A dexterity of zero, so the trip takes: `rng( 5, 20 ) > dex` is true for every draw.
    world
        .entity_mut(who)
        .insert(cdda_sim::creature::Dexterity { base: 0 });
    cdda_sim::trap_registry::set_traps(&mut world, &[a_trap("tripwire", &[])]);
    assert!(cdda_sim::square::set_trap(&mut world, (0, 0, 0), "tr_test"));
    cdda_sim::singletons::put_rolls(&mut world, cdda_core_types::rng::Rolls::fixed(0.0));
    let hp_before = cdda_sim::creature::health_of(&world, who).unwrap_or(0);
    let moves_before = cdda_sim::creature::moves_left(&world, who);
    assert!(
        cdda_sim::trap_spring::spring_with(&mut world, who, (0, 0, 0), false),
        "**the wire is tripped**"
    );
    let at = world
        .get::<cdda_sim::Position>(who)
        .map(|at| (at.x, at.y, at.z));
    assert_ne!(
        at,
        Some((0, 0, 0)),
        "**the walker is moved off the wire**: {at:?}"
    );
    if let Some((x, y, _)) = at {
        assert!(
            x.abs() <= 1 && y.abs() <= 1,
            "**to a square next to the wire at the origin**: {at:?}"
        );
    }
    assert_eq!(
        cdda_sim::creature::moves_left(&world, who),
        moves_before - 150,
        "**and loses a turn and a half**"
    );
    assert!(
        cdda_sim::creature::health_of(&world, who).unwrap_or(0) < hp_before,
        "**the trip hurts**: {hp_before} -> {:?}",
        cdda_sim::creature::health_of(&world, who)
    );
    assert!(
        cdda_sim::messages::lines(&world)
            .iter()
            .any(|line| line.contains("You trip over a tripwire")),
        "**and is told why**: {:?}",
        cdda_sim::messages::lines(&world)
    );
}

/// **A nimble walker trips but is not hurt.** `rng( 5, 20 ) > dexterity` is the whole gate: a dexterity of thirty takes every draw, so the trip costs the move and nothing else.
#[test]
fn a_nimble_walker_trips_without_being_hurt() {
    let (mut world, who) = a_world();
    give_the_walker_a_turn(&mut world, who);
    cdda_sim::missions::set_avatar(&mut world, who);
    world
        .entity_mut(who)
        .insert(cdda_sim::creature::Dexterity { base: 30 });
    cdda_sim::trap_registry::set_traps(&mut world, &[a_trap("tripwire", &[])]);
    assert!(cdda_sim::square::set_trap(&mut world, (0, 0, 0), "tr_test"));
    cdda_sim::singletons::put_rolls(&mut world, cdda_core_types::rng::Rolls::fixed(0.75));
    let hp_before = cdda_sim::creature::health_of(&world, who).unwrap_or(0);
    assert!(cdda_sim::trap_spring::spring_with(
        &mut world,
        who,
        (0, 0, 0),
        false
    ));
    assert_eq!(
        cdda_sim::creature::health_of(&world, who).unwrap_or(0),
        hp_before,
        "**a dexterity of thirty takes a five-to-twenty roll**"
    );
}

/// **A spiked pit impales one part, and its spears break.** Reference: `trapfunc::pit_spikes` (`trapfunc.cpp:1010-1093`): the fall is `eff * rng( 20, 50 )`; a dodge roll of
/// `rng( 5, 30 )` or a fall that does nothing avoids the spikes; otherwise a ten-sided roll picks the part - both legs twice, both arms twice, the torso six times in ten - the
/// spikes impale it for the whole fall as **cut**, and the wound may fester. Then, one time in four, the spears break: the pit becomes a plain one and **four pointy sticks**, each
/// one in three, lie where they were.
#[test]
fn a_spiked_pit_impales_and_its_spears_break() {
    let (mut world, who) = a_pit_for("pit_spikes");
    cdda_sim::item_registry::set_item_types(&mut world, &[a_kind("pointy_stick")]);
    cdda_sim::singletons::put_rolls(&mut world, cdda_core_types::rng::Rolls::fixed(0.0));
    let hp_before = cdda_sim::creature::health_of(&world, who).unwrap_or(0);
    assert!(
        cdda_sim::trap_spring::spring_with(&mut world, who, (0, 0, 0), false),
        "**the spiked pit is fallen into**"
    );
    // The fall is twenty (the bottom of the range) and the ten-sided roll is one: the left leg.
    assert_eq!(
        hp_before - cdda_sim::creature::health_of(&world, who).unwrap_or(0),
        20,
        "**twenty points of cut on one part**"
    );
    assert!(
        cdda_sim::messages::lines(&world)
            .iter()
            .any(|line| line.starts_with("The spikes impale your leg")),
        "**and the part is named**: {:?}",
        cdda_sim::messages::lines(&world)
    );
    assert!(
        cdda_sim::messages::lines(&world)
            .iter()
            .any(|line| line == "The spears break!"),
        "**the lowest dice break the spears**: {:?}",
        cdda_sim::messages::lines(&world)
    );
    // **Four spears, four sticks**: `for( i = 0; i < 4; i++ ) if( one_in( 3 ) )`, and the dice are all low, so every one lands.
    let sticks = cdda_sim::item_registry::id_of(&mut world, "pointy_stick");
    let lying = cdda_sim::items_at(&mut world, (0, 0))
        .into_iter()
        .filter(|item| {
            world
                .get::<cdda_sim::GroundItem>(*item)
                .is_some_and(|ground| Some(ground.kind) == sticks)
        })
        .count();
    assert_eq!(lying, 4, "**four spears to a pit**");
}

/// **The fall is the pit's own range, and the part the pit's own table.** A quarter of the way through: `rng( 20, 50 )` is twenty-eight and the ten-sided roll is three - the left
/// arm. A rule that read the low end of the range, or the plain pit's table, would answer twenty and a leg.
#[test]
fn a_spiked_pit_falls_inside_its_range_and_hits_its_own_parts() {
    let (mut world, who) = a_pit_for("pit_spikes");
    cdda_sim::singletons::put_rolls(&mut world, cdda_core_types::rng::Rolls::fixed(0.25));
    let hp_before = cdda_sim::creature::health_of(&world, who).unwrap_or(0);
    assert!(cdda_sim::trap_spring::spring_with(
        &mut world,
        who,
        (0, 0, 0),
        false
    ));
    assert_eq!(
        hp_before - cdda_sim::creature::health_of(&world, who).unwrap_or(0),
        28,
        "**a quarter of the way through twenty to fifty**"
    );
    assert!(
        cdda_sim::messages::lines(&world)
            .iter()
            .any(|line| line.starts_with("The spikes impale your arm")),
        "**and the left arm is impaled**: {:?}",
        cdda_sim::messages::lines(&world)
    );
}

/// **A glass pit slashes a foot.** Reference: `trapfunc::pit_glass` (`:1095-1180`): a fall of `eff * rng( 15, 35 )` and a ten-sided roll that reaches the **feet** where the spiked
/// pit reaches the torso - five or six is a foot. Halfway through: a fall of twenty-five and a roll of six, the right foot.
#[test]
fn a_glass_pit_slashes_a_foot() {
    let (mut world, who) = a_pit_for("pit_glass");
    cdda_sim::singletons::put_rolls(&mut world, cdda_core_types::rng::Rolls::fixed(0.5));
    let hp_before = cdda_sim::creature::health_of(&world, who).unwrap_or(0);
    assert!(
        cdda_sim::trap_spring::spring_with(&mut world, who, (0, 0, 0), false),
        "**the glass pit is fallen into**"
    );
    assert_eq!(
        hp_before - cdda_sim::creature::health_of(&world, who).unwrap_or(0),
        25,
        "**half of fifteen to thirty-five**"
    );
    assert!(
        cdda_sim::messages::lines(&world)
            .iter()
            .any(|line| line.starts_with("The glass shards slash your foot")),
        "**a foot, where the spiked pit would reach the torso**: {:?}",
        cdda_sim::messages::lines(&world)
    );
}

/// **A nimble fall misses the lining.** `rng( 5, 30 ) < dodge` is the gate: a dodge of forty takes every draw, so the fall costs nothing and the line says the lining was avoided.
#[test]
fn a_nimble_fall_misses_the_lining() {
    let (mut world, who) = a_pit_for("pit_spikes");
    world
        .entity_mut(who)
        .insert(cdda_sim::creature::Dodge { points: 40 });
    cdda_sim::singletons::put_rolls(&mut world, cdda_core_types::rng::Rolls::fixed(0.0));
    let hp_before = cdda_sim::creature::health_of(&world, who).unwrap_or(0);
    assert!(cdda_sim::trap_spring::spring_with(
        &mut world,
        who,
        (0, 0, 0),
        false
    ));
    assert_eq!(
        cdda_sim::creature::health_of(&world, who).unwrap_or(0),
        hp_before,
        "**a dodge of forty avoids the spikes**"
    );
    assert!(
        cdda_sim::messages::lines(&world)
            .iter()
            .any(|line| line == "You avoid the spikes within."),
        "**and the line says so**: {:?}",
        cdda_sim::messages::lines(&world)
    );
}

/// A world with a lava trap underfoot, the walker the avatar, a body with legs and feet, and the terrain named so the line can say what burned them.
fn a_lava_world() -> (World, Entity) {
    let (mut world, who) = a_world();
    cdda_sim::missions::set_avatar(&mut world, who);
    world.entity_mut(who).insert(cdda_sim::creature::Body {
        parts: vec![
            a_part("torso", "torso"),
            a_part("leg_l", "leg"),
            a_part("leg_r", "leg"),
            a_part("foot_l", "foot"),
            a_part("foot_r", "foot"),
        ],
    });
    give_the_walker_a_turn(&mut world, who);
    cdda_sim::mapgen_update::set_terrain_codes(
        &mut world,
        [("t_lava".to_owned(), 7_u32)].into_iter().collect(),
    );
    cdda_sim::square::set_terrain(&mut world, (0, 0, 0), 7);
    cdda_sim::trap_registry::set_traps(&mut world, &[a_trap("lava", &[])]);
    assert!(cdda_sim::square::set_trap(&mut world, (0, 0, 0), "tr_test"));
    (world, who)
}

/// **A blade hacks the torso, bash and cut.** Reference: `trapfunc::blade` (`trapfunc.cpp:668-685`): "A blade swings out and hacks your torso!" and then a damage instance of **bash
/// twelve and cut thirty** on the torso - the only trap so far that deals two kinds at once.
#[test]
fn a_blade_hacks_the_torso_with_bash_and_cut() {
    let (mut world, who) = a_world();
    give_the_walker_a_turn(&mut world, who);
    cdda_sim::missions::set_avatar(&mut world, who);
    world.entity_mut(who).insert(cdda_sim::creature::Body {
        parts: vec![
            a_part("torso", "torso"),
            a_part("leg_l", "leg"),
            a_part("leg_r", "leg"),
        ],
    });
    cdda_sim::trap_registry::set_traps(&mut world, &[a_trap("blade", &[])]);
    assert!(cdda_sim::square::set_trap(&mut world, (0, 0, 0), "tr_test"));
    let hp_before = cdda_sim::creature::health_of(&world, who).unwrap_or(0);
    assert!(
        cdda_sim::trap_spring::spring_with(&mut world, who, (0, 0, 0), false),
        "**the blade swings**"
    );
    assert_eq!(
        hp_before - cdda_sim::creature::health_of(&world, who).unwrap_or(0),
        42,
        "**twelve bash and thirty cut**"
    );
    assert!(
        cdda_sim::messages::lines(&world)
            .iter()
            .any(|line| line == "A blade swings out and hacks your torso!"),
        "**and the line names the torso**: {:?}",
        cdda_sim::messages::lines(&world)
    );
}

/// **Lava burns every part that touches it.** Reference: `trapfunc::lava` (`trapfunc.cpp:1191-1233`): the line names the terrain ("The %s burns you horribly!"), and then **two
/// loops** - `get_ground_contact_bodyparts( true )`, which is the legs, and `get_ground_contact_bodyparts()`, which is the feet - each part taking **twenty heat**. A monster
/// instead takes heat on its torso, and how much depends on what it is made of: thirty by default, fifty for flesh, eighty for veggy, two hundred for something flammable, fifteen
/// for stone and **five** for steel or kevlar - the last matching material wins.
#[test]
fn lava_burns_the_legs_and_the_feet() {
    let (mut world, who) = a_lava_world();
    let hp_before = cdda_sim::creature::health_of(&world, who).unwrap_or(0);
    assert!(
        cdda_sim::trap_spring::spring_with(&mut world, who, (0, 0, 0), false),
        "**the lava burns**"
    );
    assert_eq!(
        hp_before - cdda_sim::creature::health_of(&world, who).unwrap_or(0),
        80,
        "**twenty per leg and twenty per foot**"
    );
    assert!(
        cdda_sim::messages::lines(&world)
            .iter()
            .any(|line| line.contains("burns you horribly")),
        "**and the line says so**: {:?}",
        cdda_sim::messages::lines(&world)
    );
}

/// **What a monster is made of decides how lava hurts it.** The reference's own ladder: thirty, fifty for flesh, eighty for veggy, two hundred for anything flammable, fifteen for
/// stone and five for steel - each later question overwriting the answer before it.
#[test]
fn lava_hurts_a_monster_by_what_it_is_made_of() {
    let cases = [
        (vec![("flesh", 1)], 50),
        (vec![("veggy", 1)], 80),
        (vec![("paper", 1)], 200),
        (vec![("stone", 1)], 15),
        (vec![("steel", 1)], 5),
        (vec![("bone", 1)], 30),
        // **The order is the rule**: flesh and veggy together take veggy's eighty, and flesh with steel takes steel's five, because each later question overwrites the answer.
        (vec![("flesh", 1), ("veggy", 1)], 80),
        (vec![("flesh", 1), ("steel", 1)], 5),
    ];
    for (made_of, expected) in cases {
        let (mut world, who) = a_lava_world();
        // A monster, not a person: a creature with a kind, and the avatar is somebody else - `is_a_monster` reads the marker, not the name.
        cdda_sim::creature::set_kind(&mut world, who, cdda_sim::MonsterKindId(1));
        let somebody_else = world.spawn(()).id();
        cdda_sim::missions::set_avatar(&mut world, somebody_else);
        if let Some(mut creature) = world.get_mut::<cdda_sim::creature::Creature>(who) {
            creature.name = "a shambler".to_owned();
        }
        world.entity_mut(who).insert(cdda_sim::creature::Materials(
            made_of
                .iter()
                .map(|(name, portion)| ((*name).to_owned(), *portion))
                .collect(),
        ));
        cdda_sim::singletons::put_rolls(&mut world, cdda_core_types::rng::Rolls::new(3));
        let hp_before = cdda_sim::creature::health_of(&world, who).unwrap_or(0);
        assert!(cdda_sim::trap_spring::spring_with(
            &mut world,
            who,
            (0, 0, 0),
            false
        ));
        assert_eq!(
            hp_before - cdda_sim::creature::health_of(&world, who).unwrap_or(0),
            expected,
            "**{made_of:?} takes {expected}**"
        );
    }
}

/// **A snake hisses, and sometimes a shadow snake forms.** Reference: `trapfunc::snake` (`trapfunc.cpp:1608-1638`): "ssssssss" at volume ten of the **movement** kind; one time
/// in six the trap is gone; one time in three the trap tries ten times to put a **shadow snake** five squares out, on a square it can see from the trap, and when one lands the trap
/// is removed and the snake says "A shadowy snake forms nearby."
#[test]
fn a_snake_hisses_and_summons_a_shadow_snake() {
    let (mut world, who) = a_world();
    cdda_sim::monster_registry::set_monster_types(
        &mut world,
        &[cdda_sim::monster_registry::MonsterKind {
            id: "mon_shadow_snake",
            hp: 40,
        }],
    );
    cdda_sim::trap_registry::set_traps(&mut world, &[a_trap("snake", &[])]);
    assert!(cdda_sim::square::set_trap(&mut world, (0, 0, 0), "tr_test"));
    cdda_sim::singletons::put_rolls(&mut world, cdda_core_types::rng::Rolls::fixed(0.0));
    assert!(
        cdda_sim::trap_spring::spring_with(&mut world, who, (0, 0, 0), false),
        "**a snake always answers**"
    );
    let sounds = cdda_sim::sounds::heard_this_turn(&mut world);
    let hiss = sounds.last().cloned().expect("a hiss was heard");
    assert_eq!(hiss.description, "ssssssss");
    assert_eq!(hiss.volume, 10, "**at volume ten**");
    assert_eq!(hiss.category, cdda_sim::sounds::SoundCategory::Movement);
    assert_eq!(
        cdda_sim::square::trap_at(&world, (0, 0, 0)),
        None,
        "**the trap is gone**"
    );
    // **A shadow snake, five squares out**: the dice put it at the far corner the reference's own two rolls reach.
    let snake =
        cdda_sim::monster_registry::id_of(&mut world, "mon_shadow_snake").map(|kind| kind.0);
    let mut query = world.query::<(&cdda_sim::creature::CreatureKind, &cdda_sim::Position)>();
    let found: Vec<(u32, i32, i32)> = query
        .iter(&world)
        .map(|(kind, at)| (kind.0.0, at.x, at.y))
        .collect();
    let [placed] = found.as_slice() else {
        panic!("**one creature was placed**: {found:?}");
    };
    assert_eq!(Some(placed.0), snake, "**and it is the shadow snake**");
    assert_eq!((placed.1, placed.2), (-5, -5), "**five squares out**");
    // **And nothing is said**: the reference's line is `spawned->add_msg_if_npc( ... )` on the snake itself, and a monster is not an NPC.
    assert!(
        !cdda_sim::messages::lines(&world)
            .iter()
            .any(|line| line.contains("shadowy snake")),
        "**a monster's own line is never said**: {:?}",
        cdda_sim::messages::lines(&world)
    );
}

/// **A glow trap burns a monster and slows it.** Reference: `trapfunc::glow` (`trapfunc.cpp:1430-1472`): a monster takes `rng( 5, 10 )` of acid on its torso one time in three and
/// has its **base speed multiplied by 0.9**. "Small flashes surround you." is the else branch for anyone.
#[test]
fn a_glow_trap_burns_a_monster_and_slows_it() {
    let (mut world, who) = a_world();
    give_the_walker_a_turn(&mut world, who);
    cdda_sim::creature::set_kind(&mut world, who, cdda_sim::MonsterKindId(1));
    cdda_sim::creature::set_speed(&mut world, who, 100, 0);
    cdda_sim::trap_registry::set_traps(&mut world, &[a_trap("glow", &[])]);
    assert!(cdda_sim::square::set_trap(&mut world, (0, 0, 0), "tr_test"));
    cdda_sim::singletons::put_rolls(&mut world, cdda_core_types::rng::Rolls::fixed(0.0));
    let hp_before = cdda_sim::creature::health_of(&world, who).unwrap_or(0);
    assert!(
        cdda_sim::trap_spring::spring_with(&mut world, who, (0, 0, 0), false),
        "**the glow answers**"
    );
    assert_eq!(
        hp_before - cdda_sim::creature::health_of(&world, who).unwrap_or(0),
        5,
        "**five points of acid at the bottom of the range**"
    );
    assert_eq!(
        cdda_sim::creature::speed_of(&world, who),
        90,
        "**and the base speed times nine tenths**"
    );
}

/// **A person is bathed in radiation, or blinded, or only sees small flashes.** The reference's three-way roll: one in three says the line about radiation, and otherwise one in
/// four says the flash, and otherwise the small flashes. The **dose itself is not applied** - the port has no creature dose - so the line is what a person sees; that is the named
/// gap this rule carries.
#[test]
fn a_glow_trap_talks_to_a_person() {
    let (mut world, who) = a_world();
    give_the_walker_a_turn(&mut world, who);
    cdda_sim::missions::set_avatar(&mut world, who);
    cdda_sim::trap_registry::set_traps(&mut world, &[a_trap("glow", &[])]);
    assert!(cdda_sim::square::set_trap(&mut world, (0, 0, 0), "tr_test"));
    cdda_sim::singletons::put_rolls(&mut world, cdda_core_types::rng::Rolls::fixed(0.0));
    assert!(cdda_sim::trap_spring::spring_with(
        &mut world,
        who,
        (0, 0, 0),
        false
    ));
    let lines = cdda_sim::messages::lines(&world);
    assert!(
        lines
            .iter()
            .any(|line| line == "You're bathed in radiation!"),
        "**the lowest dice are the radiation branch**: {lines:?}"
    );
    // **The flash branch is a recording, not a hunt for a seed**: one in three false (`int 0 2 1`), then one in four true (`int 0 3 0`) - and the recording asserts the bounds, so a
    // port that asked a different range fails here rather than rolling differently.
    let (mut world, who) = a_world();
    give_the_walker_a_turn(&mut world, who);
    cdda_sim::missions::set_avatar(&mut world, who);
    cdda_sim::trap_registry::set_traps(&mut world, &[a_trap("glow", &[])]);
    assert!(cdda_sim::square::set_trap(&mut world, (0, 0, 0), "tr_test"));
    // The flashbang's effects are kinds the data names, so the world must know them before it can give them.
    cdda_sim::effect_registry::set_effect_types(
        &mut world,
        &[
            cdda_sim::effect_registry::bare("deaf"),
            cdda_sim::effect_registry::bare("blind"),
        ],
    );
    let text = "int 0 2 1\nint 0 3 0\n";
    let recording = cdda_core_types::rng::Recording::parse(text).expect("two dice parse");
    cdda_sim::singletons::put_rolls(
        &mut world,
        cdda_core_types::rng::Rolls::replaying(recording.clone()),
    );
    assert!(cdda_sim::trap_spring::spring_with(
        &mut world,
        who,
        (0, 0, 0),
        false
    ));
    assert!(
        cdda_sim::messages::lines(&world)
            .iter()
            .any(|line| line == "A blinding flash strikes you!"),
        "**one in three false, one in four true: the flash**: {:?}",
        cdda_sim::messages::lines(&world)
    );
    // **The flash actually flashes**: the person on the trap is deafened and blinded by it (`explosion_handler::flashbang`), which is what "not immune" means.
    let effects: Vec<String> = cdda_sim::applied_effects::worn(&world, who)
        .into_iter()
        .map(|effect| effect.id)
        .collect();
    assert!(
        effects.iter().any(|id| id == "deaf" || id == "blind"),
        "**the flashbang reached the person**: {effects:?}"
    );
}

/// A world with a crossbow trap, a person standing on it with every part a bolt can reach, and the trap's parts registered.
fn a_crossbow_person_world() -> (World, Entity) {
    let (mut world, who) = a_world();
    give_the_walker_a_turn(&mut world, who);
    cdda_sim::missions::set_avatar(&mut world, who);
    world.entity_mut(who).insert(cdda_sim::creature::Body {
        parts: vec![
            a_part("torso", "torso"),
            a_part("leg_l", "leg"),
            a_part("leg_r", "leg"),
            a_part("foot_l", "foot"),
            a_part("foot_r", "foot"),
            a_part("head", "head"),
        ],
    });
    cdda_sim::item_registry::set_item_types(
        &mut world,
        &[
            a_kind("crossbow"),
            a_kind("string_36"),
            a_kind("bolt_steel"),
        ],
    );
    cdda_sim::trap_registry::set_traps(&mut world, &[a_trap("crossbow", &[])]);
    assert!(cdda_sim::square::set_trap(&mut world, (0, 0, 0), "tr_test"));
    (world, who)
}

/// A world with a crossbow trap, a monster of the given size standing on it, an avatar somewhere else to see it, and the trap's parts registered.
fn a_crossbow_world_with_monster(size: cdda_sim::CreatureSize) -> (World, Entity) {
    let (mut world, who) = a_world();
    give_the_walker_a_turn(&mut world, who);
    cdda_sim::creature::set_kind(&mut world, who, cdda_sim::MonsterKindId(1));
    if let Some(mut creature) = world.get_mut::<cdda_sim::creature::Creature>(who) {
        creature.size = size;
        "zombie".clone_into(&mut creature.name);
    }
    cdda_sim::item_registry::set_item_types(
        &mut world,
        &[
            a_kind("crossbow"),
            a_kind("string_36"),
            a_kind("bolt_steel"),
        ],
    );
    cdda_sim::trap_registry::set_traps(&mut world, &[a_trap("crossbow", &[])]);
    assert!(cdda_sim::square::set_trap(&mut world, (0, 0, 0), "tr_test"));
    let watcher = cdda_sim::spawn_player(&mut world, (3, 0), 100, 0);
    cdda_sim::missions::set_avatar(&mut world, watcher);
    (world, who)
}

/// **A crossbow trap shoots the person who trips it, and leaves its parts.** Reference: `trapfunc::crossbow` (`trapfunc.cpp:468-563`): "You trigger a crossbow trap!", then - unless
/// one in four and then `rng( 8, 20 ) > dodge` both say otherwise - a ten-sided roll picks the part (a foot, then a leg, then the torso six times in ten, then the head), the bolt
/// deals `rng( 20, 30 )` of cut, and one time in ten the bolt is lost. The trap always leaves a **crossbow** and a **string**, and a bolt unless it was lost.
#[test]
fn a_crossbow_trap_shoots_the_person_who_trips_it() {
    let (mut world, who) = a_world();
    give_the_walker_a_turn(&mut world, who);
    cdda_sim::missions::set_avatar(&mut world, who);
    world.entity_mut(who).insert(cdda_sim::creature::Body {
        parts: vec![
            a_part("torso", "torso"),
            a_part("leg_l", "leg"),
            a_part("leg_r", "leg"),
            a_part("foot_l", "foot"),
            a_part("foot_r", "foot"),
            a_part("head", "head"),
        ],
    });
    cdda_sim::item_registry::set_item_types(
        &mut world,
        &[
            a_kind("crossbow"),
            a_kind("string_36"),
            a_kind("bolt_steel"),
        ],
    );
    cdda_sim::trap_registry::set_traps(&mut world, &[a_trap("crossbow", &[])]);
    assert!(cdda_sim::square::set_trap(&mut world, (0, 0, 0), "tr_test"));
    // Halfway through every range: not dodged, `rng( 8, 20 )` is fourteen, the ten-sided roll is six - the torso - and the bolt is not lost.
    cdda_sim::singletons::put_rolls(&mut world, cdda_core_types::rng::Rolls::fixed(0.5));
    let hp_before = cdda_sim::creature::health_of(&world, who).unwrap_or(0);
    assert!(
        cdda_sim::trap_spring::spring_with(&mut world, who, (0, 0, 0), false),
        "**the crossbow fires**"
    );
    assert_eq!(
        hp_before - cdda_sim::creature::health_of(&world, who).unwrap_or(0),
        25,
        "**twenty-five points of cut**"
    );
    let lines = cdda_sim::messages::lines(&world);
    assert!(
        lines
            .iter()
            .any(|line| line == "You trigger a crossbow trap!"),
        "**the trap announces itself**: {lines:?}"
    );
    assert!(
        lines.iter().any(|line| line == "Your torso is hit!"),
        "**and names the part**: {lines:?}"
    );
    assert_eq!(cdda_sim::square::trap_at(&world, (0, 0, 0)), None);
    let lying = cdda_sim::items_at(&mut world, (0, 0));
    let mut kinds: Vec<String> = lying
        .into_iter()
        .filter_map(|item| {
            let kind = world.get::<cdda_sim::GroundItem>(item)?.kind;
            cdda_sim::item_registry::name_of(&mut world, kind)
        })
        .collect();
    kinds.sort();
    assert_eq!(
        kinds,
        ["bolt_steel", "crossbow", "string_36"],
        "**the trap's own parts, and its bolt**"
    );
}

/// **A lucky trip dodges the bolt.** The reference's gate is `!one_in( 4 ) && rng( 8, 20 ) > dodge`: the lowest dice make `one_in( 4 )` true, so nothing is fired at the person, the
/// line says the shot was dodged, and the trap still leaves its crossbow, its string **and** its bolt - `add_bolt` starts true and only a hit can lose it.
#[test]
fn a_lucky_trip_dodges_the_crossbow_bolt() {
    let (mut world, who) = a_world();
    give_the_walker_a_turn(&mut world, who);
    cdda_sim::missions::set_avatar(&mut world, who);
    cdda_sim::item_registry::set_item_types(
        &mut world,
        &[
            a_kind("crossbow"),
            a_kind("string_36"),
            a_kind("bolt_steel"),
        ],
    );
    cdda_sim::trap_registry::set_traps(&mut world, &[a_trap("crossbow", &[])]);
    assert!(cdda_sim::square::set_trap(&mut world, (0, 0, 0), "tr_test"));
    cdda_sim::singletons::put_rolls(&mut world, cdda_core_types::rng::Rolls::fixed(0.0));
    let hp_before = cdda_sim::creature::health_of(&world, who).unwrap_or(0);
    assert!(cdda_sim::trap_spring::spring_with(
        &mut world,
        who,
        (0, 0, 0),
        false
    ));
    assert_eq!(
        cdda_sim::creature::health_of(&world, who).unwrap_or(0),
        hp_before,
        "**the shot was dodged**"
    );
    assert!(
        cdda_sim::messages::lines(&world)
            .iter()
            .any(|line| line == "You dodge the shot!"),
        "**and the line says so**: {:?}",
        cdda_sim::messages::lines(&world)
    );
    assert_eq!(
        cdda_sim::items_at(&mut world, (0, 0)).len(),
        3,
        "**the crossbow, the string and the bolt are all left behind**"
    );
}

/// **A crossbow's chance to hit a monster is its size.** Reference: `trapfunc.cpp:525-548` - one in **fifty** for a tiny monster, eight for small, six for medium, four for large and
/// one for huge. A tenth of the way through every range puts the boundary between medium and large: the same dice hit the large and huge monsters and miss the three smaller ones.
#[test]
fn a_crossbow_hits_a_big_monster_more_often_than_a_small_one() {
    let cases = [
        (cdda_sim::CreatureSize::Tiny, false),
        (cdda_sim::CreatureSize::Small, false),
        (cdda_sim::CreatureSize::Medium, false),
        (cdda_sim::CreatureSize::Large, true),
        (cdda_sim::CreatureSize::Huge, true),
    ];
    for (size, hit) in cases {
        let (mut world, who) = a_crossbow_world_with_monster(size);
        cdda_sim::singletons::put_rolls(&mut world, cdda_core_types::rng::Rolls::fixed(0.1));
        let hp_before = cdda_sim::creature::health_of(&world, who).unwrap_or(0);
        assert!(cdda_sim::trap_spring::spring_with(
            &mut world,
            who,
            (0, 0, 0),
            false
        ));
        let lost = hp_before - cdda_sim::creature::health_of(&world, who).unwrap_or(0);
        if hit {
            assert_eq!(
                lost,
                21,
                "**{size:?} takes the bolt**: one in {} hits",
                match size {
                    cdda_sim::CreatureSize::Large => 4,
                    _ => 1,
                }
            );
            assert!(
                cdda_sim::messages::lines(&world)
                    .iter()
                    .any(|line| line.contains("A bolt shoots out and hits")),
                "**and the line says so**: {:?}",
                cdda_sim::messages::lines(&world)
            );
        } else {
            assert_eq!(lost, 0, "**{size:?} is missed**");
            assert!(
                cdda_sim::messages::lines(&world)
                    .iter()
                    .any(|line| line.contains("A bolt shoots out, but misses")),
                "**and the line says the bolt missed**: {:?}",
                cdda_sim::messages::lines(&world)
            );
        }
    }
}

/// **A shadow trap answers only the player, and calls a shadow.** Reference: `trapfunc::shadow` (`trapfunc.cpp:1493-1520`): "Monsters and npcs are completely ignored here" - so a
/// monster stepping on it gets nothing and the trap stays - while the avatar gets ten tries at a square five out that it can see, and the first shadow that lands removes the trap.
#[test]
fn a_shadow_answers_only_the_player() {
    let (mut world, who) = a_world();
    give_the_walker_a_turn(&mut world, who);
    cdda_sim::monster_registry::set_monster_types(
        &mut world,
        &[cdda_sim::monster_registry::MonsterKind {
            id: "mon_shadow",
            hp: 30,
        }],
    );
    cdda_sim::trap_registry::set_traps(&mut world, &[a_trap("shadow", &[])]);
    assert!(cdda_sim::square::set_trap(&mut world, (0, 0, 0), "tr_test"));
    // A monster is ignored, and the trap is still there.
    cdda_sim::creature::set_kind(&mut world, who, cdda_sim::MonsterKindId(1));
    assert!(
        !cdda_sim::trap_spring::spring_with(&mut world, who, (0, 0, 0), false),
        "**a monster is ignored**"
    );
    assert_eq!(
        cdda_sim::square::trap_at(&world, (0, 0, 0)),
        Some("tr_test".to_owned()),
        "**and the trap is untouched**"
    );
    // The avatar is not, and a shadow forms five squares out.
    cdda_sim::missions::set_avatar(&mut world, who);
    cdda_sim::singletons::put_rolls(&mut world, cdda_core_types::rng::Rolls::fixed(0.0));
    assert!(
        cdda_sim::trap_spring::spring_with(&mut world, who, (0, 0, 0), false),
        "**the avatar calls a shadow**"
    );
    assert_eq!(cdda_sim::square::trap_at(&world, (0, 0, 0)), None);
    let shadow = cdda_sim::monster_registry::id_of(&mut world, "mon_shadow").map(|kind| kind.0);
    let mut query = world.query::<(&cdda_sim::creature::CreatureKind, &cdda_sim::Position)>();
    let found: Vec<(u32, i32, i32)> = query
        .iter(&world)
        .map(|(kind, at)| (kind.0.0, at.x, at.y))
        .collect();
    let placed = found
        .iter()
        .find(|(kind, _, _)| Some(*kind) == shadow)
        .expect("a shadow was placed");
    assert_eq!((placed.1, placed.2), (-5, -5), "**five squares out**");
    assert!(
        !cdda_sim::messages::lines(&world)
            .iter()
            .any(|line| line.contains("shadow forms")),
        "**and nothing is said**: the line is the monster's own `add_msg_if_npc`"
    );
}

/// **The bolt can take the head, and it has to be lost.** The ten-sided roll's tenth face is the **head** where five to nine are the torso, and the bolt is lost only on a hit that
/// then rolls one in ten - which a recording says exactly: not dodged, a fifteen that beats no dodge, a six for the torso... and here a ten for the head, twenty-five of cut, then the
/// losing roll.
#[test]
fn the_bolt_can_take_the_head_and_be_lost() {
    let (mut world, who) = a_crossbow_person_world();
    cdda_sim::singletons::put_rolls(&mut world, cdda_core_types::rng::Rolls::fixed(0.99));
    let hp_before = cdda_sim::creature::health_of(&world, who).unwrap_or(0);
    assert!(cdda_sim::trap_spring::spring_with(
        &mut world,
        who,
        (0, 0, 0),
        false
    ));
    assert_eq!(
        hp_before - cdda_sim::creature::health_of(&world, who).unwrap_or(0),
        30,
        "**the bottom... no: the top of the bolt's range, on the head**"
    );
    assert!(
        cdda_sim::messages::lines(&world)
            .iter()
            .any(|line| line == "Your head is hit!"),
        "**the tenth face is the head**: {:?}",
        cdda_sim::messages::lines(&world)
    );
    // **A bolt that is rolled away is not left behind**: the recording hits the torso and then loses the bolt on the one-in-ten.
    let (mut world, who) = a_crossbow_person_world();
    let text = "int 0 3 1\nint 8 20 15\nint 1 10 6\nint 20 30 25\nint 0 9 0\n";
    let recording = cdda_core_types::rng::Recording::parse(text).expect("five dice parse");
    cdda_sim::singletons::put_rolls(
        &mut world,
        cdda_core_types::rng::Rolls::replaying(recording.clone()),
    );
    let hp_before = cdda_sim::creature::health_of(&world, who).unwrap_or(0);
    assert!(cdda_sim::trap_spring::spring_with(
        &mut world,
        who,
        (0, 0, 0),
        false
    ));
    assert_eq!(
        hp_before - cdda_sim::creature::health_of(&world, who).unwrap_or(0),
        25,
        "**twenty-five on the torso**"
    );
    assert_eq!(
        cdda_sim::items_at(&mut world, (0, 0)).len(),
        2,
        "**the crossbow and the string, and no bolt**"
    );
}

/// **A temple toggle swaps one colour of rock for floor and back.** Reference: `trapfunc::temple_toggle` (`trapfunc.cpp:1359-1423`): only the avatar hears "You hear the grinding of
/// shifting rock.", and then every square on the level changes **by the colour of the floor it was stepped from** - from a red floor, green rock becomes green floor and green floor
/// becomes green rock; from green, blue swaps; from blue, red swaps. A red floor leaves blue alone, which is what this checks.
#[test]
fn a_temple_toggle_swaps_the_colours() {
    let (mut world, who) = a_world();
    give_the_walker_a_turn(&mut world, who);
    cdda_sim::missions::set_avatar(&mut world, who);
    cdda_sim::mapgen_update::set_terrain_codes(
        &mut world,
        [
            ("t_floor_red".to_owned(), 1_u32),
            ("t_rock_green".to_owned(), 2),
            ("t_floor_green".to_owned(), 3),
            ("t_rock_blue".to_owned(), 4),
            ("t_floor_blue".to_owned(), 5),
            ("t_rock_red".to_owned(), 6),
        ]
        .into_iter()
        .collect(),
    );
    // The floor underfoot is red, and the room holds one of each colour that red cares about, plus blue's pair to prove it is left alone.
    for (x, code) in [(0, 1_u32), (5, 2), (6, 3), (7, 5), (8, 4)] {
        cdda_sim::square::set_terrain(&mut world, (x, 0, 0), code);
        cdda_sim::PositionIndex::rebuild(&mut world);
    }
    cdda_sim::trap_registry::set_traps(&mut world, &[a_trap("temple_toggle", &[])]);
    assert!(cdda_sim::square::set_trap(&mut world, (0, 0, 0), "tr_test"));
    assert!(
        cdda_sim::trap_spring::spring_with(&mut world, who, (0, 0, 0), false),
        "**the avatar turns the room**"
    );
    assert_eq!(
        cdda_sim::square::terrain_at(&world, (5, 0, 0)),
        Some(3),
        "**green rock becomes green floor**"
    );
    assert_eq!(
        cdda_sim::square::terrain_at(&world, (6, 0, 0)),
        Some(2),
        "**and green floor becomes green rock**"
    );
    assert_eq!(
        cdda_sim::square::terrain_at(&world, (7, 0, 0)),
        Some(5),
        "**a red floor leaves blue floor alone**"
    );
    assert_eq!(
        cdda_sim::square::terrain_at(&world, (8, 0, 0)),
        Some(4),
        "**and blue rock too**"
    );
    assert!(
        cdda_sim::messages::lines(&world)
            .iter()
            .any(|line| line == "You hear the grinding of shifting rock."),
        "**and the line is the grinding**: {:?}",
        cdda_sim::messages::lines(&world)
    );
}

/// **A flood trap clears its siblings and schedules the flood.** Reference: `trapfunc::temple_flood` (`trapfunc.cpp:1334-1357`): only the avatar is answered - "Monsters and npcs
/// are completely ignored here" - with "You step on a loose tile, and water starts to flood the room!", then **every** `temple_flood` trap on the level is removed and the
/// `TEMPLE_FLOOD` event is scheduled **three turns** out. The flood itself is the timed event's work, not the trap's.
#[test]
fn a_flood_trap_clears_its_siblings_and_schedules_the_flood() {
    let (mut world, who) = a_world();
    give_the_walker_a_turn(&mut world, who);
    cdda_sim::calendar::set_turn(&mut world, 1_000);
    cdda_sim::trap_registry::set_traps(&mut world, &[a_trap("temple_flood", &[])]);
    assert!(cdda_sim::square::set_trap(&mut world, (0, 0, 0), "tr_test"));
    assert!(cdda_sim::square::set_trap(&mut world, (1, 0, 0), "tr_test"));
    // A monster is ignored and the traps stay.
    cdda_sim::creature::set_kind(&mut world, who, cdda_sim::MonsterKindId(1));
    assert!(!cdda_sim::trap_spring::spring_with(
        &mut world,
        who,
        (0, 0, 0),
        false
    ));
    assert_eq!(
        cdda_sim::square::trap_at(&world, (1, 0, 0)),
        Some("tr_test".to_owned())
    );
    let watcher = cdda_sim::spawn_player(&mut world, (3, 0), 100, 0);
    cdda_sim::missions::set_avatar(&mut world, watcher);
    assert!(
        cdda_sim::trap_spring::spring_with(&mut world, watcher, (0, 0, 0), false),
        "**the avatar steps on the tile**"
    );
    assert_eq!(
        cdda_sim::square::trap_at(&world, (0, 0, 0)),
        None,
        "**the trap underfoot is gone**"
    );
    assert_eq!(
        cdda_sim::square::trap_at(&world, (1, 0, 0)),
        None,
        "**and so is the one beside it**"
    );
    assert!(
        !cdda_sim::timed_events::has_happened(&world, "TEMPLE_FLOOD").unwrap_or(true),
        "**the flood has not come yet**"
    );
    cdda_sim::calendar::set_turn(&mut world, 1_003);
    assert_eq!(
        cdda_sim::timed_events::has_happened(&world, "TEMPLE_FLOOD"),
        Some(true),
        "**three turns later it has**"
    );
    assert!(
        cdda_sim::messages::lines(&world)
            .iter()
            .any(|line| line == "You step on a loose tile, and water starts to flood the room!"),
        "**and the line is the loose tile**: {:?}",
        cdda_sim::messages::lines(&world)
    );
}

/// A world with a dissector underfoot and a body carrying every part the beams reach. A named part may be covered by ten points of cut armour.
fn a_dissector_world(covered: Option<&str>) -> (World, Entity) {
    let (mut world, who) = a_world();
    give_the_walker_a_turn(&mut world, who);
    cdda_sim::missions::set_avatar(&mut world, who);
    cdda_sim::creature::set_health(&mut world, who, 500);
    let parts = [
        "head", "torso", "arm_r", "arm_l", "hand_r", "hand_l", "leg_r", "leg_l", "foot_l", "foot_r",
    ];
    let kind_of = |name: &str| match name {
        "head" => "head",
        "torso" => "torso",
        "arm_r" | "arm_l" => "arm",
        "hand_r" | "hand_l" => "hand",
        "leg_r" | "leg_l" => "leg",
        _ => "foot",
    };
    world.entity_mut(who).insert(cdda_sim::creature::Body {
        parts: parts
            .iter()
            .map(|name| a_part(name, kind_of(name)))
            .collect(),
    });
    if let Some(part) = covered {
        let layer = cdda_content::armour::Layer {
            damage_type: "cut".to_owned(),
            resist: 10.0,
            thickness: 1.0,
            cover: 100,
        };
        let garment = world
            .spawn(cdda_sim::creature::Garment {
                layers: [(part.to_owned(), layer)].into_iter().collect(),
                vitals: Vec::new(),
            })
            .id();
        assert_eq!(
            cdda_sim::creature::wear(&mut world, who, garment),
            cdda_sim::creature::Wore::Worn,
            "the cover goes on"
        );
        cdda_sim::creature::dress(&mut world);
    }
    cdda_sim::trap_registry::set_traps(&mut world, &[a_trap("dissector", &[])]);
    assert!(cdda_sim::square::set_trap(&mut world, (0, 0, 0), "tr_test"));
    (world, who)
}

/// **A dissector slices ten parts and stays armed.** Reference: `trapfunc::dissector` (`trapfunc.cpp:900-962`): "BRZZZAP!" at volume ten of the combat kind, and then a fixed list of
/// ten cut hits - head fifteen, torso twenty, both arms twelve, both hands ten, **the right leg twice twelve**, both feet ten. The trap is a machine and is **not** removed.
///
/// **The reference's own bug is the rule**: `leg_r` is hit twice and `leg_l` never, and the sum - a hundred and twenty-three - is the same either way. What tells them apart is
/// armour: covering the **left** leg changes nothing, and covering the **right** leg takes twenty off instead of ten.
#[test]
fn a_dissector_slices_ten_parts_and_stays_armed() {
    let (mut world, who) = a_dissector_world(None);
    let hp_before = cdda_sim::creature::health_of(&world, who).unwrap_or(0);
    assert!(
        cdda_sim::trap_spring::spring_with(&mut world, who, (0, 0, 0), false),
        "**the beams fire**"
    );
    assert_eq!(
        hp_before - cdda_sim::creature::health_of(&world, who).unwrap_or(0),
        123,
        "**fifteen, twenty, twelve twice, ten twice, twelve twice, ten twice**"
    );
    let sounds = cdda_sim::sounds::heard_this_turn(&mut world);
    let zap = sounds.last().cloned().expect("a zap was heard");
    assert_eq!(zap.description, "BRZZZAP!");
    assert_eq!(zap.volume, 10, "**at volume ten**");
    assert_eq!(zap.category, cdda_sim::sounds::SoundCategory::Combat);
    assert_eq!(
        cdda_sim::square::trap_at(&world, (0, 0, 0)),
        Some("tr_test".to_owned()),
        "**the machine stays armed**"
    );
}

/// **The left leg is never hit and the right leg is hit twice.** Armour is the only way to see it: ten points of cut cover on the left leg leaves the total at a hundred and
/// twenty-three, and the same cover on the right leg takes twenty off it - two hits of twelve against ten points each.
#[test]
fn a_dissector_hits_the_right_leg_twice_and_the_left_never() {
    let (mut world, who) = a_dissector_world(Some("leg_l"));
    let hp_before = cdda_sim::creature::health_of(&world, who).unwrap_or(0);
    assert!(cdda_sim::trap_spring::spring_with(
        &mut world,
        who,
        (0, 0, 0),
        false
    ));
    assert_eq!(
        hp_before - cdda_sim::creature::health_of(&world, who).unwrap_or(0),
        123,
        "**covering the left leg changes nothing, because the reference never hits it**"
    );
    let (mut world, who) = a_dissector_world(Some("leg_r"));
    let hp_before = cdda_sim::creature::health_of(&world, who).unwrap_or(0);
    assert!(cdda_sim::trap_spring::spring_with(
        &mut world,
        who,
        (0, 0, 0),
        false
    ));
    assert_eq!(
        hp_before - cdda_sim::creature::health_of(&world, who).unwrap_or(0),
        103,
        "**and covering the right leg takes twenty, because it is hit twice**"
    );
}

/// **A robot is refused.** Reference: `:910-921` - the dissector will not dissect a machine: it says "BEEPBOOP!  Please remove non-organic object." at volume four of the
/// electronic-speech kind, returns **false** so the trap is left standing, and deals no damage at all.
#[test]
fn a_dissector_refuses_a_robot() {
    let (mut world, who) = a_dissector_world(None);
    cdda_sim::creature::set_kind(&mut world, who, cdda_sim::MonsterKindId(1));
    // **The robot is a monster, so somebody else is the player**: `is_a_monster` asks whether this creature is the avatar.
    let somebody_else = world.spawn(()).id();
    cdda_sim::missions::set_avatar(&mut world, somebody_else);
    world.entity_mut(who).insert(cdda_sim::creature::Species(
        ["ROBOT".to_owned()].into_iter().collect(),
    ));
    let hp_before = cdda_sim::creature::health_of(&world, who).unwrap_or(0);
    assert!(
        !cdda_sim::trap_spring::spring_with(&mut world, who, (0, 0, 0), false),
        "**a robot is refused**"
    );
    assert_eq!(
        cdda_sim::creature::health_of(&world, who).unwrap_or(0),
        hp_before,
        "**and nothing is cut**"
    );
    let sounds = cdda_sim::sounds::heard_this_turn(&mut world);
    let beep = sounds.last().cloned().expect("a beep was heard");
    assert_eq!(
        beep.description,
        "BEEPBOOP!  Please remove non-organic object."
    );
    assert_eq!(
        beep.category,
        cdda_sim::sounds::SoundCategory::ElectronicSpeech
    );
    assert_eq!(
        cdda_sim::square::trap_at(&world, (0, 0, 0)),
        Some("tr_test".to_owned())
    );
}
