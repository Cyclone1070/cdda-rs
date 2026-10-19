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


def patch_ncurses_256(src: pathlib.Path) -> None:
    """Pins the colour table to the 256-colour branch.

    `color.cpp` builds two different tables and picks one on
    `COLORS >= 256`, which ncurses only knows once a terminal exists. A headless
    run therefore gets the fallback table, where `c_dark_gray_red` is an alias of
    `c_black_red` - and every colour name in a dump would then describe the
    fallback, not the game a player sees. The harness pins the modern table.
    """
    path = src / "ncurses_def.cpp"
    patch(
        path,
        """bool catacurses::supports_256_colors()
{
    return COLORS >= 256;
}""",
        """bool catacurses::supports_256_colors()
{
#ifdef CATA_HARNESS
    // No terminal in a headless run, so COLORS is zero. The port targets a
    // 256-colour terminal, so the harness reports one.
    return true;
#endif
    return COLORS >= 256;
}""",
        "colour capability",
    )


def patch_trap_header(src: pathlib.Path) -> None:
    """Exposes the three trap fields the dump needs that have no getter."""
    path = src / "trap.h"
    patch(
        path,
        "        std::optional<itype_id> trap_item_type;",
        """        std::optional<itype_id> trap_item_type;

#ifdef CATA_HARNESS
        int harness_visibility() const {
            return visibility;
        }
        int harness_difficulty() const {
            return difficulty;
        }
        const cata::flat_set<flag_id> &harness_flags() const {
            return _flags;
        }
#endif""",
        "trap header accessors",
    )


def patch_trap(src: pathlib.Path) -> None:
    """Adds the trap dump.

    The trap factory is file-static inside `trap.cpp`'s anonymous namespace, so
    the dump is written there: a walker inside the namespace, and an exported
    entry point after it closes.
    """
    path = src / "trap.cpp"
    anchor = 'generic_factory<trap> trap_factory( "trap" );\n\n} // namespace'
    replacement = r'''generic_factory<trap> trap_factory( "trap" );

#ifdef CATA_HARNESS
void write_trap_prototypes_impl( std::FILE *out )
{
    bool first = true;
    for( const trap &proto : trap_factory.get_all() ) {
        std::fprintf( out, "%s\n{\"id\":\"%s\",\"name\":\"%s\",\"symbol\":%d,\"color\":\"%s\","
                      "\"visibility\":%d,\"avoidance\":%d,\"difficulty\":%d,\"radius\":%d,"
                      "\"benign\":%s,\"always_invisible\":%s,\"flags\":[",
                      first ? "" : ",", proto.id.str().c_str(), proto.name().c_str(), proto.sym,
                      string_from_color( proto.color ).c_str(), proto.harness_visibility(),
                      proto.get_avoidance(), proto.harness_difficulty(),
                      proto.get_trap_radius(), proto.is_benign() ? "true" : "false",
                      proto.is_always_invisible() ? "true" : "false" );
        bool first_flag = true;
        for( const flag_id &flag : proto.harness_flags() ) {
            std::fprintf( out, "%s\"%s\"", first_flag ? "" : ",", flag.str().c_str() );
            first_flag = false;
        }
        std::fprintf( out, "]}" );
        first = false;
    }
}
#endif
} // namespace

#ifdef CATA_HARNESS
void harness_dump_trap_prototypes( std::FILE *out )
{
    write_trap_prototypes_impl( out );
}
#endif'''
    patch(path, anchor, replacement, "trap dump")


