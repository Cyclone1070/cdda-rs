//! Containers with the reference's semantics: stable storage, dense bitsets.
//!
//! Reference: `src/colony.h` (`cata::colony`) and `src/cata_bitset.h`
//! (`tiny_bitset`) at tag `0.I-1`, commit `7b2efa5`.
//!
//! # What is translated
//!
//! [`Colony`] keeps the reference's layout: elements sit in fixed-size groups,
//! each group carries the reference's skipfield and free list, and an erased
//! slot is remembered so the next insertion can reuse it. That preserves the
//! reference's observable behaviour:
//!
//! * iterating visits slots in order, group by group, and erasure never
//!   reorders what is left;
//! * an element's identity survives every insertion and every erasure of a
//!   *different* element -- a [`Handle`] stays valid where the reference's
//!   `colony_iterator` does, except that a destroyed group's handles report
//!   `None` instead of dangling;
//! * insertion is *not* an append: it fills the most recently erased run, from
//!   the start of that run, in the most recently erased group first, so a new
//!   element can appear in the middle of iteration order. This is the
//!   behaviour a Rust container would not have, and it is what makes the
//!   reference's own random-churn sizes reproducible (see
//!   `tests/containers.rs`, which reaches the reference's 45001);
//! * a group that loses its last element is destroyed and its capacity
//!   released, so a later insertion cannot land in that memory;
//! * capacity follows the reference's growth curve (a new group is as large as
//!   the colony was, capped at the maximum group size), and `reserve`,
//!   `shrink_to_fit`, `consolidate` and `change_group_sizes` reproduce the
//!   reference's capacity numbers exactly.
//!
//! # Guarantees that had to be weakened or moved
//!
//! * The reference lets a `pointer`, a reference or an iterator to an element be
//!   held across mutation. Rust forbids that aliasing, so the only thing that
//!   outlives a mutation is a [`Handle`]; `get()` re-derives the element from
//!   it. Element *addresses* are therefore not stable in the reference's sense
//!   -- only identity is.
//! * A [`Handle`] is an index, not an address: `Handle: Ord` follows the
//!   reference's iterator comparison (group number, then slot), and
//!   [`Colony::handle_of`] recovers a handle from a reference by address
//!   comparison, but a reference taken from a *copy* of the colony does not
//!   match.
//! * Erasing a stale handle, or advancing past either end, reports `None` where
//!   the reference asserts in debug builds and is undefined in release builds.
//! * `end()` cannot be named by a handle, so the reference's
//!   `advance( end(), -n )` idiom is [`Colony::at_index`].
//! * Copying a colony gives the copy its own groups (reference: own
//!   allocation), so a handle from the source does not resolve in the copy.
//! * The bitset asserts on out-of-range access in the reference; here the
//!   accessors report `false`. `resize` zeroes the bits it adds and masks the
//!   bits it drops, where the reference leaves fresh heap words uninitialised
//!   and can resurrect dropped bits.
//!
//! # Reference behaviour that was *not* copied, and why
//!
//! * `splice()` (`src/colony.h:3338-3466`) moves whole groups between colonies
//!   and preserves pointers into both. Preserving a handle across colonies
//!   needs globally unique slot ids; group ids are per colony here, so `splice`
//!   is left out rather than given weaker semantics.
//! * `sort()` is stable here; the reference's `std::sort` leaves equal elements
//!   in an unspecified order.
//! * The reference's copy constructor for a heap-allocated `tiny_bitset` copies
//!   `size() / kBitsPerBlock` **bytes** of a bitmap that needs whole blocks
//!   (`src/cata_bitset.h:80`, `src/cata_bitset.h:96`), so it silently truncates
//!   the copy; its `resize_heap` also leaves new words uninitialised
//!   (`src/cata_bitset.cpp:14-46`). Neither is reproduced.

/// Default minimum group size, in elements.
///
/// The reference derives the default from `sizeof( colony )` and
/// `sizeof( group )` and lands on 8 on 64-bit platforms
/// (`src/colony.h:631-639`); a port with a fixed layout states the number.
const DEFAULT_MIN_GROUP_SIZE: usize = 8;

/// Default maximum group size, in elements.
///
/// The reference's skipfield elements are `unsigned short`
/// (`src/colony.h:72`), so one group never holds more than `u16::MAX` slots.
const DEFAULT_MAX_GROUP_SIZE: usize = u16::MAX as usize;

/// A handle to an element of a [`Colony`].
///
/// The reference hands out `colony::iterator`, a pair of pointers that stays
/// valid across insertions and across erasures of *other* elements. Rust cannot
/// hand out such a pointer, so a handle identifies the group and the slot the
/// element lives in; see [`Colony`] for the guarantees that carries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Handle {
    group: usize,
    slot: usize,
}

/// A hole left by an erasure.
///
/// Reference: one skipfield node plus the free-list links the reference stores
/// in the memory of the erased element itself (`src/colony.h:146`,
/// `src/colony.h:1792-1812`).
#[derive(Debug, Clone, Copy, Default)]
struct Hole {
    /// Length of the erased run, set on the first and last slot of a run only
    /// (the reference's skipfield; `src/colony.h:142`).
    skip: usize,
    /// Next node of the group's free list, towards older erasures.
    next: Option<usize>,
    /// Previous node of the group's free list, towards newer erasures.
    prev: Option<usize>,
}

/// One slot of a group: either a live element or a hole.
#[derive(Debug, Clone)]
enum Slot<T> {
    Value(T),
    Hole(Hole),
}

