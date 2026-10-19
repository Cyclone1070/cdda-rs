//! Identifiers: typed string ids with a stable interned string, and typed
//! integer ids that are nothing but a position in a registry.
//!
//! Reference: `references/Cataclysm-DDA/src/string_id.h` (`string_id`,
//! `lexicographic`), `src/string_id.cpp` (the intern table), `src/int_id.h`
//! (`int_id`) and `src/generic_factory.h` (the version counter that invalidates
//! cached integers).
//!
//! Two ids for two jobs, as `string_id.h:56-74` puts it:
//!
//! - [`StringId`] can be kept across a data reload: its identity is the
//!   interned string, which never changes.
//! - [`IntId`] is an index into one [`Registry`]. It is **not** an identity. The
//!   same integer points at a different object after a reload, and nothing about
//!   the integer itself says which registry or which generation it came from.
//!   Turning one back into data therefore needs the registry as an argument.
//!
//! An [`IntId`] therefore cannot be resolved on its own: every use has to name
//! the registry it came from, so an index kept across a reload cannot quietly
//! address the wrong object.
//!
//! ```compile_fail
//! use cdda_core_types::ids::{Identified, IntId, StringId};
//!
//! #[derive(Default)]
//! struct Thing {
//!     id: StringId<Thing>,
//! }
//!
//! impl Identified for Thing {
//!     fn id(&self) -> &StringId<Self> {
//!         &self.id
//!     }
//! }
//!
//! let index = IntId::<Thing>::new(0);
//! let object = index.obj(); // error: the registry has to be passed in
//! ```
//!
//! Left out on purpose this cycle: the prototype registries, JSON loading, the
//! `dynamic` (non-interned) identity, `NULL_ID`/`is_null`, the `explicit
//! operator bool`, and the per-type aliases.

use core::cell::Cell;
use core::cmp::Ordering;
use core::fmt;
use core::hash::{Hash, Hasher};
use core::marker::PhantomData;
use std::collections::HashMap;
use std::sync::atomic::{AtomicI64, Ordering as AtomicOrdering};
use std::sync::{Mutex, MutexGuard, OnceLock};

/// The reference's `INVALID_VERSION` (`src/string_id.h:14`): a cached index
/// stamped with this version is never usable.
const INVALID_VERSION: i64 = -1;

/// The reference's `INVALID_CID` (`src/string_id.h:15`): "no index cached".
const INVALID_CID: i32 = -1;

/// The cached integer plus the registry version it was read under
/// (`src/string_id.h:351-353`).
#[derive(Clone, Copy)]
struct CachedIndex {
    /// The cached registry index, or [`INVALID_CID`].
    cid: i32,
    /// The registry version the index belongs to, or [`INVALID_VERSION`].
    version: i64,
}

/// A never-repeated modification token for a registry.
///
/// The reference stamps a cached index with its factory's own counter
/// (`src/string_id.h:351-353`, `src/generic_factory.h:140-151`), which is only
/// safe because there is exactly one factory per id type. A port can build two
/// registries of the same type, and two counters that happen to agree would let
/// one registry's cached index answer for the other. Taking the stamp from one
/// process-wide counter makes such a mix a cache miss instead of a wrong answer.
fn next_version() -> i64 {
    static NEXT: AtomicI64 = AtomicI64::new(0);
    NEXT.fetch_add(1, AtomicOrdering::Relaxed)
}

/// The interned strings, one process-wide table.
///
/// Reference: `src/string_id.cpp:11-21`. The reference keys an
/// `unordered_map<string, int>` by the string and keeps a vector of pointers to
/// the map's keys for the reverse lookup. Here the entry is the leaked
/// `&'static str`, which plays the same part as those pointers: every equal
/// string shares one address, so two equal ids hand out the same `&str`.
struct InternTable {
    /// String text to its intern id.
    ids: HashMap<&'static str, usize>,
    /// Intern id to string text; the id is the index.
    texts: Vec<&'static str>,
}

impl InternTable {
    fn new() -> Self {
        Self {
            ids: HashMap::new(),
            texts: Vec::new(),
        }
    }

    /// Interns `text` and returns its id, assigning the next free id on first
    /// sight (`src/string_id.cpp:23-32`).
    fn intern(&mut self, text: &str) -> usize {
        if let Some(&id) = self.ids.get(text) {
            return id;
        }
        let leaked: &'static str = Box::leak(text.to_owned().into_boxed_str());
        let id = self.texts.len();
        self.texts.push(leaked);
        self.ids.insert(leaked, id);
        id
    }

