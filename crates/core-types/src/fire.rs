//! What fire does when it is not burning anything: grows, spreads, and dies.
//!
//! Reference: `field_processor_fd_fire` (`src/map_field.cpp:945-1360`). The processor as a whole needs items, terrain flags, weather and
//! vehicles; the four rules here need none of them, and they are the ones that decide a fire's shape. Their constants are the reference's,
//! and their comparisons are the reference's strictness - "three neighbours" means `>= 3`, and fifty minutes of fuel means age `< -50`.

/// The highest intensity a fire can reach, given how long it has burned (negative age is fuel) and how many fires touch it.
///
/// Reference: `src/map_field.cpp:1222-1257`. The comments in the reference say "1 for a lone fire, 2 for at least 1 neighbor, 3 for at least
/// 2 neighbors", and the code checks **three** and **seven**. The code is the rule.
#[must_use]
pub fn growth_cap(age_minutes: i64, adjacent_fires: i32) -> i32 {
    if age_minutes < -500 {
        return 3;
    }
    let mut cap = 1 + i32::from(adjacent_fires >= 3) + i32::from(adjacent_fires >= 7);
    if cap < 2 && age_minutes < -50 {
        cap = 2;
    }
    cap
}

/// What growing to `intensity` costs, in **turns** of the fire's life.
///
/// Reference: `cur.set_field_age( cur.get_field_age() + 10_minutes * cur.get_field_intensity() )` - the intensity is read **after** it has
/// risen, so growing to three costs thirty minutes rather than twenty. A turn is a second in the reference, so ten minutes is six hundred
/// turns: the units are the whole content of this function, and my first version of it said "ten minutes per level" while returning ten,
/// which is a factor of sixty in the wrong direction.
#[must_use]
pub fn growth_cost(intensity: i32) -> i64 {
    10 * 60 * i64::from(intensity)
}

/// What a fire with nothing to burn loses, in **turns** of its life, on its turn.
///
/// Reference: `cur.mod_field_age( 10_seconds * cur.get_field_intensity() )` - **ten seconds**, not ten minutes, per level; with a turn to
/// the second that is ten turns per level. A raging fire still dies faster than a small one when it stands on bare stone, but at a
/// sixtieth of the rate the growth cost spends fuel.
#[must_use]
pub fn dying_out_cost(intensity: i32) -> i64 {
    10 * i64::from(intensity)
}

/// The chance in a hundred that a fire jumps to a neighbour, given a web in the way.
///
/// Reference: `spread_chance = 25 * ( intensity - 1 )`, and `50 + spread_chance / 2` when a web is there. A fire at intensity one cannot
/// spread at all, which is what keeps a single square of flame from crossing a field.
#[must_use]
pub fn spread_chance(intensity: i32, web: bool) -> i32 {
    let chance = 25 * (intensity - 1);
    if web { 50 + chance / 2 } else { chance }
}

/// Where a fire ends up after one turn of the rules that need nothing but its own state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FireTurn {
    /// The fire's age after the turn, in turns; negative is fuel remaining.
    pub age: i64,
    /// Its intensity after the turn.
    pub intensity: i32,
    /// Whether it grew this turn.
    pub grew: bool,
}

/// One turn of a fire: spend or gain fuel, then grow if it has earned it.
///
/// Reference: `field_processor_fd_fire`, the fuel accounting at `src/map_field.cpp:1122-1131` and the growth at `:1222-1257`, in that
/// order - the reference spends what it burned before it decides whether the flames are hot enough to grow.
///
/// * `time_added` is fuel the fire consumed this turn, in turns. It is **subtracted** from the age, because a negative age is fuel.
/// * With nothing consumed and terrain that can burn, the fire loses [`dying_out_cost`] instead: it is standing on nothing.
/// * Then, if it has fuel (`age < 0`), is not already raging, and the cap from [`growth_cap`] allows it, it grows one level and pays
///   [`growth_cost`].
#[must_use]
pub fn fire_turn(
    age: i64,
    intensity: i32,
    adjacent_fires: i32,
    can_burn: bool,
    time_added: i64,
) -> FireTurn {
    let mut age = age;
    if time_added != 0 {
        age -= time_added;
    } else if can_burn {
        age += dying_out_cost(intensity);
    }
    let mut intensity = intensity;
    let mut grew = false;
    if age < 0 && intensity < 3 {
        let cap = growth_cap(age, adjacent_fires);
        if intensity < cap && age < 0 {
            intensity += 1;
            age += growth_cost(intensity);
            grew = true;
        }
    }
    FireTurn {
        age,
        intensity,
        grew,
    }
}

