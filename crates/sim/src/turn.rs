//! A turn of the world, as a schedule with the stages declared.
//!
//! The reference's turn is one function that calls twenty subsystems in the order its own comments
//! depend on (`do_turn.cpp:642-655`). This port states that order once, as system sets, and lets the
//! ECS run them: scent, then what fell, then the fields, then the light the turn left behind.
//!
//! Everything a stage needs is in the store - the smell the character made is a component on it, the
//! wind is the weather's, the dice are a thing like any other. A stage takes `&mut World` and
//! nothing else.

use crate::{Where, singletons};
use bevy_ecs::prelude::*;
use cdda_core_types::rng::Rolls;

/// The stages of an environment turn, in the order they run.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TurnStage {
    /// The effects whose turn has come run first.
    ///
    /// Reference: `game::process_events` runs before the map and the creatures take their turn (`do_turn.cpp`), so a timed event that changes the world is already in
    /// place when the world's other stages read it.
    Scheduled,
    /// The character's smell spreads and fades.
    Scent,
    /// Every creature that can hear takes in the sounds this turn made.
    ///
    /// Reference: `sounds::process_sound` runs as the turn ends (`do_turn.cpp`), handing each sound to every monster within earshot and then clearing the list - so a
    /// creature's memory of a noise is settled **before** it takes its step.
    Sounds,
    /// Everything whose support went away falls.
    Falling,
    /// Every field takes its turn: fires, incendiaries, gases, then ageing.
    Fields,
    /// Every creature banks its speed and takes its step.
    ///
    /// Reference: `game::monmove` runs inside the turn, after the fields have had theirs and before the
    /// light is recomputed (`do_turn.cpp`). The order is declared here rather than implied by the code.
    Creatures,
    /// The light the turn left behind is recomputed.
    Light,
}

/// Where the smell comes from this turn: the character, standing at `center`, depositing `deposit`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Component)]
pub struct ScentSource {
    /// The square the smell is laid from.
    pub center: (i32, i32),
    /// How much is laid this turn; zero means nothing.
    pub deposit: i32,
}

/// Marks the one entity holding [`ScentSource`].
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Component)]
pub struct TheScentSource;

/// The dice the turn draws from, held as a thing in the store.
#[derive(Debug, Default, Component)]
pub struct Random {
    /// The generator, taken out for the length of a draw and put back.
    pub rolls: Option<Rolls>,
}

/// Marks the one entity holding [`Random`].
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Component)]
pub struct TheRandom;

/// The stages that ran this turn, in the order they ran, when anything is watching.
///
/// A receipt, not a rule: it exists so a test can prove the declared order is the executed order, and
/// so the QA surface can print what a turn did.
#[derive(Debug, Default, Clone, PartialEq, Eq, Component)]
pub struct TurnTrace {
    /// The stages, in the order they ran.
    pub stages: Vec<TurnStage>,
}

/// Marks the one entity holding [`TurnTrace`].
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Component)]
pub struct TheTurnTrace;

/// Whether a square's ground will take gas spreading into it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Component)]
pub struct GasCapacity {
    /// False for a square that holds gas out, as a sealed room does.
    pub accepts: bool,
}

/// Sets how much smell the character lays this turn, and where.
pub fn set_scent_source(world: &mut World, center: (i32, i32), deposit: i32) {
    let mut source = singletons::scent_source(world);
    source.center = center;
    source.deposit = deposit;
}

/// Puts a generator in the store, so a replay drives the same dice the reference did.
pub fn set_random(world: &mut World, rolls: Rolls) {
    singletons::put_rolls(world, rolls);
}

/// The stages that ran this turn.
#[must_use]
pub fn trace_of(world: &mut World) -> Vec<TurnStage> {
    singletons::turn_trace(world).stages.clone()
}