/// A block of contiguous slots.
///
/// Reference: `cata::colony::group` (`src/colony.h:135-171`). Groups are never
/// moved, which is why element addresses are stable in the reference; here the
/// group id is stable instead.
#[derive(Debug, Clone)]
struct Group<T> {
    /// Slots in use; the length is the reference's `last_endpoint`.
    slots: Vec<Slot<T>>,
    /// Slots allocated, the reference's `capacity`.
    capacity: usize,
    /// Live elements in this group, the reference's `number_of_elements`.
    num_elements: usize,
    /// Start of the most recently erased run, the reference's
    /// `free_list_head`.
    free_head: Option<usize>,
    /// Next group in the intrusive list of groups with holes, the reference's
    /// `erasures_list_next_group`.
    erasures_next: Option<usize>,
}

impl<T> Group<T> {
    fn new(capacity: usize) -> Self {
        Self {
            slots: Vec::new(),
            capacity,
            num_elements: 0,
            free_head: None,
            erasures_next: None,
        }
    }

    /// Skipfield value at `index`: zero for a live element or a slot outside
    /// the used region, the run length at either end of a hole run.
    fn skip(&self, index: usize) -> usize {
        match self.slots.get(index) {
            Some(Slot::Hole(hole)) => hole.skip,
            _ => 0,
        }
    }

    fn hole(&self, index: usize) -> Option<Hole> {
        match self.slots.get(index) {
            Some(Slot::Hole(hole)) => Some(*hole),
            _ => None,
        }
    }

    fn hole_mut(&mut self, index: usize) -> Option<&mut Hole> {
        match self.slots.get_mut(index) {
            Some(Slot::Hole(hole)) => Some(hole),
            _ => None,
        }
    }

    fn set_skip(&mut self, index: usize, value: usize) {
        if let Some(hole) = self.hole_mut(index) {
            hole.skip = value;
        }
    }

    fn clear_skip(&mut self, index: usize) {
        self.set_skip(index, 0);
    }

    fn is_value(&self, index: usize) -> bool {
        matches!(self.slots.get(index), Some(Slot::Value(_)))
    }

    fn first_element(&self) -> Option<usize> {
        let skip = self.skip(0);
        if skip < self.slots.len() {
            Some(skip)
        } else {
            None
        }
    }

    fn last_element(&self) -> Option<usize> {
        let last = self.slots.len().checked_sub(1)?;
        last.checked_sub(self.skip(last))
    }

    /// Slot after `index`, where `index` holds a live element.
    fn next_index(&self, index: usize) -> Option<usize> {
        let next = index + 1 + self.skip(index + 1);
        if next < self.slots.len() {
            Some(next)
        } else {
            None
        }
    }

    /// First live slot at or after `index`, which may itself be a hole.
    ///
    /// The reference's `erase()` gets this for free: it adds the length of the
    /// run it just merged to the erased slot (`src/colony.h:1882-1884`).
    fn next_value_from(&self, mut index: usize) -> Option<usize> {
        while index < self.slots.len() {
            if self.is_value(index) {
                return Some(index);
            }
            let skip = self.skip(index);
            index += if skip > 0 { skip } else { 1 };
        }
        None
    }

    /// Slot before `index`, where `index` holds a live element.
    fn prev_index(&self, index: usize) -> Option<usize> {
        let before = index.checked_sub(1)?;
        before.checked_sub(self.skip(before))
    }

    /// Appends an element to the used region of this group.
    fn push(&mut self, value: T) -> usize {
        self.slots.push(Slot::Value(value));
        self.num_elements += 1;
        self.slots.len() - 1
    }

    /// Fills the hole at `slot`, which must be the head of the free list.
    ///
    /// Reference: the "reuse previous-erased element locations" branch of
    /// `insert()` (`src/colony.h:1019-1079`).
    fn fill_hole(&mut self, slot: usize, value: T) -> usize {
        let hole = self.hole(slot).unwrap_or_default();
        // A run of one disappears from the free list; a longer run shrinks from
        // its start, so its free-list node moves one slot to the right.
        let remaining = hole.skip.saturating_sub(1);
        if remaining == 0 {
            self.free_head = hole.next;
            if let Some(next) = hole.next
                && let Some(node) = self.hole_mut(next)
            {
                node.prev = None;
            }
        } else {
            self.set_skip(slot + remaining, remaining);
            self.set_skip(slot + 1, remaining);
            self.free_head = Some(slot + 1);
            if let Some(next) = hole.next
                && let Some(node) = self.hole_mut(next)
            {
                node.prev = Some(slot + 1);
            }
            if let Some(node) = self.hole_mut(slot + 1) {
                node.next = hole.next;
                node.prev = None;
            }
        }
        if let Some(cell) = self.slots.get_mut(slot) {
            *cell = Slot::Value(value);
        }
        self.num_elements += 1;
        slot
    }

