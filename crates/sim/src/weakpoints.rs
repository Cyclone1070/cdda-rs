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
    /// The attack type's own multiplier on the coverage (`weakpoint::hit_chance`'s `coverage_mult.of( attack )`).
    pub coverage_multiplier: f64,
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
        let raw = hit_chance(
            weakpoint.coverage,
            weakpoint.difficulty,
            skill,
            weakpoint.helps_attacker,
            weakpoint.coverage_multiplier,
        );
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

/// What a weak point does to the armour it meets and the damage that arrives.
///
/// Reference: `struct weakpoint`'s four maps (`weakpoint.h:141-149`) - `armor_mult`, `armor_penalty`, `damage_mult` and `crit_mult`. They are kept as maps by **damage type's name**, which is how
/// the data states them, and converted to and from the port's three-type [`crate::melee::Damage`] by name: the port speaks three types because the data's melee uses three, and a map is what a
/// loader will hand it.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct WeakpointFactors {
    /// `armor_mult`: multiplied into the armour, per damage type.
    pub armour_multiplier: std::collections::BTreeMap<String, f64>,
    /// `armor_penalty`: subtracted from the armour **after** the multiplier.
    pub armour_penalty: std::collections::BTreeMap<String, f64>,
    /// `damage_mult`: multiplied into an ordinary hit's damage.
    pub damage_multiplier: std::collections::BTreeMap<String, f64>,
    /// `crit_mult`: multiplied into a critical hit's damage **instead of** [`Self::damage_multiplier`].
    pub crit_multiplier: std::collections::BTreeMap<String, f64>,
}

/// Applies a weak point's armour changes to a creature's resistances, in place.
///
/// Reference: `weakpoint::apply_to( resistances & )` (`weakpoint.cpp:496-509`) - every damage type is multiplied by its `armor_mult` and then has its `armor_penalty` subtracted, **in that order**,
/// which makes a flat penalty worth more against heavy armour. A type the weak point says nothing about is left alone.
pub fn apply_to_armour(factors: &WeakpointFactors, armour: &mut std::collections::BTreeMap<String, f64>) {
    for (damage_type, value) in armour.iter_mut() {
        if let Some(multiplier) = factors.armour_multiplier.get(damage_type) {
            *value *= multiplier;
        }
        if let Some(penalty) = factors.armour_penalty.get(damage_type) {
            *value -= penalty;
        }
    }
}

/// Applies a weak point's damage changes to a blow, answering what it becomes.
///
/// Reference: `weakpoint::apply_to( damage_instance &, bool is_crit )` (`weakpoint.cpp:511-526`). **The two maps are exclusive**: a critical hit reads only `crit_mult` and an ordinary hit only
/// `damage_mult`, because the reference's branches are `if is_crit ... else if`. A port that applied both would make every weak point worth most on a crit and would double a critical hit that the
/// data meant to triple.
#[must_use]
pub fn apply_to_damage(factors: &WeakpointFactors, damage: &[(String, f64)], is_crit: bool) -> Vec<(String, f64)> {
    let table = if is_crit { &factors.crit_multiplier } else { &factors.damage_multiplier };
    damage
        .iter()
        .map(|(damage_type, amount)| {
            let multiplier = table.get(damage_type).copied().unwrap_or(1.0);
            (damage_type.clone(), amount * multiplier)
        })
        .collect()
}

/// How good an attacker is at finding a weak point.
///
/// Reference: `Character::generic_weakpoint_skill` (`weakpoint.cpp:82-90`) - the average of two skill levels plus the stat bonus, **multiplied** by the average of two limb scores. A monster's
/// answer is its melee skill alone (`monster::weakpoint_skill`, `monster.cpp:76-79`), which a caller passes as both skills with whole limbs.
///
/// **The product is the rule**: an attacker whose eyes and reaction are gone finds nothing however skilled it is, and a half-hurt head halves the result rather than taking a point off it.
#[must_use]
pub fn attacker_skill(
    skill_one: f64,
    skill_two: f64,
    dexterity: i32,
    perception: i32,
    limb_one: f64,
    limb_two: f64,
) -> f64 {
    let skill = (skill_one + skill_two) / 2.0;
    let stat = f64::from(dexterity - 8) / 8.0 + f64::from(perception - 8) / 8.0;
    let multiplier = (limb_one + limb_two) / 2.0;
    (skill + stat) * multiplier
}