/// What one square next to a fire looks like, as far as spreading is concerned.
///
/// The four flags are the reference's own condition, kept together because they answer one question - would this square catch? - and
/// splitting them into separate types would only move the same four `bool`s somewhere else.
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Neighbour {
    /// A fire is already burning there; the reference boosts those elsewhere and never re-lights them here.
    pub has_fire: bool,
    /// A spider web is there, which makes the fire much more eager.
    pub web: bool,
    /// The ground is `FLAMMABLE`.
    pub flammable: bool,
    /// The ground is `FLAMMABLE_ASH`.
    pub flammable_ash: bool,
    /// The ground is `FLAMMABLE_HARD`, which needs a hotter fire.
    pub flammable_hard: bool,
    /// How strong the fire there is, or zero when there is none.
    pub fire_intensity: i32,
    /// How long the fire there has burned, which the flashpoint rule compares against ours.
    pub fire_age: i64,
    /// Whether something burnable is lying there, which is the last way a square catches.
    pub items: bool,
}

/// What the spread phase did to a fire and its neighbours.
///
/// Reference: `map_field.cpp:1163-1248` - one `one_in( 3 )` decides between **flashing a neighbour over** (making an adjacent fire bigger and older) and
/// **growing** this fire, and the two are an `if / else if` chain on the same die.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SpreadPhase {
    /// Neighbour indices whose fire was made hotter this turn.
    pub boosted: Vec<usize>,
    /// Turns of age this fire spent doing it - five minutes each, paid by the one that did the boosting.
    pub spent: i64,
    /// The fire's own new `( intensity, age )` when it grew, which happens on the turns the die **failed**.
    pub grown: Option<(i32, i64)>,
    /// How many neighbours hold a fire, counted the way the reference counts it: inside whichever branch ran.
    pub adjacent: i32,
}

/// One fire's share of the spread phase: the flashpoint die, and whichever of the two branches it chose.
///
/// Reference: `map_field.cpp:1163-1248`. The details that matter for parity:
///
/// * the die is drawn whenever the fire **may spread** and is at least intensity two, and it is drawn **before** the spread loop, so any port that skips it
///   is one draw behind for the rest of the turn;
/// * **flashpoint** (the die succeeds) scans the eight neighbours from a random start, gives five minutes of life to each fire there that is weaker or
///   older than this one, and makes it one step hotter if it was below intensity two - paying for all of it out of this fire's own age;
/// * **growth** (the die fails, and only then) can raise this fire by one level when it has fuel banked (`age < 0`) and neighbours enough, at a cost of
///   `10_minutes * new intensity` of age;
/// * `adjacent_fires` is counted inside whichever branch ran, which is why a fire that may not spread reports none.
#[must_use]
pub fn spread_phase(
    intensity: i32,
    age: i64,
    can_spread: bool,
    neighbours: &[Neighbour],
    rolls: &mut crate::rng::Rolls,
) -> SpreadPhase {
    let mut phase = SpreadPhase::default();
    if !can_spread {
        return phase;
    }
    let count = i32::try_from(neighbours.len()).unwrap_or(0);
    if count == 0 {
        return phase;
    }
    if intensity > 1 && rolls.one_in(3) {
        let scan = usize::try_from(rolls.rng(0, count - 1)).unwrap_or(0);
        for step in 1..=count {
            // The reference's loop condition is `count != neighs.size() && cur.get_field_age() < 0_turns`, re-checked every iteration
            // (`map_field.cpp:1180`): the flashpoint **pays** five minutes of the fire's own life for each neighbouring fire it wakes, so a fire with
            // nothing banked does not scan at all - and one that runs out mid-scan stops there.
            if age + phase.spent >= 0 {
                break;
            }
            let index =
                usize::try_from((i32::try_from(scan).unwrap_or(0) + step).rem_euclid(count))
                    .unwrap_or(0);
            let Some(neighbour) = neighbours.get(index) else {
                continue;
            };
            if !neighbour.has_fire {
                continue;
            }
            phase.adjacent += 1;
            let weaker_or_older = neighbour.fire_intensity <= intensity || neighbour.fire_age > age;
            if weaker_or_older {
                phase.boosted.push(index);
                phase.spent += 5 * 60;
            }
        }
        return phase;
    }
    if age >= 0 || intensity >= 3 {
        return phase;
    }
    // The growth branch counts the neighbours itself, because the flashpoint scan did not run.
    for neighbour in neighbours {
        if neighbour.has_fire {
            phase.adjacent += 1;
        }
    }
    let mut maximum = 1 + i32::from(phase.adjacent >= 3) + i32::from(phase.adjacent >= 7);
    if maximum < 2 && age < -50 * 60 {
        maximum = 2;
    }
    if age < -500 * 60 {
        maximum = 3;
    }
    if intensity < maximum {
        let grown = intensity + 1;
        phase.grown = Some((grown, age + 10 * 60 * i64::from(grown)));
    }
    phase
}

