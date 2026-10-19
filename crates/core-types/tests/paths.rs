//! Logical path roots and platform rules, ported test-first.
//!
//! Specification is the reference's `src/cata_path.h` (the `root_path` enum and
//! `get_unrelative_path`), `src/path_info.cpp` (`set_standard_filenames` and the
//! `PATH_INFO::` accessors) and `src/filesystem.cpp:133-142` (`as_norm_dir`).
//!
//! The reference reads the process environment (`HOME`, `XDG_DATA_HOME`,
//! `XDG_CONFIG_HOME`, `LOCALAPPDATA`), consults compile-time defines
//! (`USE_XDG_DIR`, `USE_HOME_DIR`, `DATA_DIR_PREFIX`, `MACOSX`, `_WIN32`) and
//! touches the filesystem (`file_exist` in `find_translated_file`). All three are
//! modelled here as plain inputs so the rules are testable without either.

use cdda_core_types::paths::{
    BuildOptions, CataPath, Environment, InitError, LogicalRoots, PathConfig, PathInfo, Platform,
    RootPath, as_norm_dir, normalize,
};

/// Root values as `set_standard_filenames` would compute them for a POSIX build
/// with `base_path_value == "opt/cdda/"` (`path_info.cpp:118-166`).
fn roots() -> LogicalRoots {
    let mut roots = LogicalRoots::new();
    roots.set(RootPath::Base, "opt/cdda/");
    roots.set(RootPath::Data, "opt/cdda/data/");
    roots.set(RootPath::User, "opt/cdda/user/");
    roots.set(RootPath::Save, "opt/cdda/user/save/");
    roots.set(RootPath::Memorial, "opt/cdda/user/memorial/");
    roots.set(RootPath::Config, "opt/cdda/user/config/");
    roots
}

/// `cata_path::get_unrelative_path()` is `root_string / relative_path` with
/// `std::filesystem::path::operator/` semantics, and a path whose root is
/// `unknown` has an empty root value, so it resolves relative to the current
/// working directory (`cata_path.h:40-49,66-70`, `path_info.cpp:593-616`).
#[test]
fn resolves_relative_path_against_each_logical_root() {
    let roots = roots();

    // Every distinct root prepends its own value: (root, relative, expected).
    let table = [
        (RootPath::Base, "gfx", "opt/cdda/gfx"),
        (
            RootPath::Data,
            "core/colors.json",
            "opt/cdda/data/core/colors.json",
        ),
        (RootPath::User, "templates", "opt/cdda/user/templates"),
        (RootPath::Save, "world1", "opt/cdda/user/save/world1"),
        (
            RootPath::Memorial,
            "memorial.txt",
            "opt/cdda/user/memorial/memorial.txt",
        ),
        (
            RootPath::Config,
            "options.json",
            "opt/cdda/user/config/options.json",
        ),
        // `unknown` is documented as "assumed relative to cwd, path is '.'".
        (RootPath::Unknown, "options.json", "options.json"),
        (RootPath::Unknown, "save/world1", "save/world1"),
    ];
    for (root, relative, expected) in table {
        let path = CataPath::new(root, relative);
        assert_eq!(path.root(), root, "root of {relative}");
        assert_eq!(path.relative(), relative, "relative of {relative}");
        assert_eq!(path.resolve(&roots), expected, "{root:?} + {relative}");
    }
}

/// The empty relative path still resolves to the root itself, and
/// `path / ""` appends a separator when the root value does not end in one —
/// this is the trailing-separator behaviour that string concatenation gets
/// wrong (`path_info.cpp:116`, `filesystem.cpp:133-142`).
#[test]
fn empty_relative_path_resolves_to_the_root_itself() {
    let mut roots = LogicalRoots::new();
    roots.set(RootPath::Base, "opt/cdda");

    // Root value without a trailing separator gains one.
    let base = CataPath::new(RootPath::Base, "");
    assert_eq!(base.resolve(&roots), "opt/cdda/");
    // A bare separator string is not absolute here: joining keeps the root.
    let gfx = base.push("gfx");
    assert_eq!(gfx.resolve(&roots), "opt/cdda/gfx");

    // An empty root value (unknown) contributes nothing at all.
    roots.set(RootPath::Unknown, "");
    assert_eq!(CataPath::new(RootPath::Unknown, "").resolve(&roots), "");
    assert_eq!(
        CataPath::new(RootPath::Unknown, "gfx").resolve(&roots),
        "gfx"
    );
}

