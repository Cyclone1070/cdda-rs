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
    pub fn uniform_real_in(&mut self, a: f64, b: f64) -> f64 {
        (b - a) * self.generate_canonical() + a
    }

    /// `std::generate_canonical<double, numeric_limits<double>::digits>`
    /// (`__random/generate_canonical.h`): builds one real from a fixed number of
    /// engine draws by summing `(draw - min) * range^i`, then dividing by
    /// `range^k`.
    pub fn generate_canonical(&mut self) -> f64 {
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
#[derive(Debug, Clone, PartialEq)]
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

/// The reference's random-number facade: one engine, plus the distribution
/// objects it keeps in function-local statics (`src/rng.cpp`).
///
/// The wrappers are worth reproducing exactly because everything that rolls
/// anything goes through them, so a wrapper that drew one extra value would
/// shift every later draw. The distributions share one cache of the second
/// normal value, exactly as the reference's statics do.
#[derive(Debug, Clone, PartialEq)]
pub struct Rolls {
    engine: MinStdRand0,
    normal: NormalDistribution,
    /// When a reference recording is attached, every draw comes from it instead of the engine.
    recording: Option<Recording>,
}

/// Every die a reference run rolled, in the order it rolled them.
///
/// The reference records its own draws (`CDDA_RNG_TRACE`), and a port that replays them is comparing **rules on identical
/// dice** rather than two independent draws that happen to agree. That is the difference between a score and a proof, and it
/// is the only way a chunk can be compared square by square.
#[derive(Debug, Clone, PartialEq)]
pub struct Recording {
    /// Every die, in the order the reference rolled them, readable so a test can say what a file actually holds.
    pub draws: Vec<Draw>,
    /// How far into the recording the replay has got.
    pub position: usize,
    /// The values the reference drew **outside** `rng()` - `field_entry::initialize_decay`'s exponential comes straight from the engine
    /// (`src/field.cpp:86-88`) - in order, labelled so a replay can ask for the one it needs.
    pub engine_draws: Vec<(String, i64)>,
}

/// One recorded die.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Draw {
    /// A raw 32-bit value (`rng_bits`).
    Bits(u32),
    /// A whole number drawn between two bounds (`rng`), with both bounds kept so a mismatched call is caught rather than
    /// silently answered.
    Int {
        /// The low bound the reference asked for.
        low: i32,
        /// The high bound.
        high: i32,
        /// What came out.
        drawn: i32,
    },
    /// A real drawn between two bounds (`rng_float`), kept at full precision: a replay has to reach the reference's **value**, not a value in its range,
    /// because what a rule like `roll_remainder( 0.0484 )` decides is whether that particular number landed.
    Real {
        /// The low bound the reference asked for.
        low: f64,
        /// The high bound.
        high: f64,
        /// What came out.
        drawn: f64,
    },
}

impl Recording {
    /// Reads a recording written by the reference's `CDDA_RNG_TRACE`.
    ///
    /// # Errors
    /// Returns the offending line when the file holds something that is not a draw.
    pub fn parse(text: &str) -> Result<Self, String> {
        let mut draws = Vec::new();
        let mut engine_draws: Vec<(String, i64)> = Vec::new();
        for line in text.lines().filter(|line| !line.trim().is_empty()) {
            let fields: Vec<&str> = line.split_whitespace().collect();
            match fields.first().copied() {
                Some("bits") => {
                    let value = fields
                        .get(1)
                        .ok_or_else(|| format!("a bits line without a value: {line}"))?
                        .parse()
                        .map_err(|_| format!("a bits value that is not a number: {line}"))?;
                    draws.push(Draw::Bits(value));
                }
                Some("int") => {
                    let number = |index: usize| -> Result<i32, String> {
                        fields
                            .get(index)
                            .ok_or_else(|| format!("an int line with too few fields: {line}"))?
                            .parse()
                            .map_err(|_| format!("an int field that is not a number: {line}"))
                    };
                    draws.push(Draw::Int {
                        low: number(1)?,
                        high: number(2)?,
                        drawn: number(3)?,
                    });
                }
                Some("real") => {
                    let number = |index: usize| -> Result<f64, String> {
                        fields
                            .get(index)
                            .ok_or_else(|| format!("a real line with too few fields: {line}"))?
                            .parse()
                            .map_err(|_| format!("a real field that is not a number: {line}"))
                    };
                    draws.push(Draw::Real {
                        low: number(1)?,
                        high: number(2)?,
                        drawn: number(3)?,
                    });
                }
                // A value the reference drew outside `rng()`: kept apart from the dice, because it is not one - it is a draw from the engine's
                // raw bits (`field.cpp:86-88`) and a replay that spent it as an `int` would shift every die after it.
                Some("drawn") => {
                    let label = fields
                        .get(1)
                        .ok_or_else(|| format!("a drawn line without a label: {line}"))?;
                    let value = fields
                        .get(2)
                        .ok_or_else(|| format!("a drawn line without a value: {line}"))?
                        .parse()
                        .map_err(|_| format!("a drawn value that is not a number: {line}"))?;
                    engine_draws.push(((*label).to_owned(), value));
                }
                other => return Err(format!("unknown draw kind {other:?} on line {line}")),
            }
        }
        Ok(Self {
            draws,
            position: 0,
            engine_draws,
        })
    }