/// The squares a fire sets alight this turn.
///
/// Reference: the spread loop at `src/map_field.cpp:1285-1345`. Its shape is the rule:
///
/// * the loop starts at a **random** neighbour - `rng( 0, n - 1 )` - and wraps, because the reference wants a fire to prefer whichever
///   side it happens to think of first, and the order is part of the world;
/// * a neighbour is skipped on `one_in( intensity * 2 )`, so a bigger fire considers fewer neighbours per turn;
/// * the chance to take is `25 * ( intensity - 1 )`, and `50 + chance / 2` when a web is there - which is why a **single square of fire
///   can cross a web** but not a lawn;
/// * the fire's `power` is `intensity + one_in( 5 )`, and the ground must accept it: `FLAMMABLE` and `FLAMMABLE_ASH` need power two with a
///   coin toss, `FLAMMABLE_HARD` needs power three with one in five, and a web needs nothing at all.
///
/// Returns the indices of the neighbours that caught, in the order the loop considered them. The dice are spent identically whether the
/// answer is yes or no - which is the property a replay depends on.
#[must_use]
pub fn spread_fire(
    rolls: &mut crate::rng::Rolls,
    intensity: i32,
    can_spread: bool,
    neighbours: &[Neighbour],
) -> Vec<usize> {
    let mut caught = Vec::new();
    if neighbours.is_empty() {
        return caught;
    }
    let Some(count) = i32::try_from(neighbours.len()).ok() else {
        return caught;
    };
    let start = rolls.rng(0, count - 1);
    for step in 1..=count {
        let index = (start + step).rem_euclid(count);
        let Some(neighbour) = usize::try_from(index)
            .ok()
            .and_then(|index| neighbours.get(index))
        else {
            continue;
        };
        if rolls.one_in(intensity * 2) {
            continue;
        }
        if neighbour.has_fire {
            continue;
        }
        let chance = spread_chance(intensity, neighbour.web);
        let power = intensity + i32::from(rolls.one_in(5));
        // The order is the reference's, and it is not the obvious one: the **chance is drawn before the terrain is asked anything**
        // (`map_field.cpp:1330-1345`). `can_spread && rng( 1, 100 ) < spread_chance && ( ... flag && one_in( n ) ... )` short-circuits left to
        // right, so a fire that fails the chance never rolls for the ground and one that passes rolls only for the flags that are set.
        let passes = can_spread && rolls.rng(1, 100) < chance;
        let takes_ground = passes
            && ((power >= 2 && neighbour.flammable && rolls.one_in(2))
                || (power >= 2 && neighbour.flammable_ash && rolls.one_in(2))
                || (power >= 3 && neighbour.flammable_hard && rolls.one_in(5)));
        // The reference's last disjunct is `( one_in( 5 ) && dst.get_item_count() > 0 && flammable_items_at( ... ) )`, and C++ draws the die **before**
        // it asks whether anything is lying there (`map_field.cpp:1343`). The die is therefore spent on every square that reaches this point, items or not;
        // what it decides is whether a square with something burnable on it catches where the ground alone would refuse.
        let takes_items =
            passes && !neighbour.web && !takes_ground && rolls.one_in(5) && neighbour.items;
        if passes && (neighbour.web || takes_ground || takes_items) {
            caught.push(usize::try_from(index).unwrap_or(0));
        }
    }
    caught
}

/// Smoke a fire left this turn, and where.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Smoke {
    /// Whether it went to the square above the fire rather than the fire's own.
    pub up: bool,
    /// How thick it is.
    pub intensity: i32,
}