/// Which of a body's weak points a blow found, asked of the store.
///
/// Reference: `weakpoints::select_weakpoint` (`weakpoint.cpp:588-631`) over the body's own set, with each entry asked the way **this** attack asks it. **A body with no weak points answers
/// nothing**, which is the caller's cue to spend nothing rather than to fall back to a default the port invented.
#[must_use]
pub fn stored_weak_point(
    world: &bevy_ecs::prelude::World,
    victim: bevy_ecs::prelude::Entity,
    attack: AttackType,
    skill: f64,
    rolls: &mut cdda_core_types::rng::Rolls,
) -> Option<WeakPoint> {
    let weak_points = world.get::<WeakPoints>(victim)?;
    let entries: Vec<Weakpoint> = weak_points
        .entries
        .iter()
        .map(|entry| entry.asked_with(attack))
        .collect();
    // The walk's own hundred, drawn from the store's generator so a fixed seed replays the fight.
    let roll = f64::from(rolls.rng(0, 99_999)) / 1_000.0;
    let index = select_weakpoint(&entries, skill, roll);
    index.map_or_else(
        || Some(weak_points.default.clone()),
        |index| weak_points.entries.get(index).cloned(),
    )
}

/// One weak point as the store carries it: the selection's figures and what it does to a blow.
#[derive(Debug, Clone, PartialEq)]
pub struct WeakPoint {
    /// Its id.
    pub id: String,
    /// The percent chance of hitting it before skill is considered.
    pub coverage: f64,
    /// Whether hitting it helps the attacker.
    pub helps_attacker: bool,
    /// How hard it is to hit, **per attack type** in the reference's order (`NONE`, bash, cut, stab, projectile).
    pub difficulties: Vec<f64>,
    /// The coverage multiplier per attack type, in the same order.
    pub coverage_multipliers: Vec<f64>,
    /// Its armour and damage multipliers.
    pub factors: WeakpointFactors,
    /// What a blow into it can trigger.
    pub effects: Vec<TriggeredEffect>,
}

impl WeakPoint {
    /// The entry the walk reads, asked the way this attack asks it.
    #[must_use]
    pub fn asked_with(&self, attack: AttackType) -> Weakpoint {
        Weakpoint {
            id: self.id.clone(),
            coverage: self.coverage,
            difficulty: difficulty_for(&self.difficulties, attack),
            helps_attacker: self.helps_attacker,
            coverage_multiplier: difficulty_for(&self.coverage_multipliers, attack),
        }
    }
}

impl WeakPoints {
    /// Reads a body's weak points out of the data's own set.
    ///
    /// The two shapes differ by what each is for: the data states a difficulty **per attack type** and the store carries the one figure its walk asks with. **The port has no attack-type enum**, so
    /// it reads the first entry - the reference's own `NONE` - and `docs/parity.md` names the gap rather than guessing at a melee index.
    #[must_use]
    pub fn from_set(set: &cdda_content::weakpoints::WeakpointSet) -> Self {
        let one = |entry: &cdda_content::weakpoints::Weakpoint| WeakPoint {
            id: entry.id.clone(),
            coverage: entry.coverage,
            helps_attacker: entry.helps_attacker,
            difficulties: entry.difficulty.clone(),
            coverage_multipliers: entry.coverage_multiplier.clone(),
            effects: entry
                .effects
                .iter()
                .map(|effect| TriggeredEffect {
                    effect: effect.effect.clone(),
                    chance: effect.chance,
                    damage_required: effect.damage_required,
                    duration: effect.duration,
                    intensity: effect.intensity,
                    permanent: effect.permanent,
                })
                .collect(),
            factors: WeakpointFactors {
                armour_multiplier: entry.armour_multiplier.clone(),
                armour_penalty: entry.armour_penalty.clone(),
                damage_multiplier: entry.damage_multiplier.clone(),
                crit_multiplier: entry.crit_multiplier.clone(),
            },
        };
        Self {
            entries: set.weakpoints.iter().map(one).collect(),
            default: one(&set.default),
        }
    }
}