/// `cata_path::operator/` and `operator+` are different operations
/// (`src/cata_path.h:93-116`): `/` is `std::filesystem::path::operator/`, which
/// replaces the left side when the right side is absolute, while `+` is raw
/// string concatenation.
#[test]
fn join_replaces_on_absolute_segments_and_append_only_concatenates() {
    let path = CataPath::new(RootPath::Data, "raw");

    // Absolute component replaces the whole relative path, it is not appended.
    assert_eq!(path.push("/etc/cdda").relative(), "/etc/cdda");
    // A path made only of a separator is absolute too.
    assert_eq!(path.push("/").relative(), "/");
    // Relative components are appended with exactly one separator.
    assert_eq!(path.push("colors.json").relative(), "raw/colors.json");
    // Pushing an empty component appends a separator and nothing else.
    assert_eq!(path.push("").relative(), "raw/");
    // The logical root is never changed by joining.
    assert_eq!(path.push("colors.json").root(), RootPath::Data);

    // `operator+=` concatenates: no separator is inserted.
    assert_eq!(path.append("colors.json").relative(), "rawcolors.json");
    assert_eq!(path.append("/colors.json").relative(), "raw/colors.json");
    assert_eq!(path.append("").relative(), "raw");

    // Joining onto an empty relative path yields the segment alone, without a
    // leading separator (`path{} / "gfx" == "gfx"`, `cata_path.h:48`).
    let empty = CataPath::new(RootPath::Data, "");
    assert_eq!(empty.push("gfx").relative(), "gfx");
    assert_eq!(empty.append("gfx").relative(), "gfx");
}

/// Joining never normalises: `a/../b` stays literal until `lexically_normal`
/// is asked for it, exactly as `std::filesystem::path` behaves
/// (`src/cata_path.h:118-122`).
#[test]
fn join_does_not_normalise_the_relative_path() {
    let roots = roots();
    let path = CataPath::new(RootPath::Data, "core").push("..").push("raw");
    assert_eq!(path.relative(), "core/../raw");
    assert_eq!(path.resolve(&roots), "opt/cdda/data/core/../raw");
    // Segments keep their own separators: no doubling, no stripping.
    assert_eq!(path.push("colors//x").relative(), "core/../raw/colors//x");
    // Because the reference joins strings for these, `..` can escape a root.
    let escape = CataPath::new(RootPath::Data, "../config/options.json");
    assert_eq!(
        escape.resolve(&roots),
        "opt/cdda/data/../config/options.json"
    );
}

/// `cata_path::lexically_normal` is `std::filesystem::path::lexically_normal`
/// (`src/cata_path.h:118-122`). The expected values below were produced by
/// compiling and running the reference's own expression against libstdc++:
/// `fs::path(case).lexically_normal().generic_u8string()`.
#[test]
fn lexically_normal_matches_std_filesystem() {
    let table = [
        ("", ""),
        (".", "."),
        ("..", ".."),
        ("/", "/"),
        ("./", "."),
        ("./.", "."),
        ("foo/.", "foo/"),
        ("foo/./", "foo/"),
        ("/..", "/"),
        ("..foo", "..foo"),
        ("a/b/../../", "."),
        ("/a/", "/a/"),
        ("//a", "/a"),
        ("a//b", "a/b"),
        ("./a", "a"),
        ("a/./b/./", "a/b/"),
        ("a/b/..", "a/"),
        ("/../a", "/a"),
        ("a/../../../b", "../../b"),
        ("a/..", "."),
        ("a/../", "."),
        ("x/./../y", "y"),
        ("a/b/c/../../..", "."),
    ];
    for (input, expected) in table {
        assert_eq!(normalize(input), expected, "normalize({input:?})");
    }

    // On a cata_path it normalises only the relative component.
    let path = CataPath::new(RootPath::Data, "core/./../raw//");
    assert_eq!(path.lexically_normal().relative(), "raw/");
    assert_eq!(path.lexically_normal().root(), RootPath::Data);
    assert_eq!(path.relative(), "core/./../raw//", "original is untouched");
}

