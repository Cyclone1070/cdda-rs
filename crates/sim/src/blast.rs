//! A blast: which squares an explosion reaches, and how hard it hits them.
//!
//! Reference: `do_blast` (`src/explosion.cpp:185-310`), the terrain half of `explosion_handler::explosion`. What is ported is the **shape**: the wavefront, the falloff, the
//! shaped-charge bonus, and the order the bashes happen in. What is not ported is named in [`blast`]'s gaps.
//!
//! Two rules are easy to transcribe wrong, and both are visible in a single blast:
//!
//! 1. **The wavefront is a priority queue by distance**, and its comparator is a **total order** — distance first, then the square — so the answers do not depend on the order the
//!    queue happens to hold equal keys in. That is what makes a recording of one blast comparable to another at all.
//! 2. **An empty square next to the wavefront makes the blast hit harder**: the force is multiplied by up to three when a square has few open neighbours, which is the
//!    "shaped charge" the reference's comment describes. A blast in a corridor hits its walls harder than one in the open.

use std::collections::{BTreeMap, BTreeSet};

/// The distance a diagonal step costs, in squares.
///
/// The reference asks the `trigdist` world option (`src/explosion.cpp:189`): with it on, a diagonal is `√2`; with it off, it is one tile like a cardinal. It is on in every
/// world the harness records, and the recorded distances are what pin it.
const DIAGONAL: f32 = std::f32::consts::SQRT_2;

/// What a step up or down costs extra (`src/explosion.cpp:190`).
const LEVEL: f32 = 2.0;

/// The ten neighbours a blast considers: the eight horizontal ones in the reference's own order, then up, then down.
///
/// ```text
/// 7 3 5
/// 1 . 2
/// 6 4 8
/// 9 and 10 are up and down
/// ```
const OFFSETS: [(i32, i32, i32); 10] = [
    (-1, 0, 0),
    (1, 0, 0),
    (0, -1, 0),
    (0, 1, 0),
    (1, -1, 0),
    (-1, 1, 0),
    (-1, -1, 0),
    (1, 1, 0),
    (0, 0, 1),
    (0, 0, -1),
];

/// The map, as far as a blast needs to know it.
pub trait BlastGrid {
    /// Whether a square cannot be walked through. The reference asks `map::impassable`, which is `move_cost == 0`.
    fn solid(&self, x: i32, y: i32) -> bool;
    /// Whether a square is inside the world the blast can reach.
    fn in_bounds(&self, x: i32, y: i32) -> bool;
    /// Whether a creature could step from one square to the other (`map::valid_move`, `src/map.cpp:2490-2504`).
    ///
    /// **Declared gap:** the vertical half of that function — ledges, ramps, climbing — is not ported, and neither is the `bash` argument, which the blast passes as false.
    fn valid_move(&self, from: (i32, i32), to: (i32, i32)) -> bool {
        let _ = from;
        !self.solid(to.0, to.1)
    }
}

/// One square a blast bashed, and the force it hit it with.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BashHit {
    /// Where, as an offset from the centre.
    pub x: i32,
    /// Where, as an offset from the centre.
    pub y: i32,
    /// How hard.
    pub force: f32,
}

/// What a blast did.
#[derive(Debug, Clone, PartialEq)]
pub struct Blast {
    /// The force the centre itself is bashed with before the wavefront starts, which the reference does with its own call (`src/explosion.cpp:197`).
    pub centre_force: f32,
    /// Every square the wavefront bashed, in the order it bashed them.
    pub hits: Vec<BashHit>,
    /// Every square the wavefront reached: where, how far it had travelled and with what force.
    pub reached: Vec<BashHit>,
}

