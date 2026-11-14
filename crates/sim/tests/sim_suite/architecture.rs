//! Architecture gates: they run the real world and check its shape, not its behaviour.
//!
//! The rule these gates defend is in `docs/01-architecture.md`: one store of entities and
//! components, an index derived from it, and nothing else authoritative. A rule like that is worth
//! nothing unless something checks it, because every drift is local and every drift looks
//! reasonable at the time.
//!
//! Most of the checks here are **dynamic**: they build a `World`, put things in it, rebuild the
//! index and compare the two. That is what catches an actual violation - an index that has drifted,
//! a despawn that left a ghost, a kind of thing that needed a container instead of a component.
//! The two ledger checks at the bottom read the sources, because the thing they count (a `Resource`
//! type, a pointer) has no runtime surface to interrogate; they are named as ledgers so nobody
//! mistakes them for proof of good behaviour.

#![allow(clippy::panic, clippy::expect_used, clippy::indexing_slicing)]

use bevy_ecs::prelude::*;
use cdda_sim::{GroundItem, ItemTypeId, Position, PositionIndex};
use std::fmt::Write as _;

/// A world with things on it: two things on one square, one thing on another square, one thing a
/// level down, one thing with no square at all, and one that is not in the world either.
fn populated() -> World {
    let mut world = World::new();
    for (x, y, z) in [(0, 0, 0), (0, 0, 0), (1, 0, 0), (0, 0, -1)] {
        world.spawn((
            Position { x, y, z },
            GroundItem {
                kind: ItemTypeId(7),
                charges: 1,
                age: 0,
                burnt: 0,
                damage: 0,
                burn: None,
            },
        ));
    }
    // No square: a thing that is held, or a global-shaped thing.
    world.spawn((GroundItem {
        kind: ItemTypeId(8),
        charges: 3,
        age: 0,
        burnt: 0,
        damage: 0,
        burn: None,
    },));
    // A creature with **no** square, for the same reason as the held item: the fixture is four things with a
    // square and two without, and a thing without one must not appear in the index at all.
    world.spawn((
        cdda_sim::creature::Creature {
            name: "a zombie".to_owned(),
            size: cdda_sim::CreatureSize::Medium,
        },
        cdda_sim::creature::Health { hp: 4, max_hp: 4 },
    ));
    world
}

/// Every entity the store knows about, paired with the square it claims.
fn positioned(world: &mut World) -> Vec<(Entity, (i32, i32, i32))> {
    let mut query = world.query::<(Entity, &Position)>();
    let mut found: Vec<(Entity, (i32, i32, i32))> = query
        .iter(world)
        .map(|(entity, at)| (entity, (at.x, at.y, at.z)))
        .collect();
    found.sort();
    found
}

/// Every square worth looking at: a box wider than the fixtures, so a ghost parked outside a
/// fixture square is still found.
fn boxed_squares() -> Vec<(i32, i32, i32)> {
    let mut squares = Vec::new();
    for z in -2..=1 {
        for y in -3..=3 {
            for x in -3..=3 {
                squares.push((x, y, z));
            }
        }
    }
    squares
}

/// What the index says, as (id, square) pairs, gathered through its public interface rather than
/// trusting its internals.
fn index_pairs(world: &mut World) -> Vec<(Entity, (i32, i32, i32))> {
    let mut found = Vec::new();
    let Some(index) = cdda_sim::index(world) else {
        return found;
    };
    for square in boxed_squares() {
        for entity in index.at_level(square.0, square.1, square.2) {
            found.push((*entity, square));
        }
    }
    found.sort();
    found
}