/// The smoke a fire makes this turn, if it makes any.
///
/// Reference: the block at the end of `field_processor_fd_fire` (`src/map_field.cpp:1407-1425`). The conditions are chained in the reference's
/// order, and the order is the world: `SUPPRESS_SMOKE` is tested first, so a sealed fireplace spends **no dice** and everything after it in the
/// turn sees the same stream it otherwise would have.
///
/// * `rng( 0, 100 - windpower ) <= smoke` - a windier turn narrows the range, which makes smoke *more* likely for a given count, not less.
/// * `rng( 3, 35 ) < intensity * 10` - a raging fire clears this almost always, a small one rarely.
/// * Where it goes: **above** if the square overhead has no floor (and the fire is not already at the top), at a strength of `rng( 1, intensity )`;
///   otherwise on the fire itself, at the fire's full intensity - thicker, because it has nowhere to rise to.
#[must_use]
pub fn smoke_from_fire(
    rolls: &mut crate::rng::Rolls,
    intensity: i32,
    smoke_produced: i32,
    windpower: i32,
    suppress_smoke: bool,
    can_rise: bool,
    above_has_no_floor: bool,
) -> Option<Smoke> {
    if suppress_smoke {
        return None;
    }
    if rolls.rng(0, 100 - windpower) > smoke_produced {
        return None;
    }
    if rolls.rng(3, 35) >= intensity * 10 {
        return None;
    }
    if can_rise && above_has_no_floor {
        return Some(Smoke {
            up: true,
            intensity: rolls.rng(1, intensity),
        });
    }
    Some(Smoke {
        up: false,
        intensity,
    })
}

/// What the fire is standing on, as far as eating it goes.
///
/// Reference: the four branches of the fuel accounting (`src/map_field.cpp:1050-1104`). They are not interchangeable and they do not all
/// roll: ordinary flammable ground feeds the fire every turn, hard ground feeds it on a die, and ash-bearing ground feeds it every turn.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Ground {
    /// Nothing here burns.
    #[default]
    Inert,
    /// `FLAMMABLE`: feeds every turn, `5 - intensity` turns at a time.
    Flammable,
    /// `FLAMMABLE_ASH`: the same, and leaves ash behind.
    FlammableAsh,
    /// `FLAMMABLE_HARD`: feeds on `one_in( 3 )`, `4 - intensity` turns at a time.
    FlammableHard,
}

/// What one turn of burning the ground did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GroundFuel {
    /// Turns of life the ground gave, to be **subtracted** from the fire's age.
    pub time_added: i64,
    /// Whether the fire bashed the ground out from under itself this turn.
    pub bashed: bool,
    /// What this turn's burning put into the air, for the smoke gate that reads it twenty lines later.
    pub smoke: i32,
}

/// One turn of the fire eating the ground it stands on.
///
/// Reference: `src/map_field.cpp:1050-1104`. The **order of the two dice is the rule**: `FLAMMABLE_HARD` asks `one_in( 3 )` first and only
/// then, if the ground did feed, asks `one_in( 200 - intensity * 50 )` about bashing it. A port that rolled the bash die first would shift every
/// later draw in the turn by one, and the reference's own dice trace shows exactly this order - `int 0 2 0` followed by `int 0 99 11` on the turn
/// the ground fed, and a single `int 0 2 1` on the turns it did not.
#[must_use]
pub fn ground_fuel(
    ground: Ground,
    intensity: i32,
    can_burn: bool,
    windpower: i32,
    rolls: &mut crate::rng::Rolls,
) -> GroundFuel {
    let mut fuel = GroundFuel {
        time_added: 0,
        bashed: false,
        smoke: 0,
    };
    if !can_burn {
        // The reference's whole fuel block sits inside `if( can_burn )` (`map_field.cpp:1075`), so a fire that may not burn its ground - water, a locker on
        // a tiled floor, anything whose terrain and furniture are both fireproof - draws **no die at all**. Drawing one to learn "nothing" is the kind of
        // difference that never shows up in a state dump and always shows up in a replay.
        return fuel;
    }
    let fed = match ground {
        Ground::Inert => return fuel,
        Ground::Flammable | Ground::FlammableAsh => {
            fuel.time_added = i64::from(5 - intensity);
            true
        }
        Ground::FlammableHard => {
            if rolls.one_in(3) {
                fuel.time_added = i64::from(4 - intensity);
                true
            } else {
                false
            }
        }
    };
    if fed {
        // Every fuel branch in the reference ends with these two lines (`map_field.cpp:1063-1096`), which makes `smoke` a local of this turn's fuel
        // block rather than a property of the square: a fire that did not feed this turn smokes only by the barest chance.
        fuel.smoke = 2 + windpower / 5;
        if intensity > 1 && rolls.one_in(200 - intensity * 50) {
            fuel.bashed = true;
        }
    }
    fuel
}

