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
            scent_stage.in_set(TurnStage::Scent),
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

fn creatures_stage(world: &mut World) {
    note(world, TurnStage::Creatures);
    // The dead are cleared before anything acts, so a creature killed this turn does not also walk.
    crate::creature::reap_dead_creatures(world);
    crate::creature::take_turns(world);
}

fn note(world: &mut World, stage: TurnStage) {
    singletons::turn_trace(world).stages.push(stage);
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
    pub fn order() -> [Self; 5] {
        [
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