    /// The text behind an intern id (`src/string_id.cpp:49-52`).
    fn text(&self, id: usize) -> &'static str {
        self.texts.get(id).copied().unwrap_or_default()
    }
}

/// The process-wide intern table, built on first use.
///
/// The reference relies on a function-local static (`src/string_id.cpp:11-21`),
/// which is not thread safe. Rust requires the table to be shared safely, so it
/// sits behind a mutex; poisoning is recovered from because a panic while
/// interning cannot leave a half-written entry (the string is leaked only after
/// the map insertion is prepared).
fn intern_table() -> MutexGuard<'static, InternTable> {
    static TABLE: OnceLock<Mutex<InternTable>> = OnceLock::new();
    let table = TABLE.get_or_init(|| Mutex::new(InternTable::new()));
    match table.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

/// A typed string identifier of an object of type `T`.
///
/// Reference: `string_id<T>` (`src/string_id.h:198-364`). `T` is only a marker:
/// it keeps ids of different object types from being mixed up, exactly as the
/// reference's template parameter does (`src/string_id.h:44-48`).
///
/// The identity is the interned string, so this value stays meaningful across a
/// data reload. Any integer derived from it does not; see [`IntId`].
pub struct StringId<T> {
    /// Dense intern id, the comparison and hash key.
    intern: usize,
    /// The interned text, shared by every id with the same `intern`.
    text: &'static str,
    /// The cached registry index and its version (`src/string_id.h:349-355`).
    /// Interior mutability because the registry stamps it through `&self`,
    /// exactly as the reference's `mutable` members allow.
    cache: Cell<CachedIndex>,
    /// `T` is a marker only, so no `T` value is ever held.
    marker: PhantomData<fn() -> T>,
}

impl<T> StringId<T> {
    /// Interns `text` and wraps it as an id.
    ///
    /// Reference: the forwarding constructor (`src/string_id.h:217-222`), which
    /// is `explicit` on purpose so that literal ids stand out.
    #[must_use]
    pub fn new(text: &str) -> Self {
        let mut table = intern_table();
        let intern = table.intern(text);
        let text = table.text(intern);
        drop(table);
        Self {
            intern,
            text,
            cache: Cell::new(CachedIndex {
                cid: INVALID_CID,
                version: INVALID_VERSION,
            }),
            marker: PhantomData,
        }
    }

    /// The interned text (`src/string_id.h:254-264`).
    ///
    /// The reference returns a reference into the intern table, so equal ids
    /// yield one string with one address; the `'static` here is that promise.
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        self.text
    }

    /// Whether this is the empty id (`src/string_id.h:300-307`).
    ///
    /// The reference compares against the cached id of `""`, which is the same
    /// thing: only the empty string interns there.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
    }
}

impl<T> StringId<T> {
    /// The cached registry index, or `None` when nothing usable is cached.
    ///
    /// This is the reference's `_cid` (`src/string_id.h:353`) laid open: `Some`
    /// means an index was stamped by some registry, not that it is still the
    /// right one — a reload leaves it behind and the version is what rejects it.
    /// Callers never need this; it exists so the caching rule can be tested.
    #[must_use]
    pub fn cached_index(&self) -> Option<i32> {
        match self.cache.get().cid {
            INVALID_CID => None,
            cid => Some(cid),
        }
    }

    /// Stamps a cached index, the reference's `set_cid_version`
    /// (`src/string_id.h:357-360`).
    fn set_cached_index(&self, cid: i32, version: i64) {
        self.cache.set(CachedIndex { cid, version });
    }
}

impl<T> Default for StringId<T> {
    /// The empty id (`src/string_id.h:228`).
    fn default() -> Self {
        Self::new("")
    }
}

impl<T> Clone for StringId<T> {
    /// Copies the value, cached index included, like the reference's copy
    /// constructor. Not `Copy`: the cache is a [`Cell`], and the crate forbids
    /// `unsafe`, so the copy is written out by hand.
    fn clone(&self) -> Self {
        Self {
            intern: self.intern,
            text: self.text,
            cache: Cell::new(self.cache.get()),
            marker: PhantomData,
        }
    }
}

impl<T> PartialEq for StringId<T> {
    /// Compares intern ids (`src/string_id.h:240-242`), which is the same as
    /// comparing text but costs one integer compare.
    fn eq(&self, other: &Self) -> bool {
        self.intern == other.intern
    }
}

impl<T> Eq for StringId<T> {}