    /// Erases the element at `index`, merging its hole into adjacent runs.
    ///
    /// Reference: `erase()`'s skipfield maintenance before any group removal
    /// (`src/colony.h:1782-1891`).
    fn erase_at(&mut self, index: usize) -> bool {
        if !self.is_value(index) {
            return false;
        }
        let before_erased = index > 0 && self.skip(index - 1) != 0;
        let after_erased = self.skip(index + 1) != 0;

        let hole = match (before_erased, after_erased) {
            // Solo hole: push a new node at the head of the free list.
            (false, false) => {
                if let Some(head) = self.free_head
                    && let Some(node) = self.hole_mut(head)
                {
                    node.prev = Some(index);
                }
                let hole = Hole {
                    skip: 1,
                    next: self.free_head,
                    prev: None,
                };
                self.free_head = Some(index);
                hole
            }
            // Extends a run to the left: the run's node stays at its start.
            (true, false) => {
                let preceding = self.skip(index - 1);
                let length = preceding + 1;
                self.set_skip(index - preceding, length);
                Hole {
                    skip: length,
                    next: None,
                    prev: None,
                }
            }
            // Extends a run to the right: the run's node moves to this slot.
            (false, true) => {
                let following = self.skip(index + 1);
                let length = following + 1;
                let node = self.hole(index + 1).unwrap_or_default();
                self.set_skip(index + following, length);
                if following > 1 {
                    self.clear_skip(index + 1);
                }
                if let Some(next) = node.next
                    && let Some(link) = self.hole_mut(next)
                {
                    link.prev = Some(index);
                }
                if let Some(prev) = node.prev {
                    if let Some(link) = self.hole_mut(prev) {
                        link.next = Some(index);
                    }
                } else {
                    self.free_head = Some(index);
                }
                Hole {
                    skip: length,
                    next: node.next,
                    prev: node.prev,
                }
            }
            // Joins two runs: the following run's node leaves the free list.
            (true, true) => {
                let preceding = self.skip(index - 1);
                let following = self.skip(index + 1);
                let length = preceding + 1 + following;
                self.set_skip(index - preceding, length);
                self.set_skip(index + following, length);
                if preceding > 1 {
                    self.clear_skip(index - 1);
                }
                if following > 1 {
                    self.clear_skip(index + 1);
                }
                let node = self.hole(index + 1).unwrap_or_default();
                if let Some(next) = node.next
                    && let Some(link) = self.hole_mut(next)
                {
                    link.prev = node.prev;
                }
                if let Some(prev) = node.prev {
                    if let Some(link) = self.hole_mut(prev) {
                        link.next = node.next;
                    }
                } else {
                    self.free_head = node.next;
                }
                Hole::default()
            }
        };

        if let Some(cell) = self.slots.get_mut(index) {
            *cell = Slot::Hole(hole);
        }
        self.num_elements -= 1;
        true
    }
}

/// What an insertion will do, decided before any element is moved.
#[derive(Debug, Clone, Copy)]
enum InsertTarget {
    /// Fill the head of some group's free list.
    Hole { group: usize, slot: usize },
    /// Append to the used region of the last group.
    Append { group: usize },
    /// Allocate a new group of the given capacity.
    NewGroup { capacity: usize },
}

/// A container whose elements stay put while other elements are inserted and
/// erased.
///
/// Reference: `cata::colony` (`src/colony.h`). The guarantees translated are:
///
/// * iterating visits the elements in the order their slots appear, group by
///   group, and erasure never reorders what is left;
/// * a [`Handle`] stays valid across any number of insertions and across
///   erasures of *other* elements (`src/colony.h:1740-1754`);
/// * erasing the last element of a group destroys the group, and handles into
///   that group stop resolving;
/// * insertion fills the most recently erased run first, so a new element can
///   appear in the middle of iteration order (`src/colony.h:1019-1079`).
///
/// Guarantees that could not be copied literally:
///
/// * the reference lets `&element` and iterators be held across mutation; in
///   Rust a `&T` borrows the colony, so only a [`Handle`] outlives it;
/// * using a stale handle is a lookup miss here, where the reference asserts
///   in debug and is undefined in release (`src/colony.h:1763`).
#[derive(Debug)]
pub struct Colony<T> {
    /// Groups in iteration order, indexed by a never-reused id; a destroyed
    /// group leaves `None` behind so that old handles cannot alias a new one.
    groups: Vec<Option<Group<T>>>,
    /// Head of the intrusive list of groups with holes.
    erasures_head: Option<usize>,
    size: usize,
    capacity: usize,
    min_group_size: usize,
    max_group_size: usize,
}

impl<T> Colony<T> {
    /// Creates an empty colony with the reference's default group sizes.
    #[must_use]
    pub fn new() -> Self {
        Self {
            groups: Vec::new(),
            erasures_head: None,
            size: 0,
            capacity: 0,
            min_group_size: DEFAULT_MIN_GROUP_SIZE,
            max_group_size: DEFAULT_MAX_GROUP_SIZE,
        }
    }

    /// Number of elements stored.
    #[must_use]
    pub fn size(&self) -> usize {
        self.size
    }