/// `as_norm_dir` (`src/filesystem.cpp:133-142`) appends a trailing separator and
/// then normalises, which is why every `*_value` the reference concatenates can
/// be assumed to end in one. Expected values again come from running the
/// reference expression (`fs::path(p) / fs::path{}` then `lexically_normal`).
#[test]
fn as_norm_dir_adds_the_trailing_separator_the_reference_relies_on() {
    let table = [
        ("", ""),
        (".", "./"),
        ("./", "./"),
        ("data", "data/"),
        ("data/", "data/"),
        ("/usr/local", "/usr/local/"),
        ("/usr/local/", "/usr/local/"),
        ("/a//b//", "/a/b/"),
        ("/a/../b", "/b/"),
        // A path of only dot-dot keeps no trailing separator: this is the one
        // case where the reference's own invariant does not hold, and
        // `base_path_value + "data/"` then produces `..data/`.
        ("a/../..", ".."),
        ("..", ".."),
        ("/", "/"),
    ];
    for (input, expected) in table {
        assert_eq!(as_norm_dir(input), expected, "as_norm_dir({input:?})");
    }
}

/// A POSIX environment with one home directory and no XDG overrides.
fn env() -> Environment {
    Environment {
        home: Some("/home/u".to_owned()),
        ..Environment::default()
    }
}

/// `init_user_dir` (`path_info.cpp:81-107`): an explicit directory is only
/// normalised, while an empty one is filled in per platform. The build options
/// select which default applies; `main.cpp:675-683` passes `""` when
/// `USE_HOME_DIR` or `USE_XDG_DIR` is set and `"."` otherwise.
#[test]
fn user_dir_defaults_follow_platform_and_build_options() {
    // (platform, options, environment, expected user directory)
    let table = [
        (Platform::Linux, BuildOptions::default(), env(), "./"),
        (
            Platform::Linux,
            BuildOptions {
                use_home_dir: true,
                ..BuildOptions::default()
            },
            env(),
            "/home/u/.cataclysm-dda/",
        ),
        (
            Platform::Linux,
            BuildOptions {
                use_xdg_dir: true,
                ..BuildOptions::default()
            },
            Environment {
                xdg_data_home: Some("/x/data".to_owned()),
                ..env()
            },
            "/x/data/cataclysm-dda/",
        ),
        (
            Platform::Linux,
            BuildOptions {
                use_xdg_dir: true,
                ..BuildOptions::default()
            },
            env(),
            "/home/u/.local/share/cataclysm-dda/",
        ),
        // macOS takes precedence over the XDG branch in the preprocessor chain,
        // so a mac build ignores USE_XDG_DIR for the user directory.
        (
            Platform::MacOs,
            BuildOptions {
                use_xdg_dir: true,
                ..BuildOptions::default()
            },
            env(),
            "/home/u/Library/Application Support/Cataclysm/",
        ),
        (
            Platform::Windows,
            BuildOptions {
                use_home_dir: true,
                ..BuildOptions::default()
            },
            Environment {
                localappdata: Some("C:/Users/u/AppData/Local".to_owned()),
                ..env()
            },
            "C:/Users/u/AppData/Local/cataclysm-dda/",
        ),
    ];
    for (platform, options, environment, expected) in table {
        let config = PathConfig {
            platform,
            options,
            env: environment,
        };
        let mut info = PathInfo::new();
        assert_eq!(info.init_user_dir(&config), Ok(()), "{platform:?}");
        assert_eq!(info.user_dir(), expected, "{platform:?} {options:?}");
        assert_eq!(info.roots().value(RootPath::User), expected);
    }

    // An explicit directory bypasses the platform defaults but is still
    // normalised to a trailing separator (`as_norm_dir`).
    let mut info = PathInfo::new();
    info.init_user_dir_explicit("some/user/dir");
    assert_eq!(info.user_dir(), "some/user/dir/");
    // `"."` is what the reference passes for a build with neither option.
    let mut info = PathInfo::new();
    info.init_user_dir_explicit(".");
    assert_eq!(info.user_dir(), "./");
}

