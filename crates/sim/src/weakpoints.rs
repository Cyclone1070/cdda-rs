//! What chance an attack has of hitting a monster's weak point.
//!
//! Reference: `weakpoint::hit_chance` (`src/weakpoint.cpp:536-578`), `weakpoints::select_weakpoint` (`:588-631`) and `reweigh` (`:583-586`). The rule is a **normal distribution's tail**: how
//! far past the weak point's difficulty the attacker's skill sits, read through `erf` for a weak point that helps the attacker and `erfc` for one that hurts the monster, then scaled by the
//! attack type's coverage multiplier and the weak point's own coverage.
//!
//! `erf` is not in Rust's standard library, so it is written here rather than pulled in as a dependency. **The oracle dump is what decides whether it is right**: `corpus/weakpoints/` is the
//! reference's own `hit_chance` over a grid of skill, difficulty and coverage, and the gate requires the port to reproduce every row.

/// The error function.
///
/// **Computed from its own series rather than from memorised rational coefficients**, because a table of magic constants is exactly the kind of number this project refuses to take on trust:
///
/// ```text
/// erf( x ) = ( 2 / sqrt( pi ) ) * sum_{n >= 0} (-1)^n * x^(2n+1) / ( n! * (2n+1) )
/// ```
///
/// The series is evaluated term by term in `f64` until a term is below a billionth of the running total, which it reaches in a few dozen terms for the arguments this rule asks about. **Outside
/// `|x| <= 4` the answer saturates**, and that is not an approximation the game can see: `erfc( 4 )` is `1.5e-8`, so the good branch's chance there is below a millionth of a percent even at full
/// coverage, and the bad branch floors its answer at a tenth long before that. The cut-off is **four rather than three** because three was not enough: the reference's own grid has a row at
/// `z = -3.18` whose chance is `0.00017`, and saturating there answered zero. The dump is what decides: the gate checks all 240 rows.
#[must_use]
pub fn erf(x: f64) -> f64 {
    if x.abs() > 4.0 {
        return if x > 0.0 { 1.0 } else { -1.0 };
    }
    let x2 = x * x;
    let mut term = x;
    let mut sum = x;
    let mut index = 0_u32;
    loop {
        index += 1;
        // term_{n} = term_{n-1} * ( -x^2 / n ) * ( 2n - 1 ) / ( 2n + 1 ), from the series' own ratio.
        let n = f64::from(index);
        term *= -x2 / n * (2.0 * n - 1.0) / (2.0 * n + 1.0);
        sum += term;
        if term.abs() < sum.abs() * 1e-17 || index > 200 {
            break;
        }
    }
    sum * 2.0 / std::f64::consts::PI.sqrt()
}

/// The complementary error function, `1 - erf( x )`.
#[must_use]
pub fn erfc(x: f64) -> f64 {
    erf(x).mul_add(-1.0, 1.0)
}

/// The chance, in percent, that a weak point is hit.
///
/// Reference: `weakpoint::hit_chance` (`weakpoint.cpp:536-578`):
///
/// ```text
/// diff = wp_skill - difficulty                                  (a weak point that helps)
/// diff = wp_skill - max( difficulty + 10, 10 )                   (one that hurts the monster)
/// difficulty_mult = 0.5 * ( 1 + erf( diff / ( 2*sqrt(2) ) ) )    (good)
/// difficulty_mult = max( 0.5 * erfc( diff / ( 2*sqrt(2) ) ), 0.1 )  (bad)
/// chance = coverage_mult * difficulty_mult * coverage
/// ```
///
/// **The bad branch shifts the difficulty by ten and floors the answer at a tenth**, which is the reference's way of saying a monster's bad weak point is hard to avoid entirely - a
/// hopeless attacker still hits it a tenth of the time. The coverage multiplier is the attack type's, one for the "unusual" type the dump holds constant.
#[must_use]
pub fn hit_chance(coverage: f64, difficulty: f64, skill: f64, helps_attacker: bool, coverage_multiplier: f64) -> f64 {
    let shifted = if helps_attacker {
        skill - difficulty
    } else {
        skill - (difficulty + 10.0).max(10.0)
    };
    let z = shifted / (2.0 * std::f64::consts::SQRT_2);
    let difficulty_multiplier = if helps_attacker {
        0.5 * (1.0 + erf(z))
    } else {
        (0.5 * erfc(z)).max(0.1)
    };
    coverage_multiplier * difficulty_multiplier * coverage
}

/// Reweighs the chance of hitting a weak point by how many "rolls" the attacker gets.
///
/// Reference: `reweigh` (`weakpoint.cpp:583-586`): `1 - ( 1 - base )^rolls` - the probability of sampling at least one value under `base` from `rolls` uniform draws, which is the reference's own
/// comment. **A hard-to-hit weak point gains the most**: one percent with five rolls becomes nearly five, while seventy-five percent becomes nearly one - the same curve the comment describes.
#[must_use]
pub fn reweigh(base: f64, rolls: f64) -> f64 {
    1.0 - (1.0 - base).powf(rolls)
}

/// How many draws an attacker of this skill gets at the weak point.
///
/// Reference: `select_weakpoint`'s `float rolls = std::max( 1.0f, 1.0f + attack.wp_skill / 2.5f );` (`weakpoint.cpp:594-596`). A beginner gets one, and every **2.5** points of weak-point skill
/// buys another.
#[must_use]
pub fn rolls_for(skill: f64) -> f64 {
    (1.0 + skill / 2.5).max(1.0)
}

/// One weak point, as the selection rule needs it.
///
/// Reference: `struct weakpoint` (`weakpoint.h:132-166`) - its coverage, its difficulty, and which way it cuts. The port keeps the three figures the **selection** reads; the multipliers and the
/// effects a hit triggers are named in `docs/parity.md` rather than carried here, because a field nothing reads is a second source of truth about what a rule needs.
#[derive(Debug, Clone, PartialEq)]
pub struct Weakpoint {
    /// Its id, which is what the data calls it.
    pub id: String,
    /// The percent chance of hitting it before skill is considered.
    pub coverage: f64,
    /// How hard it is to hit.
    pub difficulty: f64,
    /// Whether hitting it helps the attacker (`is_good`) or hurts the monster (`false`).
    pub helps_attacker: bool,
}

/// Which weak point a roll lands on, or `None` for the body itself.
///
/// Reference: `weakpoints::select_weakpoint` (`weakpoint.cpp:588-631`). Each entry's raw chance is added to a running base, the base is reweighed, and the roll is compared against the **gap** between
/// consecutive reweighed figures. **The comparison is `idx < hit_chance`, strictly**, so a roll exactly on a boundary belongs to the next entry - and the answer is the *index*, because a caller
/// wants to spend the hit, not to hold a reference the store may have moved.
#[must_use]
pub fn select_weakpoint(list: &[Weakpoint], skill: f64, roll: f64) -> Option<usize> {
    let rolls = rolls_for(skill);
    let mut base = 0.0;
    let mut reweighed = 0.0;
    let mut idx = roll;
    for (index, weakpoint) in list.iter().enumerate() {
        let raw = hit_chance(weakpoint.coverage, weakpoint.difficulty, skill, weakpoint.helps_attacker, 1.0);
        if raw == 0.0 {
            continue;
        }
        let new_base = base + raw;
        let new_reweighed = 100.0 * reweigh(new_base / 100.0, rolls);
        let chance = new_reweighed - reweighed;
        if idx < chance {
            return Some(index);
        }
        base = new_base;
        reweighed = new_reweighed;
        idx -= chance;
    }
    None
}