impl<T> Ord for StringId<T> {
    /// Orders by intern id (`src/string_id.h:234-236`): a total order that is
    /// neither lexicographic nor the same after a restart.
    fn cmp(&self, other: &Self) -> Ordering {
        self.intern.cmp(&other.intern)
    }
}

impl<T> PartialOrd for StringId<T> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<T> Hash for StringId<T> {
    /// Hashes the intern id, as the reference forwards `std::hash` to the
    /// interned identity (`src/string_id.h:366-374`). One integer hash, and it
    /// agrees with [`PartialEq`] by construction.
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.intern.hash(state);
    }
}

impl<T> fmt::Debug for StringId<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self.text, f)
    }
}

/// Lexicographic order of string ids, the reference's `LexCmp`.
///
/// Reference: `src/string_id.h:376-384` and `src/string_id_utils.h:26-38`. The
/// plain [`Ord`] of [`StringId`] follows intern order, which is fast but
/// arbitrary; this wrapper sorts by text, so a listing reads the same on every
/// run at the cost of a string comparison per step.
#[derive(Debug, Clone)]
pub struct Lexicographic<T>(pub StringId<T>);

// The marker `T` must not have to be comparable itself, so the derives are
// spelled out on the wrapped id instead.
impl<T> PartialEq for Lexicographic<T> {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl<T> Eq for Lexicographic<T> {}

impl<T> Ord for Lexicographic<T> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.0.as_str().cmp(other.0.as_str())
    }
}

impl<T> PartialOrd for Lexicographic<T> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// A typed integer identifier: an index into one [`Registry`].
///
/// Reference: `int_id<T>` (`src/int_id.h:15-133`). It is deliberately **not** an
/// identity. `src/string_id.h:59-62` says it cannot be reused after a reload:
/// depending on the loaded content the same integer points at a different
/// object, and the integer alone cannot tell. Nothing here turns an [`IntId`]
/// into data without the registry it came from, so a mistaken reuse has to name
/// a registry and be caught by that registry's validity check.
pub struct IntId<T> {
    /// The reference's `int _id` (`src/int_id.h:131`): an index, not a key.
    index: i32,
    /// `T` is a marker only.
    marker: PhantomData<fn() -> T>,
}

impl<T> IntId<T> {
    /// Wraps an index as an id.
    ///
    /// Reference: the explicit constructor (`src/int_id.h:28-34`).
    #[must_use]
    pub const fn new(index: i32) -> Self {
        Self {
            index,
            marker: PhantomData,
        }
    }

    /// The bare integer (`src/int_id.h:94-96`). Use with care: it says nothing
    /// about which registry or which generation it belongs to.
    #[must_use]
    pub const fn to_i(&self) -> i32 {
        self.index
    }
}

impl<T> Default for IntId<T> {
    /// The 0-id (`src/int_id.h:40-46`). 0 is not special; it is a normal index
    /// and means "invalid" only in a registry with no entries.
    fn default() -> Self {
        Self::new(0)
    }
}

impl<T> Clone for IntId<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> Copy for IntId<T> {}

impl<T> PartialEq for IntId<T> {
    fn eq(&self, other: &Self) -> bool {
        self.index == other.index
    }
}

impl<T> Eq for IntId<T> {}

impl<T> Ord for IntId<T> {
    /// Orders by integer (`src/int_id.h:66-69`).
    fn cmp(&self, other: &Self) -> Ordering {
        self.index.cmp(&other.index)
    }
}

impl<T> PartialOrd for IntId<T> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<T> Hash for IntId<T> {
    /// Hashes the index, as the reference forwards `std::hash`
    /// (`src/int_id.h:186-192`).
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.index.hash(state);
    }
}

impl<T> fmt::Debug for IntId<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&self.index, f)
    }
}

/// An object that knows the string id it was registered under.
///
/// Reference: the `id` member every `generic_factory` entry has
/// (`src/generic_factory.h:154-155`, and `obj.id.set_cid_version(...)` at
/// `src/generic_factory.h:386`). The registry writes its cached index back
/// through this, exactly where the reference writes `_cid`.
pub trait Identified: Sized {
    /// The id this object is known by.
    fn id(&self) -> &StringId<Self>;
}

