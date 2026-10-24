//! One number per turn, over every layer a session carries.
//!
//! The session gates each compare one layer - fields here, scent there, the sky above both - and a layer-by-layer comparison has a blind spot: a layer the port
//! forgets to compare is a layer the gates never notice. A **digest** closes that: every square of every layer is written into one canonical byte string and hashed, so
//! "the turn matched" is a single number that either side can be quoted against, and the receipt for a session is one number per turn rather than a list of squares.
//!
//! This is what the milestone's proof line asks for - *replay hash per turn* (`docs/05-roadmap.md`, M2). The hash is FNV-1a, chosen because it is ten lines, has no
//! dependencies, and its only requirement here is that two different states practically never collide; it is not a security primitive and does not need to be.

/// Hashes a session's turn: a list of `(layer, x, y, z, key, value)` entries, in canonical order.
///
/// The caller sorts before hashing, so two states that differ only in the order a container happened to iterate hash the same - the map has no order, and a digest
/// that changed when a hash map rehashed would be a gate that fails for a reason that is not the game.
#[must_use]
#[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
pub fn digest(entries: &[(i32, i32, i32, i32, i64, i64)]) -> u64 {
    let mut sorted = entries.to_vec();
    sorted.sort_unstable();
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for (layer, x, y, z, key, value) in sorted {
        for number in [layer, x, y, z] {
            hash ^= u64::from(number.cast_unsigned());
            hash = hash.wrapping_mul(0x100_0000_01b3);
        }
        // The whole sixty-four bits go in: a key is an id or an age, and folding an id to thirty-two bits would let two different fields collide in the digest far
        // more easily than the hash itself allows.
        hash ^= key.cast_unsigned();
        hash = hash.wrapping_mul(0x100_0000_01b3);
        hash ^= value as u64;
        hash = hash.wrapping_mul(0x100_0000_01b3);
    }
    hash
}

/// Folds one turn's digest into a session's running digest.
///
/// The reference's sessions are replayed turn by turn, so the receipt for a whole session is the fold of the per-turn numbers in order - a single value that changes
/// if any turn changes, in the way its turns changed, which a set-like digest of digests would not.
#[must_use]
pub fn fold(session: u64, turn: usize, turn_digest: u64) -> u64 {
    let mut hash = session;
    for number in [turn as u64, turn_digest] {
        hash ^= number;
        hash = hash.wrapping_mul(0x100_0000_01b3);
    }
    hash
}