    /// Whether the colony stores no elements.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.size == 0
    }

    /// Number of element slots allocated, the reference's `capacity()`.
    #[must_use]
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Inserts `value` and returns a handle to it.
    ///
    /// Reference: `insert( const element_type & )` (`src/colony.h:952-1101`).
    pub fn insert(&mut self, value: T) -> Handle {
        match self.insert_target() {
            InsertTarget::Hole { group, slot } => {
                if let Some(target) = self.group_mut(group) {
                    target.fill_hole(slot, value);
                    let exhausted = target.free_head.is_none();
                    self.size += 1;
                    if exhausted {
                        self.remove_from_erasures(group);
                    }
                    return Handle { group, slot };
                }
                self.append_group_with(value, self.new_group_capacity())
            }
            InsertTarget::Append { group } => {
                if let Some(target) = self.group_mut(group) {
                    let slot = target.push(value);
                    self.size += 1;
                    return Handle { group, slot };
                }
                self.append_group_with(value, self.new_group_capacity())
            }
            InsertTarget::NewGroup { capacity } => self.append_group_with(value, capacity),
        }
    }

    /// Inserts `count` copies of `value` and returns a handle to each.
    ///
    /// Reference: fill insert (`src/colony.h:1527-1663`). Like the reference,
    /// the copies take freed slots before the back of the colony, and a fill
    /// insert into a colony with no groups sizes the first group to the request
    /// (clamped to the group sizes) instead of following the single-insert
    /// growth curve (`src/colony.h:1543-1550`).
    pub fn insert_many(&mut self, count: usize, value: T) -> Vec<Handle>
    where
        T: Clone,
    {
        let mut handles = Vec::with_capacity(count);
        if count == 0 {
            return handles;
        }
        if self.live_group_count() == 0 {
            let capacity = count.clamp(self.min_group_size, self.max_group_size);
            let filled = count.min(capacity);
            let mut group = Group::new(capacity);
            for _ in 0..filled {
                let slot = group.push(value.clone());
                handles.push(Handle {
                    group: self.groups.len(),
                    slot,
                });
            }
            self.groups.push(Some(group));
            self.capacity += capacity;
            self.size += filled;
            for _ in filled..count {
                handles.push(self.insert(value.clone()));
            }
            return handles;
        }
        for _ in 0..count {
            handles.push(self.insert(value.clone()));
        }
        handles
    }

    /// Inserts every item of `items`, in order, and returns a handle to each.
    ///
    /// Reference: range insert (`src/colony.h:1665-1677`), which is a loop of
    /// single insertions and therefore has the same hole-filling behaviour.
    pub fn insert_range<I>(&mut self, items: I) -> Vec<Handle>
    where
        I: IntoIterator<Item = T>,
    {
        let mut handles = Vec::new();
        for item in items {
            handles.push(self.insert(item));
        }
        handles
    }

    /// Looks up the element a handle refers to, or `None` if the handle is not
    /// valid in this colony.
    #[must_use]
    pub fn get(&self, handle: Handle) -> Option<&T> {
        match self.group(handle.group)?.slots.get(handle.slot) {
            Some(Slot::Value(value)) => Some(value),
            _ => None,
        }
    }

    /// Removes the element `handle` refers to and returns a handle to the next
    /// element in iteration order, or `None` at the end.
    ///
    /// Reference: `erase( const const_iterator & )` (`src/colony.h:1755-1977`).
    /// Erasing a stale handle is a no-op that reports the end, where the
    /// reference asserts and is then undefined.
    pub fn erase(&mut self, handle: Handle) -> Option<Handle> {
        let group = self.group_mut(handle.group)?;
        // A group joins the erasures list when it gains its first hole
        // (reference erase() case 0, `src/colony.h:1802-1805`).
        let already_listed = group.free_head.is_some();
        if !group.erase_at(handle.slot) {
            return None;
        }
        self.size -= 1;
        if !already_listed {
            let head = self.erasures_head;
            if let Some(group) = self.group_mut(handle.group) {
                group.erasures_next = head;
            }
            self.erasures_head = Some(handle.group);
        }

        if self
            .group(handle.group)
            .is_some_and(|g| g.num_elements == 0)
        {
            // The group is empty and must leave the colony; the reference does
            // the same so that an emptied group's memory is not reused
            // (`src/colony.h:1894-1976`).
            self.remove_from_erasures(handle.group);
            if self.live_group_count() == 1 {
                // The only group: keep its capacity, forget its slots
                // (reference case 0, `src/colony.h:1898-1912`).
                if let Some(group) = self.group_mut(handle.group) {
                    group.slots.clear();
                    group.free_head = None;
                    group.num_elements = 0;
                }
            } else if let Some(group) = self.groups.get_mut(handle.group).and_then(Option::take) {
                self.capacity -= group.capacity;
            }
            return self.first_handle_from(handle.group + 1);
        }

        if let Some(next) = self
            .group(handle.group)
            .and_then(|group| group.next_value_from(handle.slot + 1))
        {
            return Some(Handle {
                group: handle.group,
                slot: next,
            });
        }
        self.first_handle_from(handle.group + 1)
    }

    /// Removes the elements from `from` up to, but not including, `to`; `None`
    /// as `to` means the end of the colony.
    ///
    /// Reference: the two-iterator `erase()` (`src/colony.h:1985-2305`). The
    /// elements are erased one at a time here, which leaves the same groups,
    /// holes and free lists as the reference's bulk path; the one visible
    /// difference is preserved: when the range empties the colony the reference
    /// calls `blank()` (`src/colony.h:2286-2289`), so capacity drops to zero,
    /// unlike a sequence of single erasures, which keeps the last group.
    ///
    /// An invalid `from`, an invalid `to`, or a `to` that precedes `from` is a
    /// no-op: the reference asserts `iterator1 <= iterator2`
    /// (`src/colony.h:1987`) and is then undefined.
    pub fn erase_range(&mut self, from: Handle, to: Option<Handle>) {
        if self.get(from).is_none() {
            return;
        }
        if let Some(end) = to
            && (end < from || self.get(end).is_none())
        {
            return;
        }
        let mut current = from;
        loop {
            if Some(current) == to {
                break;
            }
            match self.erase(current) {
                Some(handle) => current = handle,
                None => break,
            }
        }
        if self.is_empty() {
            self.clear();
        }
    }

    /// Handle of the first element, or `None` when the colony is empty.
    #[must_use]
    pub fn begin(&self) -> Option<Handle> {
        self.first_handle_from(0)
    }

    /// Handle of the last element, or `None` when the colony is empty.
    #[must_use]
    pub fn last(&self) -> Option<Handle> {
        (0..self.groups.len()).rev().find_map(|id| {
            let group = self.group(id)?;
            group.last_element().map(|slot| Handle { group: id, slot })
        })
    }

    /// Handle of the element after `handle`, or `None` at the end.
    #[must_use]
    pub fn next(&self, handle: Handle) -> Option<Handle> {
        let group = self.group(handle.group)?;
        if !group.is_value(handle.slot) {
            return None;
        }
        if let Some(slot) = group.next_index(handle.slot) {
            return Some(Handle {
                group: handle.group,
                slot,
            });
        }
        self.first_handle_from(handle.group + 1)
    }

    /// Handle of the element before `handle`, or `None` at the beginning.
    #[must_use]
    pub fn previous(&self, handle: Handle) -> Option<Handle> {
        let group = self.group(handle.group)?;
        if !group.is_value(handle.slot) {
            return None;
        }
        if let Some(slot) = group.prev_index(handle.slot) {
            return Some(Handle {
                group: handle.group,
                slot,
            });
        }
        (0..handle.group).rev().find_map(|id| {
            let group = self.group(id)?;
            group.last_element().map(|slot| Handle { group: id, slot })
        })
    }

    /// The position of `handle` in iteration order, or `None` if the handle is
    /// not valid.
    ///
    /// Reference: `get_index_from_iterator()` (`src/colony.h:3154-3184`), whose
    /// index is a temporary number that changes with insertion and erasure.
    #[must_use]
    pub fn index_of(&self, handle: Handle) -> Option<usize> {
        self.get(handle)?;
        let mut index = 0;
        for id in 0..handle.group {
            if let Some(group) = self.group(id) {
                index += group.num_elements;
            }
        }
        let group = self.group(handle.group)?;
        let mut slot = 0;
        while slot < handle.slot {
            if group.is_value(slot) {
                index += 1;
                slot += 1;
            } else {
                slot += 1 + group.skip(slot);
            }
        }
        Some(index)
    }

    /// The handle of the element at `index` in iteration order, or `None` when
    /// the index is past the end.
    ///
    /// Reference: `get_iterator_from_index()` (`src/colony.h:3205-3212`), which
    /// is `begin()` plus `advance()`.
    #[must_use]
    pub fn at_index(&self, index: usize) -> Option<Handle> {
        let mut remaining = index;
        for (id, group) in self.groups.iter().enumerate() {
            let Some(group) = group else {
                continue;
            };
            if remaining < group.num_elements {
                let mut slot = group.first_element()?;
                let mut left = remaining;
                while left > 0 {
                    slot = group.next_index(slot)?;
                    left -= 1;
                }
                return Some(Handle { group: id, slot });
            }
            remaining -= group.num_elements;
        }
        None
    }

    /// The handle `distance` elements away from `handle`, or `None` when that
    /// leaves the colony.
    ///
    /// Reference: `advance()` (`src/colony.h:2562-2955`). The reference allows
    /// `end()` as a starting point and asserts on other out-of-range moves; a
    /// handle cannot name `end()`, so the two out-of-range directions report
    /// `None` instead and `at_index()` covers end-relative arithmetic.
    #[must_use]
    pub fn advance(&self, handle: Handle, distance: isize) -> Option<Handle> {
        let index = self.index_of(handle)?;
        self.at_index(index.checked_add_signed(distance)?)
    }

    /// The signed number of elements from `from` to `to`, or `None` if either
    /// handle is not valid.
    ///
    /// Reference: `distance()` (`src/colony.h:3012-3050`), which is the
    /// difference of the two elements' indices.
    #[must_use]
    pub fn distance(&self, from: Handle, to: Handle) -> Option<isize> {
        let from_index = self.index_of(from)?;
        let to_index = self.index_of(to)?;
        Some(to_index.cast_signed() - from_index.cast_signed())
    }

    /// The handle of an element reached through a reference, or `None` if the
    /// reference is not to one of this colony's elements.
    ///
    /// Reference: `get_iterator_from_pointer()` (`src/colony.h:3120-3151`).
    /// Identity is the element's address, as in the reference.
    #[must_use]
    pub fn handle_of(&self, element: &T) -> Option<Handle> {
        for (id, group) in self.groups.iter().enumerate() {
            let Some(group) = group else {
                continue;
            };
            for (slot, cell) in group.slots.iter().enumerate() {
                if let Slot::Value(value) = cell
                    && core::ptr::eq(value, element)
                {
                    return Some(Handle { group: id, slot });
                }
            }
        }
        None
    }

    /// Sets the group size bounds, rebuilding the colony if the existing groups
    /// no longer fit.
    ///
    /// Reference: `change_group_sizes()` (`src/colony.h:2341-2357`), which
    /// asserts `min > 2` and `min <= max`; invalid bounds are rejected here
    /// instead, and rebuilding invalidates every handle, as it does in the
    /// reference.
    pub fn change_group_sizes(&mut self, min: usize, max: usize) -> bool {
        if min <= 2 || min > max {
            return false;
        }
        self.min_group_size = min;
        self.max_group_size = max;
        let smallest = (0..self.groups.len()).find_map(|id| self.group(id).map(|g| g.capacity));
        let largest = (0..self.groups.len())
            .rev()
            .find_map(|id| self.group(id).map(|g| g.capacity));
        if smallest.is_some_and(|capacity| capacity < min)
            || largest.is_some_and(|capacity| capacity > max)
        {
            self.consolidate();
        }
        true
    }

    /// Sets the minimum group size.
    ///
    /// Reference: `change_minimum_group_size()` (`src/colony.h:2363-2366`).
    pub fn change_minimum_group_size(&mut self, min: usize) -> bool {
        self.change_group_sizes(min, self.max_group_size)
    }

    /// Sets the maximum group size.
    ///
    /// Reference: `change_maximum_group_size()` (`src/colony.h:2375-2378`).
    pub fn change_maximum_group_size(&mut self, max: usize) -> bool {
        self.change_group_sizes(self.min_group_size, max)
    }

    /// Empties the colony and sets the group size bounds.
    ///
    /// Reference: `reinitialize()` (`src/colony.h:2397-2406`).
    pub fn reinitialize(&mut self, min: usize, max: usize) -> bool {
        if min <= 2 || min > max {
            return false;
        }
        self.clear();
        self.min_group_size = min;
        self.max_group_size = max;
        true
    }

    /// Preallocates room for `amount` elements without changing the contents.
    ///
    /// Reference: `reserve()` (`src/colony.h:2512-2552`). An empty colony gets
    /// one group of the requested size; a non-empty one is consolidated into
    /// groups large enough to hold the request, which invalidates every handle.
    pub fn reserve(&mut self, amount: usize) {
        if amount == 0 || amount <= self.capacity {
            return;
        }
        let wanted = amount.clamp(self.min_group_size, self.max_group_size);
        if self.size == 0 {
            self.clear();
            self.groups.push(Some(Group::new(wanted)));
            self.capacity = wanted;
        } else {
            let saved_min = self.min_group_size;
            self.min_group_size = wanted;
            self.consolidate();
            self.min_group_size = saved_min;
        }
    }

    /// Drops spare capacity and every hole.
    ///
    /// Reference: `shrink_to_fit()` (`src/colony.h:2489-2499`), which
    /// consolidates into one group of the current size (capped at the maximum
    /// group size) and invalidates every handle.
    pub fn shrink_to_fit(&mut self) {
        if self.size == self.capacity {
            return;
        }
        if self.size == 0 {
            self.clear();
            return;
        }
        self.consolidate();
    }

    /// Sorts the elements with their natural order.
    ///
    /// Reference: `sort()` (`src/colony.h:3321-3323`), which delegates to
    /// `sort( less() )`.
    pub fn sort(&mut self)
    where
        T: Ord,
    {
        self.sort_by(Ord::cmp);
    }

    /// Sorts the elements with `compare`.
    ///
    /// Reference: `sort( comparison_function )` (`src/colony.h:3253-3318`),
    /// which sorts the elements into a freshly reserved colony. That rebuild is
    /// reproduced here, so every handle from before the sort stops resolving.
    /// One guarantee is stronger: `std::sort` leaves the order of equal
    /// elements unspecified, while the sort here is stable.
    pub fn sort_by<F>(&mut self, mut compare: F)
    where
        F: FnMut(&T, &T) -> core::cmp::Ordering,
    {
        if self.size < 2 {
            return;
        }
        let total = self.size;
        let mut groups = core::mem::take(&mut self.groups);
        let mut values = Vec::with_capacity(total);
        for group in groups.iter_mut().filter_map(Option::take) {
            for slot in group.slots {
                if let Slot::Value(value) = slot {
                    values.push(value);
                }
            }
        }
        values.sort_by(&mut compare);
        let mut fresh = Self {
            groups,
            erasures_head: None,
            size: 0,
            capacity: 0,
            min_group_size: self.min_group_size,
            max_group_size: self.max_group_size,
        };
        // The reference reserves room for the whole sequence before inserting
        // it back (`src/colony.h:3277-3280`), which makes one allocation when
        // it fits in a group.
        fresh.reserve(total);
        for value in values {
            fresh.insert(value);
        }
        *self = fresh;
    }

    /// Rebuilds the colony so that no slots are wasted.
    ///
    /// Reference: `consolidate()` (`src/colony.h:1699-1719`): the first group is
    /// as large as the requested minimum (the current size, clamped to the group
    /// bounds), and later groups follow the normal growth curve.
    fn consolidate(&mut self) {
        let total = self.size;
        if total == 0 {
            self.clear();
            return;
        }
        let first_capacity = if self.min_group_size > total {
            self.min_group_size
        } else if total > self.max_group_size {
            self.max_group_size
        } else {
            total
        };
        let mut groups = core::mem::take(&mut self.groups);
        let mut values = Vec::with_capacity(total);
        for group in groups.iter_mut().filter_map(Option::take) {
            for slot in group.slots {
                if let Slot::Value(value) = slot {
                    values.push(value);
                }
            }
        }
        let mut fresh = Self {
            groups,
            erasures_head: None,
            size: 0,
            capacity: 0,
            min_group_size: first_capacity,
            max_group_size: self.max_group_size,
        };
        for value in values {
            fresh.insert(value);
        }
        fresh.min_group_size = self.min_group_size;
        *self = fresh;
    }

    /// Removes every element and every group, keeping the configured group
    /// sizes.
    ///
    /// Reference: `clear()` (`src/colony.h:2400-2410`), which destroys all
    /// groups and leaves capacity at zero.
    pub fn clear(&mut self) {
        for group in &mut self.groups {
            *group = None;
        }
        self.erasures_head = None;
        self.size = 0;
        self.capacity = 0;
    }

    /// Iterates over the stored elements in iteration order.
    #[must_use]
    pub fn iter(&self) -> Iter<'_, T> {
        Iter {
            groups: self.groups.iter(),
            slots: std::slice::Iter::default(),
        }
    }

    fn group(&self, id: usize) -> Option<&Group<T>> {
        self.groups.get(id).and_then(Option::as_ref)
    }

    fn group_mut(&mut self, id: usize) -> Option<&mut Group<T>> {
        self.groups.get_mut(id).and_then(Option::as_mut)
    }

    fn live_group_count(&self) -> usize {
        self.groups.iter().filter(|group| group.is_some()).count()
    }

    fn last_live_group_id(&self) -> Option<usize> {
        (0..self.groups.len())
            .rev()
            .find(|id| self.group(*id).is_some())
    }

    /// Handle of the first element in the first non-empty group at or after
    /// `start`.
    fn first_handle_from(&self, start: usize) -> Option<Handle> {
        self.groups
            .iter()
            .enumerate()
            .skip(start)
            .find_map(|(id, group)| {
                let group = group.as_ref()?;
                group.first_element().map(|slot| Handle { group: id, slot })
            })
    }

    /// Removes a group from the intrusive list of groups with holes.
    ///
    /// Reference: `remove_from_groups_with_erasures_list()`
    /// (`src/colony.h:1721-1736`).
    fn remove_from_erasures(&mut self, id: usize) {
        let Some(mut current) = self.erasures_head else {
            return;
        };
        if current == id {
            self.erasures_head = self.group(current).and_then(|group| group.erasures_next);
            return;
        }
        loop {
            let Some(next) = self.group(current).and_then(|group| group.erasures_next) else {
                return;
            };
            if next == id {
                let after = self.group(next).and_then(|group| group.erasures_next);
                if let Some(group) = self.group_mut(current) {
                    group.erasures_next = after;
                }
                return;
            }
            current = next;
        }
    }

    /// Decides where the next insertion goes without touching the colony.
    fn insert_target(&self) -> InsertTarget {
        if let Some(head) = self.erasures_head
            && let Some(slot) = self.group(head).and_then(|group| group.free_head)
        {
            return InsertTarget::Hole { group: head, slot };
        }
        if let Some(group) = self.last_live_group_id() {
            let full = self
                .group(group)
                .is_some_and(|group| group.slots.len() >= group.capacity);
            if !full {
                return InsertTarget::Append { group };
            }
            return InsertTarget::NewGroup {
                capacity: self.new_group_capacity(),
            };
        }
        InsertTarget::NewGroup {
            capacity: self.min_group_size,
        }
    }

    /// Capacity of the next group allocated at the back.
    ///
    /// Reference: a new group is as large as the colony currently is, capped at
    /// the maximum group size (`src/colony.h:982-984`), which produces the
    /// reference's growth curve.
    fn new_group_capacity(&self) -> usize {
        self.size.min(self.max_group_size).max(1)
    }

    /// Pushes a fresh group on the back and puts `value` in it.
    fn append_group_with(&mut self, value: T, capacity: usize) -> Handle {
        let mut group = Group::new(capacity);
        let slot = group.push(value);
        let id = self.groups.len();
        self.groups.push(Some(group));
        self.capacity += capacity;
        self.size += 1;
        Handle { group: id, slot }
    }
}