/// A minimal stand-in for the reference's `generic_factory`: a dense list of
/// objects plus the id-to-index map, and nothing else.
///
/// Reference: the parts of `generic_factory` (`src/generic_factory.h:133-560`)
/// that the id types themselves need — `insert`, `obj`, `is_valid`, `convert`.
/// Deliberately missing: JSON loading, inheritance, abstracts, `mod_id`
/// tracking, deferred loads and the `type_name` used in debug messages. Those
/// belong to the registry task, not this one.
pub struct Registry<T: Identified + Default> {
    /// Registration order; an [`IntId`] is an index into this.
    list: Vec<T>,
    /// Interned id to index, the reference's `map` (`src/generic_factory.h:155`).
    by_id: HashMap<StringId<T>, IntId<T>>,
    /// Default-constructed object handed out for invalid ids
    /// (`src/generic_factory.h:178`).
    dummy: T,
    /// The modification token every cached index is stamped with, the
    /// reference's `version` (`src/generic_factory.h:140-151`), taken from the
    /// process-wide counter in [`next_version`].
    version: i64,
    /// How many times an id was looked up in [`Self::by_id`]. The reference has
    /// no such counter; it exists so tests can prove a cached index really
    /// short-circuits the map.
    map_probes: Cell<u64>,
}

impl<T: Identified + Default> Registry<T> {
    /// An empty registry.
    #[must_use]
    pub fn new() -> Self {
        Self {
            list: Vec::new(),
            by_id: HashMap::new(),
            dummy: T::default(),
            version: next_version(),
            map_probes: Cell::new(0),
        }
    }

    /// Adds `object`, or replaces the object with the same id in place.
    ///
    /// Reference: `generic_factory::insert` (`src/generic_factory.h:372-396`).
    /// Like the reference, a duplicate id keeps its original index and the new
    /// object takes its place; there is no duplicate check (that needs the
    /// `mod_tracker` this port has not got).
    pub fn insert(&mut self, object: T) -> IntId<T> {
        // generic_factory.h:374-380: every change to the containers bumps the
        // version, which is what stops previously cached indices (including
        // cached *misses*) from being trusted.
        self.inc_version();
        let version = self.version;
        let id = object.id().clone();
        if let Some(&existing) = self.by_id.get(&id) {
            let slot = usize::try_from(existing.to_i())
                .ok()
                .and_then(|index| self.list.get_mut(index));
            if let Some(slot) = slot {
                *slot = object;
                // generic_factory.h:386: the replaced entry's own id is stamped.
                slot.id().set_cached_index(existing.to_i(), version);
            }
            return existing;
        }
        let int_id = IntId::new(i32::try_from(self.list.len()).unwrap_or(i32::MAX));
        self.list.push(object);
        if let Some(stored) = self.list.last() {
            // generic_factory.h:394: so is a freshly pushed entry's.
            stored.id().set_cached_index(int_id.to_i(), version);
        }
        self.by_id.insert(id, int_id);
        int_id
    }

    /// The object behind an int id, or the dummy object if the id is invalid.
    ///
    /// Reference: `generic_factory::obj(const int_id<T> &)`
    /// (`src/generic_factory.h:471-478`), which issues a debug message and
    /// returns `dummy_obj`; it does **not** assert. The message is left out
    /// because there is no diagnostics sink in this crate yet.
    #[must_use]
    pub fn obj(&self, id: IntId<T>) -> &T {
        usize::try_from(id.to_i())
            .ok()
            .and_then(|index| self.list.get(index))
            .unwrap_or(&self.dummy)
    }

    /// Whether the registry holds an object for an int id.
    ///
    /// Reference: `generic_factory::is_valid(const int_id<T> &)`
    /// (`src/generic_factory.h:498-500`), including the cast to an unsigned size
    /// that makes a negative index invalid instead of out of bounds.
    #[must_use]
    pub fn is_valid(&self, id: IntId<T>) -> bool {
        usize::try_from(id.to_i()).is_ok_and(|index| index < self.list.len())
    }

    /// The int id for a string id, falling back to `null_id` when absent.
    ///
    /// Reference: `generic_factory::convert` (`src/generic_factory.h:512-527`)
    /// with its `warn` argument off. The warning path needs the diagnostics sink
    /// this crate has not got yet.
    #[must_use]
    pub fn convert(&self, id: &StringId<T>, null_id: IntId<T>) -> IntId<T> {
        self.find_index(id).unwrap_or(null_id)
    }

    /// Finishes a load: invalidates the cache, then stamps every registered
    /// object's own id with its index.
    ///
    /// Reference: `generic_factory::finalize` (`src/generic_factory.h:402-410`).
    /// The deferred-JSON load and the abstracts table it also clears belong to
    /// the loading path this port leaves out.
    pub fn finalize(&mut self) {
        self.inc_version();
        let version = self.version;
        for (index, object) in self.list.iter().enumerate() {
            object
                .id()
                .set_cached_index(i32::try_from(index).unwrap_or(i32::MAX), version);
        }
    }