    /// How many draws are left unplayed.
    #[must_use]
    pub fn remaining(&self) -> usize {
        self.draws.len().saturating_sub(self.position)
    }

    #[allow(clippy::panic)]
    fn next_bits(&mut self) -> u32 {
        let draw = self.draws.get(self.position).copied();
        self.position += 1;
        match draw {
            Some(Draw::Bits(value)) => value,
            // A recording that runs out, or is asked for the wrong kind of die, is a bug in the caller rather than in the
            // data: the port must roll exactly what the reference rolled, in the same order.
            other => panic!("expected a bits draw, found {other:?}"),
        }
    }

    #[allow(clippy::panic)]
    fn next_int(&mut self, low: i32, high: i32) -> i32 {
        let draw = self.draws.get(self.position).copied();
        self.position += 1;
        match draw {
            Some(Draw::Int {
                low: recorded_low,
                high: recorded_high,
                drawn,
            }) => {
                assert_eq!(
                    (recorded_low, recorded_high),
                    (low, high),
                    "the reference asked for a different range here (draw {})",
                    self.position - 1
                );
                drawn
            }
            other => panic!("expected an int draw, found {other:?}"),
        }
    }

    /// A replayed real. A mismatch is a bug in the port, not a condition a caller can handle: the whole point of replaying is that the reference's dice
    /// are the only dice, so asking a question the reference did not ask has to stop the run and say which draw it happened on.
    #[allow(clippy::panic)]
    fn next_real(&mut self, low: f64, high: f64) -> f64 {
        let draw = self.draws.get(self.position).copied();
        self.position += 1;
        match draw {
            Some(Draw::Real {
                low: recorded_low,
                high: recorded_high,
                drawn,
            }) => {
                assert_eq!(
                    (recorded_low, recorded_high),
                    (low, high),
                    "the reference asked for a different real range here (draw {})",
                    self.position - 1
                );
                drawn
            }
            other => panic!("expected a real draw, found {other:?}"),
        }
    }
}

impl Rolls {
    /// Seeds the engine and clears the caches.
    #[must_use]
    pub const fn new(seed: u32) -> Self {
        Self {
            engine: MinStdRand0::seeded(seed),
            normal: NormalDistribution::standard(),
            recording: None,
        }
    }

    /// A run driven by the reference's own dice.
    ///
    /// With a recording attached every `rng`, `rng_bits` and `one_in` call answers from it, keeping the same order and
    /// ranges, so a port following the reference's rules step for step produces the reference's world rather than a
    /// similar one. The engine is still present and still seeded, because draws that a recording does not cover - none
    /// today - would fall through to it.
    #[must_use]
    pub const fn replaying(recording: Recording) -> Self {
        Self {
            engine: MinStdRand0::seeded(0),
            normal: NormalDistribution::standard(),
            recording: Some(recording),
        }
    }

    /// How many recorded draws have been played so far.
    #[must_use]
    pub fn recording_position(&self) -> usize {
        self.recording
            .as_ref()
            .map_or(0, |recording| recording.position)
    }

    /// How many recorded draws remain unplayed, or `None` when this run draws its own dice.
    #[must_use]
    pub fn recording_remaining(&self) -> Option<usize> {
        self.recording.as_ref().map(Recording::remaining)
    }

    /// The engine itself, for the code that draws directly.
    pub const fn engine(&mut self) -> &mut MinStdRand0 {
        &mut self.engine
    }

