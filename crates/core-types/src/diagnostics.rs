//! Diagnostics: log levels, error reporting, and the assertion policy.
//!
//! Reference: `src/debug.h`, `src/debug.cpp`, plus `cata_assert` from
//! `src/cata_assert.h`.
//!
//! The three effects of a debug message are separated here: the log line goes to
//! a sink owned by [`Diagnostics`], the popup becomes a [`PromptDisposition`] the
//! caller acts on, and the run-failed flag is [`Diagnostics::error_observed`].
//! The clock is a [`Time`] value, so output is deterministic.
//!
//! Not modelled: the log file itself (`DebugFile::init` opens, rotates at 1 MiB
//! and appends, `debug.cpp:747-794`), backtrace emission, the Windows
//! `OutputDebugString` stream, the contents of the popup drawn by
//! `debug_error_prompt`, replaying buffered prompts once a screen exists, and
//! `isDebuggerActive`, which reads `/proc/self/status` on Linux.

use std::collections::BTreeSet;

/// How critical a message is (`DebugLevel`, `src/debug.h:143-150`).
///
/// The values are a bitmask: a message may carry several levels at once, and a
/// filter is a mask to intersect with.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DebugLevel(u8);

impl DebugLevel {
    /// `D_INFO`: ordinary information.
    pub const INFO: Self = Self(1);
    /// `D_WARNING`: something looks wrong but the game continues.
    pub const WARNING: Self = Self(1 << 2);
    /// `D_ERROR`: an error; always logged and it marks the run as failed.
    pub const ERROR: Self = Self(1 << 3);
    /// `D_PEDANTIC_INFO`: chatter that is only wanted when debugging.
    pub const PEDANTIC_INFO: Self = Self(1 << 4);
    /// `DL_ALL`: every level. Prints no level name at all.
    pub const ALL: Self = Self((1 << 5) - 1);
    /// `0`: nothing at all, used to silence a category completely.
    pub const NONE: Self = Self(0);

    /// The raw bitmask.
    #[must_use]
    pub fn bits(self) -> u8 {
        self.0
    }

    /// Combine two levels (the reference's `operator|`).
    #[must_use]
    pub fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    /// True when every bit of `other` is set in `self`.
    #[must_use]
    pub fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    /// True when any bit of `other` is set in `self`, which is how the filter
    /// tests a mask (`debug.cpp:1512`).
    #[must_use]
    pub fn intersects(self, other: Self) -> bool {
        self.0 & other.0 != 0
    }

    /// The names the reference prints for this level, in its fixed order.
    ///
    /// `DL_ALL` prints nothing (`debug.cpp:853-870`); each name is written with
    /// a trailing space, so callers join them with separators of their own.
    #[must_use]
    pub fn names(self) -> Vec<&'static str> {
        if self == Self::ALL {
            return Vec::new();
        }
        let mut names = Vec::new();
        for (bit, name) in [
            (Self::INFO, "INFO"),
            (Self::WARNING, "WARNING"),
            (Self::ERROR, "ERROR"),
            (Self::PEDANTIC_INFO, "PEDANTIC"),
        ] {
            if self.intersects(bit) {
                names.push(name);
            }
        }
        names
    }
}

/// Which part of the code a message comes from (`DebugClass`, `src/debug.h:163-180`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DebugClass(u32);

impl DebugClass {
    /// `D_MAIN`: messages from `realDebugmsg`. Always logged.
    pub const MAIN: Self = Self(1);
    /// `D_MAP`: map and mapbuffer.
    pub const MAP: Self = Self(1 << 2);
    /// `D_MAP_GEN`: mapgen and overmap.
    pub const MAP_GEN: Self = Self(1 << 3);
    /// `D_GAME`: the main game class.
    pub const GAME: Self = Self(1 << 4);
    /// `D_NPC`: npcs.
    pub const NPC: Self = Self(1 << 5);
    /// `D_SDL`: SDL, tiles and everything graphical.
    pub const SDL: Self = Self(1 << 6);
    /// `D_MMAP`: map memory.
    pub const MMAP: Self = Self(1 << 7);
    /// `DC_ALL`: every class. Prints no class name at all.
    pub const ALL: Self = Self((1 << 30) - 1);
    /// `0`: nothing at all.
    pub const NONE: Self = Self(0);

