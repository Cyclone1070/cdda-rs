//! What a gunshot costs the hands that fire it, in action points.
//!
//! Reference: `Character::fire_gun` charges `time_to_attack( *this, *gun_id ) + RAS_time( *this, ammo )` (`src/ranged.cpp:1060`) and calls `mod_moves( -attack_moves )` once the burst is over
//! (`ranged.cpp:1197`). The two terms are separate rules and the port keeps them separate:
//!
//! * `time_to_attack` (`ranged.cpp:2299-2308`) is the **skill's** data, not the gun's: a straight line from `base_time` down by `time_reduction_per_level` a level of the gun's own skill, never
//!   below `min_time`. A rifle's line is `50 - 3 a level, floor 20`; a pistol's is `30 - 1 a level, floor 20`. **A gun that states no skill therefore costs its skill's figure, which is why the
//!   numbers belong to the skill and not to the weapon.**
//! * `RAS_time` (`ranged.cpp:2310-2322`) is how long the round takes to *reach* the chamber: nothing while the shooter has wind, and two moves a percent once it drops below twenty-five -
//!   plus the reload option's own moves, which is ammunition's rule and not this one.
//!
//! Neither term is written to the store here: the port spends moves in one place (`creature::spend_moves`) and reads the rule here.

/// The moves a shot costs from the skill alone: `time_to_attack` (`src/ranged.cpp:2299-2308`).
///
/// The three numbers are the skill's `time_to_attack` block - `min_time`, `base_time` and `time_reduction_per_level` - and the reference rounds the line before it takes the floor, which matters
/// only for a reduction that is not a whole number.
///
/// The reference's `skill_used->time_to_attack()` is read from the skill registry, which the port does **not** carry yet: the values arrive as arguments here so that the rule can be gated
/// on its arithmetic, and the loader that fills them is named in `docs/parity.md` rather than guessed.
#[must_use]
pub fn time_to_attack(base_time: i32, reduction_per_level: i32, min_time: i32, skill: i32) -> i32 {
    (base_time - reduction_per_level * skill).max(min_time)
}

/// How much longer a shot takes while the shooter is out of wind: the stamina half of `RAS_time` (`src/ranged.cpp:2310-2322`).
///
/// The reference computes `sta_percent = 100 * stamina / stamina_max` and adds `( 25 - sta_percent ) * 2` only **below** twenty-five - so a shooter at exactly a quarter wind pays nothing, and
/// one at no wind pays fifty moves. **It is a cliff, not a slope**: the term is zero for three quarters of the bar and then rises two moves a percent.
#[must_use]
pub fn winded_cost(stamina_percent: i32) -> i32 {
    if stamina_percent < 25 {
        (25 - stamina_percent) * 2
    } else {
        0
    }
}

/// The line a skill the port does not know falls back to: `time_info_t`'s **defaults** (`skill.h:23-30`), 220, 25 and 50.
///
/// They are marksmanship's own numbers in the data as it stands, which is why a table lookup that misses is not a disaster in the reference - but **it is still a fallback and not a
/// default**: a port that quietly priced every unknown skill at a rifle's 30/1/20 would be seven times too fast and would never notice.
#[must_use]
pub fn unknown_skill_line() -> (i32, i32, i32) {
    (220, 25, 50)
}

/// What firing a gun costs the shooter, with the skill's line looked up from the dumped table.
///
/// Reference: `Character::fire_gun` (`ranged.cpp:1060`) reads `firing.gun->skill_used->time_to_attack()`. The port takes the same route: **the gun names the skill, and the skill owns the
/// line** - so a rifle shot is priced by `rifle`'s 30/1/20 and an arrow by `archery`'s 50/3/20, from the corpus rather than from a table this function invented.
///
/// A gun whose skill is not in the table gets [`unknown_skill_line`], which the reference's own struct defaults make the marksmanship line.
#[must_use]
pub fn cost_of_firing(
    gun: &cdda_content::guns::GunType,
    skills: &[cdda_content::skills::SkillTime],
    skill_level: i32,
    stamina_percent: i32,
    reload_moves: i32,
) -> i32 {
    let line = skills.iter().find(|line| line.id == gun.skill);
    let (base, reduction, min) = line.map_or_else(unknown_skill_line, |line| {
        (line.base_time, line.time_reduction_per_level, line.min_time)
    });
    cost_of_a_shot(base, reduction, min, skill_level, stamina_percent, reload_moves)
}