impl<T: Clone> Clone for Colony<T> {
    /// Copies the elements into a colony with groups of its own, so that a
    /// handle into the source does not resolve in the copy.
    ///
    /// Reference: the copy constructor (`src/colony.h:668-687`) allocates fresh
    /// groups and copies element by element. The reference's pointers and
    /// iterators do not carry over to the copy for the same reason: the copy's
    /// elements have new addresses.
    fn clone(&self) -> Self {
        let offset = self.groups.len();
        let mut groups: Vec<Option<Group<T>>> = (0..offset).map(|_| None).collect();
        for group in &self.groups {
            groups.push(group.as_ref().map(|group| {
                let mut copy = group.clone();
                copy.erasures_next = copy.erasures_next.map(|id| id + offset);
                copy
            }));
        }
        Self {
            groups,
            erasures_head: self.erasures_head.map(|id| id + offset),
            size: self.size,
            capacity: self.capacity,
            min_group_size: self.min_group_size,
            max_group_size: self.max_group_size,
        }
    }
}

impl<T> Default for Colony<T> {
    fn default() -> Self {
        Self::new()
    }
}

/// Iterator over the live elements of a [`Colony`], in iteration order.
#[derive(Debug, Clone)]
pub struct Iter<'a, T> {
    groups: std::slice::Iter<'a, Option<Group<T>>>,
    slots: std::slice::Iter<'a, Slot<T>>,
}

