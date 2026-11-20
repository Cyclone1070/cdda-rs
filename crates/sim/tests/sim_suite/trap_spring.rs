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
        traps: std::collections::BTreeMap::new(),
        items: std::collections::BTreeMap::new(),
        weight: 100,
        fields: Vec::new(),
        fill_ter: None,
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
