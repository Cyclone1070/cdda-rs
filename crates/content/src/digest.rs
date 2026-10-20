//! The prototype digest: one 64-bit number per registry, over its sorted id set.
//!
//! Reference: `digest_strings` and `collect()` in the oracle harness, which is a port of the
//! reference's own idea of "did everything load the same way". The algorithm is FNV-1a over the
//! sorted, deduplicated ids, with a zero byte after each id so that `{"ab","c"}` and `{"a","bc"}`
//! hash differently.
//!
//! Matching a digest means the port's registry holds exactly the same ids as the reference's - not
//! merely the same count - which is what makes it a gate rather than a smoke test.

/// The FNV-1a offset basis.
const OFFSET_BASIS: u64 = 14_695_981_039_346_656_037;
/// The FNV-1a prime.
const PRIME: u64 = 1_099_511_628_211;

/// The digest of a registry's ids, sorted and deduplicated first.
#[must_use]
pub fn digest_ids<I, S>(ids: I) -> u64
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut sorted: Vec<String> = ids.into_iter().map(|id| id.as_ref().to_owned()).collect();
    sorted.sort_unstable();
    sorted.dedup();
    let mut hash = OFFSET_BASIS;
    for id in &sorted {
        for byte in id.as_bytes() {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(PRIME);
        }
        hash ^= 0;
        hash = hash.wrapping_mul(PRIME);
    }
    hash
}

/// The digest as the artifact writes it: sixteen lowercase hex digits.
#[must_use]
pub fn digest_hex(digest: u64) -> String {
    format!("{digest:016x}")
}