impl<'a, T> Iterator for Iter<'a, T> {
    type Item = &'a T;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if let Some(slot) = self.slots.next() {
                if let Slot::Value(value) = slot {
                    return Some(value);
                }
            } else if let Some(group) = self.groups.next()? {
                self.slots = group.slots.iter();
            }
        }
    }
}

impl<'a, T> IntoIterator for &'a Colony<T> {
    type Item = &'a T;
    type IntoIter = Iter<'a, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

/// Bits in one storage word.
///
/// The reference uses `uintptr_t` (`src/cata_bitset.h:20`); a u64 covers the
/// 64-bit targets the reference supports.
const BITS_PER_BLOCK: usize = u64::BITS as usize;

/// Bits the reference stores inline: one word less its control byte
/// (`src/cata_bitset.h:33-35`).
const MAX_INLINE_BITS: usize = BITS_PER_BLOCK - 8;

/// A dense bitset with a fixed number of bits.
///
/// Reference: `tiny_bitset` (`src/cata_bitset.h`, `src/cata_bitset.cpp`), the
/// bitset behind map memory and visibility. Bit `i` lives in word `i / 64`,
/// most significant bit first, as in the reference; the layout is not
/// observable through this API, but keeping it makes the port comparable.
///
/// Divergences, all deliberate:
///
/// * the reference asserts `index < size()` on every access
///   (`src/cata_bitset.h:118`); out-of-range calls report `false` here;
/// * `resize()` zeroes newly added bits and masks off bits that fall outside a
///   shrink, where the reference leaves fresh heap words uninitialised and
///   keeps stale bits, so regrowing can resurrect old values;
/// * `count()` and `flip()` are additions: the reference exposes `all()`,
///   `any()` and `none()` but never counts bits or flips them.
#[derive(Debug, Clone, Default)]
pub struct TinyBitset {
    size: usize,
    blocks: Vec<u64>,
}

impl TinyBitset {
    /// Creates a bitset with the reference's default size, 56 bits.
    ///
    /// Reference: `tiny_bitset()` (`src/cata_bitset.h:55`), which delegates to
    /// `tiny_bitset( kMaxInlineBits )`.
    #[must_use]
    pub fn new() -> Self {
        Self::with_size(MAX_INLINE_BITS)
    }