/// A missing environment variable is a failure the *caller* handles: the
/// reference calls `getenv_or_abort`, which reports a fatal debug message and
/// aborts (`path_info.cpp:66-73`, `debug.h:87-91`). Returning `Err` keeps that
/// decision out of the library.
#[test]
fn missing_environment_variables_are_reported_not_aborted() {
    let config = PathConfig {
        platform: Platform::Linux,
        options: BuildOptions {
            use_home_dir: true,
            ..BuildOptions::default()
        },
        env: Environment::default(),
    };
    let mut info = PathInfo::new();
    assert_eq!(
        info.init_user_dir(&config),
        Err(InitError::MissingEnv("HOME"))
    );

    // XDG falls back to HOME, so a set XDG_DATA_HOME needs no HOME at all.
    let config = PathConfig {
        platform: Platform::Linux,
        options: BuildOptions {
            use_xdg_dir: true,
            ..BuildOptions::default()
        },
        env: Environment {
            xdg_data_home: Some("/x/data".to_owned()),
            ..Environment::default()
        },
    };
    assert_eq!(info.init_user_dir(&config), Ok(()));
    assert_eq!(info.user_dir(), "/x/data/cataclysm-dda/");

    // Windows needs LOCALAPPDATA, macOS needs HOME. Both go through the
    // platform default only when an option selects it: `init_user_dir(".")`
    // never reads the environment, on any platform.
    let home_dir_options = BuildOptions {
        use_home_dir: true,
        ..BuildOptions::default()
    };
    let windows = PathConfig {
        platform: Platform::Windows,
        options: home_dir_options,
        ..PathConfig::default()
    };
    assert_eq!(
        info.init_user_dir(&windows),
        Err(InitError::MissingEnv("LOCALAPPDATA"))
    );
    let macos = PathConfig {
        platform: Platform::MacOs,
        options: home_dir_options,
        ..PathConfig::default()
    };
    assert_eq!(
        info.init_user_dir(&macos),
        Err(InitError::MissingEnv("HOME"))
    );
}

/// `set_standard_filenames` (`path_info.cpp:109-174`) is pure derivation: it
/// builds the string and `cata_path` forms of every standard location from the
/// base and user directories. Values below are read straight off that function.
#[test]
fn set_standard_filenames_derives_every_standard_location() {
    let config = PathConfig::default();
    let mut info = PathInfo::new();
    info.init_base_path("opt/cdda");
    info.init_user_dir_explicit("/home/u/.cataclysm-dda/");
    assert_eq!(info.set_standard_filenames(&config), Ok(()));

    let strings = info.strings();
    let cache_dir = strings.cache_dir();
    let table = [
        (strings.datadir.as_str(), "opt/cdda/data/"),
        (strings.gfxdir.as_str(), "opt/cdda/gfx/"),
        (strings.langdir.as_str(), "opt/cdda/lang/mo/"),
        (strings.motd.as_str(), "opt/cdda/data/motd/en.motd"),
        (cache_dir.as_str(), "opt/cdda/data/cache/"),
        (strings.savedir.as_str(), "/home/u/.cataclysm-dda/save/"),
        (
            strings.memorialdir.as_str(),
            "/home/u/.cataclysm-dda/memorial/",
        ),
        (
            strings.achievementdir.as_str(),
            "/home/u/.cataclysm-dda/achievements/",
        ),
        (
            strings.config_dir.as_str(),
            "/home/u/.cataclysm-dda/config/",
        ),
        (
            strings.options.as_str(),
            "/home/u/.cataclysm-dda/config/options.json",
        ),
        (
            strings.keymap.as_str(),
            "/home/u/.cataclysm-dda/config/keymap.txt",
        ),
        (
            strings.autopickup.as_str(),
            "/home/u/.cataclysm-dda/config/auto_pickup.json",
        ),
        (
            strings.autonote.as_str(),
            "/home/u/.cataclysm-dda/config/auto_note.json",
        ),
    ];
    for (actual, expected) in table {
        assert_eq!(actual, expected);
    }

    assert_standard_paths(&info);
}

