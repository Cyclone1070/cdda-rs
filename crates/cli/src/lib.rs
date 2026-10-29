//! Scripted headless playtests: a world described in a file, played step by step, checked as it goes.
//!
//! A scenario is plain text, one verb per line, and it is deliberately as small as the port is:
//!
//! ```text
//! world 12 flammable
//! seed 7
//! fire 6 6 2 500
//! turn 5
//! expect changed 1
//! expect fire 6 6
//! ```
//!
//! The point is a gate a person can read: when a behaviour changes, the scenario says which turn
//! changed how many squares, and the assertion that fails names the verb that failed. Anything the
//! script does not understand is an error, not a comment - a scenario that silently ignores half its
//! lines is worse than one that refuses to run.

use bevy_ecs::prelude::*;

/// One step of a scenario.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Step {
    /// A world of `side` by `side` squares of floor, flammable or bare.
    World {
        /// How many squares on a side.
        side: i32,
        /// Whether the floor feeds a fire.
        flammable: bool,
    },
    /// The dice every later draw comes from.
    Seed(u32),
    /// Where the character lays its smell, and how much.
    Scent {
        /// The square's x.
        x: i32,
        /// The square's y.
        y: i32,
    },
    /// A fire, with how hard it burns and how long it has burned.
    Fire {
        /// The square's x.
        x: i32,
        /// The square's y.
        y: i32,
        /// Its strength.
        intensity: i32,
        /// Its age, in the reference's units.
        age: i64,
    },
    /// Take this many turns.
    Turn(u32),
    /// The last turn changed exactly this many squares.
    ExpectChanged(usize),
    /// A fire stands on this square.
    ExpectFireAt(i32, i32),
    /// Nothing burns on this square.
    ExpectNoFireAt(i32, i32),
}

/// What running a scenario did.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Outcome {
    /// One line per step that did something worth reading.
    pub lines: Vec<String>,
    /// The assertions that did not hold, in the words of the scenario that made them.
    pub failures: Vec<String>,
}

impl Outcome {
    /// Whether every assertion held.
    #[must_use]
    pub fn passed(&self) -> bool {
        self.failures.is_empty()
    }
}

/// Reads a scenario, refusing anything it does not understand.
///
/// # Errors
///
/// An error naming the line and the word when a line is not a step this runner knows, or when a
/// number is not a number.
pub fn parse(text: &str) -> Result<Vec<Step>, String> {
    let mut steps = Vec::new();
    for (number, line) in text.lines().enumerate() {
        let line = line.split('#').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }
        let words: Vec<&str> = line.split_whitespace().collect();
        let step = match words.as_slice() {
            ["world", side, "flammable"] => Step::World {
                side: number_of(side, number, "side")?,
                flammable: true,
            },
            ["world", side, "bare"] => Step::World {
                side: number_of(side, number, "side")?,
                flammable: false,
            },
            ["seed", seed] => Step::Seed(number_of(seed, number, "seed")?),
            ["scent", x, y] => Step::Scent {
                x: number_of(x, number, "x")?,
                y: number_of(y, number, "y")?,
            },
            ["fire", x, y, intensity, age] => Step::Fire {
                x: number_of(x, number, "x")?,
                y: number_of(y, number, "y")?,
                intensity: number_of(intensity, number, "intensity")?,
                age: number_of(age, number, "age")?,
            },
            ["turn", count] => Step::Turn(number_of(count, number, "turns")?),
            ["expect", "changed", count] => Step::ExpectChanged(number_of(count, number, "squares")?),
            ["expect", "fire", x, y] => {
                Step::ExpectFireAt(number_of(x, number, "x")?, number_of(y, number, "y")?)
            }
            ["expect", "no", "fire", x, y] => {
                Step::ExpectNoFireAt(number_of(x, number, "x")?, number_of(y, number, "y")?)
            }
            _ => return Err(format!("line {}: `{line}` is not a step I know", number + 1)),
        };
        steps.push(step);
    }
    Ok(steps)
}

fn number_of<T: std::str::FromStr>(word: &str, line: usize, what: &str) -> Result<T, String> {
    word.parse()
        .map_err(|_| format!("line {}: `{word}` is not a number for {what}", line + 1))
}