/// How many squares hot air is dropped on when a fire makes it.
pub const HOT_AIR_SQUARES: usize = 5;

/// Whether a fire asks for hot air this turn, and the die it spends asking.
///
/// Reference: `if( adjacent_fires < 5 && rng( 0, 4 - adjacent_fires ) )` (`src/map_field.cpp:1429`). The gate is a **non-zero** result, not a
/// success test, and the range shrinks as neighbours multiply - which is the reference's own note that "they produce radiant heat which does what hot
/// air would do anyway". A fire with five neighbours does not roll at all.
#[must_use]
pub fn wants_hot_air(adjacent_fires: i32, rolls: &mut crate::rng::Rolls) -> bool {
    if adjacent_fires >= 5 {
        return false;
    }
    rolls.rng(0, 4 - adjacent_fires) != 0
}

/// The hot air field a fire of this intensity makes, as the reference's four field ids.
///
/// Reference: `map::create_hot_air` (`src/map_field.cpp:375-394`) - intensity one to four maps to `fd_hot_air1` to `fd_hot_air4`, and anything else
/// is a bug in the caller rather than a thing to guess at.
#[must_use]
pub fn hot_air_field(intensity: i32) -> Option<i32> {
    (1..=4).contains(&intensity).then_some(intensity)
}

/// The squares hot air lands on, as offsets from the fire.
///
/// Reference: `create_hot_air`'s loop (`src/map_field.cpp:396-399`) - **five** squares, each drawn as `rng( -1, 1 )` for x and again for y, and each
/// set to intensity one whatever the fire's own intensity was. Ten dice, in x-then-y order, is the shape a replay has to match.
#[must_use]
pub fn hot_air_squares(rolls: &mut crate::rng::Rolls) -> [(i32, i32); HOT_AIR_SQUARES] {
    let mut squares = [(0, 0); HOT_AIR_SQUARES];
    for square in &mut squares {
        *square = (rolls.rng(-1, 1), rolls.rng(-1, 1));
    }
    squares
}

/// Which way a gas went this turn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GasDirection {
    /// Straight down, which the reference tries **first** and without spending another die.
    Down,
    /// Sideways, onto one of the eight neighbours.
    Side(usize),
    /// Straight up, tried last and only when nothing sideways would take it.
    Up,
    /// Nowhere.
    None,
}

/// Where a gas moved this turn, and how the two squares changed.
///
/// Reference: `map::spread_gas` (`src/map_field.cpp:276-360`) and `gas_spread_to` (`:250-273`). The rule is shared by every gas - hot air, smoke, tear gas -
/// because what makes them gases is `percent_spread` in their data, not a processor of their own.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GasSpread {
    /// Which way it went.
    pub direction: GasDirection,
    /// What the source keeps: its own intensity minus one, and its age minus the share it gave away.
    pub source_intensity: i32,
    /// The age the source keeps.
    pub source_age: i64,
    /// The age the destination gains, which is the source's age divided by its intensity before the move.
    pub transferred_age: i64,
}

/// What the squares around a gas would do with it.
///
/// The reference asks two different questions of three different places: `spread_gas` tries the square below **first**, the eight beside it second, and the
/// square above last. Naming all ten answers in one value keeps the call readable and keeps the *order* where the rule can see it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Around {
    /// Whether the square below would take the gas (`gas_can_spread_to` and a legal move).
    pub below: bool,
    /// Whether each of the eight neighbours would take it.
    pub beside: [bool; 8],
    /// Whether the square above would take it.
    pub above: bool,
}