/// The schedule a turn runs, with its stages declared in the reference's order.
#[must_use]
pub fn environment_schedule() -> Schedule {
    let mut schedule = Schedule::default();
    schedule.configure_sets(
        (
            TurnStage::Scent,
            TurnStage::Sounds,
            TurnStage::Falling,
            TurnStage::Fields,
            TurnStage::Creatures,
            TurnStage::Light,
        )
            .chain(),
    );
    schedule.add_systems((
        scheduled_stage.in_set(TurnStage::Scheduled),
        scent_stage.in_set(TurnStage::Scent),
        sounds_stage.in_set(TurnStage::Sounds),
        needs_stage.in_set(TurnStage::Creatures),
        digest_stage.in_set(TurnStage::Creatures),
        falling_stage.in_set(TurnStage::Falling),
        fields_stage.in_set(TurnStage::Fields),
        creatures_stage.in_set(TurnStage::Creatures),
        light_stage.in_set(TurnStage::Light),
    ));
    schedule
}

/// Runs one environment turn: the stages, in the order the schedule declares.
pub fn run_turn(world: &mut World) {
    let mut schedule = environment_schedule();
    schedule.run(world);
}

/// The reference's `Character::reset_stats`, whose first act is the minute's focus change: `once_every( 1_minutes )` calls `update_mental_focus`, and focus drifts a
/// tenth of the way towards its equilibrium each time. The port runs it with the creatures, because it is a change to a creature and not to the map.
fn focus_stage(world: &mut World) {
    // `once_every( 1_minutes )` in the reference: a minute is sixty turns of a hundred moves each.
    let Some(calendar) = crate::calendar::of(world) else {
        return;
    };
    if calendar.turn % 60 != 0 {
        return;
    }
    let mut creatures: Vec<bevy_ecs::entity::Entity> = Vec::new();
    let mut query = world
        .query_filtered::<bevy_ecs::entity::Entity, bevy_ecs::prelude::With<crate::skills::Focus>>(
        );
    for creature in query.iter(world) {
        creatures.push(creature);
    }
    for creature in creatures {
        // Reference: `Character::calc_focus_equilibrium` (`character.cpp:3852-3920`) counts the creature's **perceived pain** against its morale, and a masochist minds it
        // only past twenty. The pain is the component's own number, the morale the points on the creature.
        let morale = crate::morale::net_bonus_of(world, creature);
        let pain = world
            .get::<crate::creature::Pain>(creature)
            .map_or(0, |pain| pain.0);
        let masochist = crate::mutations::has(world, creature, "MASOCHIST");
        let effective = crate::skills::effective_morale(morale, pain, masochist);
        // Reference: `Character::calc_focus_equilibrium` ends by capping the answer on the creature's own sleepiness
        // (`focus_equilibrium_sleepiness_cap`, `character.cpp:3838-3850`), so a creature that has not slept for days cannot rest its mind at all.
        let equilibrium = crate::skills::sleepiness_cap(
            crate::skills::focus_equilibrium(effective),
            crate::needs::sleepiness_of(world, creature),
        );
        crate::skills::update_focus_towards(world, creature, equilibrium);
    }
}