    /// A whole number in `lo..=hi`, swapping reversed bounds rather than
    /// refusing them (`src/rng.cpp:20-28`).
    pub fn rng(&mut self, lo: i32, hi: i32) -> i32 {
        let (lo, hi) = if lo > hi { (hi, lo) } else { (lo, hi) };
        if let Some(recording) = self.recording.as_mut() {
            return recording.next_int(lo, hi);
        }
        self.engine.uniform_int(lo, hi)
    }

    /// A real in `[lo, hi)`, swapping reversed bounds (`src/rng.cpp:30-42`).
    ///
    /// A bound that is not finite is reported and the call returns zero
    /// (`src/rng.cpp:36-41`) - and, because the reference checks before it
    /// draws, the engine does **not** advance. A caller relying on that stream
    /// position keeps it.
    pub fn rng_float(&mut self, lo: f64, hi: f64) -> f64 {
        let (lo, hi) = if lo > hi { (hi, lo) } else { (lo, hi) };
        if !lo.is_finite() || !hi.is_finite() {
            return 0.0;
        }
        if let Some(recording) = self.recording.as_mut() {
            return recording.next_real(lo, hi);
        }
        self.engine.uniform_real_in(lo, hi)
    }

    /// A normally distributed value (`src/rng.cpp:80-84`).
    pub fn normal_roll(&mut self, mean: f64, stddev: f64) -> f64 {
        // `normal_distribution::operator()` ends with `value * stddev + mean`.
        self.normal.next(&mut self.engine) * stddev + mean
    }

    /// A normal value in `lo..=hi`, clamped to the range (`src/rng.cpp:180-192`).
    ///
    /// The standard deviation is a quarter of the range, so the range is four
    /// deviations wide and the clamp only ever clips the far tail.
    pub fn rng_normal(&mut self, lo: f64, hi: f64) -> f64 {
        let (lo, hi) = if lo > hi { (hi, lo) } else { (lo, hi) };
        let range = (hi - lo) / 4.0;
        if range == 0.0 {
            return hi;
        }
        let value = self.normal_roll(f64::midpoint(hi, lo), range);
        value.clamp(lo, hi)
    }

    /// An exponentially distributed value with the given rate
    /// (`src/rng.cpp:88-92`).
    ///
    /// Reference: `exponential_distribution::operator()` in
    /// `__random/exponential_distribution.h`, which is `-log(1 - u) / lambda`
    /// over one canonical real.
    /// The next value the reference drew outside `rng()` under this label, if a recording is attached.
    ///
    /// The reference's decay draws its exponential from the engine itself (`field.cpp:86-88`), so that value never appears as a die in the
    /// trace. Replaying it is the difference between a port that decays a field on the reference's schedule and one that guesses.
    pub fn recorded_engine_draw(&mut self, label: &str) -> Option<i64> {
        let recording = self.recording.as_mut()?;
        let at = recording
            .engine_draws
            .iter()
            .position(|(name, _)| name == label)?;
        let (_, value) = recording.engine_draws.remove(at);
        Some(value)
    }

    /// An exponential draw with rate `lambda` from the engine's raw bits.
    ///
    /// Reference: `std::exponential_distribution` on `rng_get_engine()` (`src/field.cpp:86-88`). It bypasses `rng()`, so it is **not** a die in the
    /// trace: a replay spends the recorded value instead (see [`Rolls::recorded_engine_draw`]).
    pub fn exponential_roll(&mut self, lambda: f64) -> f64 {
        let drawn = self.engine.generate_canonical();
        -(1.0 - drawn).ln() / lambda
    }

    /// An exponential value measured from `min`, with `mean` as its average
    /// (`src/rng.cpp:104-113`).
    ///
    /// A mean at or below the minimum has no room to draw in, so the reference
    /// returns zero rather than dividing by it.
    pub fn rng_exponential(&mut self, min: f64, mean: f64) -> f64 {
        let adjusted_mean = mean - min;
        if adjusted_mean <= 0.0 {
            return 0.0;
        }
        min + self.exponential_roll(1.0 / adjusted_mean)
    }

    /// A whole unsigned word (`src/rng.cpp:13-18`).
    pub fn rng_bits(&mut self) -> u32 {
        if let Some(recording) = self.recording.as_mut() {
            return recording.next_bits();
        }
        self.engine.uniform_uint()
    }

    /// True with probability `1/chance`, and always for a chance of one or less
    /// (`src/rng.cpp:111-114`).
    pub fn one_in(&mut self, chance: i32) -> bool {
        chance <= 1 || self.rng(0, chance - 1) == 0
    }

    /// True with probability `x/y` (`src/rng.cpp:120-123`).
    pub fn x_in_y(&mut self, x: f64, y: f64) -> bool {
        self.rng_float(0.0, 1.0) <= x / y
    }