/// One turn of a gas trying to move.
///
/// Reference: `spread_gas` (`src/map_field.cpp:276-360`). The dice, in order:
///
/// 1. **Intensity one does not roll at all** - `cur.get_field_intensity() <= 1 ||` short-circuits before the chance. A gas with one level left is dying, and
///    a port that rolled anyway would shift every later draw.
/// 2. `rng( 1, 100 - windpower ) > percent_spread` - the gate, drawn **before** any neighbour is looked at. Wind narrows the range, which makes the gate
///    *easier* to pass, the same way wind makes smoke more likely.
/// 3. **Down first**, and for free: if the square below would take the gas and the gas can move there, it goes, with no further die spent.
/// 4. `rng( 0, 7 )` - which neighbour the lateral scan starts on; the scan then wraps, so the order is the world's.
/// 5. `one_in( spread.len() )` - whether it goes sideways at all.
/// 6. `rng( 0, spread.len() - 1 )` - which of the candidates it takes.
/// 7. **Up last**, and only when nothing sideways took it.
///
/// The port asks the caller whether each direction would accept the gas, because that is a question about the map (`gas_can_spread_to`: a weaker field of the
/// same kind, on a square that is walkable or permeable), not about the gas.
#[must_use]
pub fn spread_gas(
    intensity: i32,
    age: i64,
    percent_spread: i32,
    windpower: i32,
    around: &Around,
    rolls: &mut crate::rng::Rolls,
) -> GasSpread {
    let nowhere = GasSpread {
        direction: GasDirection::None,
        source_intensity: intensity,
        source_age: age,
        transferred_age: 0,
    };
    if intensity <= 1 {
        return nowhere;
    }
    if rolls.rng(1, 100 - windpower) > percent_spread {
        return nowhere;
    }
    let share = age / i64::from(intensity);
    let moving = |direction: GasDirection| GasSpread {
        direction,
        source_intensity: intensity - 1,
        source_age: age - share,
        transferred_age: share,
    };
    if around.below {
        return moving(GasDirection::Down);
    }
    let start = rolls.rng(0, 7);
    let mut spread: Vec<usize> = Vec::new();
    for step in 1..=8 {
        let index = usize::try_from((start + step).rem_euclid(8)).unwrap_or(0);
        if around.beside.get(index).copied().unwrap_or(false) {
            spread.push(index);
        }
    }
    if !spread.is_empty() && rolls.one_in(i32::try_from(spread.len()).unwrap_or(i32::MAX)) {
        let choice = rolls.rng(0, i32::try_from(spread.len()).unwrap_or(1) - 1);
        let to = spread
            .get(usize::try_from(choice).unwrap_or(0))
            .copied()
            .unwrap_or(0);
        return moving(GasDirection::Side(to));
    }
    if around.above {
        return moving(GasDirection::Up);
    }
    nowhere
}

/// One material's behaviour at one fire intensity, as `simulate_burn` needs it.
///
/// The reference reads these from the material data (`mat_burn_data`), which is already loaded from `corpus/prototypes/materials.json`; the shape is
/// repeated here so the rule can be tested without a content catalogue in the room.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MaterialBurn {
    /// Nothing burns, whatever the fire: an item made to shield from fire does not feed one.
    pub immune: bool,
    /// How much of the material a fire of this intensity eats per turn, in millilitres.
    pub volume_per_turn_ml: f64,
    /// Turns of a fire's life one portion gives.
    pub fuel: f64,
    /// Smoke one portion makes.
    pub smoke: f64,
    /// How much of the item is destroyed per turn, as a fraction of it.
    pub burn: f64,
}

/// What burning the items on one square gave the fire this turn.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct SimulatedBurn {
    /// Fuel, in turns of a fire's life, before the reference rounds it with `roll_remainder`.
    pub fuel: f64,
    /// Smoke, before rounding.
    pub smoke: f64,
    /// How much of the items was destroyed, before rounding into whole units of `burnt`.
    pub burn: f64,
}