    /// The raw bitmask.
    #[must_use]
    pub fn bits(self) -> u32 {
        self.0
    }

    /// Combine two classes (the reference's `operator|`).
    #[must_use]
    pub fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    /// True when every bit of `other` is set in `self`.
    #[must_use]
    pub fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    /// True when any bit of `other` is set in `self`.
    #[must_use]
    pub fn intersects(self, other: Self) -> bool {
        self.0 & other.0 != 0
    }

    /// The names the reference prints for this class, in its fixed order.
    ///
    /// `DC_ALL` prints nothing (`debug.cpp:872-898`). The print order is
    /// `MAIN, MAP, MAP_GEN, NPC, GAME, SDL, MMAP`, which is not the order the
    /// enum declares.
    #[must_use]
    pub fn names(self) -> Vec<&'static str> {
        if self == Self::ALL {
            return Vec::new();
        }
        let mut names = Vec::new();
        for (bit, name) in [
            (Self::MAIN, "MAIN"),
            (Self::MAP, "MAP"),
            (Self::MAP_GEN, "MAP_GEN"),
            (Self::NPC, "NPC"),
            (Self::GAME, "GAME"),
            (Self::SDL, "SDL"),
            (Self::MMAP, "MMAP"),
        ] {
            if self.intersects(bit) {
                names.push(name);
            }
        }
        names
    }
}

/// Which messages reach the log (`debugLevel` and `debugClass`, `debug.cpp:95-101`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Filter {
    /// Levels that are logged.
    pub level: DebugLevel,
    /// Classes that are logged.
    pub class: DebugClass,
}

impl Filter {
    /// The masks of a non-`DEBUG` build: `D_ERROR` and `D_MAIN`.
    #[must_use]
    pub fn release_default() -> Self {
        Self {
            level: DebugLevel::ERROR,
            class: DebugClass::MAIN,
        }
    }

    /// The masks of a `DEBUG` build: everything.
    #[must_use]
    pub fn debug_default() -> Self {
        Self {
            level: DebugLevel::ALL,
            class: DebugClass::ALL,
        }
    }

    /// `( level & debugLevel && class & debugClass ) || level & D_ERROR || class & D_MAIN`.
    ///
    /// Note that errors and `D_MAIN` messages bypass the masks entirely
    /// (`debug.cpp:1510-1512`).
    #[must_use]
    pub fn passes(&self, level: DebugLevel, class: DebugClass) -> bool {
        (level.intersects(self.level) && class.intersects(self.class))
            || level.intersects(DebugLevel::ERROR)
            || class.intersects(DebugClass::MAIN)
    }
}

/// A wall-clock time as the log prints it (`time_info`, `debug.cpp:425-444`).
///
/// The reference reads the clock; here it is an input, so log lines are
/// deterministic.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Time {
    /// Hour of day, 0-23.
    pub hours: u8,
    /// Minute, 0-59.
    pub minutes: u8,
    /// Second, 0-59.
    pub seconds: u8,
    /// Millisecond, 0-999 (the reference can produce 1000 from rounding).
    pub mseconds: u16,
}

impl Time {
    /// A time of day.
    #[must_use]
    pub fn new(hours: u8, minutes: u8, seconds: u8, mseconds: u16) -> Self {
        Self {
            hours,
            minutes,
            seconds,
            mseconds,
        }
    }
}

impl core::fmt::Display for Time {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "{:02}:{:02}:{:02}.{:03}",
            self.hours, self.minutes, self.seconds, self.mseconds
        )
    }
}

/// The debug log, its filters and the error flag.
///
/// Holds the reference's `debugLevel`, `debugClass`, `error_observed` and log
/// stream (`debug.cpp:95-111,1504-1547`). Output goes to a `String` sink so
/// tests never touch the filesystem.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Diagnostics {
    filter: Filter,
    sink: String,
    error_observed: bool,
    repeats: RepetitionFolder,
    buffered: Vec<BufferedPrompt>,
    ignored: BTreeSet<(&'static str, u32)>,
    capturing: Option<String>,
}