/// The two build options that change the derivation: `USE_XDG_DIR` moves the
/// config directory to `XDG_CONFIG_HOME`, and `DATA_DIR_PREFIX` moves the data
/// directory under `share/cataclysm-dda/`. An empty base path is the other
/// branch, where `gfx/` and `lang/mo/` are *not* under the data directory.
#[test]
fn standard_filenames_follow_the_xdg_and_prefix_build_options() {
    // USE_XDG_DIR: the config directory becomes its own logical root.
    let xdg = PathConfig {
        options: BuildOptions {
            use_xdg_dir: true,
            ..BuildOptions::default()
        },
        env: Environment {
            home: Some("/home/u".to_owned()),
            xdg_config_home: Some("/x/config".to_owned()),
            ..Environment::default()
        },
        ..PathConfig::default()
    };
    let mut info = PathInfo::new();
    info.init_base_path("opt/cdda");
    assert_eq!(info.set_standard_filenames(&xdg), Ok(()));
    assert_eq!(info.strings().config_dir, "/x/config/cataclysm-dda/");
    assert_eq!(
        info.strings().options,
        "/x/config/cataclysm-dda/options.json"
    );
    assert_eq!(info.strings().keymap, "/x/config/cataclysm-dda/keymap.txt");
    assert_eq!(info.paths().config_dir.root(), RootPath::Config);
    assert_eq!(
        info.resolve(&info.paths().options),
        "/x/config/cataclysm-dda/options.json"
    );

    // Without XDG_CONFIG_HOME the fallback is `$HOME/.config/cataclysm-dda/`.
    let xdg_home_only = PathConfig {
        env: Environment {
            home: Some("/home/u".to_owned()),
            ..Environment::default()
        },
        ..xdg.clone()
    };
    let mut info = PathInfo::new();
    assert_eq!(info.set_standard_filenames(&xdg_home_only), Ok(()));
    assert_eq!(info.strings().config_dir, "/home/u/.config/cataclysm-dda/");

    // Neither HOME nor XDG_CONFIG_HOME is set: reported, not fatal.
    let mut info = PathInfo::new();
    assert_eq!(
        info.set_standard_filenames(&PathConfig {
            options: BuildOptions {
                use_xdg_dir: true,
                ..BuildOptions::default()
            },
            ..PathConfig::default()
        }),
        Err(InitError::MissingEnv("HOME"))
    );

    // DATA_DIR_PREFIX: data, gfx and lang all live under `share/cataclysm-dda/`,
    // and `gfx` is then relative to the data root rather than the base root.
    let prefixed = PathConfig {
        options: BuildOptions {
            data_dir_prefix: true,
            ..BuildOptions::default()
        },
        ..PathConfig::default()
    };
    let mut info = PathInfo::new();
    info.init_base_path("opt/cdda");
    assert_eq!(info.set_standard_filenames(&prefixed), Ok(()));
    assert_eq!(info.strings().datadir, "opt/cdda/share/cataclysm-dda/");
    assert_eq!(info.strings().gfxdir, "opt/cdda/share/cataclysm-dda/gfx/");
    assert_eq!(
        info.strings().langdir,
        "opt/cdda/share/cataclysm-dda/lang/mo/"
    );
    assert_eq!(
        info.strings().motd,
        "opt/cdda/share/cataclysm-dda/motd/en.motd"
    );
    assert_eq!(info.paths().gfxdir.root(), RootPath::Data);
    assert_eq!(
        info.resolve(&info.paths().gfxdir),
        "opt/cdda/share/cataclysm-dda/gfx"
    );

    // Empty base path: `datadir` is `data/` but `prefix` stays empty, so gfx
    // and lang are `gfx/` and `lang/mo/` — not `data/gfx/`.
    let mut info = PathInfo::new();
    assert_eq!(info.set_standard_filenames(&PathConfig::default()), Ok(()));
    assert_eq!(info.strings().datadir, "data/");
    assert_eq!(info.strings().gfxdir, "gfx/");
    assert_eq!(info.strings().langdir, "lang/mo/");
    assert_eq!(info.strings().motd, "data/motd/en.motd");
    assert_eq!(info.resolve(&info.paths().datadir), "data/");
    assert_eq!(info.resolve(&info.paths().gfxdir), "gfx");
}

