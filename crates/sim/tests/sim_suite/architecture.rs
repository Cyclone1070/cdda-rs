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
        expected, actual,
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
    let thing = world
        .spawn((Position {
            x: 3,
            y: 4,
            z: 0,
        },))
        .id();

    let items = |world: &mut World| world.query::<&GroundItem>().iter(world).count();
    let monsters = |world: &mut World| world.query::<&cdda_sim::creature::Creature>().iter(world).count();

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
        for entry in std::fs::read_dir(dir).expect("a readable directory").flatten() {
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
    cdda_sim::field_registry::set_field_names(&mut world, &[("fd_null", 0), ("fd_fire", 13)]);
    cdda_sim::field_registry::set_field_names(&mut world, &[("fd_null", 0), ("fd_fire", 13)]);
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
    let effect = cdda_json::parse(br#"{"id":"stunned","max_duration":"100 turns"}"#)
        .expect("valid json");
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
    assert_eq!((fields, monsters, effects), (1, 1, 1), "one entity per registry, after two installs each");
    assert_eq!(cdda_sim::field_registry::count(&mut world), 2);
    assert_eq!(cdda_sim::monster_registry::count(&mut world), 1);
    assert_eq!(cdda_sim::effect_registry::count(&mut world), 1);
}