impl Diagnostics {
    /// A log with the given filter masks.
    #[must_use]
    pub fn new(filter: Filter) -> Self {
        Self {
            filter,
            sink: String::new(),
            error_observed: false,
            repeats: RepetitionFolder::default(),
            buffered: Vec::new(),
            ignored: BTreeSet::new(),
            capturing: None,
        }
    }

    /// The current filter masks.
    #[must_use]
    pub fn filter(&self) -> Filter {
        self.filter
    }

    /// Replace the filter masks. The reference logs the change first
    /// (`limitDebugLevel`/`limitDebugClass`, `debug.cpp:582-592`).
    pub fn set_filter(&mut self, filter: Filter) {
        self.filter = filter;
    }

    /// Everything written so far.
    #[must_use]
    pub fn sink(&self) -> &str {
        &self.sink
    }

    /// Everything written so far, leaving the sink empty.
    pub fn take_sink(&mut self) -> String {
        core::mem::take(&mut self.sink)
    }

    /// True when any error has been logged in this run
    /// (`debug_has_error_been_observed`, `debug.cpp:206-209`).
    ///
    /// The reference's test suite fails a run on this flag.
    #[must_use]
    pub fn error_observed(&self) -> bool {
        self.error_observed
    }

    /// `DebugLog`: write one message if the filter admits it.
    ///
    /// Returns whether the message was written. An error level sets
    /// [`Diagnostics::error_observed`] whether or not the message is written —
    /// in practice errors always pass the filter.
    pub fn log(&mut self, level: DebugLevel, class: DebugClass, time: Time, text: &str) -> bool {
        if level.intersects(DebugLevel::ERROR) {
            self.error_observed = true;
        }
        if !self.filter.passes(level, class) {
            return false;
        }
        // `output_repetitions` is written before the first line of the new
        // message (`debug.cpp:1515`).
        self.flush_repetitions();
        self.sink.push('\n');
        self.sink.push_str(&time.to_string());
        self.sink.push(' ');
        for name in level.names() {
            self.sink.push_str(name);
            self.sink.push(' ');
        }
        // The class is only named when it differs from the class filter mask.
        if class != self.filter.class {
            for name in class.names() {
                self.sink.push_str(name);
                self.sink.push(' ');
            }
        }
        self.sink.push_str(": ");
        self.sink.push_str(text);
        true
    }
}

/// Where a debug message came from: `realDebugmsg`'s `filename`, `line` and
/// `funcname` arguments, filled in by the `debugmsg` macro (`debug.h:73-77`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Site {
    /// `__FILE__`.
    pub file: &'static str,
    /// `STRING(__LINE__)`.
    pub line: u32,
    /// `CATA_FUNCTION_NAME`.
    pub function: &'static str,
}

impl Site {
    /// A call site.
    #[must_use]
    pub fn new(file: &'static str, line: u32, function: &'static str) -> Self {
        Self {
            file,
            line,
            function,
        }
    }
}

/// What the caller can offer the popup: whether this is a test run and whether
/// a screen exists at all.
///
/// The reference reads the globals `test_mode` and `catacurses::stdscr`
/// (`debug.cpp:532-562`); here they are part of the call.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UiState {
    /// `test_mode`: the unit test suite is running, so no prompt is shown.
    pub test_mode: bool,
    /// Whether `catacurses::stdscr` exists, i.e. a prompt can be drawn.
    pub screen_available: bool,
}

impl Default for UiState {
    /// An ordinary interactive run: screen available, not in test mode.
    fn default() -> Self {
        Self {
            test_mode: false,
            screen_available: true,
        }
    }
}

/// What happened to the popup a debug message would show.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PromptDisposition {
    /// Shown immediately (`debug_error_prompt`).
    Shown,
    /// Held for `replay_buffered_debugmsg_prompts` because no screen exists yet.
    Buffered,
    /// Not shown: test mode, or the user asked to ignore this message.
    NotShown,
}

/// A popup queued while no screen was available (`buffered_prompt_info`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BufferedPrompt {
    /// Where the message came from.
    pub site: Site,
    /// The message text.
    pub text: String,
    /// Forced prompts bypass the ignored-message list.
    pub forced: bool,
}