/// The index is a derived view, so it must agree with the store in both directions: everything with
/// a square is listed there, and nothing without a square is listed anywhere.
#[test]
fn the_index_is_exactly_the_store() {
    let mut world = populated();
    PositionIndex::rebuild(&mut world);

    let expected = positioned(&mut world);
    let actual = index_pairs(&mut world);

    assert_eq!(
        expected.len(),
        4,
        "the fixture is meant to hold four things with a square, or the comparison proves nothing"
    );
    assert!(
        world
            .iter_entities()
            .any(|entity| entity.get::<Position>().is_none()),
        "the fixture is meant to hold things with no square too, or half the comparison is dead"
    );
    assert_eq!(
        expected,
        actual,
        "the index and the store disagree: {} entities carry a square, the index lists {}. \
         A missing entry is an invisible thing; an extra entry is a ghost.",
        expected.len(),
        actual.len()
    );
}

/// The comparison above only proves something if a drifted index is detectable. Move a thing
/// without rebuilding, and the comparison must fail.
#[test]
fn a_drifted_index_is_caught() {
    let mut world = populated();
    PositionIndex::rebuild(&mut world);

    let mover = *positioned(&mut world)
        .first()
        .expect("the fixture holds a thing with a square");
    world.entity_mut(mover.0).insert(Position {
        x: mover.1.0 + 2,
        y: mover.1.1,
        z: mover.1.2,
    });

    assert_ne!(
        positioned(&mut world),
        index_pairs(&mut world),
        "a stale index was not detected, so the agreement test above is vacuous"
    );

    PositionIndex::rebuild(&mut world);
    assert_eq!(
        positioned(&mut world),
        index_pairs(&mut world),
        "a rebuild did not repair the index"
    );
}

/// Destroying a thing must leave nothing behind. A ghost entry is the classic failure of a
/// hand-maintained index, and the store must not be able to produce one.
#[test]
fn a_destroyed_thing_leaves_no_ghost() {
    let mut world = populated();
    PositionIndex::rebuild(&mut world);
    let victim = index_pairs(&mut world)
        .first()
        .map(|(entity, _)| *entity)
        .expect("the index lists something");

    world.despawn(victim);
    PositionIndex::rebuild(&mut world);

    assert!(
        !index_pairs(&mut world)
            .into_iter()
            .any(|(entity, _)| entity == victim),
        "the destroyed thing is still in the index"
    );
    assert!(
        world.get_entity(victim).is_err(),
        "the id of a destroyed thing still resolves, so references are not ids"
    );
}

/// Pure ECS, checked by doing it: what a thing *is*, is the set of components it carries. Changing
/// that set changes what systems see, without recreating the thing and without any container being
/// told about it.
#[test]
fn a_thing_is_whatever_its_components_say() {
    let mut world = World::new();
    let thing = world.spawn((Position { x: 3, y: 4, z: 0 },)).id();

    let items = |world: &mut World| world.query::<&GroundItem>().iter(world).count();
    let monsters = |world: &mut World| {
        world
            .query::<&cdda_sim::creature::Creature>()
            .iter(world)
            .count()
    };

    assert_eq!(items(&mut world), 0);
    assert_eq!(monsters(&mut world), 0);

    // Give it an item-ness. No new container and no new registration: the query that asks for items
    // now sees it.
    world.entity_mut(thing).insert(GroundItem {
        kind: ItemTypeId(1),
        charges: 1,
        age: 0,
        burnt: 0,
        damage: 0,
        burn: None,
    });
    assert_eq!(items(&mut world), 1);
    assert_eq!(monsters(&mut world), 0);

    // Take it away again: the same entity, no longer an item.
    world.entity_mut(thing).remove::<GroundItem>();
    assert_eq!(items(&mut world), 0);
    assert!(
        world.get_entity(thing).is_ok(),
        "removing a component must not destroy the entity"
    );
}

/// A system sees what it asks for and nothing else: asking for creatures must never hand back a
/// thing that merely stands on a square.
#[test]
fn a_query_never_sees_what_it_did_not_ask_for() {
    let mut world = populated();
    let mut query = world.query::<(Entity, &cdda_sim::creature::Creature)>();
    let seen: Vec<Entity> = query.iter(&world).map(|(entity, _)| entity).collect();
    assert_eq!(seen.len(), 1, "the fixture holds exactly one creature");
    let with_squares = positioned(&mut world).len();
    assert!(
        seen.len() < with_squares,
        "the creature query returned as many things as there are squares, so it is not filtering"
    );
}