/// The body's own clock: every five minutes a creature's sleepiness climbs by `roll_remainder( rates.sleepiness * five_mins )`.
///
/// Reference: `Character::update_needs` (`character.cpp:5145-5175`), which the game runs on `once_every( 5_minutes )`. The port keeps the rates on the creature
/// (`NeedsRatesOf`), so a creature that has no rates stored is one the clock does not touch.
fn needs_stage(world: &mut World) {
    let Some(calendar) = crate::calendar::of(world) else {
        return;
    };
    if calendar.turn % 300 != 0 {
        return;
    }
    let mut creatures: Vec<(bevy_ecs::entity::Entity, crate::needs::NeedsRates)> = Vec::new();
    let mut query = world.query::<(bevy_ecs::entity::Entity, &crate::needs::NeedsRatesOf)>();
    for (creature, rates) in query.iter(world) {
        creatures.push((creature, rates.0));
    }
    let mut rolls =
        singletons::take_rolls(world).unwrap_or_else(|| cdda_core_types::rng::Rolls::new(0));
    for (creature, rates) in creatures {
        // Reference: `calc_needs_rates` ends by passing hunger, sleepiness and thirst through the enchantment cache (`character.cpp:5313-5316`), so a mutation that changes
        // how fast a creature wears out changes the clock itself rather than describing it.
        let rates = crate::needs::rates_with_enchantments(world, creature, rates);
        // Reference: `update_needs` runs one clock in two directions - a waking creature gets sleepless, a sleeping one recovers - and never both at once.
        if world.get::<crate::needs::Asleep>(creature).is_some() {
            let recovery = crate::needs::recovery_rate(0);
            let _ = crate::needs::sleep_off(world, creature, recovery, 5, &mut rolls);
            continue;
        }
        crate::needs::climb_sleepiness(world, creature, rates, 5, true, false, &mut rolls);
    }
    singletons::put_rolls(world, rolls);
}

/// The body's food clock: every five minutes a creature digests what it has eaten.
///
/// Reference: `Character::update_stomach` (`character_body.cpp:935-1019`), run by the game on `once_every( 5_minutes )`. The rates, the figures and the body are all
/// components; the enchantment pair comes from the creature's own cache, and the dice are `roll_remainder( rates.* * five_mins )`, exactly as the reference draws them.
fn digest_stage(world: &mut World) {
    let Some(calendar) = crate::calendar::of(world) else {
        return;
    };
    if calendar.turn % 300 != 0 {
        return;
    }
    let turn = calendar.turn;
    let effects = crate::effect_registry::types_of(world);
    let mut creatures: Vec<(
        bevy_ecs::entity::Entity,
        crate::needs::NeedsRates,
        crate::body::BodyClock,
    )> = Vec::new();
    let mut query = world.query::<(
        bevy_ecs::entity::Entity,
        &crate::needs::NeedsRatesOf,
        &crate::body::BodyClock,
    )>();
    for (creature, rates, clock) in query.iter(world) {
        creatures.push((creature, rates.0, *clock));
    }
    let mut rolls =
        singletons::take_rolls(world).unwrap_or_else(|| cdda_core_types::rng::Rolls::new(0));
    for (creature, rates, clock) in creatures {
        let Some(mut body) = world.get::<crate::body::Body>(creature).cloned() else {
            continue;
        };
        let (add, multiply) = match crate::mutation_data::holder(world) {
            Some(data) => crate::mutations::enchantment_values(
                world,
                creature,
                &data,
                "STOMACH_SIZE_MULTIPLIER",
            ),
            None => (0.0, 0.0),
        };
        let rates = crate::needs::rates_with_enchantments(world, creature, rates);
        let digestion = crate::digest::MetabolicRates {
            kcal: rates.kcal,
            hunger: rates.hunger,
            thirst: rates.thirst,
            sleepiness: rates.sleepiness,
        };
        #[allow(clippy::cast_precision_loss)]
        let hunger_roll = crate::roll_remainder(f64::from(rates.hunger * 5.0), &mut rolls);
        #[allow(clippy::cast_precision_loss)]
        let thirst_roll = crate::roll_remainder(f64::from(rates.thirst * 5.0), &mut rolls);
        // Reference: `Character::update_stomach` takes `half_hours` as the whole half hours that have gone by, so solids move in thirty-minute steps while the tick that
        // asks is five minutes long. The clock remembers how many it has already handed over.
        let half_hours = (turn / 1_800).saturating_sub(clock.half_hours_done);
        let inputs = crate::body::BodyInputs {
            five_mins: 5,
            half_hours,
            kcal_per_time: clock.kcal_per_time,
            healthy_kcal: clock.healthy_kcal,
            hunger_roll,
            thirst_roll,
            time_since_ate: clock.time_since_ate,
            foodless: clock.foodless,
            no_thirst: clock.no_thirst,
            mycus: clock.mycus,
            height_cm: clock.height_cm,
            organic_size: clock.organic_size,
        };
        // The tables the digestion consults: an empty one is a body that holds no vitamins, which is what a world with no loader installed answers.
        let vitamins = crate::vitamin_registry::vitamins_of(world);
        let effects = effects.clone();
        body.update_stomach(&digestion, &inputs, &vitamins, &effects, (add, multiply));
        world.entity_mut(creature).insert(body);
        let mut advanced = clock;
        advanced.half_hours_done = turn / 1_800;
        world.entity_mut(creature).insert(advanced);
    }
    singletons::put_rolls(world, rolls);
}