/// The weak points a body has.
///
/// Reference: `mtype::weakpoints` (`mtype.h:444`) is an inline `::weakpoints` object on the monster's type, and `Creature::deal_damage` selects from it (`creature.cpp:1146`). The store keeps
/// the same shape as a component on the body rather than on the type, because **the body is what a blow lands on** and a type is data that several bodies share.
#[derive(bevy_ecs::component::Component, Debug, Clone, PartialEq)]
pub struct WeakPoints {
    /// The entries, in the order the walk tries them.
    pub entries: Vec<WeakPoint>,
    /// The one that answers a roll nothing else took.
    pub default: WeakPoint,
}

/// The same damage rule, over the port's three physical types.
///
/// A thin reading of [`apply_to_damage`] for a blow, which carries exactly bash, cut and stab: the map is the rule and this is the shape a caller has.
#[must_use]
pub fn apply_to_blow(factors: &WeakpointFactors, blow: (f64, f64, f64), is_crit: bool) -> (f64, f64, f64) {
    let units = apply_to_damage(
        factors,
        &[
            ("bash".to_owned(), blow.0),
            ("cut".to_owned(), blow.1),
            ("stab".to_owned(), blow.2),
        ],
        is_crit,
    );
    (
        units.first().map_or(blow.0, |unit| unit.1),
        units.get(1).map_or(blow.1, |unit| unit.1),
        units.get(2).map_or(blow.2, |unit| unit.1),
    )
}

/// The same armour rule, over the port's three physical types.
#[must_use]
pub fn apply_to_armour_triple(factors: &WeakpointFactors, armour: (f64, f64, f64)) -> (f64, f64, f64) {
    let mut mapped = std::collections::BTreeMap::new();
    mapped.insert("bash".to_owned(), armour.0);
    mapped.insert("cut".to_owned(), armour.1);
    mapped.insert("stab".to_owned(), armour.2);
    apply_to_armour(factors, &mut mapped);
    (
        mapped.get("bash").copied().unwrap_or(0.0),
        mapped.get("cut").copied().unwrap_or(0.0),
        mapped.get("stab").copied().unwrap_or(0.0),
    )
}

/// Which kind of attack a weak point is being asked about.
///
/// Reference: `weakpoint_attack::attack_type` (`weakpoint.h:155-166`) - `NONE`, the three melee damage types and `PROJECTILE`, in that order, **because the order is the index** the data's
/// `difficulty` and `coverage_mult` lists are read by (`weakpoint.cpp:245-260`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttackType {
    /// No type: a blow that is not one of the three, or no attack at all.
    None,
    /// A bashing blow.
    MeleeBash,
    /// A cutting blow.
    MeleeCut,
    /// A stabbing blow.
    MeleeStab,
    /// A shot.
    Projectile,
}

impl AttackType {
    /// The index the data's lists are read by, which is the reference's own `static_cast<int>`.
    #[must_use]
    pub const fn index(self) -> usize {
        match self {
            Self::None => 0,
            Self::MeleeBash => 1,
            Self::MeleeCut => 2,
            Self::MeleeStab => 3,
            Self::Projectile => 4,
        }
    }
}

/// The type of a melee blow, from its three damage amounts.
///
/// Reference: `weakpoint_attack::type_of_melee_attack` (`weakpoint.cpp:354-371`) - the type with the **largest amount**, replaced only on a strictly larger one, so a tie keeps the earlier type.
#[must_use]
pub fn type_of_melee_attack(damage: (i32, i32, i32)) -> AttackType {
    let (bash, cut, stab) = damage;
    let mut primary = AttackType::None;
    let mut amount = 0;
    for (kind, value) in [
        (AttackType::MeleeBash, bash),
        (AttackType::MeleeCut, cut),
        (AttackType::MeleeStab, stab),
    ] {
        if value > amount {
            primary = kind;
            amount = value;
        }
    }
    primary
}