/// The effects of one `debugmsg` call.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DebugOutcome {
    /// Whether a log line was written.
    pub logged: bool,
    /// Whether the caller must end the run, as `cata_fatal` requires.
    pub abort: bool,
    /// What became of the popup.
    pub prompt: PromptDisposition,
    /// True at the repetition threshold, which triggers an extra forced prompt.
    pub excessive_repetition: bool,
}

/// Repeats closer together than this are folded into one log line.
const REPETITION_TIMEOUT_MS: i32 = 100;
/// At this repeat count the reference adds an "excessive repetition" prompt.
const EXCESSIVE_REPETITION_THRESHOLD: u32 = 10000;

/// `repetition_folder` (`debug.cpp:448-507`): folds identical messages that
/// arrive within [`REPETITION_TIMEOUT_MS`] of each other.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct RepetitionFolder {
    site: Option<Site>,
    text: String,
    time: Time,
    repeat_count: u32,
}

impl RepetitionFolder {
    /// Milliseconds since midnight, as the reference's `*_raw` arithmetic does.
    fn raw_ms(time: Time) -> i32 {
        i32::from(time.hours) * 3_600_000
            + i32::from(time.minutes) * 60_000
            + i32::from(time.seconds) * 1000
            + i32::from(time.mseconds)
    }

    fn matches(&self, site: Site, text: &str, now: Time) -> bool {
        self.site == Some(site)
            && self.text == text
            && Self::raw_ms(now) - Self::raw_ms(self.time) <= REPETITION_TIMEOUT_MS
    }

    fn set(&mut self, site: Site, text: &str, now: Time) {
        self.site = Some(site);
        text.clone_into(&mut self.text);
        self.time = now;
        self.repeat_count = 0;
    }

    fn increment(&mut self, now: Time) {
        self.repeat_count += 1;
        self.time = now;
    }
}

impl Diagnostics {
    /// `realDebugmsg` (`debug.cpp:512-577`): log the message, mark the run as
    /// failed, and answer for the popup.
    ///
    /// Capturing (the test-suite helper) bypasses all three effects.
    pub fn debugmsg(&mut self, site: Site, ui: UiState, time: Time, text: &str) -> DebugOutcome {
        if let Some(captured) = self.capturing.as_mut() {
            captured.push_str(text);
            return DebugOutcome {
                logged: false,
                abort: false,
                prompt: PromptDisposition::NotShown,
                excessive_repetition: false,
            };
        }

        let logged = if self.repeats.matches(site, text, time) {
            self.repeats.increment(time);
            false
        } else {
            let line = format!("{}:{} [{}] {}", site.file, site.line, site.function, text);
            // The reference logs first and records the message afterwards, so a
            // pending repeat summary is still flushed by this very call.
            let written = self.log(DebugLevel::ERROR, DebugClass::MAIN, time, &line);
            self.repeats.set(site, text, time);
            written
        };

        let excessive_repetition = self.repeats.repeat_count == EXCESSIVE_REPETITION_THRESHOLD;

        if ui.test_mode {
            return DebugOutcome {
                logged,
                abort: false,
                prompt: PromptDisposition::NotShown,
                excessive_repetition,
            };
        }

        let ignored = self.ignored.contains(&(site.file, site.line));
        let prompt = if !ui.screen_available {
            self.buffered.push(BufferedPrompt {
                site,
                text: text.to_owned(),
                forced: false,
            });
            if excessive_repetition {
                self.buffered.push(BufferedPrompt {
                    site,
                    text: excessive_repetition_message(text),
                    forced: true,
                });
            }
            PromptDisposition::Buffered
        } else if ignored && !excessive_repetition {
            PromptDisposition::NotShown
        } else {
            PromptDisposition::Shown
        };

        DebugOutcome {
            logged,
            abort: false,
            prompt,
            excessive_repetition,
        }
    }

    /// `cata_fatal` (`debug.h:87-91`): report a debug message and then abort the
    /// program — in every build, unlike an assertion.
    ///
    /// The library reports; ending the run is [`DebugOutcome::abort`]'s caller.
    pub fn cata_fatal(&mut self, site: Site, ui: UiState, time: Time, text: &str) -> DebugOutcome {
        let mut outcome = self.debugmsg(site, ui, time, text);
        outcome.abort = true;
        outcome
    }