/// Hands this turn's sounds to the creatures that can hear them.
fn sounds_stage(world: &mut World) {
    note(world, TurnStage::Sounds);
    crate::hearing::hear_sounds(world);
}

fn creatures_stage(world: &mut World) {
    note(world, TurnStage::Creatures);
    // Reference: `Creature::process_turn` opens with `decrement_summon_timer()` (`creature.cpp:372-378`), so a summon whose lease ran out is gone **before** anything else in the
    // turn looks at it - which is why this is the first thing the creatures stage does.
    crate::summoned::expire(world);
    // What every creature is wearing is worked out before anything acts: a blow this turn asks what the armour
    // was at the start of it, not what it became afterwards.
    focus_stage(world);
    crate::creature::dress(world);
    // Reference: `monster::process_turn` mends a monster before it acts (`monster.cpp:4094`), so a creature one point from death this turn can still take its step.
    crate::regeneration::mend_everyone(world);
    // Reference: `monster::process_turn` regrows a monster's nerve in the same pass that mends it (`monster.cpp:3493-3512`), so a monster that calmed down and one that mended happen
    // before anything acts this turn.
    crate::creature::regen_everyones_nerve(world);
    // Reference: the `CORNERED_FIGHTER` block sits in the same per-turn pass (`monster.cpp:3518`), so a cornered monster is spooked before it decides where to walk.
    crate::creature::cornered_fighters_lose_their_nerve(world);
    // Reference: `Character::update_stamina` gives a character its wind back every turn (`character.cpp:7022-7082`), which is what makes a long fight cost something and a short
    // one recoverable. A creature with no wind has nothing to recover, so the sweep is written to skip it rather than to give it a pool it never had.
    let windy: Vec<Entity> = world
        .query::<(Entity, &crate::creature::Stamina)>()
        .iter(world)
        .map(|(entity, _)| entity)
        .collect();
    for entity in windy {
        let winded = crate::applied_effects::has(world, entity, "winded");
        let _back = crate::regeneration::recover_wind(
            world,
            entity,
            crate::regeneration::BASE_WIND_BACK_PER_TURN,
            winded,
            1,
        );
    }
    // The dead are cleared before anything acts, so a creature killed this turn does not also walk.
    // The dead are cleared before anything acts, so a creature killed this turn does not also walk. When the content has said what a corpse is, the sweep leaves one behind -
    // `monster::die` puts a corpse on the map (`monster.cpp`), and a corpse is what can be butchered and what can get up again.
    if let Some((corpse_item, volumes)) = crate::creature::corpse_kind(world) {
        let _bodies = crate::creature::reap_dead_creatures_as_corpses(world, corpse_item, |kind| {
            volumes.get(&kind).copied().unwrap_or(0)
        });
    } else {
        crate::creature::reap_dead_creatures(world);
    }
    crate::creature::take_turns(world);
}

fn note(world: &mut World, stage: TurnStage) {
    singletons::turn_trace(world).stages.push(stage);
}

/// Runs the effects whose turn has come - the reference's `game::process_events`, which the turn loop calls before anything acts.
fn scheduled_stage(world: &mut World) {
    note(world, TurnStage::Scheduled);
    if let Some(speaker) = crate::missions::avatar(world) {
        crate::scheduled::run_due(world, speaker);
    }
}

