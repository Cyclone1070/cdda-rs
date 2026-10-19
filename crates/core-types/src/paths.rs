//! Logical path roots and platform rules.
//!
//! Reference: `src/path_info.h`, `src/path_info.cpp`, plus `cata_path` from
//! `src/cata_path.h` and `as_norm_dir` from `src/filesystem.cpp:133-142`.
//!
//! Everything the reference reads from outside the program is an input here:
//! the platform, the build options and the environment variables
//! ([`PathConfig`]). Nothing touches the filesystem, so the rules are testable
//! on one host.
//!
//! Not modelled, because they need a real filesystem, a locale or a platform
//! API: `find_translated_file` (the localized `credits`/`motd`/`title`/`names`
//! accessors, which call `file_exist` and read the `USE_LANG` option), the
//! `PREFIX`/Android base paths, and Windows native separator handling — the
//! path strings here are always in the generic `/` form.

/// A logical root a relative path is resolved against.
///
/// Mirrors `cata_path::root_path` (`src/cata_path.h:40-49`). The default is
/// `Unknown`, documented there as "assumed relative to cwd, path is '.'".
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum RootPath {
    /// Where the game's own files live (`PREFIX`, or empty for a local build).
    Base,
    /// Configuration directory: `user/config/` or `$XDG_CONFIG_HOME/cataclysm-dda/`.
    Config,
    /// Game data directory: `base/data/` or `base/share/cataclysm-dda/`.
    Data,
    /// Memorial directory, always under the user directory.
    Memorial,
    /// Save directory, its own root even though it lives in the user directory.
    Save,
    /// Per-user writable directory.
    User,
    /// No root: the relative path is used as-is, i.e. relative to the cwd.
    #[default]
    Unknown,
}

/// The string value of every logical root (`PATH_INFO`'s `*_value` statics).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LogicalRoots {
    base: String,
    config: String,
    data: String,
    memorial: String,
    save: String,
    user: String,
}

impl LogicalRoots {
    /// All roots empty, which is what the reference's statics start as.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the value a logical root resolves to.
    pub fn set(&mut self, root: RootPath, value: &str) {
        match root {
            RootPath::Base => value.clone_into(&mut self.base),
            RootPath::Config => value.clone_into(&mut self.config),
            RootPath::Data => value.clone_into(&mut self.data),
            RootPath::Memorial => value.clone_into(&mut self.memorial),
            RootPath::Save => value.clone_into(&mut self.save),
            RootPath::User => value.clone_into(&mut self.user),
            // `unknown` has no value: `get_logical_root_path` returns an empty
            // string for it (`path_info.cpp:610-614`).
            RootPath::Unknown => {}
        }
    }

    /// The value a logical root resolves to, empty for `Unknown`.
    #[must_use]
    pub fn value(&self, root: RootPath) -> &str {
        match root {
            RootPath::Base => &self.base,
            RootPath::Config => &self.config,
            RootPath::Data => &self.data,
            RootPath::Memorial => &self.memorial,
            RootPath::Save => &self.save,
            RootPath::User => &self.user,
            RootPath::Unknown => "",
        }
    }
}

/// A logical root paired with a path relative to it.
///
/// Mirrors `cata_path` (`src/cata_path.h:34-163`): the relative path is kept
/// alone, so callers can inspect it before the root is prepended.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CataPath {
    root: RootPath,
    relative: String,
}

impl CataPath {
    /// Pair a logical root with a relative path.
    #[must_use]
    pub fn new(root: RootPath, relative: &str) -> Self {
        Self {
            root,
            relative: relative.to_owned(),
        }
    }

    /// The logical root this path is relative to.
    #[must_use]
    pub fn root(&self) -> RootPath {
        self.root
    }

    /// The path component, without the root.
    #[must_use]
    pub fn relative(&self) -> &str {
        &self.relative
    }

    /// Join a segment onto the relative path, keeping the root (`operator/`).
    #[must_use]
    pub fn push(&self, segment: &str) -> Self {
        Self {
            root: self.root,
            relative: join(&self.relative, segment),
        }
    }

    /// Concatenate a segment onto the relative path, keeping the root (`operator+`).
    ///
    /// Plain string concatenation: no separator is inserted (`cata_path.h:105-116`).
    #[must_use]
    pub fn append(&self, segment: &str) -> Self {
        Self {
            root: self.root,
            relative: format!("{}{}", self.relative, segment),
        }
    }

    /// Normalise the relative path, keeping the root (`lexically_normal`).
    #[must_use]
    pub fn lexically_normal(&self) -> Self {
        Self {
            root: self.root,
            relative: normalize(&self.relative),
        }
    }