/// The store has no hidden shelves: every live entity carries at least one component, so no state
/// can exist that is not a component on a thing.
#[test]
fn no_entity_is_an_empty_shelf() {
    let mut world = populated();
    PositionIndex::rebuild(&mut world);
    let empty: Vec<Entity> = world
        .iter_entities()
        .filter(|entity| entity.archetype().components().is_empty())
        .map(|entity| entity.id())
        .collect();
    assert!(
        empty.is_empty(),
        "{} entities carry no components at all, so something was spawned without saying what it \
         is: {empty:?}",
        empty.len()
    );
}

// ---------------------------------------------------------------------------------------------
// Ledgers. These read the sources because the thing they count has no runtime surface: a
// `Resource` is a type, not a value the world can enumerate, and a pointer is written in code.
// ---------------------------------------------------------------------------------------------

/// The globals that stand beside the store today. Each should become an entity with a marker
/// component, so this list may only ever shrink.
const GLOBALS: &[&str] = &[];

fn workspace() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("the workspace root is two levels above crates/sim")
}

/// Every `.rs` file under `crates/*/src`, which is where the port's own code lives.
fn sources() -> Vec<(String, String)> {
    fn walk(dir: &std::path::Path, out: &mut Vec<(String, String)>) {
        for entry in std::fs::read_dir(dir)
            .expect("a readable directory")
            .flatten()
        {
            let path = entry.path();
            if path.is_dir() {
                walk(&path, out);
            } else if path.extension().is_some_and(|extension| extension == "rs") {
                out.push((
                    path.display().to_string(),
                    std::fs::read_to_string(&path).expect("a readable source file"),
                ));
            }
        }
    }

    let mut out = Vec::new();
    for crate_dir in std::fs::read_dir(workspace().join("crates"))
        .expect("the crates directory")
        .flatten()
    {
        let src = crate_dir.path().join("src");
        if src.is_dir() {
            walk(&src, &mut out);
        }
    }
    assert!(
        out.len() > 40,
        "the scan found {} source files, which is too few to be the workspace",
        out.len()
    );
    out
}

/// The names of every type declared with `#[derive(...)]` naming `kind`, and where it was found.
fn declared_types(kind: &str) -> std::collections::BTreeMap<String, String> {
    let mut found = std::collections::BTreeMap::new();
    for (path, text) in sources() {
        let lines: Vec<&str> = text.lines().collect();
        for (index, line) in lines.iter().enumerate() {
            if !(line.trim_start().starts_with("#[derive(") && line.contains(kind)) {
                continue;
            }
            for (offset, line) in lines.iter().skip(index).take(5).enumerate() {
                let candidate = line.trim_start();
                if let Some(name) = candidate
                    .strip_prefix("pub struct ")
                    .or_else(|| candidate.strip_prefix("struct "))
                {
                    let name: String = name
                        .chars()
                        .take_while(|character| character.is_alphanumeric() || *character == '_')
                        .collect();
                    found.insert(name, format!("{path}:{}", index + offset + 1));
                    break;
                }
            }
        }
    }
    found
}

/// A new global is a second place where the truth can live, which is the one thing the architecture
/// forbids. Removing one is progress: delete its line here in the same commit.
#[test]
fn no_global_stands_beside_the_store() {
    let found = declared_types("Resource");
    let expected: std::collections::BTreeSet<&str> = GLOBALS.iter().copied().collect();
    let actual: std::collections::BTreeSet<&str> = found.keys().map(String::as_str).collect();
    let mut message = String::new();
    for name in actual.difference(&expected) {
        let _ = writeln!(message, "\n  added `{name}` at {}", found[*name]);
    }
    for name in expected.difference(&actual) {
        let _ = writeln!(
            message,
            "\n  `{name}` is listed but no longer declared - if it became an entity, drop the line"
        );
    }
    assert!(
        message.is_empty(),
        "the set of globals changed:\n{message}\n\
         The architecture is one store. A global is a singleton that has not moved yet, so this \
         list may only shrink."
    );
    assert_eq!(
        found.len(),
        GLOBALS.len(),
        "the ledger is not seeing the globals it expects, so it is checking nothing"
    );
}

