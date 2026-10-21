//! Identifiers, translated from the reference's own tests.
//!
//! Reference: `references/Cataclysm-DDA/src/string_id.h`, `src/string_id.cpp`,
//! `src/int_id.h`, `src/generic_factory.h` and
//! `references/Cataclysm-DDA/tests/string_ids_test.cpp`. Reference test names
//! are quoted above each case; reference line numbers are cited where a single
//! rule is pinned.

// The ids carry a cached index in a `Cell`, so clippy calls them mutable key
// types. Hash, equality and ordering read only the interned identity, which
// never changes, so they are sound as keys - and keys are exactly what the
// reference puts them in (its `string_id` has the same mutable cache inside a
// map key).
#![allow(clippy::mutable_key_type)]

use cdda_core_types::ids::{Identified, IntId, Lexicographic, Registry, StringId};

/// Stands in for the reference's anonymous `struct test_obj {}` marker types.
struct TestObj;

#[test]
fn equal_strings_intern_to_one_stable_string() {
    // string_ids_test.cpp:46-52 (`string_ids_intern_test`): `&id.str() == &string_id<test_obj>( id.str() ).str()`.
    // string_id.h:262-264 documents `str()` as returning the interned string, so
    // equal ids must hand out one and the same string, not a copy each.
    let a: StringId<TestObj> = StringId::new("test_id0");
    let b: StringId<TestObj> = StringId::new("test_id0");

    assert_eq!(a, b);
    assert_eq!(a.as_str(), "test_id0");
    assert!(
        std::ptr::eq(a.as_str(), b.as_str()),
        "interning must share one string between equal ids"
    );

    // The same check at scale, which is what the reference's loop is for: a lot
    // of ids make the intern map grow, and each text must still be one string.
    let many: Vec<StringId<TestObj>> = (0..1000u16)
        .map(|i| StringId::new(&format!("intern_bulk_{i}")))
        .collect();
    for id in &many {
        let same = StringId::<TestObj>::new(id.as_str());
        assert_eq!(*id, same);
        assert!(std::ptr::eq(id.as_str(), same.as_str()));
    }
}

#[test]
fn ids_compare_by_text_and_the_default_id_is_empty() {
    // string_ids_test.cpp:23-35 (`static_string_ids_equality_test`).
    let smoke: StringId<TestObj> = StringId::new("fd_smoke");
    assert_eq!(smoke, StringId::new("fd_smoke"));
    assert_ne!(smoke, StringId::new("fd_smoke1"));
    assert!(!smoke.is_empty());

    // string_id.h:223-228: the default constructor builds the empty id, and
    // string_id.h:300-307: emptiness is only about the string being empty.
    let empty: StringId<TestObj> = StringId::default();
    assert!(empty.is_empty());
    assert_eq!(empty, StringId::new(""));
}

#[test]
fn ord_follows_intern_order_not_text_order() {
    // string_id.h:229-236: `operator<` compares the interned identity, i.e. the
    // intern id, so the order is the order of first interning. string_id.h:231
    // says so outright: a total order is guaranteed, a restart-stable one is not.
    let b: StringId<TestObj> = StringId::new("cycle2_b");
    let a: StringId<TestObj> = StringId::new("cycle2_a");

    assert!(b < a, "the id interned first sorts first");
    assert!(a >= b);
    // string_ids_intern_test:54-60: for two distinct ids exactly one order holds,
    // and an id never sorts before itself.
    assert_ne!(a, b);
    assert!((a < b) ^ (b < a));
    assert!(a >= a);
    assert_eq!(a.cmp(&a), core::cmp::Ordering::Equal);
}

#[test]
fn lexicographic_wrapper_sorts_by_text() {
    // string_id_sorting_test (`vector ids sorting`) sorts through `sorted_lex`,
    // which uses `LexCmp` (string_id.h:376-384, string_id_utils.h:26-38): by
    // text, not by intern order. Interning in reverse makes the two orders differ.
    let mut ids: Vec<Lexicographic<TestObj>> = Vec::new();
    for i in (0..10u8).rev() {
        ids.push(Lexicographic(StringId::new(&format!("sort_id_{i}"))));
    }
    ids.sort();

    for (i, id) in ids.iter().enumerate() {
        assert_eq!(id.0.as_str(), format!("sort_id_{i}"));
    }

    // `map sorting` in the reference: a map keyed by id is ordered by intern id,
    // and `sorted_lex` turns that into text order.
    let mut map: std::collections::BTreeMap<StringId<TestObj>, u8> =
        std::collections::BTreeMap::new();
    for i in (0..10u8).rev() {
        map.insert(StringId::new(&format!("sort_map_{i}")), i);
    }
    let mut by_text: Vec<Lexicographic<TestObj>> =
        map.keys().map(|id| Lexicographic(id.clone())).collect();
    by_text.sort();
    for (i, id) in by_text.iter().enumerate() {
        assert_eq!(id.0.as_str(), format!("sort_map_{i}"));
    }
}