    /// Resolve to a single string: the root's value joined with the relative path.
    ///
    /// `get_unrelative_path` is `root_value / relative_path`, i.e.
    /// `std::filesystem::path::operator/` (`src/cata_path.h:66-70`): a separator
    /// is inserted only when the root does not already end in one.
    #[must_use]
    pub fn resolve(&self, roots: &LogicalRoots) -> String {
        join(roots.value(self.root), &self.relative)
    }
}

/// `std::filesystem::path{ lhs } / rhs`.
///
/// An absolute `rhs` replaces `lhs` outright; otherwise a separator is inserted
/// between them unless `lhs` is empty or already ends in one. Note that a
/// non-empty `lhs` gains a separator even when `rhs` is empty.
fn join(lhs: &str, rhs: &str) -> String {
    if rhs.starts_with('/') {
        return rhs.to_owned();
    }
    let mut out = String::with_capacity(lhs.len() + rhs.len() + 1);
    out.push_str(lhs);
    if !lhs.is_empty() && !lhs.ends_with('/') {
        out.push('/');
    }
    out.push_str(rhs);
    out
}

/// `std::filesystem::path::lexically_normal`.
///
/// Dot elements are dropped, dot-dot elements remove the preceding ordinary
/// element (or are discarded at an absolute root, or kept at the front of a
/// relative path), repeated separators collapse, and an empty result becomes
/// `.` — unless the input was empty, which stays empty. A trailing separator is
/// kept when the input ended in one or in a dot element, except when the result
/// ends in `..`.
#[must_use]
pub fn normalize(input: &str) -> String {
    if input.is_empty() {
        return String::new();
    }
    let absolute = input.starts_with('/');
    let mut segments: Vec<&str> = Vec::new();
    let mut last_raw = "";
    for raw in input.split('/') {
        if raw.is_empty() {
            continue;
        }
        last_raw = raw;
        match raw {
            "." => {}
            ".." => {
                if segments.last().is_some_and(|last| *last != "..") {
                    segments.pop();
                } else if !absolute {
                    segments.push(raw);
                }
            }
            other => segments.push(other),
        }
    }
    if segments.is_empty() {
        return if absolute {
            "/".to_owned()
        } else {
            ".".to_owned()
        };
    }
    let keep_separator =
        (input.ends_with('/') || matches!(last_raw, "." | "..")) && segments.last() != Some(&"..");
    let mut out = String::with_capacity(input.len() + 1);
    if absolute {
        out.push('/');
    }
    for (index, segment) in segments.iter().enumerate() {
        if index > 0 {
            out.push('/');
        }
        out.push_str(segment);
    }
    if keep_separator {
        out.push('/');
    }
    out
}

/// `as_norm_dir` (`src/filesystem.cpp:133-142`): normalise as a directory, so
/// the result ends in a separator and never in `.`.
#[must_use]
pub fn as_norm_dir(path: &str) -> String {
    let normalized = normalize(&join(path, ""));
    if normalized == "." {
        "./".to_owned()
    } else {
        normalized
    }
}

/// The operating system the reference was built for.
///
/// These are compile-time defines there (`_WIN32`, `MACOSX`); modelling them as
/// an enum keeps every branch testable on one host.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Platform {
    /// No `_WIN32` and no `MACOSX`.
    #[default]
    Linux,
    /// `MACOSX` defined.
    MacOs,
    /// `_WIN32` defined.
    Windows,
}

/// Build-time options that move the default directories.
///
/// `USE_XDG_DIR` and `USE_HOME_DIR` are mutually exclusive: the reference's
/// build fails when both are set (`Makefile:1044-1056`, `CMakeLists.txt:159-161`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BuildOptions {
    /// `USE_XDG_DIR`: use `XDG_DATA_HOME`/`XDG_CONFIG_HOME`.
    pub use_xdg_dir: bool,
    /// `USE_HOME_DIR`: use `$HOME` instead of the current directory.
    pub use_home_dir: bool,
    /// `DATA_DIR_PREFIX`: data lives in `share/cataclysm-dda/` under `PREFIX`.
    pub data_dir_prefix: bool,
}

/// The process environment the reference reads.
///
/// Each field is one `getenv` call in `path_info.cpp`: `HOME`,
/// `XDG_DATA_HOME`, `XDG_CONFIG_HOME` and `LOCALAPPDATA`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Environment {
    /// `HOME`.
    pub home: Option<String>,
    /// `XDG_DATA_HOME`, only consulted under `USE_XDG_DIR`.
    pub xdg_data_home: Option<String>,
    /// `XDG_CONFIG_HOME`, only consulted under `USE_XDG_DIR`.
    pub xdg_config_home: Option<String>,
    /// `LOCALAPPDATA`, only consulted on Windows.
    pub localappdata: Option<String>,
}

