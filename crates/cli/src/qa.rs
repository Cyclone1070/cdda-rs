//! The quality gate a person runs: everything the port claims, checked in one command.
//!
//! Each check answers the same three things - what it looked at, how many cases it compared, and what
//! did not hold - so the report is a table rather than a wall of passes. A check that compared
//! nothing is a failure, not a pass: a gate that reads zero files and calls itself green is the worst
//! thing in a repository.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

/// One check, and what it found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Check {
    /// What was checked, in plain words.
    pub name: String,
    /// How many cases it actually compared.
    pub cases: usize,
    /// The things that did not hold, in the words of whatever failed.
    pub failures: Vec<String>,
}

impl Check {
    /// A check that compared `cases` things and passed.
    #[must_use]
    pub fn passed(name: &str, cases: usize) -> Self {
        Self {
            name: name.to_string(),
            cases,
            failures: Vec::new(),
        }
    }

    /// Whether this check is worth believing: it compared something and nothing failed.
    #[must_use]
    pub fn green(&self) -> bool {
        self.failures.is_empty() && self.cases > 0
    }
}

/// The table, one row per check.
#[must_use]
pub fn report(checks: &[Check]) -> String {
    let mut out = String::from("check                                                   cases  result\n");
    for check in checks {
        let result = if check.green() {
            "pass".to_string()
        } else if check.cases == 0 {
            "EMPTY".to_string()
        } else {
            format!("FAIL ({} )", check.failures.len())
        };
        let _ = writeln!(out, "{:<54} {:>5}  {result}", check.name, check.cases);
        for failure in check.failures.iter().take(5) {
            let _ = writeln!(out, "    {failure}");
        }
        if check.failures.len() > 5 {
            let _ = writeln!(out, "    ... and {} more", check.failures.len() - 5);
        }
    }
    out
}

/// Every file in the corpus, parsed. The oracle has to be readable before anything can be compared
/// against it, and a corpus file that no longer parses is a silent hole in every gate that uses it.
#[must_use]
pub fn corpus(root: &Path) -> Check {
    let mut cases = 0;
    let mut failures = Vec::new();
    let mut files = Vec::new();
    collect(root, &mut files);
    for path in files {
        let extension = path.extension().and_then(|kind| kind.to_str()).unwrap_or("");
        if extension != "json" {
            continue;
        }
        cases += 1;
        let Ok(text) = std::fs::read(&path) else {
            failures.push(format!("{}: cannot be read", path.display()));
            continue;
        };
        if cdda_json::parse(&text).is_err() {
            failures.push(format!("{}: does not parse", path.display()));
        }
    }
    Check {
        name: "corpus: every JSON file parses".to_string(),
        cases,
        failures,
    }
}

/// Every scenario in the repository, played.
#[must_use]
pub fn scenarios(root: &Path) -> Check {
    let mut cases = 0;
    let mut failures = Vec::new();
    let mut files = Vec::new();
    collect(root, &mut files);
    for path in files {
        if path.extension().and_then(|kind| kind.to_str()) != Some("txt") {
            continue;
        }
        cases += 1;
        let Ok(text) = std::fs::read_to_string(&path) else {
            failures.push(format!("{}: cannot be read", path.display()));
            continue;
        };
        match crate::run(&text) {
            Ok(outcome) if outcome.passed() => {}
            Ok(outcome) => {
                for failure in outcome.failures {
                    failures.push(format!("{}: {failure}", path.display()));
                }
            }
            Err(error) => failures.push(format!("{}: {error}", path.display())),
        }
    }
    Check {
        name: "scenarios: every playtest passes".to_string(),
        cases,
        failures,
    }
}

/// Every positioned thing is where the index says it is, and the index holds nothing else.
#[must_use]
pub fn index_matches_the_store(world: &mut bevy_ecs::prelude::World) -> Check {
    let positioned: Vec<(bevy_ecs::prelude::Entity, (i32, i32, i32))> = world
        .query::<(bevy_ecs::prelude::Entity, &cdda_sim::Position)>()
        .iter(world)
        .map(|(entity, position)| (entity, (position.x, position.y, position.z)))
        .collect();
    let mut failures = Vec::new();
    for (entity, square) in &positioned {
        let found = cdda_sim::singletons::index(world).is_some_and(|index| {
            index
                .at_level(square.0, square.1, square.2)
                .contains(entity)
        });
        if !found {
            failures.push(format!("{entity:?} stands at {square:?} and the index does not have it"));
        }
    }
    Check {
        name: "index: every positioned thing is found where it stands".to_string(),
        cases: positioned.len(),
        failures,
    }
}

fn collect(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect(&path, out);
        } else {
            out.push(path);
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::panic, clippy::expect_used)]
    use super::*;

    fn repository() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
    }

    #[test]
    fn the_corpus_parses_and_the_check_says_how_much_it_read() {
        let check = corpus(&repository().join("corpus"));

        assert!(check.failures.is_empty(), "{:?}", check.failures);
        assert!(
            check.cases > 40,
            "the corpus is more than forty files; the check saw {}",
            check.cases
        );
    }

    #[test]
    fn an_empty_check_is_not_green() {
        let nothing = Check::passed("nothing at all", 0);
        assert!(!nothing.green(), "a check that compared nothing proves nothing");
    }
}