/// Runs one blast.
///
/// **Declared gaps:** the creatures and items on the squares, the fragments and shrapnel, the fire the blast starts, the sound it makes, the `NATURAL_UNDERGROUND` roof it
/// puts back afterwards, and the drawing. `rolls` is the jitter the reference draws once per square it pops — handed in rather than drawn, so a recording is the rule and not the
/// state of the generator; a short list is padded with ones, which is the unjittered geometry.
#[must_use]
pub fn blast(grid: &impl BlastGrid, power: f32, factor: f32, fire: bool, rolls: &[f32]) -> Blast {
    let mut outcome = Blast {
        centre_force: if fire { power } else { 2.0 * power },
        hits: Vec::new(),
        reached: Vec::new(),
    };
    // The queue is the reference's own: a binary heap ordered by distance **only**. See [`DistanceQueue`] for why that has to be a transcription rather than a `BTreeSet`.
    let mut queue = DistanceQueue::new();
    let mut closed: BTreeSet<(i32, i32, i32)> = BTreeSet::new();
    // **Keyed by all three coordinates.** A blast bashes the square above and the square below as well as the eight around it, and keying this set by `(x, y)` would let a
    // vertical bash mark the square under it as done - which is how the port lost the whole near wall before this was fixed.
    let mut bashed: BTreeSet<(i32, i32, i32)> = BTreeSet::new();
    let mut distance_of: BTreeMap<(i32, i32, i32), f32> = BTreeMap::new();
    queue.push(0.0, (0, 0, 0));
    distance_of.insert((0, 0, 0), 0.0);
    let mut next_roll = 0_usize;
    // A square bashed by a blast is bashed **once**: the reference keeps a set of them, so a wavefront that reaches a square twice does not hit it twice.
    bashed.insert((0, 0, 0));
    while let Some((distance, (x, y, z))) = queue.pop() {
        let roll = rolls.get(next_roll).copied().unwrap_or(1.0);
        next_roll += 1;
        let distance = distance * roll;
        if !closed.insert((x, y, z)) {
            continue;
        }
        // The reference does this in `double` and truncates to `float` for the bash (`src/explosion.cpp:254`), and the difference shows: a ninth of a point of force at the
        // third ring is enough to change how hard a wall is hit.
        let force = f64::from(power) * f64::from(factor).powf(f64::from(distance));
        if force <= 1.0 {
            continue;
        }
        #[allow(clippy::cast_possible_truncation)]
        let force = force as f32;
        if grid.solid(x, y) && (x, y, z) != (0, 0, 0) {
            // A square that cannot be walked through is bashed but not passed through: this is what keeps a blast inside a room.
            continue;
        }
        outcome.reached.push(BashHit {
            x,
            y,
            force,
        });
        let empty_neighbors = OFFSETS
            .iter()
            .take(8)
            .filter(|(dx, dy, dz)| {
                let to = (x + dx, y + dy);
                !closed.contains(&(to.0, to.1, z + dz)) && grid.valid_move((x, y), to)
            })
            .count()
            .max(1);
        for (index, (dx, dy, dz)) in OFFSETS.iter().enumerate() {
            let to = (x + dx, y + dy);
            let dest = (to.0, to.1, z + dz);
            if closed.contains(&dest) || !grid.in_bounds(to.0, to.1) {
                continue;
            }
            if bashed.insert(dest) {
                // Up to a twofold bonus for a shaped charge - but a fiery blast gets half the force and no bonus.
                #[allow(clippy::cast_precision_loss)]
                let share = force + (2.0 * force / empty_neighbors as f32);
                let bash_force = if fire { force / 2.0 } else { share };
                // Only bashes on the room's own level are recorded, and only from a node on that level: the wavefront climbs, and a bash it makes one level up is not a
                // bash of the square underneath it.
                if *dz == 0 && z == 0 {
                    outcome.hits.push(BashHit {
                        x: to.0,
                        y: to.1,
                        force: bash_force,
                    });
                }
                // **Declared gap:** the vertical bash. The reference also bashes the square above (with `bash_floor`), and bashes the square it is **standing on** when the
                // square below cannot be reached (`src/explosion.cpp:297-305`) — which bashes one square twice, and which is why a real blast's log is longer than the set of
                // squares it hit. Neither is modelled, because both are about the level above and below, which this port has no view of yet.
            }
            let mut next_distance = distance
                + if dx == &0 || dy == &0 {
                    1.0
                } else {
                    DIAGONAL
                };
            if *dz != 0 {
                if !grid.valid_move((x, y), to) {
                    continue;
                }
                next_distance += LEVEL;
            }
            let better = distance_of.get(&dest).is_none_or(|known| *known > next_distance);
            if better {
                queue.push(next_distance, dest);
                distance_of.insert(dest, next_distance);
            }
            let _ = index;
        }
    }
    outcome
}


/// The reference's wavefront queue, transcribed from the standard library the reference is **built against**.
///
/// It is `std::priority_queue< std::pair<float, tripoint>, std::vector<...>, pair_greater_cmp_first >` (`src/explosion.cpp:238`), whose comparator compares the distance and
/// **nothing else** (`src/cata_utility.h:35-41`). Equal distances therefore come out in whatever order the library's binary heap holds them — not a rule that can be read
/// off the comparator, and not the same rule in every standard library: libstdc++ picks the right child on a tie, `libc++` picks the left, and the two answer different
/// sequences for identical input. The oracle is built with `clang` and `libc++`, so these are `push_heap`'s `__sift_up` and `pop_heap`'s `__floyd_sift_down` from
/// `libc++`'s `<__algorithm/…>` headers, with their exact index arithmetic.
///
/// This is not pedantry: a tie decides which square bashes a neighbour first, and that decides the force the neighbour keeps, because a square is only ever bashed once.
/// `corpus/explosions/heap.json` is the reference draining a fixed list, and the unit test below is the gate on this struct.
pub(crate) struct DistanceQueue {
    items: Vec<(f32, (i32, i32, i32))>,
}

// The indices here are the standard library's own heap arithmetic, and every one of them is bounded by the length of the array it walks; making them checked would be
// transcribing a different algorithm from the one the reference runs.
#[allow(clippy::indexing_slicing)]
impl DistanceQueue {
    pub(crate) fn new() -> Self {
        Self {
            items: Vec::new(),
        }
    }

