//! Diagnostics: log levels, message format, the error path and the assertion
//! policy, ported test-first.
//!
//! Specification is the reference's `src/debug.h` (level and class enums,
//! `cata_assert` wrapper in `src/cata_assert.h`) and `src/debug.cpp`
//! (`DebugLog`, `realDebugmsg`, `setupDebug`). The reference's own tests drive
//! a run to failure through `debug_has_error_been_observed()`.
//!
//! The reference writes to a process-global log file, pops a curses prompt and
//! reads the clock. Here the clock is a value, the sink is a parameter and the
//! prompt is a decision returned to the caller.

use std::cell::Cell;

use cdda_core_types::diagnostics::{
    AssertOutcome, AssertPolicy, DebugClass, DebugLevel, Diagnostics, Filter, PromptDisposition,
    Site, Time, UiState,
};

/// The bit values are ABI in the reference: log files are grepped by name and
/// `setupDebug` builds masks from individual defines (`debug.h:143-180`).
#[test]
fn levels_and_classes_keep_the_reference_bit_values() {
    assert_eq!(DebugLevel::INFO.bits(), 1);
    assert_eq!(DebugLevel::WARNING.bits(), 1 << 2);
    assert_eq!(DebugLevel::ERROR.bits(), 1 << 3);
    assert_eq!(DebugLevel::PEDANTIC_INFO.bits(), 1 << 4);
    assert_eq!(DebugLevel::ALL.bits(), (1 << 5) - 1);
    assert_eq!(DebugLevel::NONE.bits(), 0);

    assert_eq!(DebugClass::MAIN.bits(), 1);
    assert_eq!(DebugClass::MAP.bits(), 1 << 2);
    assert_eq!(DebugClass::MAP_GEN.bits(), 1 << 3);
    assert_eq!(DebugClass::GAME.bits(), 1 << 4);
    assert_eq!(DebugClass::NPC.bits(), 1 << 5);
    assert_eq!(DebugClass::SDL.bits(), 1 << 6);
    assert_eq!(DebugClass::MMAP.bits(), 1 << 7);
    assert_eq!(DebugClass::ALL.bits(), (1 << 30) - 1);

    // Union and intersection are bitwise, so combined levels are usable.
    let combined = DebugLevel::WARNING.union(DebugLevel::ERROR);
    assert_eq!(
        combined.bits(),
        DebugLevel::WARNING.bits() | DebugLevel::ERROR.bits()
    );
    assert!(combined.contains(DebugLevel::ERROR));
    assert!(!combined.contains(DebugLevel::INFO));
    assert!(combined.intersects(DebugLevel::ALL));
    assert!(!DebugLevel::NONE.intersects(DebugLevel::ALL));

    // Names are printed in the reference's fixed order, and the all-bits value
    // prints nothing at all (`debug.cpp:853-898`).
    assert_eq!(combined.names(), vec!["WARNING", "ERROR"]);
    assert_eq!(
        DebugLevel::INFO
            .union(DebugLevel::WARNING)
            .union(DebugLevel::ERROR)
            .union(DebugLevel::PEDANTIC_INFO)
            .names(),
        vec!["INFO", "WARNING", "ERROR", "PEDANTIC"]
    );
    assert!(DebugLevel::ALL.names().is_empty());
    assert!(DebugClass::ALL.names().is_empty());
    assert_eq!(
        DebugClass::MAIN.union(DebugClass::SDL).names(),
        vec!["MAIN", "SDL"]
    );
    assert_eq!(
        DebugClass::ALL
            .union(DebugClass::GAME)
            .union(DebugClass::MAIN)
            .bits(),
        (1 << 30) - 1
    );
    // `DC_ALL` on its own prints no names either.
    assert!(DebugClass::ALL.names().is_empty());
    // The class order the reference prints in is MAIN, MAP, MAP_GEN, NPC, GAME,
    // SDL, MMAP — not the order the enum declares.
    assert_eq!(
        DebugClass::MMAP
            .union(DebugClass::SDL)
            .union(DebugClass::GAME)
            .union(DebugClass::NPC)
            .union(DebugClass::MAP_GEN)
            .union(DebugClass::MAP)
            .union(DebugClass::MAIN)
            .names(),
        vec!["MAIN", "MAP", "MAP_GEN", "NPC", "GAME", "SDL", "MMAP"]
    );
}

