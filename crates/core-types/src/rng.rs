//! The random number engine and distributions, matching the reference draw for draw.
//!
//! Reference: `src/rng.h:30` names the engine `std::minstd_rand0`, a linear
//! congruential generator with multiplier 16807 and modulus `2^31 - 1`. The
//! seeding rule below is the standard library's: reduce the seed modulo the
//! modulus, and substitute `1` if that lands on zero (`[rand.eng.lcong]`).
//!
//! The reference never uses an engine value directly: it draws through
//! default-constructed distributions (`src/rng.cpp:16-36`). Those distributions
//! are implementation-defined, so each one here is transcribed from the libc++
//! headers the reference is built with — the arithmetic, the rejection loops
//! and the order engine values are consumed in all have to be identical:
//!
//! - `uniform_int_distribution` — `__random/uniform_int_distribution.h`,
//!   including the internal `__independent_bits_engine`.
//! - `uniform_real_distribution` — `__random/generate_canonical.h`.
//! - `normal_distribution` — `__random/normal_distribution.h` (Marsaglia polar,
//!   which caches the second value of each pair).
//!
//! Everything that draws from this engine — map generation, combat rolls —
//! inherits its behaviour, so the stream is fixed here and checked against
//! `corpus/rng/engine.csv`, which is produced by the same standard library.

/// The multiplier and modulus of `std::minstd_rand0`.
const MULTIPLIER: u64 = 16807;
const MODULUS: u32 = 2_147_483_647;

/// `std::minstd_rand0`, the reference's engine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MinStdRand0 {
    state: u32,
}

impl MinStdRand0 {
    /// `minstd_rand0::min()`, the smallest value the engine returns.
    pub const MIN: u32 = 1;

    /// `minstd_rand0::max()`, the largest value the engine returns (`m - 1`).
    pub const MAX: u32 = MODULUS - 1;

    /// Seeds the engine, following the standard library's reduction rule.
    #[must_use]
    pub const fn seeded(seed: u32) -> Self {
        let reduced = seed % MODULUS;
        let state = if reduced == 0 { 1 } else { reduced };
        Self { state }
    }

    /// Draws the next value, exactly as `minstd_rand0::operator()` does.
    pub fn next_u32(&mut self) -> u32 {
        let next = (MULTIPLIER * u64::from(self.state)) % u64::from(MODULUS);
        self.state = u32::try_from(next).unwrap_or(1);
        self.state
    }

    /// Draws a uniform integer in `lo..=hi`, matching `std::uniform_int_distribution<int>`
    /// as used by `src/rng.cpp:26`.
    ///
    /// Reference: `__random/uniform_int_distribution.h`, `operator()(g, param)`.
    pub fn uniform_int(&mut self, lo: i32, hi: i32) -> i32 {
        // The library does the arithmetic in `uint32_t` and casts the biased
        // value back to `int`, so a negative `lo` arrives as a wrapped value.
        self.uniform_uint_in(lo.cast_unsigned(), hi.cast_unsigned())
            .cast_signed()
    }

    /// Draws a uniform `unsigned int` over its full range, matching
    /// `std::uniform_int_distribution<unsigned int>()` as used by `src/rng.cpp:16`.
    ///
    /// Reference: `__random/uniform_int_distribution.h`, `operator()(g, param)`
    /// with the default `param_type(0, UINT_MAX)`.
    pub fn uniform_uint(&mut self) -> u32 {
        self.uniform_uint_in(0, u32::MAX)
    }

    /// The unsigned core of `uniform_int_distribution::operator()`, returning
    /// `__u + __p.a()` in `lo..=hi`.
    fn uniform_uint_in(&mut self, lo: u32, hi: u32) -> u32 {
        let range = hi.wrapping_sub(lo).wrapping_add(1);
        if range == 1 {
            return lo;
        }
        if range == 0 {
            // The full 32-bit range wraps `__rp` to 0; libc++ asks for `__dt`
            // bits and returns the draw with no rejection test and *without*
            // adding `__p.a()`, so this branch is transcribed verbatim.
            return IndependentBitsEngine::new(self, RESULT_DIGITS).next();
        }
        let mut engine = IndependentBitsEngine::new(self, bit_width(range));
        let mut value = engine.next();
        while value >= range {
            value = engine.next();
        }
        value.wrapping_add(lo)
    }
}

/// `numeric_limits<uint32_t>::digits`: `__dt` in `uniform_int_distribution` and
/// `__w_rt` in `__independent_bits_engine`.
const RESULT_DIGITS: u32 = 32;