    /// `output_repetitions` (`debug.cpp:1485-1502`): write the summary of the
    /// repeated message that the preceding log line stood for, then forget it.
    fn flush_repetitions(&mut self) {
        if self.repeats.repeat_count == 0 {
            return;
        }
        let RepetitionFolder {
            site,
            text,
            time,
            repeat_count,
        } = core::mem::take(&mut self.repeats);
        self.sink.push('\n');
        if repeat_count > 1 {
            let note = format!("[ Previous repeated {} times ]\n", repeat_count - 1);
            self.sink.push_str(&note);
        }
        self.sink.push_str(&time.to_string());
        self.sink.push_str(" ERROR : ");
        if let Some(site) = site {
            self.sink.push_str(site.file);
            self.sink.push(':');
            self.sink.push_str(&site.line.to_string());
            self.sink.push_str(" [");
            self.sink.push_str(site.function);
            self.sink.push_str("] ");
        }
        self.sink.push_str(&text);
    }

    /// Prompts waiting for a screen (`buffered_prompts`).
    #[must_use]
    pub fn buffered(&self) -> &[BufferedPrompt] {
        &self.buffered
    }

    /// Add a message to the ignore list, as pressing `I` does
    /// (`debug.cpp:395-398`). Keyed on file and line.
    pub fn ignore_site(&mut self, site: Site) {
        self.ignored.insert((site.file, site.line));
    }

    /// `capture_debugmsg_during` (`debug.cpp:182-204`): collect the messages
    /// raised by `func` instead of logging or prompting them.
    pub fn capture_debugmsg_during<F>(&mut self, func: F) -> String
    where
        F: FnOnce(&mut Self),
    {
        self.capturing = Some(String::new());
        func(self);
        let captured = self.capturing.take();
        captured.unwrap_or_default()
    }
}

/// The text the reference prepends when a message repeats too often
/// (`debug.cpp:556-559`).
fn excessive_repetition_message(text: &str) -> String {
    format!(
        "Excessive error repetition detected.  Please file a bug report at \
         https://github.com/CleverRaven/Cataclysm-DDA/issues\n            {text}"
    )
}

/// Which build the assertion policy belongs to: whether `NDEBUG` is defined.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum BuildProfile {
    /// No `NDEBUG`: assertions are evaluated and a failure aborts.
    #[default]
    Debug,
    /// `NDEBUG`: `cata_assert` expands to an unevaluated expression.
    Release,
}

/// What an assertion did.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AssertOutcome {
    /// The condition held.
    Passed,
    /// The condition failed; the reference reports this text and aborts.
    Aborted {
        /// The message `cata_assert` writes to stderr (`cata_assert.h:26-33`).
        report: String,
    },
    /// The condition was never evaluated, as in a release build.
    NotEvaluated,
}

/// `cata_assert` as a value (`src/cata_assert.h`).
///
/// The condition is passed as a closure precisely so that a release build can
/// decline to call it: there `cata_assert(exp)` expands to
/// `decltype((exp) ? void() : __builtin_unreachable())()`, which is unevaluated.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AssertPolicy {
    profile: BuildProfile,
}

impl AssertPolicy {
    /// The policy of a build without `NDEBUG`.
    #[must_use]
    pub fn debug() -> Self {
        Self {
            profile: BuildProfile::Debug,
        }
    }

    /// The policy of a release build: assertions are compiled out.
    #[must_use]
    pub fn release() -> Self {
        Self {
            profile: BuildProfile::Release,
        }
    }

    /// The profile this policy was built for.
    #[must_use]
    pub fn profile(&self) -> BuildProfile {
        self.profile
    }

    /// Evaluate an assertion. In a release build `condition` is not called.
    pub fn check<F>(&self, expression: &str, site: Site, condition: F) -> AssertOutcome
    where
        F: FnOnce() -> bool,
    {
        if self.profile == BuildProfile::Release {
            return AssertOutcome::NotEvaluated;
        }
        if condition() {
            return AssertOutcome::Passed;
        }
        AssertOutcome::Aborted {
            report: format!(
                "{} at {}:{}: Assertion `{}` failed.\n",
                site.function, site.file, site.line, expression
            ),
        }
    }
}