/// The filter admits a message when its level and class both intersect the
/// masks, *or* when it is an error, *or* when it comes from `D_MAIN`
/// (`debug.cpp:1510-1512`).
#[test]
fn filter_admits_errors_and_main_messages_regardless_of_the_masks() {
    // A release build filters to D_ERROR and D_MAIN only (`debug.cpp:95-101`).
    let release = Filter::release_default();
    assert_eq!(release.level, DebugLevel::ERROR);
    assert_eq!(release.class, DebugClass::MAIN);
    let debug = Filter::debug_default();
    assert_eq!(debug.level, DebugLevel::ALL);
    assert_eq!(debug.class, DebugClass::ALL);

    // Plain info from a non-main class is filtered out in a release build.
    assert!(!release.passes(DebugLevel::INFO, DebugClass::MAP));
    assert!(debug.passes(DebugLevel::INFO, DebugClass::MAP));
    // ... but a matching level with a matching class passes.
    assert!(release.passes(DebugLevel::ERROR, DebugClass::MAIN));

    // Errors always pass, whatever the class mask says.
    assert!(release.passes(DebugLevel::ERROR, DebugClass::MAP));
    assert!(Filter::default().passes(DebugLevel::ERROR, DebugClass::NPC));
    // Messages from D_MAIN always pass, whatever the level mask says.
    assert!(release.passes(DebugLevel::INFO, DebugClass::MAIN));
    assert!(Filter::default().passes(DebugLevel::PEDANTIC_INFO, DebugClass::MAIN));

    // Both halves of the AND are required for the first branch.
    let map_only = Filter {
        level: DebugLevel::INFO,
        class: DebugClass::MAP,
    };
    assert!(map_only.passes(DebugLevel::INFO, DebugClass::MAP));
    assert!(!map_only.passes(DebugLevel::INFO, DebugClass::NPC));
    assert!(!map_only.passes(DebugLevel::WARNING, DebugClass::MAP));
}

/// Each logged message is one line: a newline, `HH:MM:SS.mmm`, a space, the
/// level names, the class name, `": "` and the text (`debug.cpp:1515-1523`).
/// The class name is omitted when it equals the *filter* mask, which is why a
/// stock release log shows `ERROR :` with a space before the colon.
#[test]
fn log_lines_have_the_reference_format() {
    let time = Time::new(1, 2, 3, 4);
    let mut log = Diagnostics::new(Filter::release_default());

    assert!(log.log(DebugLevel::ERROR, DebugClass::MAIN, time, "boom"));
    assert_eq!(log.sink(), "\n01:02:03.004 ERROR : boom");

    // Several levels print in their fixed order; a class that differs from the
    // filter mask is named after them.
    let mut log = Diagnostics::new(Filter::release_default());
    assert!(log.log(
        DebugLevel::WARNING.union(DebugLevel::ERROR),
        DebugClass::MAP,
        time,
        "mixed"
    ));
    assert_eq!(log.sink(), "\n01:02:03.004 WARNING ERROR MAP : mixed");

    // With the debug filter the class mask is `DC_ALL`, so any concrete class
    // is printed even though nothing is filtered out.
    let mut log = Diagnostics::new(Filter::debug_default());
    assert!(log.log(DebugLevel::INFO, DebugClass::MAP, time, "info"));
    assert_eq!(log.sink(), "\n01:02:03.004 INFO MAP : info");

    // Two messages are two lines, and a second line starts with its own newline.
    assert!(log.log(
        DebugLevel::INFO,
        DebugClass::GAME,
        Time::new(23, 59, 58, 1000),
        "x"
    ));
    assert_eq!(
        log.sink(),
        "\n01:02:03.004 INFO MAP : info\n23:59:58.1000 INFO GAME : x"
    );
}