/// The `cata_path` forms carry a logical root, which is what makes them
/// relocatable: the same path can be re-rooted without touching its segments.
fn assert_standard_paths(info: &PathInfo) {
    let paths = info.paths();
    let path_table = [
        (&paths.datadir, RootPath::Data, "", "opt/cdda/data/"),
        (&paths.gfxdir, RootPath::Base, "gfx", "opt/cdda/gfx"),
        (
            &paths.langdir,
            RootPath::Base,
            "lang/mo",
            "opt/cdda/lang/mo",
        ),
        (
            &paths.savedir,
            RootPath::Save,
            "",
            "/home/u/.cataclysm-dda/save/",
        ),
        (
            &paths.memorialdir,
            RootPath::User,
            "memorial",
            "/home/u/.cataclysm-dda/memorial",
        ),
        (
            &paths.achievementdir,
            RootPath::User,
            "achievements",
            "/home/u/.cataclysm-dda/achievements",
        ),
        // Without `USE_XDG_DIR` the config directory is a plain user subdirectory.
        (
            &paths.config_dir,
            RootPath::User,
            "config",
            "/home/u/.cataclysm-dda/config",
        ),
        (
            &paths.options,
            RootPath::User,
            "config/options.json",
            "/home/u/.cataclysm-dda/config/options.json",
        ),
        (
            &paths.autopickup,
            RootPath::User,
            "config/auto_pickup.json",
            "/home/u/.cataclysm-dda/config/auto_pickup.json",
        ),
        (
            &paths.autonote,
            RootPath::User,
            "config/auto_note.json",
            "/home/u/.cataclysm-dda/config/auto_note.json",
        ),
    ];
    for (path, root, relative, expected) in path_table {
        assert_eq!(path.root(), root, "{relative}");
        assert_eq!(path.relative(), relative);
        assert_eq!(info.resolve(path), expected);
    }

    // Root values are registered so that a bare `cata_path` for each root
    // resolves to the string form above.
    assert_eq!(info.roots().value(RootPath::Data), "opt/cdda/data/");
    assert_eq!(
        info.roots().value(RootPath::Save),
        "/home/u/.cataclysm-dda/save/"
    );
    assert_eq!(
        info.roots().value(RootPath::Memorial),
        "/home/u/.cataclysm-dda/memorial/"
    );
}

/// The `set_*` overrides (`path_info.cpp:534-591`) each rebind one location and
/// its logical root, and several of them deliberately *fake* a root: `set_options`
/// and `set_autopickup` produce an `unknown`-rooted path from an arbitrary
/// string, while `set_keymap` and `set_motd` update only the string form.
#[test]
fn setters_rebind_roots_and_some_bypass_them() {
    let config = PathConfig::default();
    let mut info = PathInfo::new();
    info.init_base_path("opt/cdda");
    info.init_user_dir_explicit("/home/u/.cataclysm-dda/");
    assert_eq!(info.set_standard_filenames(&config), Ok(()));

    // set_datadir: the data root becomes the new directory and the gfx and motd
    // strings are built by concatenation from the *verbatim* argument
    // (`path_info.cpp:534-545`, reached from `--datadir` at `main.cpp:455-458`).
    // A directory without a trailing separator therefore produces a wrong
    // string, while the `cata_path` form stays correct because it goes through
    // the root: this is the concatenation bug in its original form.
    info.set_datadir("/srv/data");
    assert_eq!(info.strings().datadir, "/srv/data");
    assert_eq!(info.strings().gfxdir, "/srv/datagfx/");
    assert_eq!(info.strings().motd, "/srv/datamotd/en.motd");
    assert_eq!(info.roots().value(RootPath::Data), "/srv/data");
    assert_eq!(info.paths().gfxdir.root(), RootPath::Data);
    assert_eq!(info.paths().gfxdir.relative(), "gfx");
    assert_eq!(info.resolve(&info.paths().gfxdir), "/srv/data/gfx");
    assert_eq!(info.resolve(&info.paths().datadir), "/srv/data/");
    // Passing the trailing separator the field wants keeps the strings right.
    info.set_datadir("/srv/data2/");
    assert_eq!(info.strings().gfxdir, "/srv/data2/gfx/");
    assert_eq!(info.strings().motd, "/srv/data2/motd/en.motd");

    // set_config_dir: the config directory becomes its own root, even when the
    // build does not use XDG directories.
    info.set_config_dir("/etc/cdda/");
    assert_eq!(info.strings().config_dir, "/etc/cdda/");
    assert_eq!(info.strings().options, "/etc/cdda/options.json");
    assert_eq!(info.strings().keymap, "/etc/cdda/keymap.txt");
    assert_eq!(info.paths().options.root(), RootPath::Config);
    assert_eq!(
        info.resolve(&info.paths().options),
        "/etc/cdda/options.json"
    );
    assert_eq!(info.paths().autopickup.root(), RootPath::Config);

    // set_savedir and set_memorialdir each become their own root.
    info.set_savedir("/srv/saves/");
    assert_eq!(info.strings().savedir, "/srv/saves/");
    assert_eq!(info.resolve(&info.paths().savedir), "/srv/saves/");
    info.set_memorialdir("/srv/memorial/");
    assert_eq!(info.paths().memorialdir.root(), RootPath::Memorial);
    assert_eq!(info.resolve(&info.paths().memorialdir), "/srv/memorial/");

    // set_options and set_autopickup take arbitrary strings: the path is
    // `unknown`-rooted, i.e. used verbatim relative to the cwd.
    info.set_options("/custom/options.json");
    assert_eq!(info.strings().options, "/custom/options.json");
    assert_eq!(info.paths().options.root(), RootPath::Unknown);
    assert_eq!(info.paths().options.relative(), "/custom/options.json");
    assert_eq!(info.resolve(&info.paths().options), "/custom/options.json");
    info.set_autopickup("pickup.json");
    assert_eq!(info.paths().autopickup.root(), RootPath::Unknown);
    assert_eq!(info.resolve(&info.paths().autopickup), "pickup.json");

    // set_keymap only changes the string: the path form keeps its root.
    let before = info.paths().options.clone();
    info.set_keymap("/custom/keymap.txt");
    assert_eq!(info.strings().keymap, "/custom/keymap.txt");
    assert_eq!(info.paths().options, before);
}