/// Everything outside the program that the path rules depend on.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PathConfig {
    /// Platform the build targets.
    pub platform: Platform,
    /// Build options.
    pub options: BuildOptions,
    /// Environment variables.
    pub env: Environment,
}

/// Why a path could not be derived.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InitError {
    /// A required environment variable is unset. The reference calls
    /// `cata_fatal`, which logs and aborts (`path_info.cpp:66-73`).
    MissingEnv(&'static str),
}

/// The derived directories and files, mirroring the `PATH_INFO` statics.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PathInfo {
    roots: LogicalRoots,
    strings: StandardStrings,
    paths: StandardPaths,
}

impl PathInfo {
    /// Nothing initialised, so every root is empty.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The current root values.
    #[must_use]
    pub fn roots(&self) -> &LogicalRoots {
        &self.roots
    }

    /// `init_base_path` (`path_info.cpp:75-79`): normalise the base directory,
    /// so it always ends in a separator.
    pub fn init_base_path(&mut self, path: &str) {
        self.roots.set(RootPath::Base, &as_norm_dir(path));
    }

    /// The user directory value (`PATH_INFO::user_dir()`).
    #[must_use]
    pub fn user_dir(&self) -> &str {
        self.roots.value(RootPath::User)
    }

    /// Choose and apply the default user directory the way `main.cpp:675-683`
    /// does: an empty directory (platform default) when `USE_HOME_DIR` or
    /// `USE_XDG_DIR` is set, `"."` otherwise.
    ///
    /// Setting both options is a build error in the reference
    /// (`Makefile:1044-1056`); if a caller does it anyway, the XDG branch wins
    /// because it is tested first.
    ///
    /// # Errors
    ///
    /// Returns [`InitError::MissingEnv`] when the platform default needs an
    /// environment variable that is unset.
    pub fn init_user_dir(&mut self, config: &PathConfig) -> Result<(), InitError> {
        if config.options.use_home_dir || config.options.use_xdg_dir {
            self.init_user_dir_from_platform(config)
        } else {
            self.init_user_dir_explicit(".");
            Ok(())
        }
    }

    /// `init_user_dir("")`: fill in the platform default (`path_info.cpp:81-107`).
    ///
    /// # Errors
    ///
    /// Returns [`InitError::MissingEnv`] when a required environment variable
    /// is unset: `LOCALAPPDATA` on Windows, `HOME` on macOS and on Linux
    /// without `XDG_DATA_HOME`.
    pub fn init_user_dir_from_platform(&mut self, config: &PathConfig) -> Result<(), InitError> {
        let dir = match config.platform {
            Platform::Windows => {
                let local = config
                    .env
                    .localappdata
                    .as_deref()
                    .ok_or(InitError::MissingEnv("LOCALAPPDATA"))?;
                format!("{local}/cataclysm-dda/")
            }
            // The macOS branch precedes the XDG one in the preprocessor chain, so
            // a mac build ignores USE_XDG_DIR here.
            Platform::MacOs => {
                let home = config
                    .env
                    .home
                    .as_deref()
                    .ok_or(InitError::MissingEnv("HOME"))?;
                format!("{home}/Library/Application Support/Cataclysm/")
            }
            Platform::Linux if config.options.use_xdg_dir => {
                if let Some(data) = config.env.xdg_data_home.as_deref() {
                    format!("{data}/cataclysm-dda/")
                } else {
                    let home = config
                        .env
                        .home
                        .as_deref()
                        .ok_or(InitError::MissingEnv("HOME"))?;
                    format!("{home}/.local/share/cataclysm-dda/")
                }
            }
            Platform::Linux => {
                let home = config
                    .env
                    .home
                    .as_deref()
                    .ok_or(InitError::MissingEnv("HOME"))?;
                format!("{home}/.cataclysm-dda/")
            }
        };
        self.init_user_dir_explicit(&dir);
        Ok(())
    }

    /// `init_user_dir(dir)` with an explicit directory: normalised, no lookups.
    pub fn init_user_dir_explicit(&mut self, dir: &str) {
        self.roots.set(RootPath::User, &as_norm_dir(dir));
    }
}