    /// The sum of `number` rolls of `1..=sides` (`src/rng.cpp:125-133`).
    pub fn dice(&mut self, number: i32, sides: i32) -> i32 {
        (0..number).map(|_| self.rng(1, sides)).sum()
    }

    /// Rounds probabilistically: the fraction is the chance of rounding away
    /// from zero (`src/rng.cpp:137-150`).
    pub fn roll_remainder(&mut self, value: f64) -> i32 {
        let whole = value.trunc();
        let fraction = value - whole;
        let mut out = whole;
        if value > 0.0 && value > whole && self.x_in_y(fraction, 1.0) {
            out += 1.0;
        } else if value < 0.0 && value < whole && self.x_in_y(-fraction, 1.0) {
            out -= 1.0;
        }
        #[allow(clippy::cast_possible_truncation)]
        let out = out as i32;
        out
    }

    /// One entry of a container, or `default` when it is empty
    /// (`src/rng.h:106-115`).
    ///
    /// Nothing is drawn for an empty container: the reference returns before it
    /// rolls, so asking for an entry that cannot exist does not move the stream.
    pub fn random_entry_or<'a, T>(&mut self, container: &'a [T], default: &'a T) -> &'a T {
        self.random_entry(container).unwrap_or(default)
    }

    /// One entry of a container, or nothing when it is empty
    /// (`src/rng.h:106-115`).
    pub fn random_entry<'a, T>(&mut self, container: &'a [T]) -> Option<&'a T> {
        if container.is_empty() {
            return None;
        }
        #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
        let last = (container.len() - 1) as i32;
        let index = self.rng(0, last);
        #[allow(clippy::cast_sign_loss)]
        container.get(index as usize)
    }
}

/// The chance that a normal roll on a curve comes out at `difficulty` or above
/// (`src/rng.cpp:47-77`).
///
/// This is arithmetic rather than a draw, and it is arithmetic in **single**
/// precision: the reference holds every term in a `float`, so rounding the
/// last place differently would show up in the tail, where the values are
/// small and the comparisons are made.
///
/// A curve with no spread is not a curve: the roll always returns the centre,
/// so the chance is one or zero depending on which side the difficulty falls.
#[must_use]
#[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)]
pub fn normal_roll_chance(center: f32, stddev: f32, difficulty: f32) -> f32 {
    if stddev == 0.0 {
        return if center > difficulty { 1.0 } else { 0.0 };
    }
    let numerator = difficulty - center;
    let denominator = std::f32::consts::SQRT_2 * stddev;
    let complement = 1.0 - erf(numerator / denominator);
    0.5 * complement
}

/// The error function, which the reference gets from `std::erf`.
///
/// The port cannot borrow the platform's, so this reproduces libc++'s
/// approximation closely enough to agree in single precision across the range
/// the game uses, and the corpus pins the values it is held to.
#[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)]
fn erf(x: f32) -> f32 {
    // Abramowitz and Stegun 7.1.26, the form used by the reference's own
    // library for small arguments; the reflection keeps it valid for negatives.
    if x == 0.0 {
        return 0.0;
    }
    let sign = if x < 0.0 { -1.0 } else { 1.0 };
    let x = x.abs();
    let t = 1.0 / (1.0 + 0.327_591_1 * x);
    let y = 1.0
        - (((((1.061_405_4 * t - 1.453_152_1) * t) + 1.421_413_8) * t - 0.284_496_74) * t
            + 0.254_829_6)
            * t
            * (-x * x).exp();
    sign * y
}

/// A reproducible sequence from its own engine, which leaves the shared one
/// alone (`src/rng.cpp:158-175`).
#[must_use]
pub fn rng_sequence(count: usize, lo: i32, hi: i32, seed: u32) -> Vec<i32> {
    let (lo, hi) = if lo > hi { (hi, lo) } else { (lo, hi) };
    let mut engine = MinStdRand0::seeded(seed);
    (0..count).map(|_| engine.uniform_int(lo, hi)).collect()
}

/// `djb2`, the world-seeding hash (`src/rng.cpp:152-165`).
#[must_use]
pub fn djb2_hash(input: &str) -> i32 {
    let mut hash: u32 = 5381;
    for byte in input.bytes() {
        hash = hash.wrapping_mul(33).wrapping_add(u32::from(byte));
    }
    i32::from_ne_bytes(hash.to_ne_bytes())
}