#[test]
fn ids_work_as_map_and_set_keys() {
    // string_ids_collection_equality: sets built in different insertion orders
    // are equal, vectors of the same ids become equal once sorted.
    let mut hashed: std::collections::HashSet<StringId<TestObj>> = std::collections::HashSet::new();
    let mut ordered: std::collections::BTreeMap<StringId<TestObj>, i32> =
        std::collections::BTreeMap::new();
    for (i, text) in ["coll_test1", "coll_test3", "coll_test2"]
        .iter()
        .enumerate()
    {
        hashed.insert(StringId::new(text));
        let id = i32::try_from(i).unwrap_or(i32::MAX);
        ordered.insert(StringId::new(text), id);
    }

    let mut hashed_reordered: std::collections::HashSet<StringId<TestObj>> =
        std::collections::HashSet::new();
    for text in ["coll_test2", "coll_test1", "coll_test3"] {
        hashed_reordered.insert(StringId::new(text));
    }
    assert_eq!(hashed, hashed_reordered);
    assert_eq!(ordered.len(), 3);

    let mut vec1: Vec<StringId<TestObj>> = ["coll_a", "coll_c", "coll_b"]
        .iter()
        .map(|t| StringId::new(t))
        .collect();
    let mut vec2: Vec<StringId<TestObj>> = ["coll_b", "coll_a", "coll_c"]
        .iter()
        .map(|t| StringId::new(t))
        .collect();
    assert_ne!(vec1, vec2);
    vec1.sort();
    vec2.sort();
    assert_eq!(vec1, vec2);
}

#[test]
fn equal_ids_hash_to_the_same_value() {
    // string_id.h:366-374: the hash is forwarded from the interned identity, so
    // it is one integer hash and it agrees with equality.
    use core::hash::{Hash, Hasher};
    let hash_of = |id: &StringId<TestObj>| {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        id.hash(&mut hasher);
        hasher.finish()
    };

    let a: StringId<TestObj> = StringId::new("hash_test_id");
    let b: StringId<TestObj> = StringId::new("hash_test_id");
    assert_eq!(a, b);
    assert_eq!(hash_of(&a), hash_of(&b));
    assert_eq!(hash_of(&a), hash_of(&a), "hashing is deterministic");
}

#[test]
fn int_id_is_an_ordinal_and_zero_is_not_special() {
    // int_id.h:28-46: the constructor is explicit and takes an int, the default
    // is 0, and "0 as id is just as normal as any other integer value".
    let zero = IntId::<TestObj>::default();
    assert_eq!(zero.to_i(), 0);
    assert_eq!(IntId::<TestObj>::new(0), zero);

    let three = IntId::<TestObj>::new(3);
    assert_ne!(three, zero);
    assert_eq!(three.to_i(), 3);

    // int_id.h:67-78: ordering and equality are the plain integer ones.
    assert!(IntId::<TestObj>::new(2) < three);
    assert_eq!(three.cmp(&IntId::new(3)), core::cmp::Ordering::Equal);
    assert_eq!(three, IntId::<TestObj>::new(3));
}

/// A registry entry, shaped like the reference's loaded objects: the id lives
/// inside the object and is what the registry indexes it by.
#[derive(Default)]
struct Thing {
    id: StringId<Thing>,
    name: &'static str,
}

fn thing(name: &'static str) -> Thing {
    Thing {
        id: StringId::new(name),
        name,
    }
}

impl Identified for Thing {
    fn id(&self) -> &StringId<Self> {
        &self.id
    }
}

#[test]
fn validity_is_asked_of_the_registry_not_of_the_id() {
    // generic_factory.h:496-511: an id is valid when the factory holds something
    // for it. For int ids the test is `to_i() < list.size()` after casting to
    // size_t, so a negative index is invalid rather than out of bounds.
    let mut registry: Registry<Thing> = Registry::new();
    registry.insert(thing("val_mon_a"));
    registry.insert(thing("val_mon_b"));

    assert!(StringId::<Thing>::new("val_mon_a").is_valid(&registry));
    assert!(!StringId::<Thing>::new("val_mon_none").is_valid(&registry));

    assert!(IntId::<Thing>::new(0).is_valid(&registry));
    assert!(IntId::<Thing>::new(1).is_valid(&registry));
    assert!(!IntId::<Thing>::new(2).is_valid(&registry));
    assert!(!IntId::<Thing>::new(-1).is_valid(&registry));

    // generic_factory.h:471-495: an invalid id is not a panic and not an
    // assertion - the factory reports and hands back its default-constructed
    // dummy object.
    assert_eq!(IntId::<Thing>::new(0).obj(&registry).name, "val_mon_a");
    assert_eq!(IntId::<Thing>::new(1).obj(&registry).name, "val_mon_b");
    assert_eq!(IntId::<Thing>::new(9).obj(&registry).name, "");
    assert_eq!(IntId::<Thing>::new(-1).obj(&registry).name, "");
    assert_eq!(
        StringId::<Thing>::new("val_mon_none").obj(&registry).name,
        ""
    );
}