/// The whole matrix in one table: platform × build option, with the user
/// directory resolved from the environment and everything downstream derived
/// from it (`path_info.cpp:81-174`).
#[test]
fn platform_matrix_produces_consistent_derived_directories() {
    let home = || Environment {
        home: Some("/home/u".to_owned()),
        localappdata: Some("C:/Users/u/AppData/Local".to_owned()),
        xdg_data_home: Some("/x/data".to_owned()),
        xdg_config_home: Some("/x/config".to_owned()),
    };
    let cases = [
        (
            PathConfig {
                platform: Platform::Linux,
                ..PathConfig::default()
            },
            "./",
            "./save/",
            "./config/",
        ),
        (
            PathConfig {
                platform: Platform::Linux,
                options: BuildOptions {
                    use_home_dir: true,
                    ..BuildOptions::default()
                },
                env: home(),
            },
            "/home/u/.cataclysm-dda/",
            "/home/u/.cataclysm-dda/save/",
            "/home/u/.cataclysm-dda/config/",
        ),
        (
            PathConfig {
                platform: Platform::Linux,
                options: BuildOptions {
                    use_xdg_dir: true,
                    ..BuildOptions::default()
                },
                env: home(),
            },
            "/x/data/cataclysm-dda/",
            "/x/data/cataclysm-dda/save/",
            "/x/config/cataclysm-dda/",
        ),
        (
            PathConfig {
                platform: Platform::MacOs,
                options: BuildOptions {
                    use_home_dir: true,
                    ..BuildOptions::default()
                },
                env: home(),
            },
            "/home/u/Library/Application Support/Cataclysm/",
            "/home/u/Library/Application Support/Cataclysm/save/",
            "/home/u/Library/Application Support/Cataclysm/config/",
        ),
        (
            PathConfig {
                platform: Platform::Windows,
                options: BuildOptions {
                    use_home_dir: true,
                    ..BuildOptions::default()
                },
                env: home(),
            },
            "C:/Users/u/AppData/Local/cataclysm-dda/",
            "C:/Users/u/AppData/Local/cataclysm-dda/save/",
            "C:/Users/u/AppData/Local/cataclysm-dda/config/",
        ),
    ];
    for (config, user_dir, savedir, config_dir) in cases {
        let mut info = PathInfo::new();
        info.init_base_path("opt/cdda");
        assert_eq!(info.init_user_dir(&config), Ok(()), "{config:?}");
        assert_eq!(info.set_standard_filenames(&config), Ok(()), "{config:?}");
        assert_eq!(info.strings().savedir, savedir, "{config:?}");
        assert_eq!(info.strings().config_dir, config_dir, "{config:?}");
        assert_eq!(info.roots().value(RootPath::User), user_dir, "{config:?}");
        // The data directory never depends on the platform, only on the base path.
        assert_eq!(info.strings().datadir, "opt/cdda/data/", "{config:?}");
        assert_eq!(info.resolve(&info.paths().savedir), savedir, "{config:?}");
    }
}