    /// Creates a bitset of `bits` bits, all zero.
    ///
    /// Reference: `explicit tiny_bitset( size_t initial_capacity )`
    /// (`src/cata_bitset.h:57-60`). The reference tracks size and capacity as
    /// one number, so this is both.
    #[must_use]
    pub fn with_size(bits: usize) -> Self {
        let mut set = Self {
            size: 0,
            blocks: Vec::new(),
        };
        set.resize(bits);
        set
    }

    /// Creates a bitset whose bit `i` is `bools[i]`.
    ///
    /// This constructor has no counterpart in the reference, which only offers
    /// size construction; it is the bool-list construction of a standard bitset.
    #[must_use]
    pub fn from_bools(bools: &[bool]) -> Self {
        let mut set = Self::with_size(bools.len());
        for (index, value) in bools.iter().enumerate() {
            if *value {
                set.set(index);
            }
        }
        set
    }

    /// Number of bits, the reference's `size()`.
    #[must_use]
    pub fn size(&self) -> usize {
        self.size
    }

    /// Number of bits, the reference's `capacity()`.
    ///
    /// The reference tracks size and capacity as one number
    /// (`src/cata_bitset.h:189-192`).
    #[must_use]
    pub fn capacity(&self) -> usize {
        self.size
    }

    /// Reads bit `index`; `false` when the index is out of range.
    ///
    /// Reference: `test()` (`src/cata_bitset.h:122-127`).
    #[must_use]
    pub fn test(&self, index: usize) -> bool {
        if index >= self.size {
            return false;
        }
        match self.blocks.get(index / BITS_PER_BLOCK) {
            Some(word) => word & Self::bit(index) != 0,
            None => false,
        }
    }