/// Plays a scenario from its text.
///
/// # Errors
///
/// Whatever [`parse`] refuses: a scenario that does not parse is not played at all.
pub fn run(text: &str) -> Result<Outcome, String> {
    let steps = parse(text)?;
    let mut world = World::new();
    let mut outcome = Outcome::default();
    let mut last_changed = 0;
    for step in steps {
        match step {
            Step::World { side, flammable } => {
                build_world(&mut world, side, flammable);
                outcome
                    .lines
                    .push(format!("world: {side} by {side}, flammable: {flammable}"));
            }
            Step::Seed(seed) => {
                cdda_sim::turn::set_random(&mut world, cdda_core_types::rng::Rolls::new(seed));
            }
            Step::Scent { x, y } => {
                cdda_sim::turn::set_scent_source(&mut world, (x, y), 0);
            }
            Step::Fire {
                x,
                y,
                intensity,
                age,
            } => {
                light_a_fire(&mut world, x, y, intensity, age);
            }
            Step::Turn(count) => {
                for _ in 0..count {
                    last_changed = take_turn(&mut world);
                }
                outcome.lines.push(format!(
                    "turn {count}: {last_changed} square(s) changed in the last of them"
                ));
            }
            Step::ExpectChanged(count) => {
                if last_changed != count {
                    outcome.failures.push(format!(
                        "expected {count} changed square(s), the last turn changed {last_changed}"
                    ));
                }
            }
            Step::ExpectFireAt(x, y) => {
                if cdda_sim::fire_facts::fire_at(&world, (x, y, 0)).is_none() {
                    outcome.failures.push(format!("expected a fire at ({x}, {y})"));
                }
            }
            Step::ExpectNoFireAt(x, y) => {
                if cdda_sim::fire_facts::fire_at(&world, (x, y, 0)).is_some() {
                    outcome.failures.push(format!("expected no fire at ({x}, {y})"));
                }
            }
        }
    }
    Ok(outcome)
}

/// Builds a world of `side` by `side` squares, which is what a scenario starts from.
pub fn build_world(world: &mut World, side: i32, flammable: bool) {
    let mut level = cdda_core_types::level::Level::new();
    for y in 0..side {
        for x in 0..side {
            level.set_terrain(x, y, 1, 0);
        }
    }
    cdda_sim::load_level(world, &level);
    for y in 0..side {
        for x in 0..side {
            cdda_sim::fire_facts::set_fire_facts(
                world,
                (x, y, 0),
                cdda_sim::TerrainFacts {
                    flammable,
                    ..cdda_sim::TerrainFacts::default()
                },
            );
        }
    }
}

/// Puts a fire on a square.
pub fn light_a_fire(world: &mut World, x: i32, y: i32, intensity: i32, age: i64) {
    let mut entry = cdda_core_types::field::FieldEntry::new(3, 1_000_000, false, true);
    entry.set_age(age);
    entry.set_intensity(intensity);
    let entity = world
        .spawn((
            cdda_sim::Field {
                id: cdda_sim::FIRE,
                entry,
                priority: 0,
            },
            cdda_sim::Position { x, y, z: 0 },
        ))
        .id();
    cdda_sim::singletons::index_mut(world).insert(entity, (x, y, 0));
}

/// Takes one turn, and answers how many squares it changed.
pub fn take_turn(world: &mut World) -> usize {
    let before = cdda_sim::inspect::world_fingerprints(world);
    cdda_sim::turn::run_turn(world);
    let after = cdda_sim::inspect::world_fingerprints(world);
    cdda_sim::inspect::squares_that_differ(&before, &after).len()
}

#[cfg(test)]
mod tests {
    #![allow(clippy::panic, clippy::expect_used)]
    use super::*;

    #[test]
    fn a_scenario_that_says_what_it_means_runs_and_passes() {
        let outcome = run(
            "world 12 flammable\n\
             seed 7\n\
             fire 6 6 2 500\n\
             turn 3\n\
             expect changed 1\n\
             expect fire 6 6\n",
        )
        .expect("the scenario parses");

        assert!(outcome.passed(), "the assertions held: {:?}", outcome.failures);
        assert!(
            outcome.lines.iter().any(|line| line.contains("1 square(s) changed")),
            "the run reports what happened: {:?}",
            outcome.lines
        );
    }

    #[test]
    fn an_assertion_that_does_not_hold_is_named() {
        let outcome = run("world 12 bare\nfire 6 6 2 500\nturn 1\nexpect no fire 6 6\n")
            .expect("the scenario parses");

        assert!(!outcome.passed(), "the fire is still there after one turn");
        assert_eq!(
            outcome.failures,
            vec!["expected no fire at (6, 6)".to_string()],
            "and the failure says which assertion failed"
        );
    }

    #[test]
    fn every_scenario_in_the_repository_passes() {
        // The directory is the gate: a scenario committed here is one that runs, so a behaviour change
        // that breaks a playtest shows up as a failing test and not as a file nobody executed.
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../scenarios");
        let mut ran = 0;
        let mut failures = Vec::new();
        let entries = std::fs::read_dir(&dir).expect("the scenarios directory");
        for entry in entries {
            let entry = entry.expect("a directory entry");
            let path = entry.path();
            if path.extension().and_then(|kind| kind.to_str()) != Some("txt") {
                continue;
            }
            let text = std::fs::read_to_string(&path).expect("a scenario");
            match run(&text) {
                Ok(outcome) if outcome.passed() => ran += 1,
                Ok(outcome) => failures.push(format!("{}: {:?}", path.display(), outcome.failures)),
                Err(error) => failures.push(format!("{}: {error}", path.display())),
            }
        }
        assert!(ran >= 2, "the gate ran {ran} scenario(s), which is not enough to say anything");
        assert!(failures.is_empty(), "scenarios failed:\n{}", failures.join("\n"));
    }

    #[test]
    fn a_line_nobody_understands_stops_the_run() {
        let error = parse("world 12 flammable\ndance\n").expect_err("dance is not a step");

        assert!(
            error.contains("line 2") && error.contains("dance"),
            "the error names the line and the word: {error}"
        );
    }
}