/// Filtered messages produce no output at all, and the all-bits level and class
/// print no names: `DebugLog( DL_ALL, DC_ALL )` yields just `time : text`
/// (`debug.cpp:853-898,1520-1523`).
#[test]
fn filtered_messages_write_nothing_and_all_bits_print_no_names() {
    let time = Time::new(0, 0, 0, 0);
    let mut log = Diagnostics::new(Filter::release_default());
    assert!(!log.log(DebugLevel::INFO, DebugClass::MAP, time, "dropped"));
    assert_eq!(log.sink(), "");

    // The reference logs `DL_ALL, DC_ALL` when the limits themselves are set.
    let mut log = Diagnostics::new(Filter::default());
    assert!(log.log(
        DebugLevel::ALL,
        DebugClass::ALL,
        time,
        "Set debug level to: 31"
    ));
    assert_eq!(log.sink(), "\n00:00:00.000 : Set debug level to: 31");

    // A filtered-out message must not record an error, but an error is recorded
    // even when it is the only thing written (`debug.cpp:1504-1508`).
    let mut log = Diagnostics::new(Filter::default());
    assert!(!log.error_observed());
    log.log(DebugLevel::INFO, DebugClass::NPC, time, "noise");
    assert!(!log.error_observed());
    log.log(DebugLevel::ERROR, DebugClass::NPC, time, "real error");
    assert!(log.error_observed());
    // Taking the log clears the sink but not the flag: the flag is per run.
    let taken = log.take_sink();
    assert_eq!(taken, "\n00:00:00.000 ERROR NPC : real error");
    assert_eq!(log.sink(), "");
    assert!(log.error_observed());
}

/// `realDebugmsg` logs `file:line [function] text` at `D_ERROR`/`D_MAIN`
/// (`debug.cpp:512-530`) and sets the error flag that the reference's test
/// suite uses to fail a run (`debug.cpp:206-209`). The popup is a separate
/// effect, and the caller decides whether one can be shown.
#[test]
fn debugmsg_logs_the_site_and_marks_the_run_as_failed() {
    let site = Site::new("path_info.cpp", 42, "init_user_dir");
    let mut log = Diagnostics::new(Filter::release_default());

    let outcome = log.debugmsg(
        site,
        UiState::default(),
        Time::new(1, 2, 3, 4),
        "Required environment variable HOME was not set",
    );
    assert!(outcome.logged);
    assert_eq!(outcome.prompt, PromptDisposition::Shown);
    assert_eq!(
        log.sink(),
        "\n01:02:03.004 ERROR : path_info.cpp:42 [init_user_dir] \
         Required environment variable HOME was not set"
    );
    assert!(log.error_observed());

    // D_MAIN messages are always logged, even when both masks exclude
    // everything. The class name appears because `D_MAIN` no longer equals the
    // class *filter* mask, which is all the print rule looks at.
    let mut quiet = Diagnostics::new(Filter {
        level: DebugLevel::NONE,
        class: DebugClass::NONE,
    });
    let outcome = quiet.debugmsg(
        site,
        UiState::default(),
        Time::new(0, 0, 0, 0),
        "still logged",
    );
    assert!(outcome.logged);
    assert_eq!(
        quiet.sink(),
        "\n00:00:00.000 ERROR MAIN : path_info.cpp:42 [init_user_dir] still logged"
    );
}

/// The three ways a prompt can be dealt with are caller decisions
/// (`debug.cpp:532-562`): test mode suppresses it, an unavailable screen buffers
/// it for later replay, and a captured message never reaches the log at all.
#[test]
fn prompt_disposition_follows_test_mode_screen_and_capture() {
    let site = Site::new("map.cpp", 7, "load");
    let time = Time::new(12, 0, 0, 500);

    // Test mode: logged, no prompt. `if( test_mode ) return;`
    let mut log = Diagnostics::new(Filter::release_default());
    let outcome = log.debugmsg(
        site,
        UiState {
            test_mode: true,
            screen_available: true,
        },
        time,
        "boom",
    );
    assert_eq!(outcome.prompt, PromptDisposition::NotShown);
    assert!(outcome.logged);
    assert!(log.buffered().is_empty());

    // No screen available: the prompt is buffered for `replay_buffered_debugmsg_prompts`.
    let mut log = Diagnostics::new(Filter::release_default());
    let outcome = log.debugmsg(
        site,
        UiState {
            test_mode: false,
            screen_available: false,
        },
        time,
        "boom",
    );
    assert_eq!(outcome.prompt, PromptDisposition::Buffered);
    assert_eq!(log.buffered().len(), 1);
    assert!(!log.buffered().first().is_some_and(|prompt| prompt.forced));

    // Capturing: the text is collected for the test and nothing is logged, so
    // the run is *not* marked as failed (`debug.cpp:519-523`).
    let mut log = Diagnostics::new(Filter::release_default());
    let outcome = log.capture_debugmsg_during(|log| {
        log.debugmsg(site, UiState::default(), time, "captured text");
    });
    assert_eq!(outcome, "captured text");
    assert_eq!(log.sink(), "");
    assert!(!log.error_observed());

    // A message the user chose to ignore is not shown, and the ignore list is
    // keyed on the file and line, not the function (`debug.cpp:309-313`).
    let mut log = Diagnostics::new(Filter::release_default());
    log.ignore_site(site);
    let outcome = log.debugmsg(site, UiState::default(), time, "boom");
    assert_eq!(outcome.prompt, PromptDisposition::NotShown);
    assert!(
        outcome.logged,
        "ignoring a message does not stop it being logged"
    );
    let other_line = Site::new("map.cpp", 8, "load");
    let outcome = log.debugmsg(other_line, UiState::default(), time, "boom");
    assert_eq!(outcome.prompt, PromptDisposition::Shown);
}

