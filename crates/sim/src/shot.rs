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

/// How far a pellet's own flight wanders: the ammunition's `shot_spread`, drawn as a normal angle and capped at thirty degrees.
///
/// Reference: `ballistics.cpp:395-403` - `rng_normal( -proj.shot_spread, proj.shot_spread )`, the **absolute value** of the draw, clamped to `30_degrees` of arc, and then turned into a
/// fraction of the range by `tan( arc / 2 ) * 2.0`. **The cap is the rule that keeps a 300-arcmin birdshot sane**: thirty degrees is 1,800 arcmin, and without the clamp a wide round would
/// rotate its target square further than the map allows.
#[must_use]
pub fn shot_spread_arcmin(shot_spread: i32, dice: &mut cdda_core_types::rng::Rolls) -> f64 {
    if shot_spread <= 0 {
        return 0.0;
    }
    let spread = f64::from(shot_spread);
    dice.rng_normal(-spread, spread).abs().min(30.0 * 60.0)
}

/// What an arc of spread comes to as a fraction of a square's width, and whether it is enough to leave the target square entirely.
///
/// Reference: `ballistics.cpp:399-405`: `spread = tan( arc / 2 ) * 2.0` and `max_spread = spread * ( trajectory.size() - 1 )`, and **only a `max_spread` of one or more moves the target
/// square** - so a single-tile shot's spread never displaces its aim, however wide the round. That is why a shotgun's pellets scatter at range and a rifle's do not, from one formula.
#[must_use]
pub fn spread_across_tiles(arcmin: f64, tiles: usize) -> f64 {
    let radians = crate::ballistics::arcmin_to_radians(arcmin);
    let spread = (radians / 2.0).tan() * 2.0;
    #[allow(clippy::cast_precision_loss)]
    let span = spread * (tiles.saturating_sub(1)) as f64;
    span
}

/// How much recoil a creature is carrying, in the reference's own units.
///
/// Reference: `Character::recoil` (`character.h:3115-3120`), a `double` capped at `MAX_RECOIL` = **3000** (`game_constants.h:101`). It accumulates as `5 * gun_recoil` a shot
/// (`ranged.cpp:1152-1154`, the reference's own "temporarily scale by 5x" comment) and it is **added to the shot's dispersion**, so a second shot from the same hands is worse than the
/// first. Kept as a component so that "how shaken is this shooter" is one field on one entity.
#[derive(bevy_ecs::prelude::Component, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Recoil {
    /// The points of accumulated kick.
    pub points: i32,
}

/// How fouled a weapon is: the reference's `dirt` variable on the gun item (`ranged.cpp:704`), whose maximum is **10,000**.
///
/// Reference: `it.get_var( "dirt", 0 )` read at the top of `handle_gun_damage` and written back at its end. Kept as a component on the gun entity, so "how dirty is this gun" is one field on the
/// thing that is dirty.
#[derive(bevy_ecs::prelude::Component, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Dirt {
    /// The points of fouling, capped by the reference at 10,000.
    pub points: i32,
}

/// A weapon that cannot jam: the reference's `flag_NEVER_JAMS` (`ranged.cpp:709`), carried by the item and read by every gun-damage branch.
///
/// A marker component rather than a boolean parameter, because it is a **fact about the gun** and not about the shot: the flag lives on the item in the reference too, and the port puts it on the
/// item's entity.
#[derive(bevy_ecs::prelude::Component, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct NeverJams;

/// The most dirt a gun can carry: the reference's own `dirt_max_dbl` (`ranged.cpp:695`), which is also where the misfire chance reaches one.
pub const MAX_DIRT: i32 = 10_000;

/// The most recoil the reference lets a shooter carry: `MAX_RECOIL` (`game_constants.h:101`).
pub const MAX_RECOIL: i32 = 3000;

/// What one shot adds to the shooter's recoil: `recoil += 5.0 * qty` (`ranged.cpp:1152-1154`), capped at [`MAX_RECOIL`].
///
/// The five is the reference's own: it is there because `MAX_RECOIL` is being rescaled, and a port that "cleaned it up" by dividing everywhere by five would break the cap and the dispersion
/// it feeds in the same way.
#[must_use]
pub fn recoil_after_a_shot(current: i32, kick: i32) -> i32 {
    (current + 5 * kick).min(MAX_RECOIL)
}

