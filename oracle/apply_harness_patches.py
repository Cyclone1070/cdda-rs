#!/usr/bin/env python3
"""Applies the harness patch series to the oracle's working copy.

The working copy (`oracle/src`) is a disposable rsync of the pinned checkout, so
this script edits it in place. Patches are generated from it afterwards with
`diff -ruN references/Cataclysm-DDA/src oracle/src/src`, which keeps the pinned
checkout untouched and the patch reviewable.

Idempotent: running it twice changes nothing the second time.
"""

from __future__ import annotations

import pathlib
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
ORACLE = ROOT / "oracle"
SRC = ORACLE / "src" / "src"
HARNESS = ORACLE / "harness"


def patch(path: pathlib.Path, old: str, new: str, label: str) -> bool:
    text = path.read_text()
    if new in text:
        return False
    if old not in text:
        sys.exit(f"anchor not found for {label} in {path}")
    path.write_text(text.replace(old, new, 1))
    return True


def patch_generic_factory(src: pathlib.Path) -> None:
    path = src / "generic_factory.h"

    patch(
        path,
        '#include "cached_options.h"',
        '#ifdef CATA_HARNESS\n#include "harness.h"\n#endif\n#include "cached_options.h"',
        "harness include",
    )

    patch(
        path,
        """        const std::vector<T> &get_all() const {
            return list;
        }""",
        """        const std::vector<T> &get_all() const {
            return list;
        }
#ifdef CATA_HARNESS
        // Harness only: every id this factory currently holds, for the
        // prototype digest. Never called by the game.
        std::vector<std::string> debug_prototype_ids() const {
            std::vector<std::string> result;
            result.reserve( list.size() );
            for( const T &obj : list ) {
                result.push_back( obj.id.str() );
            }
            return result;
        }
#endif""",
        "digest accessor",
    )

    patch(
        path,
        """            : type_name( type_name ),
              id_member_name( id_member_name ),
              dummy_obj(),
              initialized( true ) {
        }""",
        """            : type_name( type_name ),
              id_member_name( id_member_name ),
              dummy_obj(),
              initialized( true ) {
#ifdef CATA_HARNESS
            harness::register_prototype_source( type_name, []( void *self ) {
                return static_cast<generic_factory<T> *>( self )->debug_prototype_ids();
            }, this );
#endif
        }""",
        "registry hook",
    )


def patch_game_header(src: pathlib.Path) -> None:
    """Adds the two accessors the harness needs on `game`.

    `start_game()` and the map member are private; rather than make the harness a
    friend of everything, expose exactly the two things it uses, under the same
    compile flag as the rest of the instrumentation.
    """
    patch(
        src / "game.h",
        """        void setup();
        /** Saving and loading functions. */""",
        """        void setup();
#ifdef CATA_HARNESS
        // Harness hooks. Compiled only when CATA_HARNESS is defined, and never
        // called by the game itself. See docs/11-oracle-harness.md.
        bool harness_start_game() {
            return start_game();
        }
        map &harness_map() {
            return m;
        }
#endif
        /** Saving and loading functions. */""",
        "game accessors",
    )