    /// Removes everything, invalidating every cached index, cached miss
    /// included.
    ///
    /// Reference: `generic_factory::reset` (`src/generic_factory.h:434-444`).
    /// The deferred-JSON queue is part of the loading path this port leaves out.
    pub fn reset(&mut self) {
        self.list.clear();
        self.by_id.clear();
        self.inc_version();
    }

    /// The number of id-map probes so far. Not part of the reference; the cache
    /// rule is only observable through it.
    #[must_use]
    pub fn map_probes(&self) -> u64 {
        self.map_probes.get()
    }

    /// Takes a fresh modification token, invalidating every cached index.
    ///
    /// Reference: `generic_factory::inc_version` (`src/generic_factory.h:147-151`),
    /// which also guarantees the counter never lands on `INVALID_VERSION`.
    fn inc_version(&mut self) {
        self.version = next_version();
    }

    /// The id-to-index lookup with the version cache, the reference's `find_id`
    /// (`src/generic_factory.h:161-176`).
    ///
    /// A cached index stamped with the current version is returned without
    /// touching the map. A cached *miss* is stamped too (`INVALID_CID`), so it
    /// is not re-probed either. Anything older is looked up again; that re-check
    /// is what keeps a [`StringId`] usable across a reload.
    fn find_index(&self, id: &StringId<T>) -> Option<IntId<T>> {
        let cached = id.cache.get();
        if cached.version == self.version {
            let int_id = IntId::new(cached.cid);
            return self.is_valid(int_id).then_some(int_id);
        }
        self.map_probes.set(self.map_probes.get() + 1);
        if let Some(&int_id) = self.by_id.get(id) {
            id.set_cached_index(int_id.to_i(), self.version);
            Some(int_id)
        } else {
            // A miss is cached too: the reference stamps `INVALID_CID` with the
            // current version so a not-yet-inserted id is not re-probed until
            // the registry changes (src/generic_factory.h:167-172).
            id.set_cached_index(INVALID_CID, self.version);
            None
        }
    }

    /// The fallback object for invalid ids.
    fn dummy(&self) -> &T {
        &self.dummy
    }
}

impl<T: Identified + Default> Default for Registry<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: Identified + Default> StringId<T> {
    /// Whether the registry holds an object for this id
    /// (`src/string_id.h:296-299`).
    #[must_use]
    pub fn is_valid(&self, registry: &Registry<T>) -> bool {
        registry.find_index(self).is_some()
    }

    /// The object this id refers to, or the dummy object when it is invalid.
    ///
    /// Reference: `generic_factory::obj(const string_id<T> &)`
    /// (`src/generic_factory.h:485-495`): debug message, then `dummy_obj`.
    #[must_use]
    pub fn obj<'a>(&self, registry: &'a Registry<T>) -> &'a T {
        match registry.find_index(self) {
            Some(int_id) => registry.obj(int_id),
            None => registry.dummy(),
        }
    }

    /// The int id for this string id, or `fallback` when it is not registered.
    ///
    /// Reference: `string_id::id_or` (`src/string_id.h:277-282`), the quiet
    /// variant of `id()`.
    #[must_use]
    pub fn id_or(&self, registry: &Registry<T>, fallback: IntId<T>) -> IntId<T> {
        registry.find_index(self).unwrap_or(fallback)
    }
}

impl<T: Identified + Default> IntId<T> {
    /// Whether the registry holds an object at this index
    /// (`src/int_id.h:129-131`).
    #[must_use]
    pub fn is_valid(&self, registry: &Registry<T>) -> bool {
        registry.is_valid(*self)
    }

    /// The object at this index, or the dummy object when the index is invalid.
    ///
    /// Reference: `generic_factory::obj(const int_id<T> &)`
    /// (`src/generic_factory.h:471-478`).
    #[must_use]
    pub fn obj<'a>(&self, registry: &'a Registry<T>) -> &'a T {
        registry.obj(*self)
    }

    /// The string id of the object at this index.
    ///
    /// Reference: `int_id::id()` through `convert(const int_id<T> &)`
    /// (`src/generic_factory.h:529-534`), which returns `obj(id).id` — so an
    /// invalid index yields the dummy object's id, not the caller's.
    #[must_use]
    pub fn id(&self, registry: &Registry<T>) -> StringId<T> {
        registry.obj(*self).id().clone()
    }
}
