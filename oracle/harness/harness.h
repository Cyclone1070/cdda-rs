#pragma once
#ifndef CATA_SRC_HARNESS_H
#define CATA_SRC_HARNESS_H

// Instrumentation for comparing this implementation against the port.
//
// Everything here is compiled out entirely unless CATA_HARNESS is defined at
// build time, and no function in this file changes game behaviour when the
// harness flags are not passed on the command line. See
// docs/11-oracle-harness.md for the modes and their contract.

#include <cstdint>
#include <cstdio>
#include <string>
#include <vector>

#include "input_enums.h"

namespace harness
{

// A prototype registry that can list its own ids.
//
// Every generic_factory instance registers itself here at construction, which is
// what makes a digest of all loaded definitions possible without listing the
// eightysomething registries by hand.
struct prototype_source {
    std::string type_name;
    std::vector<std::string> ( *list_ids )( void *factory );
    void *factory;
};

// Called by generic_factory's constructor. Safe before main().
void register_prototype_source( const std::string &type_name,
                                std::vector<std::string>( *list_ids )( void * ),
                                void *factory );

// FNV-1a, 64 bit, over each value's bytes followed by a single zero byte. The
// definition is part of the contract: the port has to compute the same number,
// so it is spelled out here rather than left to a library.
uint64_t digest_strings( const std::vector<std::string> &values );

// `--dump-prototype-digest <file>`: one JSON object per registry, sorted by type
// name, each with a count and a digest of its sorted ids. Written to `path`.
//
// A file rather than stdout on purpose: the curses build initializes the
// terminal before the data load, so stdout carries escape sequences.
void dump_prototype_digest( const std::string &path );

// `--dump-prototype-ids <file>`: the same registries, but with every id listed,
// so a difference can be read directly instead of hunted for.
void dump_prototype_ids( const std::string &path );

// `--dump-env <file>`: the state that exists before any world does — the random
// engine's position, the calendar, and the simulation options those two depend
// on. This is the first, smallest state fingerprint: two runs of the same build
// with the same seed must agree on every field.
void dump_environment( const std::string &path );

// Starts a new game by the real path (world creation, avatar birth, start game)
// and stops where the first input would be needed. Returns false and fills
// `error` if the reference refuses.
bool start_headless_game( const std::string &world_name, const std::string &start_loc,
                          std::string &error );

// --- Replay -----------------------------------------------------------------

// Arms the recorded input. While armed, the input layer serves these events
// instead of reading a terminal, the terminal-size and encoding loops are
// skipped, and reaching a popup is an error rather than a wait.
void arm_recorded_input( std::vector<std::string> keys );

// True while the recorded input is armed.
bool terminal_is_simulated();

// True while recorded keys remain.
bool has_recorded_input();

// The next recorded key as the reference's input event. Never called when
// `has_recorded_input()` is false.
input_event next_input_event();

// Called by the input layer when something wants a raw key while the terminal is
// simulated: that is a popup or a menu, which a replay must not enter.
[[noreturn]] void note_blocked_input( const char *where );

// Reads a recording: one key per line, `#` for comments. The first byte of each
// line is the key the reference will see.
std::vector<std::string> read_key_file( const std::string &path, std::string &error );

// Runs turns until the recording is exhausted.
void run_replay();

// `--dump-terrain <file>`: every terrain and furniture prototype's resolved
// fields, which is what a loader port has to reproduce. Filled in by mapdata.cpp
// because the factories there are file-static; this is the entry point.
void dump_terrain( const std::string &path );


// `--dump-traps <file>`: every trap's resolved fields.
void dump_traps( const std::string &path );

// `--dump-materials <file>`: every material's resolved fields.
void dump_materials( const std::string &path );

// `--dump-fields <file>`: every field type's resolved intensity levels.
void dump_fields( const std::string &path );

// `--dump-items <file>`: every item type's basic resolved fields.
void dump_items( const std::string &path );

// `--dump-state <file>`: the world the reference actually produced — random
// stream position, calendar, avatar, and a tile-level digest of the reality
// bubble. Requires start_headless_game first.
void dump_state( const std::string &path );

} // namespace harness

#endif // CATA_SRC_HARNESS_H

// Provided by mapdata.cpp, which owns the terrain factories: writes the dump
// body for both prototype lists. Declared outside the namespace because the
// definition sits outside mapdata.cpp's anonymous namespace.
void harness_dump_terrain_prototypes( std::FILE *out );

// Provided by trap.cpp, for the same reason.
void harness_dump_trap_prototypes( std::FILE *out );