def patch_main(src: pathlib.Path) -> None:
    path = src / "main.cpp"

    patch(
        path,
        """    bool verifyexit = false;""",
        """    bool verifyexit = false;
    // Harness: write a digest of every loaded prototype (or the ids themselves)
    // to this file and exit. See docs/11-oracle-harness.md.
    std::string dump_prototype_digest;
    std::string dump_prototype_ids;
    // Harness: the state that exists before any world does.
    std::string dump_env;
    // Harness: the state of a freshly started game.
    std::string dump_state;
    std::string world_name;
    std::string start_location;""",
        "cli fields",
    )

    patch(
        path,
        """            {
                "--jsonverify", {},""",
        """            {
                "--dump-prototype-digest", "<file>",
                "Writes a digest of every loaded prototype to <file> and exits",
                section_default,
                1,
                [&result]( int, const char **params ) -> int {
                    result.dump_prototype_digest = params[0];
                    return 1;
                }
            },
            {
                "--dump-prototype-ids", "<file>",
                "Writes every loaded prototype id to <file> and exits",
                section_default,
                1,
                [&result]( int, const char **params ) -> int {
                    result.dump_prototype_ids = params[0];
                    return 1;
                }
            },
            {
                "--dump-state", "<file>",
                "Starts a new game headlessly and writes its state to <file>",
                section_default,
                1,
                [&result]( int, const char **params ) -> int {
                    result.dump_state = params[0];
                    return 1;
                }
            },
            {
                "--world-name", "<name>",
                "World name used by --dump-state (default: harness-fixed)",
                section_default,
                1,
                [&result]( int, const char **params ) -> int {
                    result.world_name = params[0];
                    return 1;
                }
            },
            {
                "--start-location", "<id>",
                "Start location used by --dump-state (default: none, scenario decides)",
                section_default,
                1,
                [&result]( int, const char **params ) -> int {
                    result.start_location = params[0];
                    return 1;
                }
            },
            {
                "--dump-env", "<file>",
                "Writes the pre-world state (rng, calendar, options) to <file> and exits",
                section_default,
                1,
                [&result]( int, const char **params ) -> int {
                    result.dump_env = params[0];
                    return 1;
                }
            },
            {
                "--jsonverify", {},""",
        "cli handlers",
    )

    patch(
        path,
        """        g->load_static_data();
        if( cli.verifyexit ) {""",
        """        g->load_static_data();
        if( !cli.dump_state.empty() ) {
            // Test mode keeps the loading screen and popups from trying to draw to
            // a terminal the harness does not have. It also means the reference's
            // input layer throws on any read, which is why this mode must never
            // be used for replay.
            test_mode = true;
            std::string error;
            const std::string world_name =
                cli.world_name.empty() ? "harness-fixed" : cli.world_name;
            if( !harness::start_headless_game( world_name, cli.start_location, error ) ) {
                std::fprintf( stderr, "harness: %s\\n", error.c_str() );
                exit_handler( 2 );
            }
            harness::dump_state( cli.dump_state );
            exit_handler( 0 );
        }
        if( !cli.dump_env.empty() ) {
            harness::dump_environment( cli.dump_env );
            exit_handler( 0 );
        }
        if( !cli.dump_prototype_digest.empty() || !cli.dump_prototype_ids.empty() ) {
            // Load exactly what a player's game loads: the core data, then the
            // content pack. `check_mod_data` is the reference's own path for
            // that (it is what --check-mods uses), so the digest cannot drift
            // from the real loading order.
            //
            // Test mode, as --check-mods sets it, so that a data error is
            // reported instead of opening a popup that waits for input and hangs
            // an unattended run. This is the one mode where that is safe: no
            // menu is built. The replay harness must not do this.
            test_mode = true;
            g->check_mod_data( { mod_id( "dda" ) } );
            if( !cli.dump_prototype_ids.empty() ) {
                harness::dump_prototype_ids( cli.dump_prototype_ids );
            } else {
                harness::dump_prototype_digest( cli.dump_prototype_digest );
            }
            exit_handler( 0 );
        }
        if( cli.verifyexit ) {""",
        "dump branch",
    )

    patch(
        path,
        '#include "get_version.h"',
        '#include "get_version.h"\n#ifdef CATA_HARNESS\n#include "harness.h"\n#endif',
        "main include",
    )


def copy_harness_sources(src: pathlib.Path) -> None:
    """Puts the harness translation units into the working copy.

    They live in `oracle/harness` so that the working copy can be regenerated
    from the pinned checkout at any time without losing them.
    """
    for name in ("harness.h", "harness.cpp"):
        (src / name).write_text((HARNESS / name).read_text())


def main() -> None:
    if not SRC.is_dir():
        sys.exit(f"oracle working copy missing at {SRC}; run oracle/build.sh first")
    copy_harness_sources(SRC)
    patch_generic_factory(SRC)
    patch_game_header(SRC)
    patch_main(SRC)
    print("harness patches applied")


def verify() -> None:
    """Proves the script reproduces the working copy from the pinned checkout."""
    import subprocess

    ref = ROOT / "references" / "Cataclysm-DDA" / "src"
    scratch = ORACLE / ".verify"
    if scratch.exists():
        import shutil

        shutil.rmtree(scratch)
    scratch.mkdir()
    for name in ("generic_factory.h", "main.cpp", "game.h"):
        (scratch / name).write_text((ref / name).read_text())
    copy_harness_sources(scratch)
    patch_generic_factory(scratch)
    patch_game_header(scratch)
    patch_main(scratch)

    failed = False
    for name in ("generic_factory.h", "main.cpp", "game.h", "harness.h", "harness.cpp"):
        expected = (SRC / name).read_text()
        got = (scratch / name).read_text()
        if expected != got:
            failed = True
            diff = subprocess.run(
                ["diff", "-u", str(scratch / name), str(SRC / name)],
                capture_output=True, text=True).stdout
            print(f"{name}: script does not reproduce the working copy\n{diff}")
    import shutil

    shutil.rmtree(scratch)
    if failed:
        sys.exit(1)
    print("verify: the script reproduces the working copy exactly")


if __name__ == "__main__":
    if "--verify" in sys.argv:
        verify()
    else:
        main()