    /// `push_heap`, which is `__sift_up` over a range of one more than before.
    pub(crate) fn push(&mut self, distance: f32, at: (i32, i32, i32)) {
        self.items.push((distance, at));
        let len = self.items.len();
        self.sift_up(0, len, len);
    }

    /// `pop_heap` followed by a `pop_back`.
    pub(crate) fn pop(&mut self) -> Option<(f32, (i32, i32, i32))> {
        let len = self.items.len();
        if len == 0 {
            return None;
        }
        let top = *self.items.first()?;
        if len == 1 {
            self.items.pop();
            return Some(top);
        }
        let hole = self.floyd_sift_down(0, len);
        let last = len - 1;
        if hole == last {
            self.items[hole] = top;
        } else {
            self.items[hole] = self.items[last];
            let hole = hole + 1;
            self.items[last] = top;
            self.sift_up(0, hole, hole);
        }
        self.items.pop();
        Some(top)
    }

    /// `libc++`'s `__sift_up( first, last, comp, len )`: the element at `last - 1` climbs while it compares "greater" than its parent.
    fn sift_up(&mut self, first: usize, last: usize, len: usize) {
        if len <= 1 {
            return;
        }
        let mut len = (len - 2) / 2;
        let mut ptr = first + len;
        let mut last = last - 1;
        if !less(self.items[ptr], self.items[last]) {
            return;
        }
        let value = self.items[last];
        loop {
            self.items[last] = self.items[ptr];
            last = ptr;
            if len == 0 {
                break;
            }
            len = (len - 1) / 2;
            ptr = first + len;
            if !less(self.items[ptr], value) {
                break;
            }
        }
        self.items[last] = value;
    }

    /// `libc++`'s `__floyd_sift_down`: walk the hole to the bottom, moving the "greater" of the two children up, and answer where it ended.
    fn floyd_sift_down(&mut self, first: usize, len: usize) -> usize {
        let mut it = first;
        loop {
            let child = it - first;
            if len < 2 || (len - 2) / 2 < child {
                break;
            }
            let mut child = 2 * child + 1;
            let mut child_i = first + child;
            if (child + 1) < len && less(self.items[child_i], self.items[child_i + 1]) {
                child_i += 1;
                child += 1;
            }
            let _ = child;
            self.items[it] = self.items[child_i];
            it = child_i;
        }
        it
    }
}

/// The comparator: **distance only**, so an equal pair is neither less nor greater — which is exactly the ambiguity the transcription above has to resolve by hand.
fn less(left: (f32, (i32, i32, i32)), right: (f32, (i32, i32, i32))) -> bool {
    left.0 > right.0
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::panic, clippy::indexing_slicing)]
mod tests {
    //! The queue against the reference's own (`corpus/explosions/heap.json`): the same pushes, the same pops, in the same order.

    use super::DistanceQueue;

    /// The distances the dump pushes, in its order.
    const DISTANCES: [f32; 40] = [
        0.0, 1.0, 1.0, 1.414, 2.0, 2.0, 2.0, 1.0, 2.828, 3.0, 3.0, 2.414, 1.414, 1.414, 4.0, 3.414,
        3.414, 2.0, 5.0, 4.414, 0.5, 0.5, 0.5, 6.0, 6.0, 2.0, 2.0, 7.0, 1.414, 8.0, 0.25, 0.25, 9.0,
        9.0, 9.0, 3.0, 3.0, 10.0, 1.0, 0.75,
    ];

    #[test]
    fn the_queue_drains_in_the_reference_order() {
        let text = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/explosions/heap.json"),
        )
        .expect("the heap probe");
        let mut ours = Vec::new();
        let mut queue = DistanceQueue::new();
        for (index, distance) in DISTANCES.iter().enumerate() {
            queue.push(*distance, (i32::try_from(index).expect("small"), 0, 0));
            if (index + 1) % 7 == 0 {
                while let Some((distance, (at, _, _))) = queue.pop() {
                    ours.push((distance, at));
                }
            }
        }
        while let Some((distance, (at, _, _))) = queue.pop() {
            ours.push((distance, at));
        }
        assert_eq!(ours.len(), 40, "every push comes out");
        let cdda_json::Value::Object(root) =
            cdda_json::parse(text.as_bytes()).expect("the probe is JSON")
        else {
            panic!("root")
        };
        let Some(cdda_json::Value::Array(popped)) = root.get("popped") else {
            panic!("popped")
        };
        let theirs: Vec<(f32, i32)> = popped
            .iter()
            .map(|entry| match entry {
                cdda_json::Value::Array(parts) => {
                    #[allow(clippy::cast_possible_truncation)]
                    match (&parts[0], &parts[1]) {
                        (cdda_json::Value::Float(distance), cdda_json::Value::Int(index)) => {
                            (*distance as f32, i32::try_from(*index).expect("small"))
                        }
                        other => panic!("a pop: {other:?}"),
                    }
                }
                other => panic!("a pop: {other:?}"),
            })
            .collect();
        assert_eq!(theirs.len(), 40, "the dump holds forty pops");
        assert_eq!(ours, theirs, "the port's queue answers the reference's order");
    }
}