#[test]
fn the_same_int_id_means_another_object_after_a_reload() {
    // string_id.h:59-62: an int id can point at a different entity after the
    // data reloads and "there is no way to tell" from the id. Two registries
    // holding the same objects in a different order are that situation.
    let mut first: Registry<Thing> = Registry::new();
    first.insert(thing("reload_a"));
    first.insert(thing("reload_b"));

    let mut second: Registry<Thing> = Registry::new();
    second.insert(thing("reload_b"));
    second.insert(thing("reload_a"));

    let a = StringId::<Thing>::new("reload_a");
    assert_eq!(a.id_or(&first, IntId::new(-1)), IntId::new(0));
    assert_eq!(a.id_or(&second, IntId::new(-1)), IntId::new(1));
    assert_eq!(a.id_or(&first, IntId::new(-1)).obj(&first).name, "reload_a");
    assert_eq!(
        a.id_or(&second, IntId::new(-1)).obj(&second).name,
        "reload_a"
    );

    // One and the same integer, two different objects.
    assert_eq!(IntId::<Thing>::new(0).obj(&first).name, "reload_a");
    assert_eq!(IntId::<Thing>::new(0).obj(&second).name, "reload_b");

    // The string id is what survives the reload unchanged, and it converts back.
    assert_eq!(IntId::<Thing>::new(0).id(&first), a);
    assert_ne!(IntId::<Thing>::new(0).id(&second), a);
    // A missing id falls back without touching the registry (string_id.h:277-282).
    assert_eq!(
        StringId::<Thing>::new("reload_missing").id_or(&first, IntId::new(-1)),
        IntId::new(-1)
    );
}

#[test]
fn the_cached_index_is_replaced_after_a_reload() {
    // string_id.h:349-360 keeps `_version` next to `_cid`; generic_factory.h:161-176
    // compares the two and looks the id up again when they disagree. reset()
    // (generic_factory.h:434-444) clears the data and bumps the version, which is
    // how a reload shows up.
    let mut registry: Registry<Thing> = Registry::new();
    registry.insert(thing("cache_a"));
    registry.insert(thing("cache_b"));

    let a = StringId::<Thing>::new("cache_a");
    assert_eq!(a.cached_index(), None, "no index cached yet");
    assert_eq!(a.id_or(&registry, IntId::new(-1)), IntId::new(0));
    assert_eq!(a.cached_index(), Some(0));

    registry.reset();
    // The raw cached integer is still there - only the version moved on, and the
    // version is what decides whether the cache may be used.
    assert_eq!(a.cached_index(), Some(0));

    registry.insert(thing("cache_b"));
    registry.insert(thing("cache_a"));
    assert_eq!(
        a.id_or(&registry, IntId::new(-1)),
        IntId::new(1),
        "a stale cached index must not win"
    );
    assert_eq!(a.cached_index(), Some(1));
    assert_eq!(a.obj(&registry).name, "cache_a");
}

#[test]
fn a_matching_version_skips_the_map_lookup() {
    // generic_factory.h:162-165: when the cached version matches, the cached
    // index is returned and the id map is not touched.
    let mut registry: Registry<Thing> = Registry::new();
    registry.insert(thing("probe_a"));

    let a = StringId::<Thing>::new("probe_a");
    let before = registry.map_probes();

    assert_eq!(a.id_or(&registry, IntId::new(-1)), IntId::new(0));
    assert_eq!(
        registry.map_probes(),
        before + 1,
        "first resolve probes the map"
    );

    assert_eq!(a.id_or(&registry, IntId::new(-1)), IntId::new(0));
    assert_eq!(
        registry.map_probes(),
        before + 1,
        "second resolve answers from the cached index"
    );
}

#[test]
fn finalize_stamps_every_registered_id() {
    // generic_factory.h:403-410: finalize bumps the version and then writes the
    // current index into every entry's own id, so each cached index is current
    // again after a load. Inserting one more object invalidates all of them
    // (generic_factory.h:374-380), which is what the probe count shows.
    let mut registry: Registry<Thing> = Registry::new();
    registry.insert(thing("finalize_a"));
    registry.finalize();
    registry.insert(thing("finalize_b"));

    let probes = registry.map_probes();
    let own = registry.obj(IntId::new(0)).id();
    assert_eq!(own.id_or(&registry, IntId::new(-1)), IntId::new(0));
    assert_eq!(
        registry.map_probes(),
        probes + 1,
        "the insert made this entry's cached index stale"
    );

    registry.finalize();
    let probes = registry.map_probes();
    let own = registry.obj(IntId::new(0)).id();
    assert_eq!(own.id_or(&registry, IntId::new(-1)), IntId::new(0));
    assert_eq!(
        registry.map_probes(),
        probes,
        "finalize stamped every entry, so no probe is needed"
    );
}