fn scent_stage(world: &mut World) {
    note(world, TurnStage::Scent);
    let source = *singletons::scent_source(world);
    crate::scent::advance_scent(world, source.center, source.deposit);
}

fn falling_stage(world: &mut World) {
    note(world, TurnStage::Falling);
    crate::process_falling_turn(world);
}

fn fields_stage(world: &mut World) {
    note(world, TurnStage::Fields);
    let windpower = crate::weather::wind_of(world, 0);
    let facts = crate::MapFacts::from_world(world);
    let refusing = singletons::refusing_gas(world);
    let mut rolls = singletons::take_rolls(world).unwrap_or_else(|| Rolls::new(0));
    crate::run_field_sweep(
        world,
        &facts,
        windpower,
        |_, to| !refusing.contains(&to),
        &mut rolls,
    );
    burn_whoever_stands_in_fire(world, &mut rolls);
    singletons::put_rolls(world, rolls);
}

/// Burns the creatures standing in fire, once a turn.
///
/// Reference: the fire field's own creature work runs in `map::process_fields_in_submap` (`map_field.cpp:1626-1702`), which is the same pass that ages the fields - so the port runs it
/// here, in the fields stage, after the sweep. **The per-part draws are the reference's** ([`crate::burning::fire_burn`]); what the port cannot yet do is keep a body part's own hit points,
/// so the draws are **summed onto the creature's pool** and the summary says so.
fn burn_whoever_stands_in_fire(world: &mut World, rolls: &mut Rolls) {
    /// `fd_fire` in the reference's `field_type` numbering.
    const FIRE: crate::FieldId = crate::FieldId(14);

    let mut standing: Vec<bevy_ecs::entity::Entity> = Vec::new();
    let Some(mut query) = world.try_query_filtered::<bevy_ecs::entity::Entity, bevy_ecs::prelude::With<crate::creature::Health>>() else {
        return;
    };
    standing.extend(query.iter(world));
    for creature in standing {
        let Some(at) = world.get::<crate::Position>(creature).copied() else {
            continue;
        };
        let intensity = crate::field_strength_at(world, (at.x, at.y, at.z), FIRE);
        if intensity < 1 {
            continue;
        }
        // The reference asks whether the creature is on the ground; the port's creatures do not fall prone yet, so every one of them stands.
        let burns = crate::burning::fire_burn(intensity, false, &[], rolls);
        let total: i32 = burns.iter().map(|(_, damage)| *damage).sum();
        if total > 0 {
            #[allow(clippy::cast_precision_loss)]
            crate::creature::hurt(world, creature, total as f32);
        }
        // Reference: `deal_damage` with heat sets the creature alight with a chance equal to the damage (`creature.cpp:1482-1484`), which is what keeps a burning creature burning.
        let mut lit = None;
        if let Some(turns) = crate::creature::heat_sets_alight(total, rolls) {
            lit = Some(turns);
        }
        if let Some(turns) = lit
            && let Some(kind) = crate::effect_registry::kind_of(world, "onfire")
        {
            crate::applied_effects::add(
                world,
                creature,
                crate::effects::ActiveEffect::new(&kind, turns, 1),
            );
        }
    }
}

fn light_stage(world: &mut World) {
    note(world, TurnStage::Light);
    crate::clear_light(world);
    crate::cast_lights_with(world);
}

impl TurnStage {
    /// The stages in the order they run, for a reader that wants to check the schedule.
    #[must_use]
    pub fn order() -> [Self; 7] {
        [
            Self::Scheduled,
            Self::Scent,
            Self::Sounds,
            Self::Falling,
            Self::Fields,
            Self::Creatures,
            Self::Light,
        ]
    }
}

impl Where {
    /// Whether this scope covers a square.
    #[must_use]
    pub fn covers(self, square: (i32, i32, i32)) -> bool {
        self.holds(square)
    }
}