/// The whole cost of a shot: the skill's line plus the ammunition's reach, which is `RAS_time`'s own sum.
///
/// Reference: `attack_moves = time_to_attack( *this, *gun_id ) + RAS_time( *this, ammo )` (`src/ranged.cpp:1060`). The reload option's moves are the caller's business - they belong to the
/// round being fetched, not to the shot being fired - so they arrive as `reload_moves` and are added here exactly as the reference adds them.
#[must_use]
pub fn cost_of_a_shot(
    base_time: i32,
    reduction_per_level: i32,
    min_time: i32,
    skill: i32,
    stamina_percent: i32,
    reload_moves: i32,
) -> i32 {
    time_to_attack(base_time, reduction_per_level, min_time, skill)
        + winded_cost(stamina_percent)
        + reload_moves
}

/// The blow a gun and the round in it make together.
///
/// Reference: `item::gun_damage( with_ammo = true )` (`item.cpp:11160-11180`) - the weapon's own damage resolved for the barrel, **plus** the round's `shot_damage` resolved for the same
/// barrel. The port keeps the four kinds the data uses for firearms, and the round's blow is added to the gun's rather than replacing it: **a bow's six stab and its arrow's three are seven**,
/// and a rifle's zero plus its cartridge's thirty-nine is thirty-nine.
///
/// What the barrel does to the numbers (`di_considering_length`) is **not** ported: the port has no barrel metric, so every round is priced as if fired from the barrel the data was written for.
/// Said here rather than left to look accidental.
#[must_use]
pub fn blow_of(gun: &cdda_content::guns::GunType, round: Option<&cdda_content::ammo::AmmoType>) -> crate::melee::Damage {
    let round = round.map(|round| (round.damage_bash, round.damage_cut, round.damage_stab, round.damage_bullet));
    let (bash, cut, stab, bullet) = round.unwrap_or((0, 0, 0, 0));
    crate::melee::Damage {
        bash: gun.damage_bash + bash,
        cut: gun.damage_cut + cut,
        // A bullet is a stab in the port's three kinds: the reference carries `bullet` as its own damage type, and the port's armour has no bullet column, so a shot's bullet damage is
        // resolved as **stab** - the closest kind the port can honour - and the docs name it rather than inventing an armour the data does not have.
        stab: gun.damage_stab + stab + gun.damage_bullet + bullet,
        penetration: 0,
    }
}

/// What a shot did, in the terms a caller can act on.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Fired {
    /// The moves the shot cost, already taken from the shooter.
    pub cost: i32,
    /// Where the shot was aimed and how badly it missed.
    pub aim: crate::ballistics::Aim,
    /// The creature standing where it was aimed, when the aim says the shot hit; `None` for a miss or an empty square.
    pub hit: Option<bevy_ecs::prelude::Entity>,
    /// How much the creature lost, which is zero when nothing was there.
    pub hurt: i32,
}

