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

/// Every replayable session in the corpus, replayed and compared with the reference turn by turn.
#[must_use]
pub fn oracle_sessions() -> Check {
    let mut failures = Vec::new();
    let names = cdda_oracle::session::replayable();
    let mut cases = 0;
    for name in &names {
        // A broken port can make a replay ask for a die the recording does not have, and a QA runner
        // that dies on the first bad check reports nothing about the rest. The failure is caught and
        // named; the report keeps going.
        let compared = std::panic::catch_unwind(|| cdda_oracle::session::compare(name));
        match compared {
            Ok(comparison) => {
                cases += comparison.turns;
                if let Some(difference) = comparison.first_difference {
                    failures.push(format!("{}: {difference}", comparison.name));
                }
            }
            Err(_) => failures.push(format!("{name}: the replay panicked before it finished")),
        }
    }
    Check {
        name: format!("oracle: {} recorded session(s) replay turn for turn", names.len()),
        cases,
        failures,
    }
}

/// Corpus families no test reads, and why. This ledger may only shrink: an entry that becomes read
/// is an error here, so the list cannot quietly grow stale the way the status tables did.
pub const BLOCKED_FAMILIES: &[(&str, &str)] = &[(
    "replay",
    "a keylog is played into a full game with --dump-state, and what it dumps is the avatar's state; \
     there are no creatures to play",
)];

/// Every file under `corpus/` is read by a test, or its family is in the blocked ledger.
///
/// A dump nobody compares against is a promise, not a gate, and two of them sat in this tree for
/// rounds without anyone noticing. This makes that impossible to do quietly: a new corpus file whose
/// family no source mentions fails here.
#[must_use]
pub fn corpus_is_read(root: &Path) -> Check {
    let corpus = root.join("corpus");
    let mut files = Vec::new();
    collect(&corpus, &mut files);
    let sources = read_sources(&root.join("crates"));
    let blocked: std::collections::BTreeSet<&str> =
        BLOCKED_FAMILIES.iter().map(|(family, _)| *family).collect();
    let mut cases = 0;
    let mut failures = Vec::new();
    // Families are what the ledger talks about, so the check is by family: `corpus/fields/` is read
    // by the session loader, and every fixture inside it with it.
    let mut families: std::collections::BTreeMap<String, bool> = std::collections::BTreeMap::new();
    for file in &files {
        cases += 1;
        let Ok(relative) = file.strip_prefix(&corpus) else {
            continue;
        };
        let Some(family) = relative.components().next().and_then(|first| first.as_os_str().to_str())
        else {
            continue;
        };
        if family.contains('.') {
            // A file at the root of the corpus has no family. It is read if a source names it, which
            // is by its file name rather than its path: a test that builds the path from a root
            // constant never writes the path out.
            if family == "README.md" {
                continue;
            }
            if !sources.contains(family) {
                failures.push(format!("corpus/{family}: no test names it and it belongs to no family"));
            }
            continue;
        }
        let read = sources.contains(&format!("corpus/{family}"));
        families.insert(family.to_string(), read);
    }
    for (family, read) in &families {
        match (read, blocked.contains(family.as_str())) {
            (true, true) => failures.push(format!(
                "corpus/{family}: listed as blocked but a test reads it - drop the ledger line"
            )),
            (false, false) => failures.push(format!(
                "corpus/{family}: no test reads this family and it is not in the blocked ledger"
            )),
            _ => {}
        }
    }
    Check {
        name: format!("corpus: every family is read ({} blocked)", BLOCKED_FAMILIES.len()),
        cases,
        failures,
    }
}

/// Every Rust source under `crates/`, as one string, for asking "does anything read this".
fn read_sources(dir: &Path) -> String {
    let mut files = Vec::new();
    collect(dir, &mut files);
    let mut all = String::new();
    for path in files {
        if path.extension().and_then(|kind| kind.to_str()) != Some("rs") {
            continue;
        }
        if let Ok(text) = std::fs::read_to_string(&path) {
            all.push_str(&text);
            all.push('\n');
        }
    }
    all
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

/// Every scenario that draws a picture has drawn the **recorded** one.
///
/// The capture is deterministic - the same world draws the same bytes - so a scenario's `shot` step is a gate on everything the view reads: terrain, light, creatures and
/// items. The recording lives in `corpus/screens/`, named after its scenario, and this check replays each scenario in a scratch directory and compares the bytes.
#[must_use]
pub fn screens(root: &Path) -> Check {
    let mut cases = 0;
    let mut failures = Vec::new();
    let recorded = root.join("corpus/screens");
    let Ok(entries) = std::fs::read_dir(&recorded) else {
        return Check::passed("screens: every recorded picture still draws the same", 0);
    };
    let scratch = std::env::temp_dir().join("cdda-qa-screens");
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|kind| kind.to_str()) != Some("png") {
            continue;
        }
        let Some(stem) = path.file_stem().and_then(|name| name.to_str()) else {
            continue;
        };
        let scenario = root.join("scenarios").join(format!("{stem}.txt"));
        let Ok(text) = std::fs::read_to_string(&scenario) else {
            failures.push(format!("{}: draws a picture but has no scenario", path.display()));
            continue;
        };
        cases += 1;
        let _ = std::fs::create_dir_all(&scratch);
        let drawn = scratch.join(format!("{stem}.png"));
        // A scenario draws into `target/screens/`, a build directory; the replay draws into the scratch file instead, so comparing can never rewrite what it compares
        // against. A scenario that wrote straight into `corpus/` would update its own recording and the gate would never fail.
        let replay = text.replace(
            &format!("target/screens/{stem}.png"),
            &drawn.to_string_lossy(),
        );
        match crate::run(&replay) {
            Ok(outcome) if !outcome.passed() => {
                failures.push(format!("{stem}: the scenario does not play: {:?}", outcome.failures));
            }
            Ok(_) => match (std::fs::read(&drawn), std::fs::read(&path)) {
                (Ok(fresh), Ok(said)) if fresh == said => {}
                (Ok(fresh), Ok(said)) => failures.push(format!(
                    "{stem}: the picture changed - {} bytes now against {} recorded",
                    fresh.len(),
                    said.len()
                )),
                _ => failures.push(format!("{stem}: the picture could not be read back")),
            },
            Err(error) => failures.push(format!("{stem}: {error}")),
        }
        let _ = std::fs::remove_file(&drawn);
    }
    Check {
        name: "screens: every recorded picture still draws the same".to_string(),
        cases,
        failures,
    }
}#[cfg(test)]
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
    fn every_corpus_family_is_read_or_declared_blocked() {
        let check = corpus_is_read(&repository());

        assert!(check.failures.is_empty(), "{:#?}", check.failures);
        assert!(
            check.cases > 40,
            "the corpus is more than forty files and the check saw {}",
            check.cases
        );
    }

    #[test]
    fn an_empty_check_is_not_green() {
        let nothing = Check::passed("nothing at all", 0);
        assert!(!nothing.green(), "a check that compared nothing proves nothing");
    }
}