    /// Sets bit `index`, reporting whether it was in range.
    ///
    /// Reference: `set()` (`src/cata_bitset.h:112-120`).
    pub fn set(&mut self, index: usize) -> bool {
        if index >= self.size {
            return false;
        }
        match self.blocks.get_mut(index / BITS_PER_BLOCK) {
            Some(word) => {
                *word |= Self::bit(index);
                true
            }
            None => false,
        }
    }

    /// Clears bit `index`, reporting whether it was in range.
    ///
    /// Reference: `clear()` (`src/cata_bitset.h:129-136`), the reference's
    /// name for a reset.
    pub fn clear(&mut self, index: usize) -> bool {
        if index >= self.size {
            return false;
        }
        match self.blocks.get_mut(index / BITS_PER_BLOCK) {
            Some(word) => {
                *word &= !Self::bit(index);
                true
            }
            None => false,
        }
    }

    /// Flips bit `index`, reporting whether it was in range.
    ///
    /// No counterpart in the reference.
    pub fn flip(&mut self, index: usize) -> bool {
        if index >= self.size {
            return false;
        }
        match self.blocks.get_mut(index / BITS_PER_BLOCK) {
            Some(word) => {
                *word ^= Self::bit(index);
                true
            }
            None => false,
        }
    }

    /// Whether every bit is set; true for an empty bitset, as in the reference.
    ///
    /// Reference: `all()` (`src/cata_bitset.h:197-202`), whose `and_mapper`
    /// starts at `~0`.
    #[must_use]
    pub fn all(&self) -> bool {
        let Some((last, full)) = self.blocks.split_last() else {
            return true;
        };
        full.iter().all(|word| *word == u64::MAX)
            && *last & Self::used_mask(self.size) == Self::used_mask(self.size)
    }

    /// Whether any bit is set.
    ///
    /// Reference: `any()` (`src/cata_bitset.h:204-209`).
    #[must_use]
    pub fn any(&self) -> bool {
        let Some((last, full)) = self.blocks.split_last() else {
            return false;
        };
        full.iter().any(|word| *word != 0) || *last & Self::used_mask(self.size) != 0
    }

    /// Whether no bit is set.
    ///
    /// Reference: `none()` (`src/cata_bitset.h:211-213`).
    #[must_use]
    pub fn none(&self) -> bool {
        !self.any()
    }

    /// Number of set bits.
    ///
    /// No counterpart in the reference.
    #[must_use]
    pub fn count(&self) -> usize {
        let Some((last, full)) = self.blocks.split_last() else {
            return 0;
        };
        let whole: usize = full.iter().map(|word| word.count_ones() as usize).sum();
        whole + (*last & Self::used_mask(self.size)).count_ones() as usize
    }

    /// Sets every bit.
    ///
    /// Reference: `set_all()` (`src/cata_bitset.h:231-233`).
    pub fn set_all(&mut self) {
        self.blocks.fill(u64::MAX);
        let used = Self::used_mask(self.size);
        if let Some(last) = self.blocks.last_mut() {
            *last &= used;
        }
    }

    /// Clears every bit.
    ///
    /// Reference: `clear_all()` (`src/cata_bitset.h:227-229`).
    pub fn clear_all(&mut self) {
        self.blocks.fill(0);
    }

    /// Resizes to `bits` bits, keeping the bits that stay in range.
    ///
    /// Reference: `resize()` (`src/cata_bitset.h:138-148`), whose heap path
    /// (`src/cata_bitset.cpp:14-46`) never initialises the words it adds.
    /// This port zeroes them, and masks the bits a shrink drops, so that a
    /// resize is deterministic.
    pub fn resize(&mut self, bits: usize) {
        let words = bits.div_ceil(BITS_PER_BLOCK);
        self.blocks.resize(words, 0);
        self.size = bits;
        let used = Self::used_mask(bits);
        if let Some(last) = self.blocks.last_mut() {
            *last &= used;
        }
    }

    /// The mask of bit `index` within its word: the reference's
    /// `kHighBit >> ( idx % kBitsPerBlock )` (`src/cata_bitset.h:116`).
    fn bit(index: usize) -> u64 {
        1_u64 << (BITS_PER_BLOCK - 1 - index % BITS_PER_BLOCK)
    }

    /// Mask of the bits of the final word that are inside the range.
    fn used_mask(bits: usize) -> u64 {
        let remainder = bits % BITS_PER_BLOCK;
        if remainder == 0 {
            u64::MAX
        } else {
            u64::MAX << (BITS_PER_BLOCK - remainder)
        }
    }
}