def patch_mapdata(src: pathlib.Path) -> None:
    """Adds the terrain and furniture dump.

    The factories are file-static inside `mapdata.cpp`'s anonymous namespace, so
    the walker has to live in that namespace. The exported entry point sits after
    the namespace closes, which is also where it becomes visible to the harness.
    """
    path = src / "mapdata.cpp"
    patch(
        path,
        '#include "mapdata.h"',
        '#include "mapdata.h"\n#ifdef CATA_HARNESS\n#include "color.h"\n#include "harness.h"\n#endif',
        "mapdata include",
    )
    anchor = 'generic_factory<ter_t> terrain_data( "terrain" );\ngeneric_factory<furn_t> furniture_data( "furniture" );\n\n} // namespace'
    replacement = r'''generic_factory<ter_t> terrain_data( "terrain" );
generic_factory<furn_t> furniture_data( "furniture" );

#ifdef CATA_HARNESS
template<typename T>
void write_one_prototype( std::FILE *out, const T &proto, const char *kind, bool &first )
{
    std::fprintf( out, "%s\n{\"kind\":\"%s\",\"id\":\"%s\",\"name\":\"%s\",",
                  first ? "" : ",", kind, proto.id.str().c_str(), proto.name().c_str() );
    std::fprintf( out, "\"symbol\":[%d,%d,%d,%d],\"color\":\"%s\",\"move_cost\":%d,"
                  "\"light\":%d,\"coverage\":%d,\"flags\":[",
                  proto.symbol_[0], proto.symbol_[1], proto.symbol_[2], proto.symbol_[3],
                  string_from_color( proto.color() ).c_str(), proto.movecost,
                  proto.light_emitted, proto.coverage );
    bool first_flag = true;
    for( const std::string &flag : proto.get_flags() ) {
        std::fprintf( out, "%s\"%s\"", first_flag ? "" : ",", flag.c_str() );
        first_flag = false;
    }
    std::fprintf( out, "]}" );
    first = false;
}

void write_terrain_prototypes_impl( std::FILE *out )
{
    bool first = true;
    for( const ter_t &proto : terrain_data.get_all() ) {
        write_one_prototype( out, proto, "terrain", first );
    }
    for( const furn_t &proto : furniture_data.get_all() ) {
        write_one_prototype( out, proto, "furniture", first );
    }
}
#endif
} // namespace

#ifdef CATA_HARNESS
void harness_dump_terrain_prototypes( std::FILE *out )
{
    write_terrain_prototypes_impl( out );
}
#endif'''
    patch(path, anchor, replacement, "terrain dump")


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
    // Harness: resolved terrain and furniture prototypes.
    std::string dump_terrain;
    // Harness: resolved traps and materials.
    std::string dump_traps;
    std::string dump_materials;
    std::string replay;
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
                "--replay", "<file>",
                "Replays recorded keys instead of reading a terminal (needs --dump-state)",
                section_default,
                1,
                [&result]( int, const char **params ) -> int {
                    result.replay = params[0];
                    return 1;
                }
            },
            {
                "--dump-traps", "<file>",
                "Writes every resolved trap prototype to <file>",
                section_default,
                1,
                [&result]( int, const char **params ) -> int {
                    result.dump_traps = params[0];
                    return 1;
                }
            },
            {
                "--dump-materials", "<file>",
                "Writes every resolved material prototype to <file>",
                section_default,
                1,
                [&result]( int, const char **params ) -> int {
                    result.dump_materials = params[0];
                    return 1;
                }
            },
            {
                "--dump-terrain", "<file>",
                "Writes every resolved terrain and furniture prototype to <file>",
                section_default,
                1,
                [&result]( int, const char **params ) -> int {
                    result.dump_terrain = params[0];
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
        r"""        g->load_static_data();
        if( !cli.dump_state.empty() || !cli.replay.empty() ) {
            const std::string world_name =
                cli.world_name.empty() ? "harness-fixed" : cli.world_name;
            std::string error;
            try {
                if( cli.replay.empty() ) {
                    // A plain dump reads nothing, so test mode is safe and keeps the
                    // interface out of the way entirely.
                    test_mode = true;
                } else {
                    // A replay must run the simulation exactly as a player's would,
                    // so test mode stays off; the interface layer is taught to run
                    // without a terminal instead (see apply_harness_patches.py).
                    std::vector<std::string> keys = harness::read_key_file( cli.replay, error );
                    if( keys.empty() ) {
                        std::fprintf( stderr, "harness: %s\n", error.c_str() );
                        exit_handler( 2 );
                    }
                    harness::arm_recorded_input( keys );
                }
                if( !harness::start_headless_game( world_name, cli.start_location, error ) ) {
                    std::fprintf( stderr, "harness: %s\n", error.c_str() );
                    exit_handler( 2 );
                }
                if( !cli.replay.empty() ) {
                    harness::run_replay();
                }
            } catch( const std::exception &e ) {
                std::fprintf( stderr, "harness: %s\n", e.what() );
                exit_handler( 3 );
            }
            harness::dump_state( cli.dump_state );
            exit_handler( 0 );
        }
        if( !cli.dump_traps.empty() ) {
            test_mode = true;
            g->check_mod_data( { mod_id( "dda" ) } );
            harness::dump_traps( cli.dump_traps );
            exit_handler( 0 );
        }
        if( !cli.dump_materials.empty() ) {
            test_mode = true;
            g->check_mod_data( { mod_id( "dda" ) } );
            harness::dump_materials( cli.dump_materials );
            exit_handler( 0 );
        }
        if( !cli.dump_terrain.empty() ) {
            test_mode = true;
            g->check_mod_data( { mod_id( "dda" ) } );
            harness::dump_terrain( cli.dump_terrain );
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


def patch_ncurses(src: pathlib.Path) -> None:
    """Teaches the interface layer to run without a terminal.

    A replay serves recorded keys instead of reading the screen, so the pieces
    that wait for a real terminal have to be bypassed. Each hook is inert unless
    the harness has armed a recording, and each is narrow on purpose:

    * the loading screen is not drawn (it is the one thing that blocks with no
      terminal, and it has no effect on simulation),
    * the terminal-size and encoding loops do not run,
    * the event pump does not drain input,
    * the event source serves the recording,
    * a popup returns "no" instead of waiting for a key it will never get.
    """
    def add_include( path: pathlib.Path, after: str, label: str ) -> None:
        patch(
            path,
            after,
            after + '\n#ifdef CATA_HARNESS\n#include "harness.h"\n#endif',
            label,
        )

    ncurses = src / "ncurses_def.cpp"
    add_include( ncurses, '#include "cursesdef.h"', "ncurses include" )
    patch(
        ncurses,
        """void ensure_term_size()
{""",
        """void ensure_term_size()
{
    if( harness::terminal_is_simulated() ) {
        return;
    }""",
        "terminal size loop",
    )
    patch(
        ncurses,
        """void check_encoding()
{""",
        """void check_encoding()
{
    if( harness::terminal_is_simulated() ) {
        return;
    }""",
        "encoding loop",
    )
    patch(
        ncurses,
        """void input_manager::pump_events()
{
    if( test_mode ) {""",
        """void input_manager::pump_events()
{
    if( harness::terminal_is_simulated() ) {
        return;
    }
    if( test_mode ) {""",
        "event pump",
    )
    patch(
        ncurses,
        """    if( test_mode ) {
        // input should be skipped in caller's code
        throw std::runtime_error( "input_manager::get_input_event called in test mode" );""",
        """    if( harness::has_recorded_input() ) {
        return harness::next_input_event();
    }
    if( test_mode ) {
        // input should be skipped in caller's code
        throw std::runtime_error( "input_manager::get_input_event called in test mode" );""",
        "recorded events",
    )

    loading = src / "loading_ui.cpp"
    add_include( loading, '#include "loading_ui.h"', "loading include" )
    patch(
        loading,
        """void loading_ui::show( const std::string &context, const std::string &step )
{
    if( test_mode ) {
        return;
    }""",
        """void loading_ui::show( const std::string &context, const std::string &step )
{
    if( test_mode || harness::terminal_is_simulated() ) {
        return;
    }""",
        "loading screen",
    )

    # A modal prompt waits for a key that a replay will never provide. Decline
    # it, and say so: a replay that silently took the "no" branch would be worse
    # than one that says which prompt it declined.
    popup = src / "popup.cpp"
    add_include( popup, '#include "popup.h"', "popup include" )
    patch(
        popup,
        """query_popup::result query_popup::query()
{""",
        """query_popup::result query_popup::query()
{
    if( harness::terminal_is_simulated() ) {
        std::fprintf( stderr, "harness: declined a popup: %s\\n", text.c_str() );
        return { false, "ERROR", {} };
    }""",
        "declined popup",
    )

    # An error in the reference opens a modal prompt and waits for a key. A
    # replay has no keyboard, so turn it into a failure with the message in it -
    # that is far more useful than a hang, and it is the same information the
    # prompt would have shown.
    debug = src / "debug.cpp"
    add_include( debug, '#include "debug.h"', "debug include" )
    patch(
        debug,
        """    debug_error_prompt( filename, line, funcname, text.c_str(), false );""",
        """    if( harness::terminal_is_simulated() ) {
        throw std::runtime_error( "reference error: " + text );
    }
    debug_error_prompt( filename, line, funcname, text.c_str(), false );""",
        "error prompt without a keyboard",
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
    patch_ncurses(SRC)
    patch_trap_header(SRC)
    patch_trap(SRC)
    patch_mapdata(SRC)
    patch_ncurses_256(SRC)
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
    for name in ("generic_factory.h", "main.cpp", "game.h", "ncurses_def.cpp",
                 "loading_ui.cpp", "debug.cpp", "popup.cpp", "mapdata.cpp",
                 "trap.cpp", "trap.h"):
        (scratch / name).write_text((ref / name).read_text())
    copy_harness_sources(scratch)
    patch_generic_factory(scratch)
    patch_game_header(scratch)
    patch_ncurses(scratch)
    patch_trap_header(scratch)
    patch_trap(scratch)
    patch_mapdata(scratch)
    patch_ncurses_256(scratch)
    patch_main(scratch)

    failed = False
    for name in ("generic_factory.h", "main.cpp", "game.h", "ncurses_def.cpp",
                 "loading_ui.cpp", "debug.cpp", "popup.cpp", "mapdata.cpp",
                 "harness.h", "harness.cpp"):
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