/// Fires a gun at a square, spending the shooter's moves and rolling the shot's own error.
///
/// Reference: `Character::fire_gun` (`src/ranged.cpp:1050-1200`) in the order the port can honour: **the dispersion is composed** (`get_weapon_dispersion`), **the moves are charged**
/// (`mod_moves( -attack_moves )`, `ranged.cpp:1197`), and **the aim is rolled** against the target's size (`projectile_attack_roll`). The reference also consumes ammunition, energy and
/// cycles the action; **ammunition is the half not written yet**, so this spends moves and decides where the round goes, and `hit` says which creature the aim reached without applying
/// damage.
#[allow(clippy::too_many_arguments)]
pub fn fire_at(
    world: &mut bevy_ecs::prelude::World,
    shooter: bevy_ecs::prelude::Entity,
    gun: &cdda_content::guns::GunType,
    lines: &[cdda_content::skills::SkillTime],
    target: (i32, i32, i32),
    divider: f64,
    round: Option<&cdda_content::ammo::AmmoType>,
) -> Fired {
    let blow = blow_of(gun, round);
    let level = crate::creature::skill_level(world, shooter, &gun.skill);
    let shotmaker = crate::dispersion::shotmaker_for(world, shooter, gun, divider);
    let cost = cost_of_firing(gun, lines, level, stamina_percent(world, shooter), 0);
    crate::creature::spend_moves(world, shooter, f64::from(cost));

    // Reference: the shot's own error is drawn from the composed dispersion, one draw per source.
    let sources = crate::dispersion::Dispersion {
        linear: vec![crate::dispersion::weapon_dispersion(shotmaker)],
        ..crate::dispersion::Dispersion::default()
    };
    let rolled = match crate::singletons::take_rolls(world) {
        Some(mut dice) => {
            let drawn = sources.roll(&mut dice);
            crate::singletons::put_rolls(world, dice);
            drawn
        }
        None => 0.0,
    };
    let range = range_to(world, shooter, target);
    let size = crate::throwing::target_size(world, shooter, target);
    let aim = crate::ballistics::aim_of(rolled, range, size);
    // The reference's convention: a fraction missed of one is a miss, and anything less reached the target.
    let struck = if aim.missed_by < 1.0 {
        crate::creature::creature_at(world, target)
            .into_iter()
            .find(|entity| *entity != shooter && crate::creature::health_of(world, *entity).is_some())
    } else {
        None
    };
    let hurt = struck.map_or(0, |struck| hurt_once(world, struck, &blow));
    Fired { cost, aim, hit: struck, hurt }
}

/// The shooter's wind as a whole percentage, which is what `RAS_time` compares against twenty-five.
///
/// Reference: `int sta_percent = ( 100 * p.get_stamina() ) / p.get_stamina_max()` (`ranged.cpp:2314`), integer division and all. A creature with no [`crate::creature::Stamina`] component is
/// full, which is the same convention the throw's arm cost uses.
fn stamina_percent(world: &bevy_ecs::prelude::World, shooter: bevy_ecs::prelude::Entity) -> i32 {
    world
        .get::<crate::creature::Stamina>(shooter)
        .map_or(100, |stamina| {
            #[allow(clippy::cast_possible_truncation)]
            let percent = (100.0 * stamina.cur / stamina.max.max(1.0)) as i32;
            percent
        })
}

/// How far the shooter is from the square it aims at, in squares: `rl_dist`, which counts a diagonal as one.
///
/// Reference: `rl_dist( source, target )` (`src/point.h`) - the **larger** of the two offsets. A shooter with no position is at zero range, which keeps the aim arithmetic finite.
fn range_to(world: &bevy_ecs::prelude::World, shooter: bevy_ecs::prelude::Entity, target: (i32, i32, i32)) -> f64 {
    let Some(from) = world.get::<crate::Position>(shooter) else {
        return 0.0;
    };
    f64::from((target.0 - from.x).abs().max((target.1 - from.y).abs()))
}

/// Applies a shot's blow to the creature it reached, and answers how much it took.
///
/// Reference: `projectile_attack`'s hit branch calls `deal_projectile_attack`, which hands the impact to the target's `absorb_hit` (`monster.cpp:2085`) - the resistance is subtracted
/// **capped at the blow**, so armour can stop a round and never heals what it hits. The port resolves each kind against the same kind of armour and adds what got through.
fn hurt_once(world: &mut bevy_ecs::prelude::World, struck: bevy_ecs::prelude::Entity, blow: &crate::melee::Damage) -> i32 {
    let armour = world.get::<crate::creature::Armour>(struck).copied().unwrap_or_default();
    let mut total = 0;
    for (amount, resist) in [
        (blow.bash, armour.bash),
        (blow.cut, armour.cut),
        (blow.stab, armour.stab),
    ] {
        #[allow(clippy::cast_precision_loss)]
        let through = crate::melee::after_armour(amount as f32, resist as f32, 0.0, 0.0).floor();
        #[allow(clippy::cast_possible_truncation)]
        let through = through as i32;
        total += through.max(0);
    }
    crate::damage_monster(world, struck, total);
    total
}
