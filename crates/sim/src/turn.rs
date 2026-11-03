//! A turn of the world, as a schedule with the stages declared.
//!
//! The reference's turn is one function that calls twenty subsystems in the order its own comments
//! depend on (`do_turn.cpp:642-655`). This port states that order once, as system sets, and lets the
//! ECS run them: scent, then what fell, then the fields, then the light the turn left behind.
//!
//! Everything a stage needs is in the store - the smell the character made is a component on it, the
//! wind is the weather's, the dice are a thing like any other. A stage takes `&mut World` and
//! nothing else.

use crate::{singletons, Where};
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
            TurnStage::Falling,
            TurnStage::Fields,
            TurnStage::Creatures,
            TurnStage::Light,
        )
            .chain(),
    );
    schedule.add_systems(
        (
            scheduled_stage.in_set(TurnStage::Scheduled),
            scent_stage.in_set(TurnStage::Scent),
            needs_stage.in_set(TurnStage::Creatures),
            digest_stage.in_set(TurnStage::Creatures),
            falling_stage.in_set(TurnStage::Falling),
            fields_stage.in_set(TurnStage::Fields),
            creatures_stage.in_set(TurnStage::Creatures),
            light_stage.in_set(TurnStage::Light),
        ),
    );
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
    let mut query = world.query_filtered::<bevy_ecs::entity::Entity, bevy_ecs::prelude::With<crate::skills::Focus>>();
    for creature in query.iter(world) {
        creatures.push(creature);
    }
    for creature in creatures {
        // Reference: `Character::calc_focus_equilibrium` (`character.cpp:3852-3920`) counts the creature's **perceived pain** against its morale, and a masochist minds it
        // only past twenty. The pain is the component's own number, the morale the points on the creature.
        let morale = crate::morale::net_bonus_of(world, creature);
        let pain = world.get::<crate::creature::Pain>(creature).map_or(0, |pain| pain.0);
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
    let mut rolls = singletons::take_rolls(world).unwrap_or_else(|| cdda_core_types::rng::Rolls::new(0));
    for (creature, rates) in creatures {
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
    let mut creatures: Vec<(bevy_ecs::entity::Entity, crate::needs::NeedsRates, crate::body::BodyClock)> = Vec::new();
    let mut query = world.query::<(
        bevy_ecs::entity::Entity,
        &crate::needs::NeedsRatesOf,
        &crate::body::BodyClock,
    )>();
    for (creature, rates, clock) in query.iter(world) {
        creatures.push((creature, rates.0, *clock));
    }
    let mut rolls = singletons::take_rolls(world).unwrap_or_else(|| cdda_core_types::rng::Rolls::new(0));
    for (creature, rates, clock) in creatures {
        let Some(mut body) = world.get::<crate::body::Body>(creature).cloned() else {
            continue;
        };
        let (add, multiply) = match crate::mutation_data::holder(world) {
            Some(data) => crate::mutations::enchantment_values(world, creature, &data, "STOMACH_SIZE_MULTIPLIER"),
            None => (0.0, 0.0),
        };
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

fn creatures_stage(world: &mut World) {
    note(world, TurnStage::Creatures);
    // What every creature is wearing is worked out before anything acts: a blow this turn asks what the armour
    // was at the start of it, not what it became afterwards.
    focus_stage(world);
    crate::creature::dress(world);
    // The dead are cleared before anything acts, so a creature killed this turn does not also walk.
    crate::creature::reap_dead_creatures(world);
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
    singletons::put_rolls(world, rolls);
}

fn light_stage(world: &mut World) {
    note(world, TurnStage::Light);
    crate::clear_light(world);
    crate::cast_lights_with(world);
}

impl TurnStage {
    /// The stages in the order they run, for a reader that wants to check the schedule.
    #[must_use]
    pub fn order() -> [Self; 6] {
        [
            Self::Scheduled,
            Self::Scent,
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