/// The string form of every derived standard location (`PATH_INFO`'s
/// `*_value` statics, `path_info.cpp:35-48`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct StandardStrings {
    /// `datadir()`: where game data lives.
    pub datadir: String,
    /// `gfxdir()`.
    pub gfxdir: String,
    /// `langdir()`: compiled translations.
    pub langdir: String,
    /// `motd()`: the realised message-of-the-day path, not the translated one.
    pub motd: String,
    /// `savedir()`.
    pub savedir: String,
    /// `memorialdir()`.
    pub memorialdir: String,
    /// `achievementdir()`.
    pub achievementdir: String,
    /// `config_dir()`.
    pub config_dir: String,
    /// `options()` as a string: `config_dir + "options.json"`.
    pub options: String,
    /// `keymap()`: `config_dir + "keymap.txt"`.
    pub keymap: String,
    /// `autopickup()` as a string: `config_dir + "auto_pickup.json"`.
    pub autopickup: String,
    /// `autonote` as a string: `config_dir + "auto_note.json"`.
    pub autonote: String,
}

impl StandardStrings {
    /// `cache_dir()`: derived on demand, like the reference accessor.
    #[must_use]
    pub fn cache_dir(&self) -> String {
        format!("{}cache/", self.datadir)
    }
}

/// The `cata_path` form of every derived standard location that the reference
/// keeps as a path (`path_info.cpp:50-63`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct StandardPaths {
    /// `datadir_path()`.
    pub datadir: CataPath,
    /// `gfxdir()`.
    pub gfxdir: CataPath,
    /// `langdir_path()`.
    pub langdir: CataPath,
    /// `savedir_path()`.
    pub savedir: CataPath,
    /// `memorialdir_path()`.
    pub memorialdir: CataPath,
    /// `achievementdir_path()`.
    pub achievementdir: CataPath,
    /// `config_dir_path()`.
    pub config_dir: CataPath,
    /// `options()`.
    pub options: CataPath,
    /// `autopickup()`.
    pub autopickup: CataPath,
    /// `autonote()`.
    pub autonote: CataPath,
}

impl PathInfo {
    /// The derived string locations, empty until [`PathInfo::set_standard_filenames`].
    #[must_use]
    pub fn strings(&self) -> &StandardStrings {
        &self.strings
    }

    /// The derived path locations, empty until [`PathInfo::set_standard_filenames`].
    #[must_use]
    pub fn paths(&self) -> &StandardPaths {
        &self.paths
    }

    /// Resolve a derived path against the current roots.
    #[must_use]
    pub fn resolve(&self, path: &CataPath) -> String {
        path.resolve(&self.roots)
    }

    /// `set_standard_filenames` (`path_info.cpp:109-174`).
    ///
    /// Pure derivation from the base and user directories plus the build
    /// options: no filesystem access, and the only environment reads are the
    /// `USE_XDG_DIR` config-directory lookup.
    ///
    /// # Errors
    ///
    /// Returns [`InitError::MissingEnv`] when `USE_XDG_DIR` is set and neither
    /// `XDG_CONFIG_HOME` nor `HOME` is available.
    pub fn set_standard_filenames(&mut self, config: &PathConfig) -> Result<(), InitError> {
        let base = self.roots.value(RootPath::Base).to_owned();

        // Data is always relative to itself; with DATA_DIR_PREFIX it also moves
        // the gfx and lang prefixes, and without a base path the prefix stays
        // empty even though the data directory is `data/`.
        let (datadir, prefix, prefix_root) = if base.is_empty() {
            ("data/".to_owned(), String::new(), RootPath::Base)
        } else if config.options.data_dir_prefix {
            let datadir = format!("{base}share/cataclysm-dda/");
            (datadir.clone(), datadir, RootPath::Data)
        } else {
            (format!("{base}data/"), base.clone(), RootPath::Base)
        };
        self.roots.set(RootPath::Data, &datadir);
        let datadir_path = CataPath::new(RootPath::Data, "");

        let gfxdir = format!("{prefix}gfx/");
        let langdir = format!("{prefix}lang/mo/");
        let motd = format!("{datadir}motd/en.motd");

        let user = self.roots.value(RootPath::User).to_owned();
        let savedir = format!("{user}save/");
        // `savedir` is its own root even though it sits in the user directory.
        self.roots.set(RootPath::Save, &savedir);
        let memorialdir = format!("{user}memorial/");
        self.roots.set(RootPath::Memorial, &memorialdir);
        let achievementdir = format!("{user}achievements/");

        let (config_dir, config_path) = if config.options.use_xdg_dir {
            let dir = if let Some(dir) = config.env.xdg_config_home.as_deref() {
                format!("{dir}/cataclysm-dda/")
            } else {
                let home = config
                    .env
                    .home
                    .as_deref()
                    .ok_or(InitError::MissingEnv("HOME"))?;
                format!("{home}/.config/cataclysm-dda/")
            };
            self.roots.set(RootPath::Config, &dir);
            (dir, CataPath::new(RootPath::Config, ""))
        } else {
            (
                format!("{user}config/"),
                CataPath::new(RootPath::User, "config"),
            )
        };

        self.strings = StandardStrings {
            datadir,
            gfxdir,
            langdir,
            motd,
            savedir,
            memorialdir,
            achievementdir,
            options: format!("{config_dir}options.json"),
            keymap: format!("{config_dir}keymap.txt"),
            autopickup: format!("{config_dir}auto_pickup.json"),
            autonote: format!("{config_dir}auto_note.json"),
            config_dir,
        };
        self.paths = StandardPaths {
            datadir: datadir_path,
            gfxdir: CataPath::new(prefix_root, "gfx"),
            langdir: CataPath::new(prefix_root, "lang/mo"),
            savedir: CataPath::new(RootPath::Save, ""),
            memorialdir: CataPath::new(RootPath::User, "memorial"),
            achievementdir: CataPath::new(RootPath::User, "achievements"),
            options: config_path.push("options.json"),
            autopickup: config_path.push("auto_pickup.json"),
            autonote: config_path.push("auto_note.json"),
            config_dir: config_path,
        };
        Ok(())
    }
}