/// What a shot leaves behind when the weapon reloads as it fires, which is a bow.
///
/// Reference: `ranged.cpp:1230-1233` - **if the weapon has `RELOAD_AND_SHOOT`, the shooter's recoil is reset to `MAX_RECOIL`** rather than added to, because a bow's "recoil" is the draw
/// that has to be built again from nothing. A gun instead carries its kick forward with [`recoil_after_a_shot`]. **Two rules, one field of data** (`RELOAD_AND_SHOOT` is a flag on the item,
/// and a longbow and a compound bow both state it while a rifle states nothing), so the port reads the flag rather than guessing from a volume.
#[must_use]
pub fn recoil_after_firing(current: i32, kick: i32, reload_and_shoot: bool) -> i32 {
    if reload_and_shoot {
        MAX_RECOIL
    } else {
        recoil_after_a_shot(current, kick)
    }
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
    /// Whether the rounds the shot needed were in the gun and were spent. A shot that could not load is **not fired**: it costs nothing and hits nothing.
    pub loaded: bool,
    /// How loud the shot was, in the reference's units - the number of squares over which it is still heard. Zero for a shot that never loaded.
    pub noise: i32,
    /// Whether the gun misfired instead of firing: the round was **not** spent, nothing was hit, and no moves were charged.
    ///
    /// Reference: `if( !handle_gun_damage( gun ) ) break;` at the top of the shot loop (`ranged.cpp:1078`), which stops the burst **before** the projectile is made.
    pub misfired: bool,
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
    gun_entity: Option<bevy_ecs::prelude::Entity>,
    gun_weight_g: f64,
    reload_and_shoot: bool,
) -> Fired {
    // Reference: `handle_gun_damage` is called **before** the shot is loosed (`ranged.cpp:1078`), and a gun that misfires spends no round, hits nothing and costs no moves.
    let dirt = gun_entity
        .and_then(|gun_entity| world.get::<Dirt>(gun_entity))
        .map_or(0, |dirt| dirt.points);
    let mut misfired = false;
    if let Some(mut dice) = crate::singletons::take_rolls(world) {
        let never_jams = gun_entity.is_some_and(|gun_entity| world.get::<NeverJams>(gun_entity).is_some());
        misfired = misfires(dirt, gun.durability, never_jams, &mut dice);
        crate::singletons::put_rolls(world, dice);
    }
    if misfired {
        return Fired { cost: 0, aim: crate::ballistics::aim_of(0.0, 0.0, 1.0), hit: None, hurt: 0, loaded: true, noise: 0, misfired: true };
    }
    // Reference: `fire_gun` checks `gun.ammo_required()` and consumes the rounds **before** the shot is loosed (`ranged.cpp:1168-1172`), and breaks out of the burst when the gun cannot
    // load. A gun the port has no entity for is treated as loaded, which keeps this rule usable without one.
    let loaded = gun_entity.is_none_or(|gun_entity| crate::shot::spend_rounds(world, gun_entity, gun.ammo_to_fire));
    if !loaded {
        return Fired { cost: 0, aim: crate::ballistics::aim_of(0.0, 0.0, 1.0), hit: None, hurt: 0, loaded, noise: 0, misfired: false };
    }
    let blow = blow_of(gun, round);
    let level = crate::creature::skill_level(world, shooter, &gun.skill);
    let shotmaker = crate::dispersion::shotmaker_for(world, shooter, gun, divider);
    let cost = cost_of_firing(gun, lines, level, stamina_percent(world, shooter), 0);
    crate::creature::spend_moves(world, shooter, f64::from(cost));

    // Reference: the shot's own error is drawn from the composed dispersion, one draw per source.
    // Reference: `total_gun_dispersion( gun, recoil_total(), proj.shot_spread )` (`ranged.cpp:1108`) folds the shooter's **accumulated recoil** into the same dispersion the weapon's own
    // figures make - so a shooter who has already fired is aiming worse with the identical gun.
    let carried = world.get::<Recoil>(shooter).map_or(0, |recoil| recoil.points);
    let sources = crate::dispersion::Dispersion {
        linear: vec![crate::dispersion::weapon_dispersion(shotmaker) + f64::from(carried)],
        ..crate::dispersion::Dispersion::default()
    };
    let range = range_to(world, shooter, target);
    let size = crate::throwing::target_size(world, shooter, target);
    // Reference: `for( int j = 0; j < proj.count; ++j )` (`ballistics.cpp:388`) - **a round with a count fires that many pellets**, each with its own spread, its own aim and its own blow.
    // A cartridge's count is one, so a rifle is unchanged; a birdshot's is twenty, which is what makes a shotgun a shotgun rather than a heavy rifle.
    let pellets = round.map_or(1, |round| round.count).max(1);
    let mut hits = 0;
    let mut hurt = 0;
    let mut last_aim = crate::ballistics::aim_of(0.0, range, size);
    for _ in 0..pellets {
        let mut scatter = 0.0;
        let rolled = match crate::singletons::take_rolls(world) {
            Some(mut dice) => {
                let arcmin = shot_spread_arcmin(round.map_or(0, |round| round.shot_spread), &mut dice);
                #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                let tiles = range.max(1.0).round() as usize;
                scatter = spread_across_tiles(arcmin, tiles);
                let drawn = sources.roll(&mut dice);
                crate::singletons::put_rolls(world, dice);
                drawn
            }
            None => 0.0,
        };
        let mut aim = crate::ballistics::aim_of(rolled, range, size);
        // The reference's own threshold: a scatter of a whole square or more takes the pellet off the target square, so it misses whatever the aim roll said.
        if scatter >= 1.0 {
            aim.missed_by = aim.missed_by.max(1.0);
        }
        last_aim = aim;
        // The reference's convention: a fraction missed of one is a miss, and anything less reached the target.
        if aim.missed_by < 1.0
            && let Some(struck) = crate::creature::creature_at(world, target)
                .into_iter()
                .find(|entity| *entity != shooter && crate::creature::health_of(world, *entity).is_some())
        {
            hits += 1;
            hurt += hurt_once(world, struck, &blow);
        }
    }
    let aim = last_aim;
    let struck = crate::creature::creature_at(world, target)
        .into_iter()
        .find(|entity| *entity != shooter && crate::creature::health_of(world, *entity).is_some())
        .filter(|_| hits > 0);
    // Reference: `recoil += 5 * qty` after the shot (`ranged.cpp:1152-1154`), which is what makes the second shot worse than the first.
    let kick = gun_recoil(
        gun.recoil,
        round.map_or(0, |round| round.recoil),
        gun.handling,
        gun_weight_g,
        crate::creature::effective_strength(world, shooter),
    );
    let carried = world.get::<Recoil>(shooter).map_or(0, |recoil| recoil.points);
    let after = recoil_after_firing(carried, kick, reload_and_shoot);
    if let Some(mut recoil) = world.get_mut::<Recoil>(shooter) {
        recoil.points = after;
    } else {
        world.entity_mut(shooter).insert(Recoil { points: after });
    }
    // Reference: `sounds::sound( pos, gun_noise.volume, ... )` after the shot (`ranged.cpp:1180-1190`), which is what makes firing a gun a thing the world hears rather than a private
    // transaction between the shooter and its target.
    let noise = gun_noise(gun.loudness, round.map_or(0, |round| round.loudness));
    if let Some(place) = world.get::<crate::Position>(shooter) {
        crate::sounds::make_sound(world, (place.x, place.y, place.z), noise);
    }
    // Reference: `it.set_var( "dirt", ... )` at the end of `handle_gun_damage` (`ranged.cpp:864-872`), which is why a gun that keeps firing eventually jams.
    if let Some(gun_entity) = gun_entity {
        let dirt = world.get::<Dirt>(gun_entity).map_or(0, |dirt| dirt.points);
        let fouled = dirt_after_a_shot(dirt, false, 0, false);
        if let Some(mut holder) = world.get_mut::<Dirt>(gun_entity) {
            holder.points = fouled;
        } else {
            world.entity_mut(gun_entity).insert(Dirt { points: fouled });
        }
    }
    Fired { cost, aim, hit: struck, hurt, loaded: true, noise, misfired: false }
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

/// Takes the rounds a shot needs out of the gun, and answers whether there were enough.
///
/// Reference: `gun.ammo_consume( required, ... )` (`ranged.cpp:1167-1172`) returns how many it took, and `fire_gun` breaks out of the burst when that is fewer than required. The port keeps a
/// gun's rounds in the item's own `charges`, which is the store's one place for "how many of this thing are here" - so loading a shot is a component write, and a gun with nothing left is a
/// gun that does not fire rather than one that fires a ghost round.
#[must_use]
pub fn spend_rounds(world: &mut bevy_ecs::prelude::World, gun: bevy_ecs::prelude::Entity, rounds: i32) -> bool {
    let Some(mut held) = world.get_mut::<crate::GroundItem>(gun) else {
        return true;
    };
    if rounds <= 0 {
        return true;
    }
    if held.charges < rounds {
        return false;
    }
    held.charges -= rounds;
    true
}

/// How hard one shot kicks: `item::gun_recoil` (`item.cpp:11232-11266`).
///
/// The reference's shape, in its own order:
///
/// 1. **weight is capped by the arm's strength** - `min( gun_weight, arm_strength * 333 g ) / 333 g` - so a stronger arm does not make a light gun kick less but does stop a heavy one from
///    kicking as much as its whole weight would say;
/// 2. handling is the gun's own figure (plus its gunmods', which the port has none of) **divided by ten**, because the data writes it in whole units;
/// 3. the two are combined as `weight^0.8 * handling^1.2`, which is the reference's own bias: **a heavier weapon benefits more from good handling**;
/// 4. and the kick is the gun's `recoil` plus the round's, **divided** by that factor when it is above one and **multiplied** by `1 + |factor|` when it is below - so poor handling hurts
///    more than good handling helps.
#[must_use]
#[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)]
pub fn gun_recoil(
    gun_recoil: i32,
    round_recoil: i32,
    handling: i32,
    gun_weight_g: f64,
    arm_strength: i32,
) -> i32 {
    let capped_weight = gun_weight_g.min(f64::from(arm_strength) * 333.0);
    let weight = capped_weight / 333.0;
    let handling = f64::from(handling) / 10.0;
    let factor = weight.powf(0.8) * handling.powf(1.2);
    let kick = gun_recoil + round_recoil;
    // The reference divides and multiplies in floating point and returns the cast to an `int`, so the **division keeps its fraction until the end** - rounding the factor first would
    // turn 2.297 into 2 and halve the kick to 750 instead of 652.
    if factor > 1.0 {
        (f64::from(kick) / factor) as i32
    } else {
        (f64::from(kick) * (1.0 + factor.abs())) as i32
    }
}

/// How loud a shot is: `item::gun_noise` (`ranged.cpp:2394-2445`), before the reference's per-ammunition special cases.
///
/// The rule is one addition and a floor - the gun's own `loudness` plus the round's, clamped at zero - and **the round carries almost all of it**: a Mini-14's own figure is **zero** while its
/// .223 is loud, which is why a port that read only the gun would think a rifle silent.
///
/// The reference's gunmod terms (`loudness` and `loudness_multiplier`) are absent because the port has no gunmods, and its **special cases by ammunition type** - a grenade launcher's
/// `8`, a railgun's `24`, a rocket's `4`, and the arrows' and lasers' own sounds - are named in `docs/parity.md` rather than folded in as if they were the general rule.
#[must_use]
pub fn gun_noise(gun_loudness: i32, round_loudness: i32) -> i32 {
    (gun_loudness + round_loudness).max(0)
}

/// Fires a burst, one shot at a time, and stops when the gun will not load.
///
/// Reference: `Character::fire_gun` (`ranged.cpp:1130-1240`): `for( int curshot = 0; curshot < burst_size; ++curshot )`, with `ammo_required()` spent inside the loop and
/// `if( !gun.is_gun() ) { break; }` guarding the rest of the body - **so a burst that runs dry fires fewer shots rather than failing**. Each shot carries its own recoil into the next, which is
/// what makes a burst worse than the same number of separate shots made a turn apart.
///
/// The port keeps the loop here and returns one [`Fired`] per shot actually made, so a caller can read the whole burst - including the one that found the gun empty, which is **not**
/// included because the reference does not fire it.
#[must_use]
#[allow(clippy::too_many_arguments)]
pub fn fire_burst(
    world: &mut bevy_ecs::prelude::World,
    shooter: bevy_ecs::prelude::Entity,
    gun: &cdda_content::guns::GunType,
    lines: &[cdda_content::skills::SkillTime],
    target: (i32, i32, i32),
    divider: f64,
    round: Option<&cdda_content::ammo::AmmoType>,
    gun_entity: Option<bevy_ecs::prelude::Entity>,
    gun_weight_g: f64,
    reload_and_shoot: bool,
    burst: i32,
    wont_train_marksmanship: bool,
) -> Vec<Fired> {
    let mut made = Vec::new();
    for _ in 0..burst.max(0) {
        let shot = fire_at(world, shooter, gun, lines, target, divider, round, gun_entity, gun_weight_g, reload_and_shoot);
        if !shot.loaded {
            break;
        }
        made.push(shot);
    }
    // Reference: `ranged.cpp:1207-1213`, once per burst and not once per shot.
    let hits = i32::try_from(made.iter().filter(|shot| shot.hit.is_some()).count()).unwrap_or(0);
    let shots = i32::try_from(made.len()).unwrap_or(0);
    practise_after_a_burst(world, shooter, &gun.skill, hits, shots, gun.skill == "launcher", wont_train_marksmanship);
    made
}

/// Practices the skills a burst trains, once per burst and in the reference's own proportions.
///
/// Reference: `ranged.cpp:1207-1213`:
///
/// ```text
/// if( !gun.has_flag( flag_WONT_TRAIN_MARKSMANSHIP ) ) practice( skill_gun, ( hits + 1 ) * 5 );
/// int practice_units = gun_skill == skill_launcher ? curshot : hits;
/// practice( gun_skill, ( practice_units + 1 ) * 5 );
/// ```
///
/// **Two skills and two rules.** The **base gun skill** trains by hits but **always by at least five**, so a shooter who missed every shot still learns from having fired; the **weapon's own
/// skill** trains by hits too, except that a **launcher trains by shots fired** whether they landed or not - because aiming a grenade launcher is the hard part, not hitting with it.
///
/// A weapon carrying `WONT_TRAIN_MARKSMANSHIP` (a nail gun, a water cannon) trains **neither**, and the flag is the data's own.
pub fn practise_after_a_burst(
    world: &mut bevy_ecs::prelude::World,
    shooter: bevy_ecs::prelude::Entity,
    gun_skill: &str,
    hits: i32,
    shots: i32,
    launcher: bool,
    wont_train_marksmanship: bool,
) {
    if !wont_train_marksmanship {
        crate::creature::practise_skill(world, shooter, "gun", (hits + 1) * 5);
    }
    let units = if launcher { shots } else { hits };
    crate::creature::practise_skill(world, shooter, gun_skill, (units + 1) * 5);
}

/// Whether a shot misfires before it is fired: the two dice `Character::handle_gun_damage` rolls first (`ranged.cpp:700-800`).
///
/// Reference, in its own order:
///
/// 1. **dirt**: `x_in_y( dirt^3, 10^12 )` - a chance that is nothing at low dirt and geometric as the dirt approaches the maximum of **10,000**, where it is a certainty. The reference's comment
///    says the formula was fitted by eye to feel right at high, medium and low dirt, so the cube is the rule and not an approximation of one;
/// 2. **mechanical malfunction**: `one_in( ( 2 << durability ) * 3 )` - a gun of durability 5 misfires once in 192, one of durability 9 once in 3,072. The shift is why the data's narrow range
///    matters: **every point of durability doubles the odds against it**.
///
/// `NEVER_JAMS` skips **both**, which is the flag's whole meaning. The reference's magazine branch, its underwater branch and its consumable-gunmod branch are named in `docs/parity.md` rather
/// than guessed at.
#[must_use]
pub fn misfires(dirt: i32, durability: i32, never_jams: bool, dice: &mut cdda_core_types::rng::Rolls) -> bool {
    if never_jams {
        return false;
    }
    let dirt = f64::from(dirt.max(0));
    if dirt > 0.0 && dice.x_in_y(dirt * dirt * dirt, 1_000_000_000_000.0) {
        return true;
    }
    let shift = u32::try_from(durability.clamp(0, 20)).unwrap_or(0);
    let odds = i32::try_from((2_i64 << shift) * 3).unwrap_or(i32::MAX);
    dice.one_in(odds)
}

/// The dirt one shot leaves behind: `ranged.cpp:864-872`, the fouling half of `handle_gun_damage`.
///
/// Reference, in its own order:
///
/// 1. **nothing at all** if the round's effects or the gun's flags say `NON_FOULING`;
/// 2. otherwise, while the gun is below the maximum, a normal cartridge adds **one**, and the reference **caps ordinary ammunition at 7,150** - not at the absolute 10,000 - because
///    "firing with regular gunpowder can never make the gun quite as bad as firing it with black gunpowder";
/// 3. black powder adds `200 - blackpowder_tolerance * 2` more, floored at zero, and **is** allowed all the way to the maximum.
///
/// The port takes the black-powder terms as arguments because the ammo dump does not carry the round's `ammo_effects` yet, so every round it knows is ordinary - **a named gap, and the cap of
/// 7,150 is the half that matters on every shot**.
#[must_use]
pub fn dirt_after_a_shot(dirt: i32, blackpowder: bool, blackpowder_tolerance: i32, non_fouling: bool) -> i32 {
    if non_fouling || dirt >= MAX_DIRT {
        return dirt.clamp(0, MAX_DIRT);
    }
    let added = if blackpowder {
        (200 - blackpowder_tolerance * 2).max(0)
    } else {
        0
    };
    if blackpowder || dirt < 7_150 {
        (dirt + added + 1).min(MAX_DIRT)
    } else {
        dirt
    }
}