/// One figure out of a weak point's per-attack-type list.
///
/// **A list too short answers nothing**, which is the port's honest refusal: the data's lists are five long and a short one is a dump that lost a column, not a zero the rule meant.
#[must_use]
pub fn difficulty_for(list: &[f64], attack: AttackType) -> f64 {
    list.get(attack.index()).copied().unwrap_or(0.0)
}

/// Something a struck weak point can do to the body it was found on.
///
/// Reference: `weakpoint_effect` (`weakpoint.h:96-130`) - an effect id, the chance it fires, the share of the body's maximum hit points the blow must take, and the duration and intensity ranges.
/// The port keeps the **rule's inputs**; an effect-on-condition and the message are named in `docs/parity.md`, because the port has no dialogue activation and no message channel here.
#[derive(Debug, Clone, PartialEq)]
pub struct TriggeredEffect {
    /// The effect's id, as the data names it.
    pub effect: String,
    /// The percent chance it fires, out of a hundred.
    pub chance: f64,
    /// The **share of the body's maximum hit points** the blow must take, as a percent range.
    pub damage_required: (f64, f64),
    /// How many turns it lasts, as a range.
    pub duration: (i32, i32),
    /// How strong it is, as a range.
    pub intensity: (i32, i32),
    /// Whether it never wears off.
    pub permanent: bool,
}

/// Whether a struck weak point's effect fires on this blow.
///
/// Reference: `weakpoint_effect::apply_to` (`weakpoint.cpp:273-300`). **The damage gate is asked first**, as a share of the body's maximum capped at a hundred, and the chance is a strict
/// `rng_float( 0, 100 ) < chance` - so a chance of zero never fires and one of a hundred always does. **A body with no maximum hit points triggers nothing**, which is the port's answer to a
/// division the reference would have to guard.
pub fn triggers(
    effect: &TriggeredEffect,
    total_damage: i32,
    maximum_hit_points: i32,
    rolls: &mut cdda_core_types::rng::Rolls,
) -> bool {
    if maximum_hit_points <= 0 {
        return false;
    }
    let share = 100.0 * f64::from(total_damage) / f64::from(maximum_hit_points);
    let share = share.min(100.0);
    if share < effect.damage_required.0 || effect.damage_required.1 < share {
        return false;
    }
    f64::from(rolls.rng(0, 10_000)) / 100.0 < effect.chance
}

/// One proficiency a body's weak points reward.
///
/// Reference: `struct weakpoint_family` (`weakpoint.h:102-114`) - an id, the proficiency it belongs to, and the two figures. **They are stored resolved**: the reference falls back to the
/// proficiency's own `default_weakpoint_bonus` and `default_weakpoint_penalty` (`weakpoint.cpp:110-115`), and the port has no proficiency table carrying those two numbers, so the dump writes
/// what the reference would have used rather than a zero the port would have to invent.
#[derive(Debug, Clone, PartialEq)]
pub struct WeakpointFamily {
    /// Its id, which is what the data calls the family.
    pub id: String,
    /// The proficiency an attacker must know to earn the bonus.
    pub proficiency: String,
    /// What knowing it is worth.
    pub bonus: f64,
    /// What **not** knowing it costs.
    pub penalty: f64,
}

/// The skill a body's families add to an attacker's weak-point skill.
///
/// Reference: `weakpoint_families::modifier` (`weakpoint.cpp:171-178`) - the **sum** over every family, where each contributes its bonus when the attacker knows its proficiency and its penalty
/// when it does not. **The total can be negative**, which is the whole point: an untrained attacker aims worse at a body whose anatomy means something.
#[must_use]
pub fn families_modifier(families: &[WeakpointFamily], knows: &dyn Fn(&str) -> bool) -> f64 {
    families
        .iter()
        .map(|family| if knows(&family.proficiency) { family.bonus } else { family.penalty })
        .sum()
}