impl PathInfo {
    /// `set_datadir` (`path_info.cpp:534-545`): move the data root and the two
    /// locations that hang off it directly. The value is stored verbatim, and
    /// the language directory is *not* updated.
    pub fn set_datadir(&mut self, datadir: &str) {
        datadir.clone_into(&mut self.strings.datadir);
        self.roots.set(RootPath::Data, datadir);
        self.paths.datadir = CataPath::new(RootPath::Data, "");
        self.strings.gfxdir = format!("{datadir}gfx/");
        self.paths.gfxdir = CataPath::new(RootPath::Data, "gfx");
        self.strings.motd = format!("{datadir}motd/en.motd");
    }

    /// `set_config_dir` (`path_info.cpp:547-557`): make the config directory its
    /// own logical root, even for a build that does not use XDG directories.
    /// `autonote` is left untouched, as in the reference.
    pub fn set_config_dir(&mut self, config_dir: &str) {
        config_dir.clone_into(&mut self.strings.config_dir);
        self.roots.set(RootPath::Config, config_dir);
        let root = CataPath::new(RootPath::Config, "");
        self.strings.options = format!("{config_dir}options.json");
        self.paths.options = root.push("options.json");
        self.strings.keymap = format!("{config_dir}keymap.txt");
        self.strings.autopickup = format!("{config_dir}auto_pickup.json");
        self.paths.autopickup = root.push("auto_pickup.json");
        self.paths.config_dir = root;
    }

    /// `set_savedir` (`path_info.cpp:559-563`): the save directory is its own root.
    pub fn set_savedir(&mut self, savedir: &str) {
        savedir.clone_into(&mut self.strings.savedir);
        self.roots.set(RootPath::Save, savedir);
        self.paths.savedir = CataPath::new(RootPath::Save, "");
    }

    /// `set_memorialdir` (`path_info.cpp:565-569`): the memorial directory
    /// becomes its own root instead of a user subdirectory.
    pub fn set_memorialdir(&mut self, memorialdir: &str) {
        memorialdir.clone_into(&mut self.strings.memorialdir);
        self.roots.set(RootPath::Memorial, memorialdir);
        self.paths.memorialdir = CataPath::new(RootPath::Memorial, "");
    }

    /// `set_options` (`path_info.cpp:571-575`): the path is `unknown`-rooted, so
    /// the given string is used as-is relative to the working directory.
    pub fn set_options(&mut self, options: &str) {
        options.clone_into(&mut self.strings.options);
        self.paths.options = CataPath::new(RootPath::Unknown, options);
    }

    /// `set_autopickup` (`path_info.cpp:582-586`): `unknown`-rooted, like
    /// [`PathInfo::set_options`].
    pub fn set_autopickup(&mut self, autopickup: &str) {
        autopickup.clone_into(&mut self.strings.autopickup);
        self.paths.autopickup = CataPath::new(RootPath::Unknown, autopickup);
    }

    /// `set_keymap` (`path_info.cpp:577-580`): only the string form exists.
    pub fn set_keymap(&mut self, keymap: &str) {
        keymap.clone_into(&mut self.strings.keymap);
    }

    /// `set_motd` (`path_info.cpp:588-591`): only the string form exists.
    pub fn set_motd(&mut self, motd: &str) {
        motd.clone_into(&mut self.strings.motd);
    }
}