#[test]
fn a_cached_miss_is_invalidated_by_the_next_data_change() {
    // generic_factory.h:167-172 caches the *absence* of an id, and
    // generic_factory.h:375-379 explains why insert must then bump the version:
    // a cached "not found yet" would otherwise survive the object arriving.
    let mut registry: Registry<Thing> = Registry::new();
    registry.insert(thing("miss_a"));

    let late = StringId::<Thing>::new("miss_late");
    let probes = registry.map_probes();
    assert!(!late.is_valid(&registry));
    assert_eq!(registry.map_probes(), probes + 1, "the miss is looked up");

    assert!(!late.is_valid(&registry));
    assert_eq!(registry.map_probes(), probes + 1, "the cached miss answers");

    registry.insert(thing("miss_late"));
    assert!(late.is_valid(&registry), "the cached miss must not stick");
    assert_eq!(late.cached_index(), Some(1));
    assert_eq!(late.obj(&registry).name, "miss_late");
}

#[test]
fn a_duplicate_id_keeps_its_index_and_takes_the_new_object() {
    // generic_factory.h:381-390: an id that is already known keeps its index and
    // the stored object is replaced in place.
    let mut registry: Registry<Thing> = Registry::new();
    let first = registry.insert(thing("dup_a"));
    let second = registry.insert(thing("dup_b"));
    let again = registry.insert(Thing {
        id: StringId::new("dup_a"),
        name: "dup_a_replaced",
    });

    assert_eq!(first, IntId::new(0));
    assert_eq!(second, IntId::new(1));
    assert_eq!(again, first, "a duplicate keeps the original index");
    assert_eq!(registry.obj(first).name, "dup_a_replaced");
    assert_eq!(registry.obj(second).name, "dup_b");
}

#[test]
fn int_ids_are_ordered_and_hashed_by_index() {
    // int_id.h:66-78 and int_id.h:186-192: ordering and hashing are the plain
    // integer's, which is why int ids are the fast key where they may be used.
    use std::collections::{BTreeMap, HashMap};

    let mut ordered: BTreeMap<IntId<TestObj>, &str> = BTreeMap::new();
    let mut hashed: HashMap<IntId<TestObj>, &str> = HashMap::new();
    for (i, name) in ["by_index_c", "by_index_a", "by_index_b"]
        .iter()
        .enumerate()
    {
        let index = i32::try_from(i).unwrap_or(i32::MAX);
        ordered.insert(IntId::new(index), name);
        hashed.insert(IntId::new(index), name);
    }

    let keys: Vec<i32> = ordered.keys().map(IntId::to_i).collect();
    assert_eq!(
        keys,
        vec![0, 1, 2],
        "integer order, not insertion or text order"
    );
    assert_eq!(hashed.get(&IntId::<TestObj>::new(1)), Some(&"by_index_a"));
    assert_eq!(hashed.get(&IntId::<TestObj>::new(9)), None);
}

#[test]
fn invalid_int_ids_resolve_to_the_dummy_entry() {
    // generic_factory.h:529-534: convert(int_id) is `obj(id).id`, so an invalid
    // index yields the *dummy* object's id - the empty one - not the caller's.
    let mut registry: Registry<Thing> = Registry::new();
    registry.insert(thing(""));

    assert_eq!(
        IntId::<Thing>::new(-1).id(&registry),
        StringId::<Thing>::default()
    );

    // string_id.h:223-228: "Every string (including the empty one) may be a
    // valid id" - so registering the empty id makes it resolve.
    assert!(StringId::<Thing>::default().is_valid(&registry));
    assert_eq!(
        StringId::<Thing>::default().id_or(&registry, IntId::new(-1)),
        IntId::new(0)
    );
}

#[test]
fn insert_stamps_the_stored_objects_own_id() {
    // generic_factory.h:386 and :394: insert writes the index into the stored
    // entry's own id, so that entry answers from its cache straight away.
    let mut registry: Registry<Thing> = Registry::new();
    registry.insert(thing("stamp_a"));

    let probes = registry.map_probes();
    let own = registry.obj(IntId::new(0)).id();
    assert_eq!(own.cached_index(), Some(0));
    assert_eq!(own.id_or(&registry, IntId::new(-1)), IntId::new(0));
    assert_eq!(
        registry.map_probes(),
        probes,
        "the stored id was stamped by insert, so no probe is needed"
    );
}