/// `_WDt`, `numeric_limits<_Working_result_type>::digits`. libc++ picks
/// `_Working_result_type` as the wider of the engine's result type and the
/// distribution's; both are 32-bit unsigned here, so this is 32.
const WORKING_DIGITS: u32 = 32;

/// `_EDt`, `numeric_limits<_Engine_result_type>::digits`. `minstd_rand0`'s
/// `result_type` is `unsigned int` (32-bit) on this platform — its values stay
/// below `2^31`, but the type, and therefore this width, does not shrink.
const ENGINE_DIGITS: u32 = 32;

/// `_Rp` in `__independent_bits_engine`: the number of values the engine can
/// return, `minstd_rand0::max() - min() + 1`. Note this is the engine's *range*,
/// `2^31 - 2`, not its modulus `2^31 - 1`.
const ENGINE_RANGE: u32 = MinStdRand0::MAX - MinStdRand0::MIN + 1;

/// `__log2<_Working_result_type, _Rp>` (`__random/log2.h`): the index of the
/// highest set bit of `_Rp`, i.e. how many bits one engine draw contributes.
const ENGINE_RANGE_LOG2: u32 = ENGINE_RANGE.ilog2();

/// `std::__independent_bits_engine<minstd_rand0, uint32_t>`
/// (`__random/uniform_int_distribution.h`).
///
/// Splits the engine's `ENGINE_RANGE_LOG2` usable bits into `n` chunks of
/// `w0` or `w0 + 1` bits, rejecting draws that would bias the high chunk, and
/// concatenates them.
struct IndependentBitsEngine<'a> {
    engine: &'a mut MinStdRand0,
    /// `__n_`: total number of engine draws per value.
    n: u32,
    /// `__w0_`: bits taken from each of the first `__n0_` draws.
    w0: u32,
    /// `__n0_`: number of draws that contribute `w0` bits (the rest take `w0 + 1`).
    n0: u32,
    /// `__y0_`: exclusive upper bound for accepted engine values in the first loop.
    y0: u32,
    /// `__y1_`: the same bound for the second loop, which takes one more bit.
    y1: u32,
    /// `__mask0_`: keeps the low `w0` bits.
    mask0: u32,
    /// `__mask1_`: keeps the low `w0 + 1` bits.
    mask1: u32,
}

impl<'a> IndependentBitsEngine<'a> {
    fn new(engine: &'a mut MinStdRand0, w: u32) -> Self {
        let mut n = w / ENGINE_RANGE_LOG2 + u32::from(!w.is_multiple_of(ENGINE_RANGE_LOG2));
        let mut w0 = w / n;
        let mut y0 = rounded_down_range(w0);
        // `_Rp` is never 0 for this engine, so libc++ takes this branch too.
        if ENGINE_RANGE - y0 > y0 / n {
            n += 1;
            w0 = w / n;
            y0 = rounded_down_range(w0);
        }
        let n0 = n - w % n;
        let y1 = if w0 < ENGINE_DIGITS - 1 {
            rounded_down_range(w0 + 1)
        } else {
            0
        };
        let mask0 = if w0 > 0 {
            u32::MAX >> (ENGINE_DIGITS - w0)
        } else {
            0
        };
        let mask1 = if w0 < ENGINE_DIGITS - 1 {
            u32::MAX >> (ENGINE_DIGITS - (w0 + 1))
        } else {
            u32::MAX
        };
        Self {
            engine,
            n,
            w0,
            n0,
            y0,
            y1,
            mask0,
            mask1,
        }
    }

    /// `__eval(true_type)`: the engine's range is non-zero, so draws may need
    /// rejecting before their low bits are concatenated.
    fn next(&mut self) -> u32 {
        let mut value: u32 = 0;
        for _ in 0..self.n0 {
            let draw = self.draw_below(self.y0);
            value = shift_in(value, self.w0).wrapping_add(draw & self.mask0);
        }
        for _ in self.n0..self.n {
            let draw = self.draw_below(self.y1);
            value = shift_in(value, self.w0 + 1).wrapping_add(draw & self.mask1);
        }
        value
    }

    /// Draws `engine() - min()` repeatedly until it lands below `bound`.
    fn draw_below(&mut self, bound: u32) -> u32 {
        loop {
            let draw = self.engine.next_u32() - MinStdRand0::MIN;
            if draw < bound {
                return draw;
            }
        }
    }
}

/// Chooses the number of bits a `uniform_int_distribution` asks the independent
/// bits engine for: `__w = __dt - countl_zero(__rp) - 1`, rounded up when the
/// range does not fill those bits (`__random/uniform_int_distribution.h`).
fn bit_width(range: u32) -> u32 {
    let w = RESULT_DIGITS - range.leading_zeros() - 1;
    if range & (u32::MAX >> (RESULT_DIGITS - w)) != 0 {
        w + 1
    } else {
        w
    }
}