/// An id is a number the store resolves, never a pointer into it. The simulation is the part that
/// must stay free of shared ownership and raw pointers.
#[test]
fn the_simulation_holds_no_shared_or_raw_pointers() {
    let mut offenders = Vec::new();
    for (path, text) in sources() {
        if !path.contains("/sim/src/") {
            continue;
        }
        for (index, line) in text.lines().enumerate() {
            let code = line.split("//").next().unwrap_or("");
            for needle in ["Arc<", "Rc<", "*const ", "*mut ", "RefCell<"] {
                if code.contains(needle) {
                    offenders.push(format!("{}:{}: {}", path, index + 1, line.trim()));
                }
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "the simulation reaches for a pointer or shared ownership:\n{}\n\
         References are entity ids that the store resolves.",
        offenders.join("\n")
    );
}

/// The per-square arrays the simulation still holds, by type name. Like the global ledger this may
/// only shrink: a result that is a property of a place is a component on the place, and a square is
/// an entity like anything else.
const PER_SQUARE_ARRAYS: &[&str] = &["MapFacts"];

/// The index is the one array of the architecture: every square to the things standing on it. It is
/// derived, it is never authoritative, and a gate below proves it agrees with the store. It is named
/// here so the scan can skip it rather than needing a ledger line that excuses it.
const THE_INDEX: &str = "PositionIndex";

/// The types in the simulation that hold a collection keyed by a square.
fn per_square_types() -> std::collections::BTreeMap<String, String> {
    let needles = [
        "BTreeMap<(i32, i32, i32)",
        "HashMap<(i32, i32, i32)",
        "Vec<Vec<",
        "Vec<(i32, i32, i32)>",
    ];
    let mut found = std::collections::BTreeMap::new();
    for (path, text) in sources() {
        if !path.contains("/sim/src/") {
            continue;
        }
        let mut current: Option<(String, usize)> = None;
        let mut depth = 0_usize;
        for (index, line) in text.lines().enumerate() {
            let code = line.split("//").next().unwrap_or("");
            if let Some(after) = code.split("struct ").nth(1)
                && current.is_none()
            {
                let name: String = after
                    .chars()
                    .take_while(|c| c.is_alphanumeric() || *c == '_')
                    .collect();
                if !name.is_empty() && name != THE_INDEX {
                    current = Some((name, index + 1));
                }
            }
            if let Some((name, line_number)) = &current {
                for needle in needles {
                    if code.contains(needle) {
                        found.insert(
                            name.clone(),
                            format!("{path}:{line_number} holds `{needle}`"),
                        );
                    }
                }
                depth += code.matches('{').count();
                depth = depth.saturating_sub(code.matches('}').count());
                if depth == 0 && code.contains('}') {
                    current = None;
                }
            }
        }
    }
    found
}

#[test]
fn no_component_holds_a_per_square_array() {
    let found = per_square_types();
    let expected: std::collections::BTreeSet<&str> = PER_SQUARE_ARRAYS.iter().copied().collect();
    let actual: std::collections::BTreeSet<&str> = found.keys().map(String::as_str).collect();
    let mut message = String::new();
    for name in actual.difference(&expected) {
        let _ = writeln!(message, "\n  added `{name}` at {}", found[*name]);
    }
    for name in expected.difference(&actual) {
        let _ = writeln!(
            message,
            "\n  `{name}` is listed but no longer holds one - if it moved onto entities, drop the line"
        );
    }
    assert!(
        message.is_empty(),
        "the set of per-square arrays changed:\n{message}\n\
         A square's answer is a component on the square. An array keyed by position is a second \
         place to look for the truth, and this list may only shrink."
    );
    assert_eq!(
        found.len(),
        PER_SQUARE_ARRAYS.len(),
        "the ledger is not seeing the arrays it expects, so it is checking nothing"
    );
}

#[test]
fn the_per_square_ledger_is_not_empty_by_accident() {
    // The gate above passes trivially if the scan finds nothing at all, so this proves it can see.
    let found = per_square_types();
    assert!(
        found.contains_key("MapFacts"),
        "the scan must at least find the array that is still there: {found:?}"
    );
}

/// Installing data twice leaves one entity holding it, for every registry the store has.
///
/// The registries are singletons - an entity with a marker beside the data - and the whole point of the pattern is that
/// there is exactly one of each. A `set_` that spawned a second entity would give the store two answers to "which number
/// is `fd_fire`", and every caller would keep whichever it happened to look up first.
#[test]
fn installing_a_registry_twice_leaves_one_of_it() {
    use bevy_ecs::prelude::*;

    let mut world = World::new();
    cdda_sim::field_registry::set_field_names(&mut world, &[("fd_null", 0, 3), ("fd_fire", 13, 3)]);
    cdda_sim::field_registry::set_field_names(&mut world, &[("fd_null", 0, 3), ("fd_fire", 13, 3)]);
    cdda_sim::monster_registry::set_monster_types(
        &mut world,
        &[cdda_sim::monster_registry::MonsterKind {
            id: "mon_zombie",
            hp: 80,
        }],
    );
    cdda_sim::monster_registry::set_monster_types(
        &mut world,
        &[cdda_sim::monster_registry::MonsterKind {
            id: "mon_zombie",
            hp: 80,
        }],
    );
    let effect =
        cdda_json::parse(br#"{"id":"stunned","max_duration":"100 turns"}"#).expect("valid json");
    let cdda_json::Value::Object(entry) = effect else {
        panic!("an effect type is an object");
    };
    let kind = cdda_content::effects::read_one("stunned", &entry);
    cdda_sim::effect_registry::set_effect_types(&mut world, std::slice::from_ref(&kind));
    cdda_sim::effect_registry::set_effect_types(&mut world, std::slice::from_ref(&kind));

    let fields = world
        .query_filtered::<Entity, With<cdda_sim::field_registry::TheFieldNames>>()
        .iter(&world)
        .count();
    let monsters = world
        .query_filtered::<Entity, With<cdda_sim::monster_registry::TheMonsterTypes>>()
        .iter(&world)
        .count();
    let effects = world
        .query_filtered::<Entity, With<cdda_sim::effect_registry::TheEffectTypes>>()
        .iter(&world)
        .count();
    assert_eq!(
        (fields, monsters, effects),
        (1, 1, 1),
        "one entity per registry, after two installs each"
    );
    assert_eq!(cdda_sim::field_registry::count(&mut world), 2);
    assert_eq!(cdda_sim::monster_registry::count(&mut world), 1);
    assert_eq!(cdda_sim::effect_registry::count(&mut world), 1);
}

/// A drawn chunk lands as terrain on its own squares, and a square keeps exactly one ground.
///
/// Reference: `mapgen::draw_map` (`mapgen.cpp`) writes the chunk's terrain where the chunk was placed. The port writes into the store everything
/// else lives in, so a generated chunk is entities rather than an array beside them - and the write must read back through the same index.
#[test]
fn a_drawn_chunk_lands_as_terrain_on_its_own_squares() {
    use cdda_core_types::level::Level;

    let mut world = World::new();
    let mut level = Level::new();
    for x in 0..6 {
        for y in 0..6 {
            level.set_terrain(x, y, 0, 0);
        }
    }
    cdda_sim::load_level(&mut world, &level);
    let before = cdda_sim::square::extent(&world);

    let symbol = |text: &str| match cdda_json::parse(text.as_bytes()).expect("json") {
        cdda_json::Value::Object(found) => found,
        _ => unreachable!("an object"),
    };
    let lookup = |name: &str| match name {
        "t_grass" => Some(symbol(r#"{"id": "t_grass"}"#)),
        "t_wall" => Some(symbol(r#"{"id": "t_wall"}"#)),
        _ => None,
    };
    let symbols = symbol(r#"{"A": "t_grass", "B": "t_wall"}"#);
    let drawn = cdda_content::mapgen::draw_chunk(
        &["AB".to_owned(), " A".to_owned()],
        Some(&symbols),
        &[],
        &lookup,
        Some("t_dirt"),
    )
    .expect("drawn");
    assert_eq!(cdda_sim::mapgen_place::drawn_extent(&drawn), (2, 2));

    let code_of = |terrain: &str| match terrain {
        "t_grass" => Some(1),
        "t_wall" => Some(2),
        _ => None,
    };
    let written = cdda_sim::mapgen_place::place_drawn(&mut world, &drawn, (1, 1, 0), &code_of);
    assert_eq!(written, 3, "three symbols had terrain under them");

    assert_eq!(
        cdda_sim::square::terrain_at(&world, (1, 1, 0)),
        Some(1),
        "A is grass"
    );
    assert_eq!(
        cdda_sim::square::terrain_at(&world, (2, 1, 0)),
        Some(2),
        "B is wall"
    );
    assert_eq!(
        cdda_sim::square::terrain_at(&world, (2, 2, 0)),
        Some(1),
        "the second row's A, one column right of the blank"
    );
    assert_eq!(
        cdda_sim::square::terrain_at(&world, (1, 2, 0)),
        Some(0),
        "the blank square kept the level's own terrain"
    );
    assert_eq!(
        cdda_sim::square::extent(&world),
        before,
        "no square was added"
    );

    // And the store holds exactly one ground per square: a write that made a second one would read back whichever it found first.
    let mut grounds: std::collections::BTreeMap<(i32, i32, i32), usize> =
        std::collections::BTreeMap::new();
    let mut query = world.query::<(&cdda_sim::Position, Option<&cdda_sim::square::Terrain>)>();
    for (position, terrain) in query.iter(&world) {
        if terrain.is_some() {
            *grounds
                .entry((position.x, position.y, position.z))
                .or_insert(0) += 1;
        }
    }
    assert_eq!(
        grounds.values().filter(|count| **count > 1).count(),
        0,
        "one ground per square"
    );
}

/// State that outlives every world: a debt register, not an allowance.
///
/// The architecture keeps the truth in the store, so anything here is a fact living somewhere a world cannot see it - a counter a test reads, or the intern table a
/// `StringId` needs to make equal strings the same address. Each line says what it holds and what would have to exist for it to be a component instead. Adding a line
/// is a decision; removing one is progress, and the ledger must shrink in the same commit.
const PROCESS_STATE: &[(&str, &str, &str)] = &[
    (
        "crates/core-types/src/ids.rs",
        "NEXT",
        "a version stamp for a registry's cached index; two registries of one type must not agree on it",
    ),
    (
        "crates/core-types/src/ids.rs",
        "TABLE",
        "the intern table: every equal `StringId` shares one address, which a per-world table could not promise across worlds",
    ),
    (
        "crates/cli/src/lib.rs",
        "ITEMS",
        "the item table, parsed once for the process: two scenarios in one run install the same data into two worlds",
    ),
    (
        "crates/cli/src/lib.rs",
        "MONSTER_PARSES",
        "a parse counter a test reads, to prove the monster dump is read once",
    ),
    (
        "crates/cli/src/lib.rs",
        "TYPES",
        "the monster table, parsed once for the process, for the same reason as `ITEMS`",
    ),
    (
        "crates/content/src/items.rs",
        "LOADS",
        "a load counter a test reads, to prove the item tree is deserialized once",
    ),
    (
        "crates/content/src/lib.rs",
        "DISABLED",
        "the parse memo's kill switch, read once for the process: `CDDA_NO_PARSE_MEMO=1` measures the parser itself",
    ),
    (
        "crates/content/src/lib.rs",
        "MEMO",
        "the parse memo: the whole JSON tree parsed once for the process, which is what makes a gate that loads items and then monsters affordable",
    ),
    (
        "crates/content/src/lib.rs",
        "PARSES",
        "a parse counter a test reads, to prove the memo hits",
    ),
    (
        "crates/sim/src/inspect.rs",
        "FINGERPRINTS",
        "a walk counter a test reads, to prove an unwatched turn pays nothing",
    ),
    (
        "crates/sim/src/turn.rs",
        "SCHEDULES",
        "a build counter a test reads, to prove a turn reuses its schedule",
    ),
];

/// Every process-wide piece of mutable state in the workspace, by file and name.
///
/// Three shapes: a `thread_local!`, a `static mut`, and a `static` holding an atomic or a lock - the last one is how a function-local `static` in the reference is
/// written safely here, and it is just as much of a second store.
fn process_state() -> std::collections::BTreeMap<(String, String), usize> {
    let root = workspace();
    let mut found = std::collections::BTreeMap::new();
    for (path, text) in sources() {
        let file = path
            .strip_prefix(&format!("{}/", root.display()))
            .unwrap_or(&path)
            .to_owned();
        let lines: Vec<&str> = text.lines().collect();
        for (index, line) in lines.iter().enumerate() {
            let trimmed = line.trim_start();
            let name = if trimmed.starts_with("static mut ") {
                trimmed.split_whitespace().nth(2).map(str::to_owned)
            } else if trimmed.contains("Atomic") || trimmed.contains("OnceLock") {
                trimmed
                    .strip_prefix("static ")
                    .and_then(|rest| rest.split(':').next())
                    .map(|name| name.trim().to_owned())
            } else {
                None
            };
            if let Some(name) = name {
                found.insert((file.clone(), name), index + 1);
            }
            // A `thread_local!` names its entries one per line inside the block.
            if trimmed.starts_with("std::thread_local!") || trimmed == "thread_local! {" {
                for (offset, inner) in lines.iter().skip(index + 1).take(8).enumerate() {
                    let inner = inner.trim();
                    if inner == "}" {
                        break;
                    }
                    if let Some(name) = inner
                        .strip_prefix("static ")
                        .and_then(|rest| rest.split([':', ' ']).next())
                        .filter(|name| !name.is_empty())
                    {
                        found.insert((file.clone(), name.to_owned()), index + offset + 2);
                    }
                }
            }
        }
    }
    found
}

/// **A new global is a second store.** This gate names every one that exists, so a new one cannot arrive unnoticed, and a line that stops being true is deleted rather
/// than left to rot.
#[test]
fn process_state_is_ledgered_and_the_ledger_may_only_shrink() {
    let found = process_state();
    let expected: std::collections::BTreeSet<(String, String)> = PROCESS_STATE
        .iter()
        .map(|(path, name, _)| ((*path).to_owned(), (*name).to_owned()))
        .collect();
    let actual: std::collections::BTreeSet<(String, String)> = found.keys().cloned().collect();

    let mut message = String::new();
    for (path, name) in actual.difference(&expected) {
        let _ = writeln!(
            message,
            "\n  added `{name}` at {path}:{} - if it must exist, add a line to PROCESS_STATE saying why it cannot be a component",
            found[&(path.clone(), name.clone())]
        );
    }
    for (path, name) in expected.difference(&actual) {
        let _ = writeln!(
            message,
            "\n  `{name}` ({path}) is listed but no longer exists - if it moved into the store, delete the line"
        );
    }
    assert!(
        message.is_empty(),
        "the process-wide state changed:\n{message}"
    );
    assert_eq!(
        found.len(),
        PROCESS_STATE.len(),
        "the scan found {} entries and the ledger lists {}: the gate is not seeing what it claims to check",
        found.len(),
        PROCESS_STATE.len()
    );
    // A ledger of five with a scan that finds nothing would pass the two assertions above if both sides were empty, which is exactly how the globals gate sat
    // green for rounds while checking `0 == 0`.
    assert!(
        found.len() >= 5,
        "the scan found only {} entries, which is too few to be the workspace",
        found.len()
    );
}