/// Repeated identical messages inside 100 ms are folded into the first log line
/// and summarised when the next message is written (`debug.cpp:448-510,1485-1502`):
/// the summary is a duplicate of the folded line, prefixed with
/// `[ Previous repeated N times ]` only when the message repeated more than once.
#[test]
fn repeated_messages_are_folded_and_summarised() {
    let site = Site::new("a.cpp", 1, "f");
    let other = Site::new("b.cpp", 2, "g");
    let mut log = Diagnostics::new(Filter::release_default());

    assert!(
        log.debugmsg(site, UiState::default(), Time::new(1, 0, 0, 0), "a")
            .logged
    );
    // Two repeats within 100 ms: neither is logged on its own.
    assert!(
        !log.debugmsg(site, UiState::default(), Time::new(1, 0, 0, 10), "a")
            .logged
    );
    assert!(
        !log.debugmsg(site, UiState::default(), Time::new(1, 0, 0, 20), "a")
            .logged
    );
    // A different message flushes the summary, then logs itself.
    assert!(
        log.debugmsg(other, UiState::default(), Time::new(1, 0, 0, 30), "b")
            .logged
    );
    assert_eq!(
        log.sink(),
        "\n01:00:00.000 ERROR : a.cpp:1 [f] a\
         \n[ Previous repeated 1 times ]\
         \n01:00:00.020 ERROR : a.cpp:1 [f] a\
         \n01:00:00.030 ERROR : b.cpp:2 [g] b"
    );
}

/// The fold window is 100 ms with a strict `>` comparison, a single repeat
/// produces no `[ Previous repeated ]` note, and the 10 000th repeat triggers
/// the forced "excessive error repetition" prompt (`debug.cpp:455-456,550-576`).
#[test]
fn fold_window_and_excessive_repetition_threshold() {
    let site = Site::new("a.cpp", 1, "f");
    let mut log = Diagnostics::new(Filter::release_default());

    assert!(
        log.debugmsg(site, UiState::default(), Time::new(0, 0, 0, 0), "a")
            .logged
    );
    // Exactly 100 ms later is still within the window (the test is `>` 100).
    assert!(
        !log.debugmsg(site, UiState::default(), Time::new(0, 0, 0, 100), "a")
            .logged
    );
    // 201 ms after the last repeat is outside it.
    assert!(
        log.debugmsg(site, UiState::default(), Time::new(0, 0, 0, 301), "a")
            .logged
    );
    // repeat_count == 1, so the summary is a bare duplicate with no note.
    assert_eq!(
        log.sink(),
        "\n00:00:00.000 ERROR : a.cpp:1 [f] a\
         \n00:00:00.100 ERROR : a.cpp:1 [f] a\
         \n00:00:00.301 ERROR : a.cpp:1 [f] a"
    );

    // 10 000 folds in the same millisecond reach the threshold.
    let mut log = Diagnostics::new(Filter::release_default());
    let mut last = log.debugmsg(site, UiState::default(), Time::new(0, 0, 0, 0), "a");
    assert!(!last.excessive_repetition);
    for _ in 0..10_000 {
        last = log.debugmsg(site, UiState::default(), Time::new(0, 0, 0, 0), "a");
    }
    assert!(last.excessive_repetition);
    assert_eq!(log.buffered().len(), 0);
    // The 10 001st fold is no longer at the threshold.
    let afterwards = log.debugmsg(site, UiState::default(), Time::new(0, 0, 0, 0), "a");
    assert!(!afterwards.excessive_repetition);

    // Without a screen every message is buffered, folded or not, and the
    // excessive-repetition prompt adds a second, forced entry that bypasses the
    // ignore list when replayed (`debug.cpp:552-561`).
    let mut log = Diagnostics::new(Filter::release_default());
    let no_screen = UiState {
        test_mode: false,
        screen_available: false,
    };
    // The first message of the run is logged rather than folded, so the
    // threshold is reached after 10 000 folds following it.
    let _ = log.debugmsg(site, no_screen, Time::new(0, 0, 0, 0), "a");
    for _ in 0..10_000 {
        let _ = log.debugmsg(site, no_screen, Time::new(0, 0, 0, 0), "a");
    }
    assert_eq!(log.buffered().len(), 10_002);
    let forced = log.buffered().last();
    assert_eq!(forced.map(|prompt| prompt.forced), Some(true));
    assert!(forced.is_some_and(|prompt| {
        prompt
            .text
            .starts_with("Excessive error repetition detected.")
    }));
    // Past the threshold the forced prompt is gone again.
    let outcome = log.debugmsg(site, no_screen, Time::new(0, 0, 0, 0), "a");
    assert!(!outcome.excessive_repetition);
    assert_eq!(log.buffered().len(), 10_003);
    assert_eq!(
        log.buffered().last().map(|prompt| prompt.forced),
        Some(false)
    );
}