/// `(_Rp >> bits) << bits`, or 0 when `bits` is a whole word — libc++ guards
/// every one of these shifts so it never shifts by the type's width.
fn rounded_down_range(bits: u32) -> u32 {
    if bits < WORKING_DIGITS {
        (ENGINE_RANGE >> bits) << bits
    } else {
        0
    }
}

/// `__sp <<= bits`, or 0 when the shift would discard the whole word
/// (`if (__w0_ < __w_rt)` in `__eval`).
fn shift_in(value: u32, bits: u32) -> u32 {
    if bits < RESULT_DIGITS {
        value << bits
    } else {
        0
    }
}

impl MinStdRand0 {
    /// Draws a uniform real in `[0, 1)`, matching
    /// `std::uniform_real_distribution<double>()` as used by `src/rng.cpp:31`.
    ///
    /// Reference: `__random/uniform_real_distribution.h`. Its default range is
    /// `[0, 1)`, and `(b - a) * canonical + a` is exact at both ends.
    pub fn uniform_real(&mut self) -> f64 {
        self.uniform_real_in(0.0, 1.0)
    }

    /// `uniform_real_distribution<double>(a, b)`: `(b - a) * canonical + a`.
    /// `normal_distribution` draws its `[-1, 1)` values this way.
    fn uniform_real_in(&mut self, a: f64, b: f64) -> f64 {
        (b - a) * self.generate_canonical() + a
    }

    /// `std::generate_canonical<double, numeric_limits<double>::digits>`
    /// (`__random/generate_canonical.h`): builds one real from a fixed number of
    /// engine draws by summing `(draw - min) * range^i`, then dividing by
    /// `range^k`.
    fn generate_canonical(&mut self) -> f64 {
        /// `numeric_limits<double>::digits`: `__dt`, and also the `__bits`
        /// argument `uniform_real_distribution` passes, so `__b` is 53.
        const DIGITS: u32 = 53;
        /// `__log2<uint64_t, __URNG::max() - __URNG::min() + 1>` — the usable
        /// bits per draw, the same 30 bits the integer path gets.
        const LOG_R: u32 = ENGINE_RANGE.ilog2();
        // `__k = __b / __log_r + (__b % __log_r != 0) + (__b == 0)`.
        let k = DIGITS / LOG_R + u32::from(!DIGITS.is_multiple_of(LOG_R)) + u32::from(DIGITS == 0);
        // `__rp = static_cast<double>(max() - min()) + 1`: the engine's range
        // width as a real, `2147483646`, not the modulus `2147483647`.
        let rp = f64::from(MinStdRand0::MAX - MinStdRand0::MIN) + 1.0;
        let mut base = rp;
        let mut sum = f64::from(self.next_u32() - MinStdRand0::MIN);
        for _ in 1..k {
            sum += f64::from(self.next_u32() - MinStdRand0::MIN) * base;
            base *= rp;
        }
        sum / base
    }
}

/// `std::normal_distribution<>` (`src/rng.cpp:80-83`).
pub struct NormalDistribution {
    /// libc++'s `__v_` / `__v_hot_`: the polar method produces two values per
    /// accepted pair, so the second one is stored and returned next time.
    cached: Option<f64>,
}

impl NormalDistribution {
    /// The default-constructed distribution `src/rng.cpp:82` uses: mean 0, standard
    /// deviation 1.
    #[must_use]
    pub const fn standard() -> Self {
        Self { cached: None }
    }

    /// Draws the next standard normal value, consuming engine output only when
    /// the cache is empty.
    ///
    /// Reference: `__random/normal_distribution.h`, `operator()(g, param)` —
    /// Marsaglia's polar method. Rejected pairs (`s > 1` or `s == 0`) consume
    /// both draws again, so the number of engine draws per pair is not fixed.
    pub fn next(&mut self, engine: &mut MinStdRand0) -> f64 {
        if let Some(value) = self.cached.take() {
            return value;
        }
        let (u, v, s) = loop {
            let u = engine.uniform_real_in(-1.0, 1.0);
            let v = engine.uniform_real_in(-1.0, 1.0);
            let s = u * u + v * v;
            if s <= 1.0 && s != 0.0 {
                break (u, v, s);
            }
        };
        let factor = (-2.0 * s.ln() / s).sqrt();
        self.cached = Some(v * factor);
        // The library's final `__up * stddev() + mean()` is the identity for the
        // default `normal_distribution<>()`.
        u * factor
    }
}