/// What one item made of these materials contributes to a fire of this intensity.
///
/// Reference: `item::simulate_burn` (`src/item.cpp:12543-12596`). Three things about it are easy to get wrong and all three are the reference's own
/// arrangement, not an invention of the port:
///
/// - a material with a `volume_per_turn` burns **in proportion to the item's own volume**, so a plank gives a fraction of a turn's fuel per turn and a
///   rag gives a different fraction;
/// - a material with **no** `volume_per_turn` burns flat, and so does everything in a **contained** fire - which is what keeps a brazier alight;
/// - a multi-material item is divided by its **portion count**, not by the number of materials, so a two-to-one mix weights accordingly.
#[must_use]
pub fn simulate_burn(
    materials: &[(&[MaterialBurn], i32)],
    volume_ml: f64,
    intensity: i32,
    contained: bool,
) -> SimulatedBurn {
    let effective = if contained { 3 } else { intensity };
    let mut out = SimulatedBurn::default();
    let mut portions = 0;
    for (levels, count) in materials {
        // `burn_data( intensity )` is `_burn_data[ min( intensity, size ) - 1 ]` (`material.cpp:350-353`): the level is the fire's, and an item whose
        // material lists fewer levels than the fire has uses its last one rather than failing.
        let levels_listed = i32::try_from(levels.len().max(1)).unwrap_or(1);
        let level = usize::try_from(effective.max(1).min(levels_listed)).unwrap_or(1);
        let Some(material) = levels.get(level - 1) else {
            continue;
        };
        if material.immune {
            return SimulatedBurn::default();
        }
        let multiplier = f64::from(*count);
        portions += count;
        let flat = contained || material.volume_per_turn_ml == 0.0;
        let rate = if flat || volume_ml <= 0.0 {
            1.0
        } else {
            material.volume_per_turn_ml / volume_ml
        };
        out.fuel += material.fuel * rate * multiplier;
        out.smoke += material.smoke * rate * multiplier;
        out.burn += material.burn * rate * multiplier;
    }
    if materials.len() > 1 {
        let total = f64::from(portions.max(1));
        out.fuel /= total;
        out.smoke /= total;
        out.burn /= total;
    }
    out
}

/// One item lying on the ground under a fire, as far as burning is concerned.
///
/// The material tables are resolved once, when the item is put there, so the burning rule never needs a content catalogue in the room - which is also why
/// this can be a component: a thing on the ground with state of its own is a thing, not a fact about a square.
#[derive(Debug, Clone, PartialEq)]
pub struct BurningItem {
    /// The item's own volume in millilitres, which decides how fast its material burns.
    pub volume_ml: f64,
    /// Each material it is made of, with the three intensity levels the reference lists, and how many portions of it there are.
    pub materials: Vec<(Vec<MaterialBurn>, i32)>,
    /// How much of it the fire has already destroyed, before this turn's rounding.
    pub burnt: f64,
}

/// What burning the items on one square gave the fire this turn.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct ItemBurn {
    /// Whole turns of a fire's life, after `roll_remainder`.
    pub time_added: i64,
    /// Whole smoke, after `roll_remainder`.
    pub smoke: i32,
}

/// Burns the items on a square, destroying what a fire of this intensity can, and says what they gave.
///
/// Reference: the item block of the fire processor (`map_field.cpp:976-1048`) plus `item::burn` (`item.cpp:12598-12640`).
///
/// The order of the dice is the rule, again: each surviving item spends one die rounding how much of itself it lost, and then the two totals spend one die
/// each - **smoke first, then fuel**, because that is the order the reference assigns them in.
pub fn burn_the_items(
    items: &mut Vec<BurningItem>,
    intensity: i32,
    contained: bool,
    rolls: &mut crate::rng::Rolls,
) -> ItemBurn {
    let max_consume = intensity * 2;
    let mut consumed = 0;
    let mut fuel_total = 0.0;
    let mut smoke_total = 0.0;
    let mut index = 0;
    while index < items.len() && consumed < max_consume {
        let Some(item) = items.get_mut(index) else {
            break;
        };
        let borrowed: Vec<(&[MaterialBurn], i32)> = item
            .materials
            .iter()
            .map(|(levels, count)| (levels.as_slice(), *count))
            .collect();
        let simulated = simulate_burn(&borrowed, item.volume_ml, intensity, contained);
        if simulated.burn <= 0.0 {
            // `item::burn` returns false before touching anything: `burn_added <= 0` means this item is not being consumed at all.
            index += 1;
            continue;
        }
        fuel_total += simulated.fuel;
        smoke_total += simulated.smoke;
        // A whole item is destroyed once it has lost `base_volume / 250ml * 3` - integer division of the volume, as the reference does it.
        let volume_units = (item.volume_ml / 250.0).floor();
        item.burnt += f64::from(rolls.roll_remainder(simulated.burn));
        let destroyed = item.burnt >= volume_units * 3.0;
        if destroyed {
            items.remove(index);
            consumed += 1;
        } else {
            index += 1;
        }
    }
    // The reference assigns smoke first and fuel second (`map_field.cpp:1046-1047`), so the reals come out in that order.
    let smoke = rolls.roll_remainder(smoke_total);
    let time_added = rolls.roll_remainder(fuel_total);
    ItemBurn {
        time_added: i64::from(time_added),
        smoke,
    }
}