/// `cata_assert` (`src/cata_assert.h:22-37`): in a debug build a failed
/// condition is reported and aborts; in a release build (`NDEBUG`) the
/// expression is never evaluated at all, so the failure is silently ignored.
///
/// Whether the process actually aborts is the caller's decision — the library
/// returns the report instead of ending the run.
#[test]
fn assertion_policy_reports_in_debug_and_skips_in_release() {
    let site = Site::new("path_info.cpp", 77, "init_user_dir");
    let evaluations = Cell::new(0);

    // Debug build: a passing condition reports nothing and the expression runs.
    let debug = AssertPolicy::debug();
    let outcome = debug.check("!path.empty()", site, || {
        evaluations.set(evaluations.get() + 1);
        true
    });
    assert_eq!(outcome, AssertOutcome::Passed);
    assert_eq!(evaluations.get(), 1);

    // Debug build: a failing condition produces the reference's exact report.
    let outcome = debug.check("!path.empty()", site, || false);
    assert_eq!(
        outcome,
        AssertOutcome::Aborted {
            report: "init_user_dir at path_info.cpp:77: Assertion `!path.empty()` failed.\n"
                .to_owned()
        }
    );

    // Release build: the expression is not evaluated, so its side effects do
    // not happen and no report is produced.
    let release = AssertPolicy::release();
    let outcome = release.check("!path.empty()", site, || {
        evaluations.set(evaluations.get() + 1);
        false
    });
    assert_eq!(outcome, AssertOutcome::NotEvaluated);
    assert_eq!(
        evaluations.get(),
        1,
        "the release build must not evaluate it"
    );

    // Both profiles accept the `NDEBUG`-equivalent switch at construction.
    assert_eq!(AssertPolicy::default(), AssertPolicy::debug());
}

/// `cata_fatal` (`debug.h:87-91`) is not an assertion: it reports a debug
/// message and aborts in *every* build, so a release build still dies.
#[test]
fn fatal_reports_and_requests_an_abort_in_every_profile() {
    let site = Site::new("path_info.cpp", 70, "getenv_or_abort");
    for policy in [AssertPolicy::debug(), AssertPolicy::release()] {
        let mut log = Diagnostics::new(Filter::release_default());
        let outcome = log.cata_fatal(
            site,
            UiState::default(),
            Time::new(3, 4, 5, 6),
            "Required environment variable HOME was not set",
        );
        assert!(outcome.logged);
        assert!(outcome.abort, "cata_fatal aborts in a {policy:?} build");
        assert_eq!(
            log.sink(),
            "\n03:04:05.006 ERROR : path_info.cpp:70 [getenv_or_abort] \
             Required environment variable HOME was not set"
        );
        assert!(log.error_observed());
        // The abort is requested, never performed; the assertion policy is
        // unrelated to it and stays whatever the build profile says.
        assert_eq!(policy.profile(), policy.profile());
    }
}
