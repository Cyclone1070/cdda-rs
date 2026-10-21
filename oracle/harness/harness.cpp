#include "harness.h"
#include "recipe_dictionary.h"
#include "requirements.h"

#include <algorithm>

#include "avatar.h"
#include "calendar.h"
#include "do_turn.h"
#include "game.h"
#include "map.h"
#include "mapgendata.h"
#include "mapgen_functions.h"
#include "enums.h"
#include "item_factory.h"
#include "item_group.h"
#include "mapgen.h"
#include "veh_type.h"
#include "scenario.h"
#include "profession.h"
#include "omdata.h"
#include "overmapbuffer.h"
#include "itype.h"
#include "field_type.h"
#include "mtype.h"
#include "monstergenerator.h"
#include "generic_factory.h"
#include "filesystem.h"
#include "options.h"
#include "path_info.h"
#include "rng.h"
#include "start_location.h"
#include "string_id.h"
#include "worldfactory.h"
#include <cstdio>
#include <fstream>
#include <stdexcept>
#include <utility>
#include <string>
#include <vector>

namespace harness
{

namespace
{

// Function-local static: generic_factory instances are static globals, and the
// order in which they are constructed is not defined, so the registry must not
// be a namespace-scope global itself.
std::vector<prototype_source> &registry()
{
    static std::vector<prototype_source> sources;
    return sources;
}

std::string harness_escape( const std::string &value )
{
    std::string out;
    out.reserve( value.size() + 2 );
    for( const char c : value ) {
        switch( c ) {
            case '"':
                out += "\\\"";
                break;
            case '\\':
                out += "\\\\";
                break;
            case '\n':
                out += "\\n";
                break;
            case '\t':
                out += "\\t";
                break;
            case '\r':
                out += "\\r";
                break;
            default:
                out += c;
        }
    }
    return out;
}

} // namespace

void register_prototype_source( const std::string &type_name,
                                std::vector<std::string>( *list_ids )( void * ),
                                void *factory )
{
    registry().push_back( prototype_source{ type_name, list_ids, factory } );
}

uint64_t digest_strings( const std::vector<std::string> &values )
{
    constexpr uint64_t offset_basis = 14695981039346656037ULL;
    constexpr uint64_t prime = 1099511628211ULL;
    uint64_t hash = offset_basis;
    for( const std::string &value : values ) {
        for( const char c : value ) {
            hash ^= static_cast<unsigned char>( c );
            hash *= prime;
        }
        // Separator, so that {"ab","c"} and {"a","bc"} differ.
        hash ^= 0x00;
        hash *= prime;
    }
    return hash;
}

namespace
{

// Every registry's ids, sorted and deduplicated, with the registry name.
std::vector<std::pair<std::string, std::vector<std::string>>> collect()
{
    std::vector<std::pair<std::string, std::vector<std::string>>> out;
    out.reserve( registry().size() );
    for( const prototype_source &source : registry() ) {
        std::vector<std::string> ids = source.list_ids( source.factory );
        std::sort( ids.begin(), ids.end() );
        ids.erase( std::unique( ids.begin(), ids.end() ), ids.end() );
        out.emplace_back( source.type_name, std::move( ids ) );
    }
    std::sort( out.begin(), out.end(),
    []( const auto & l, const auto & r ) {
        return l.first < r.first;
    } );
    return out;
}

} // namespace

void dump_prototype_digest( const std::string &path )
{
    const auto collected = collect();
    std::FILE *out = std::fopen( path.c_str(), "wb" );
    if( out == nullptr ) {
        std::fprintf( stderr, "harness: cannot write %s\n", path.c_str() );
        return;
    }
    std::fprintf( out, "{\"harness\":\"prototype-digest\",\"version\":1,\"registries\":[\n" );
    bool first = true;
    for( const auto &entry : collected ) {
        std::fprintf( out, "%s{\"type\":\"%s\",\"count\":%zu,\"digest\":\"%016llx\"}",
                      first ? "" : ",\n", harness_escape( entry.first ).c_str(),
                      entry.second.size(),
                      static_cast<unsigned long long>( digest_strings( entry.second ) ) );
        first = false;
    }
    std::fprintf( out, "\n]}\n" );
    std::fclose( out );
}

void dump_prototype_ids( const std::string &path )
{
    const auto collected = collect();
    std::FILE *out = std::fopen( path.c_str(), "wb" );
    if( out == nullptr ) {
        std::fprintf( stderr, "harness: cannot write %s\n", path.c_str() );
        return;
    }
    std::fprintf( out, "{\"harness\":\"prototype-ids\",\"version\":1,\"registries\":[\n" );
    bool first_registry = true;
    for( const auto &entry : collected ) {
        std::fprintf( out, "%s{\"type\":\"%s\",\"ids\":[", first_registry ? "" : ",\n",
                      harness_escape( entry.first ).c_str() );
        bool first_id = true;
        for( const std::string &id : entry.second ) {
            std::fprintf( out, "%s\"%s\"", first_id ? "" : ",", harness_escape( id ).c_str() );
            first_id = false;
        }
        std::fprintf( out, "]}" );
        first_registry = false;
    }
    std::fprintf( out, "\n]}\n" );
    std::fclose( out );
}

void dump_astronomy( const std::string &path )
{
    std::FILE *out = std::fopen( path.c_str(), "wb" );
    if( out == nullptr ) {
        std::fprintf( stderr, "harness: cannot write %s\n", path.c_str() );
        return;
    }
    std::fprintf( out, "{\"harness\":\"astronomy\",\"version\":1,\n" );
    // The calendar the samples were taken under, because a season length other than the
    // default changes the year length and the whole curve with it. A dump that does not say
    // which calendar it used cannot be gated against.
    std::fprintf( out, "\"year_length_days\":%d,\"season_length_days\":%d,\n",
                  to_days<int>( calendar::year_length() ),
                  to_days<int>( calendar::season_length() ) );
    std::fprintf( out, "\"samples\":[\n" );
    // A leap year's worth of days at three-hour intervals: every season, every hour,
    // including the ones where the sun never rises.
    bool first = true;
    for( int day = 0; day < 365; day += 7 ) {
        for( int hour = 0; hour < 24; hour += 3 ) {
            const time_point t = calendar::turn_zero + time_duration::from_days( day )
                                 + time_duration::from_hours( hour );
            const units::angle altitude = sun_azimuth_altitude( t ).second;
            const std::array<double, 6> internals = harness_sun_internals( t );
            std::fprintf( out,
                          "%s{\"day\":%d,\"hour\":%d,\"altitude\":%.9f,\"light\":%.9f,"
                          "\"ra\":%.9f,\"dec\":%.9f,\"sidereal\":%.9f,\"hour_angle\":%.9f,"
                          "\"days\":%.9f}",
                          first ? "" : ",\n", day, hour, to_degrees( altitude ),
                          sun_light_at( t ), internals[0], internals[1], internals[2],
                          internals[3], internals[4] );
            first = false;
        }
    }
    std::fprintf( out, "]}\n" );
    std::fclose( out );
}

void dump_environment( const std::string &path )
{
    std::FILE *out = std::fopen( path.c_str(), "wb" );
    if( out == nullptr ) {
        std::fprintf( stderr, "harness: cannot write %s\n", path.c_str() );
        return;
    }

    std::fprintf( out, "{\"harness\":\"environment\",\"version\":1,\n" );

    // The engine's position, fingerprinted by drawing from a copy so that the
    // game's own stream is not advanced by measuring it.
    cata_default_random_engine probe = rng_get_engine();
    std::fprintf( out, "\"rng_next\":[" );
    for( int i = 0; i < 4; i++ ) {
        std::fprintf( out, "%s%llu", i == 0 ? "" : ",",
                      static_cast<unsigned long long>( probe() ) );
    }
    std::fprintf( out, "],\n" );

    const time_point now = calendar::turn;
    std::fprintf( out,
                  "\"turn\":%d,\"season\":%d,\"day_of_season\":%d,\"hour\":%d,"
                  "\"sunrise\":%d,\"sunset\":%d,\n",
                  to_turn<int>( now ), static_cast<int>( season_of_year( now ) ),
                  day_of_season<int>( now ), hour_of_day<int>( now ),
                  to_turn<int>( sunrise( now ) ), to_turn<int>( sunset( now ) ) );

    // The options the calendar and spawning read. The full table is a separate
    // task; these are the ones that change simulation behaviour, not presentation.
    std::fprintf( out,
                  "\"options\":{\"SEASON_LENGTH\":%d,\"ETERNAL_SEASON\":%s,"
                  "\"ETERNAL_TIME_OF_DAY\":\"%s\",\"MONSTER_SPEED\":%d,"
                  "\"SPAWN_DENSITY\":%.6f}\n}\n",
                  get_option<int>( "SEASON_LENGTH" ),
                  get_option<bool>( "ETERNAL_SEASON" ) ? "true" : "false",
                  get_option<std::string>( "ETERNAL_TIME_OF_DAY" ).c_str(),
                  get_option<int>( "MONSTER_SPEED" ),
                  get_option<float>( "SPAWN_DENSITY" ) );

    std::fclose( out );
}

namespace
{
bool simulated_terminal = false;
std::vector<std::string> recorded_keys;
std::size_t next_key = 0;
} // namespace

void arm_recorded_input( std::vector<std::string> keys )
{
    recorded_keys = std::move( keys );
    next_key = 0;
    simulated_terminal = true;
}

bool terminal_is_simulated()
{
    return simulated_terminal;
}

bool has_recorded_input()
{
    return simulated_terminal && next_key < recorded_keys.size();
}

input_event next_input_event()
{
    // A recorded key is one byte; the reference's keyboard_char event carries
    // exactly that in its sequence.
    const std::string &key = recorded_keys[next_key++];
    return input_event( static_cast<int>( key.empty() ? 0 : key[0] ),
                        input_event_t::keyboard_char );
}

[[noreturn]] void note_blocked_input( const char *where )
{
    throw std::runtime_error( std::string( "harness: input requested outside the "
                                           "recorded stream at " ) + where );
}

std::vector<std::string> read_key_file( const std::string &path, std::string &error )
{
    std::vector<std::string> keys;
    std::ifstream in( path );
    if( !in ) {
        error = "cannot read " + path;
        return keys;
    }
    std::string line;
    while( std::getline( in, line ) ) {
        if( line.empty() || line[0] == '#' ) {
            continue;
        }
        keys.push_back( line );
    }
    return keys;
}

void run_replay()
{
    // One turn per recorded key. `do_turn` consumes exactly one action, so the
    // loop stops when the recording does; the guard is a runaway check, not a
    // turn limit.
    std::size_t guard = 0;
    while( has_recorded_input() ) {
        if( ++guard > 100000 ) {
            throw std::runtime_error( "harness: replay exceeded 100000 turns" );
        }
        do_turn();
    }
}

void dump_terrain( const std::string &path )
{
    std::FILE *out = std::fopen( path.c_str(), "wb" );
    if( out == nullptr ) {
        std::fprintf( stderr, "harness: cannot write %s\n", path.c_str() );
        return;
    }
    std::fprintf( out, "{\"harness\":\"terrain\",\"version\":1,\n\"prototypes\":[\n" );
    harness_dump_terrain_prototypes( out );
    std::fprintf( out, "]}\n" );
    std::fclose( out );
}

void dump_traps( const std::string &path )
{
    std::FILE *out = std::fopen( path.c_str(), "wb" );
    if( out == nullptr ) {
        std::fprintf( stderr, "harness: cannot write %s\n", path.c_str() );
        return;
    }
    std::fprintf( out, "{\"harness\":\"traps\",\"version\":1,\n\"prototypes\":[\n" );
    harness_dump_trap_prototypes( out );
    std::fprintf( out, "]}\n" );
    std::fclose( out );
}

void dump_vehicle_parts( const std::string &path )
{
    std::FILE *out = std::fopen( path.c_str(), "wb" );
    if( out == nullptr ) {
        std::fprintf( stderr, "harness: cannot write %s\n", path.c_str() );
        return;
    }
    std::vector<std::string> parts;
    for( const vpart_info &part : vehicles::parts::get_all() ) {
        parts.push_back( part.id.str() );
    }
    std::vector<std::string> prototypes;
    for( const vehicle_prototype &proto : vehicles::get_all_prototypes() ) {
        prototypes.push_back( proto.id.str() );
    }
    std::sort( parts.begin(), parts.end() );
    std::sort( prototypes.begin(), prototypes.end() );
    std::fprintf( out, "{\"harness\":\"vehicles\",\"version\":1,\n\"parts\":[\n" );
    bool first = true;
    for( const std::string &id : parts ) {
        std::fprintf( out, "%s\"%s\"", first ? "" : ",\n", id.c_str() );
        first = false;
    }
    std::fprintf( out, "\n],\"part_count\":%zu,\n\"prototypes\":[\n", parts.size() );
    first = true;
    for( const std::string &id : prototypes ) {
        std::fprintf( out, "%s\"%s\"", first ? "" : ",\n", id.c_str() );
        first = false;
    }
    std::fprintf( out, "\n],\"prototype_count\":%zu}\n", prototypes.size() );
    std::fclose( out );
}

void dump_starting_kit( const std::string &path )
{
    std::FILE *out = std::fopen( path.c_str(), "wb" );
    if( out == nullptr ) {
        std::fprintf( stderr, "harness: cannot write %s\n", path.c_str() );
        return;
    }
    std::vector<std::string> professions;
    for( const profession &prof : profession::get_all() ) {
        professions.push_back( prof.ident().str() );
    }
    std::vector<std::string> scenarios;
    for( const scenario &scen : scenario::get_all() ) {
        scenarios.push_back( scen.ident().str() );
    }
    std::sort( professions.begin(), professions.end() );
    std::sort( scenarios.begin(), scenarios.end() );
    std::fprintf( out, "{\"harness\":\"starting_kit\",\"version\":1,\n\"professions\":[\n" );
    bool first = true;
    for( const std::string &id : professions ) {
        std::fprintf( out, "%s\"%s\"", first ? "" : ",\n", id.c_str() );
        first = false;
    }
    std::fprintf( out, "\n],\"profession_count\":%zu,\n\"scenarios\":[\n", professions.size() );
    first = true;
    for( const std::string &id : scenarios ) {
        std::fprintf( out, "%s\"%s\"", first ? "" : ",\n", id.c_str() );
        first = false;
    }
    std::fprintf( out, "\n],\"scenario_count\":%zu}\n", scenarios.size() );
    std::fclose( out );
}

void load_game_data()
{
    // The game's own startup order (game::load_core_data then finalize_loaded_data). The dump
    // paths use check_mod_data, which leaves some registries unreadable - mapgens empty, npc
    // classes hanging - so this exists to test whether the difference is the load, not the dump.
    g->load_core_data();
    DynamicDataLoader::get_instance().finalize_loaded_data();
}

void load_core_data_only()
{
    // Everything but the final consistency sweep. `finalize_loaded_data` crashes in
    // `MonsterGenerator::check_monster_definitions` under the harness, and the mapgen registry is
    // already filled by this point, which is what the mapgen probe needs to see.
    g->load_core_data();
}


void dump_load_probe( const std::string &path )
{
    // What is actually in memory at this point: how much data loaded at all, and whether the
    // mapgen registry knows any real overmap terrain ids.
    std::FILE *out = std::fopen( path.c_str(), "wb" );
    if( out == nullptr ) {
        std::fprintf( stderr, "harness: cannot write %s\n", path.c_str() );
        return;
    }
    size_t terrains = 0;
    for( const oter_t &oter : overmap_terrains::get_all() ) {
        ( void )oter;
        terrains++;
    }
    size_t by_mapgen_id = 0;
    size_t by_oter_id = 0;
    for( const oter_t &oter : overmap_terrains::get_all() ) {
        if( has_mapgen_for( oter.get_mapgen_id() ) ) {
            by_mapgen_id++;
        }
        if( has_mapgen_for( oter.id.str() ) ) {
            by_oter_id++;
        }
    }
    const char *probes[] = { "house", "field", "forest", "road", "house_north", "shelter",
                             "apartments_mod_tower_110", "farm_field" };
    std::fprintf( out, "{\"harness\":\"load_probe\",\"version\":1,\"overmap_terrains\":%zu,"
                  "\"with_mapgen_by_mapgen_id\":%zu,\"with_mapgen_by_oter_id\":%zu,"
                  "\"items\":%zu,\"monsters\":%zu,\"materials\":%zu,\"mapgens\":{",
                  terrains, by_mapgen_id, by_oter_id, item_controller->all().size(),
                  MonsterGenerator::generator().get_all_mtypes().size(),
                  materials::get_all().size() );
    bool first = true;
    for( const char *name : probes ) {
        std::fprintf( out, "%s\"%s\":%s", first ? "" : ",", name,
                      has_mapgen_for( name ) ? "true" : "false" );
        first = false;
    }
    std::fprintf( out, "}}\n" );
    std::fclose( out );
}

void dump_mapgens( const std::string &path )
{
    std::FILE *out = std::fopen( path.c_str(), "wb" );
    if( out == nullptr ) {
        std::fprintf( stderr, "harness: cannot write %s\n", path.c_str() );
        return;
    }
    // Forcing the weights does not make the JSON mapgens appear: the registry holds only the
    // built-in C++ ones (325 keys) at this point, and finalising here aborts, because the data
    // was already finalised by check_mod_data. Both are recorded in the roadmap.
    std::vector<std::string> keys;
    for( const oter_t &oter : overmap_terrains::get_all() ) {
        // The reference looks mapgen up by the terrain's *mapgen id*, not its own id: 24,191 of
        // 24,218 terrains have one under that key, and only 325 match the terrain id.
        if( has_mapgen_for( oter.get_mapgen_id() ) ) {
            keys.push_back( oter.get_mapgen_id() );
        }
    }
    std::sort( keys.begin(), keys.end() );
    // Several terrains can share one mapgen, so the raw list repeats keys; the artifact is a set.
    keys.erase( std::unique( keys.begin(), keys.end() ), keys.end() );
    std::fprintf( out, "{\"harness\":\"mapgens\",\"version\":1,\n\"keys\":[\n" );
    bool first = true;
    for( const std::string &key : keys ) {
        std::fprintf( out, "%s\"%s\"", first ? "" : ",\n", key.c_str() );
        first = false;
    }
    std::fprintf( out, "\n],\"count\":%zu}\n", keys.size() );
    std::fclose( out );
}

void dump_item_groups( const std::string &path )
{
    std::FILE *out = std::fopen( path.c_str(), "wb" );
    if( out == nullptr ) {
        std::fprintf( stderr, "harness: cannot write %s\n", path.c_str() );
        return;
    }
    std::vector<item_group_id> groups = item_controller->get_all_group_names();
    std::sort( groups.begin(), groups.end(),
    []( const item_group_id & a, const item_group_id & b ) {
        return a.str() < b.str();
    } );
    std::fprintf( out, "{\"harness\":\"item_groups\",\"version\":1,\n\"groups\":[\n" );
    bool first = true;
    for( const item_group_id &group : groups ) {
        std::fprintf( out, "%s{\"id\":\"%s\",\"items\":[", first ? "" : ",\n",
                      group.str().c_str() );
        first = false;
        // every_item_min_max walks the whole tree, so nested groups are already expanded and
        // the numbers are what a spawn can produce rather than what the JSON literally says.
        Item_spawn_data *data = item_group::spawn_data_from_group( group );
        bool first_item = true;
        if( data != nullptr ) {
            for( const auto &entry : data->every_item_min_max() ) {
                std::fprintf( out, "%s{\"item\":\"%s\",\"min\":%d,\"max\":%d}",
                              first_item ? "" : ",", entry.first->get_id().str().c_str(),
                              entry.second.first, entry.second.second );
                first_item = false;
            }
        }
        std::fprintf( out, "]}" );
    }
    std::fprintf( out, "\n],\"count\":%zu}\n", groups.size() );
    std::fclose( out );
}

void dump_recipes( const std::string &path )
{
    std::FILE *out = std::fopen( path.c_str(), "wb" );
    if( out == nullptr ) {
        std::fprintf( stderr, "harness: cannot write %s\n", path.c_str() );
        return;
    }
    std::fprintf( out, "{\"harness\":\"recipes\",\"version\":1,\n\"recipes\":[\n" );
    bool first = true;
    for( auto it = recipe_dict.begin(); it != recipe_dict.end(); ++it ) {
        const recipe &r = it->second;
        std::fprintf( out, "%s{\"id\":\"%s\",\"result\":\"%s\",\"blacklisted\":%s,\"components\":[",
                      first ? "" : ",\n", r.ident().str().c_str(), r.result().str().c_str(),
                      r.is_blacklisted() ? "true" : "false" );
        // Alternatives are the structure that matters: each inner list is a set of things any
        // one of which satisfies that requirement, so flattening them would change the recipe.
        bool first_group = true;
        for( const auto &group : r.simple_requirements().get_components() ) {
            std::fprintf( out, "%s[", first_group ? "" : "," );
            first_group = false;
            bool first_alt = true;
            for( const item_comp &comp : group ) {
                std::fprintf( out, "%s{\"item\":\"%s\",\"count\":%d}",
                              first_alt ? "" : ",", comp.type.str().c_str(), comp.count );
                first_alt = false;
            }
            std::fprintf( out, "]" );
        }
        std::fprintf( out, "],\"tools\":[" );
        bool first_tool_group = true;
        for( const auto &group : r.simple_requirements().get_tools() ) {
            std::fprintf( out, "%s[", first_tool_group ? "" : "," );
            first_tool_group = false;
            bool first_tool = true;
            for( const tool_comp &tool : group ) {
                std::fprintf( out, "%s{\"item\":\"%s\",\"count\":%d}",
                              first_tool ? "" : ",", tool.type.str().c_str(), tool.count );
                first_tool = false;
            }
            std::fprintf( out, "]" );
        }
        std::fprintf( out, "],\"qualities\":[" );
        bool first_quality_group = true;
        for( const auto &group : r.simple_requirements().get_qualities() ) {
            std::fprintf( out, "%s[", first_quality_group ? "" : "," );
            first_quality_group = false;
            bool first_quality = true;
            for( const quality_requirement &quality : group ) {
                std::fprintf( out, "%s{\"quality\":\"%s\",\"level\":%d,\"count\":%d}",
                              first_quality ? "" : ",", quality.type.str().c_str(), quality.level,
                              quality.count );
                first_quality = false;
            }
            std::fprintf( out, "]" );
        }
        // What the recipe asks of the crafter: the skill it trains and how hard it is
        // (`recipe.h:133-137` and `:188`).
        std::fprintf( out, "],\"skill\":\"%s\",\"difficulty\":%d}",
                      harness_escape( r.skill_used.str() ).c_str(), r.difficulty );
        first = false;
    }
    std::fprintf( out, "\n],\"count\":%zu}\n", recipe_dict.size() );
    std::fclose( out );
}

void dump_materials( const std::string &path )
{
    std::FILE *out = std::fopen( path.c_str(), "wb" );
    if( out == nullptr ) {
        std::fprintf( stderr, "harness: cannot write %s\n", path.c_str() );
        return;
    }
    std::fprintf( out, "{\"harness\":\"materials\",\"version\":1,\n\"prototypes\":[\n" );
    bool first = true;
    for( const material_type &material : materials::get_all() ) {
        std::fprintf( out,
                      "%s\n{\"id\":\"%s\",\"name\":\"%s\",\"density\":%.6g,"
                      "\"chip_resist\":%d,\"breathability\":%d,"
                      "\"specific_heat_liquid\":%.6g,\"specific_heat_solid\":%.6g,"
                      "\"latent_heat\":%.6g,\"burn\":",
                      first ? "" : ",", harness_escape( material.id.str() ).c_str(),
                      harness_escape( material.name() ).c_str(),
                      static_cast<double>( material.density() ), material.chip_resist(),
                      material.breathability(),
                      static_cast<double>( material.specific_heat_liquid() ),
                      static_cast<double>( material.specific_heat_solid() ),
                      static_cast<double>( material.latent_heat() ) );
        // Per-intensity burn behaviour: immune, volume per turn, fuel, smoke and burn rates.
        std::fprintf( out, "[" );
        bool first_burn = true;
        for( size_t intensity = 1; intensity <= 3; intensity++ ) {
            const mat_burn_data &burn = material.burn_data( intensity );
            std::fprintf( out,
                          "%s{\"intensity\":%zu,\"immune\":%s,\"volume_per_turn\":%.6g,"
                          "\"fuel\":%.6g,\"smoke\":%.6g,\"burn\":%.6g}",
                          first_burn ? "" : ",", intensity, burn.immune ? "true" : "false",
                          static_cast<double>( units::to_milliliter( burn.volume_per_turn ) ),
                          static_cast<double>( burn.fuel ), static_cast<double>( burn.smoke ),
                          static_cast<double>( burn.burn ) );
            first_burn = false;
        }
        // What burning this material leaves behind (`material.h:115-116`), which the fire loop
        // spawns by weight when an item loses mass without being destroyed.
        std::fprintf( out, "],\"burn_products\":[" );
        bool first_product = true;
        for( const std::pair<itype_id, float> &product : material.burn_products() ) {
            std::fprintf( out, "%s[\"%s\",%.6g]", first_product ? "" : ",",
                          harness_escape( product.first.str() ).c_str(),
                          static_cast<double>( product.second ) );
            first_product = false;
        }
        std::fprintf( out, "]}" );
        first = false;
    }
    std::fprintf( out, "]}\n" );
    std::fclose( out );
}

void dump_fields( const std::string &path )
{
    std::FILE *out = std::fopen( path.c_str(), "wb" );
    if( out == nullptr ) {
        std::fprintf( stderr, "harness: cannot write %s\n", path.c_str() );
        return;
    }
    std::fprintf( out, "{\"harness\":\"fields\",\"version\":1,\n\"prototypes\":[\n" );
    bool first = true;
    for( const field_type &field : get_all_field_types().get_all() ) {
        std::fprintf( out, "%s\n{\"id\":\"%s\",\"priority\":%d,"
                      "\"underwater_age_speedup\":%d,\"outdoor_age_speedup\":%d,"
                      "\"percent_spread\":%d,\"levels\":[",
                      first ? "" : ",", harness_escape( field.id.str() ).c_str(), field.priority,
                      to_turns<int>( field.underwater_age_speedup ),
                      to_turns<int>( field.outdoor_age_speedup ),
                      field.percent_spread );
        bool first_level = true;
        for( const field_intensity_level &level : field.intensity_levels ) {
            std::fprintf( out, "%s\n{\"name\":\"%s\",\"sym\":%u,\"color\":\"%s\","
                          "\"dangerous\":%s,\"transparent\":%s,\"move_cost\":%d,"
                          "\"concentration\":%d,\"light\":%.6g,"
                          "\"upgrade_chance\":%d,\"upgrade_duration\":%d,"
                          "\"spawn_chance\":%d,\"spawn_count\":%d,\"spawn_radius\":%d,"
                          "\"spawn_group\":\"%s\"}",
                          first_level ? "" : ",", harness_escape( level.name.translated() ).c_str(),
                          level.symbol, string_from_color( level.color ).c_str(),
                          level.dangerous ? "true" : "false",
                          level.transparent ? "true" : "false", level.move_cost,
                          level.concentration, static_cast<double>( level.light_emitted ),
                          level.intensity_upgrade_chance,
                          to_turns<int>( level.intensity_upgrade_duration ),
                          level.monster_spawn_chance, level.monster_spawn_count,
                          level.monster_spawn_radius,
                          harness_escape( level.monster_spawn_group.str() ).c_str() );
            first_level = false;
        }
        std::fprintf( out, "]}" );
        first = false;
    }
    std::fprintf( out, "]}\n" );
    std::fclose( out );
}

void dump_items( const std::string &path )
{
    std::FILE *out = std::fopen( path.c_str(), "wb" );
    if( out == nullptr ) {
        std::fprintf( stderr, "harness: cannot write %s\n", path.c_str() );
        return;
    }
    std::fprintf( out, "{\"harness\":\"items\",\"version\":1,\n\"prototypes\":[\n" );
    bool first = true;
    for( const itype *proto : item_controller->all() ) {
        std::fprintf( out, "%s\n{\"id\":\"%s\",\"name\":\"%s\",\"name_plural\":\"%s\","
                      "\"symbol\":\"%s\",\"color\":\"%s\",\"weight\":%lld,\"volume\":%lld,"
                      "\"price\":%lld,\"category\":\"%s\",\"materials\":[",
                      first ? "" : ",", harness_escape( proto->id.str() ).c_str(),
                      harness_escape( proto->nname( 1 ) ).c_str(),
                      harness_escape( proto->nname( 2 ) ).c_str(),
                      // The glyph is UTF-8 text, not a byte: a multi-byte symbol
                      // would otherwise be dumped as its first signed byte.
                      harness_escape( proto->sym ).c_str(),
                      string_from_color( proto->color ).c_str(),
                      static_cast<long long>( units::to_gram( proto->weight ) ),
                      static_cast<long long>( units::to_milliliter( proto->volume ) ),
                      static_cast<long long>( units::to_cent( proto->price ) ),
                      harness_escape( proto->category_force.str() ).c_str() );
        bool first_material = true;
        for( const std::pair<const material_id, int> &entry : proto->materials ) {
            std::fprintf( out, "%s[\"%s\",%d]", first_material ? "" : ",",
                          harness_escape( entry.first.str() ).c_str(), entry.second );
            first_material = false;
        }
        std::fprintf( out, "],\"armor_data\":[" );
        if( proto->armor ) {
            bool first_row = true;
            for( const armor_portion_data &row : proto->armor->data ) {
                std::fprintf( out, "%s{\"covers\":[", first_row ? "" : "," );
                bool first_cover = true;
                for( const bodypart_str_id &bp : row.covers.value() ) {
                    std::fprintf( out, "%s\"%s\"", first_cover ? "" : ",",
                                  harness_escape( bp.str() ).c_str() );
                    first_cover = false;
                }
                std::fprintf( out, "],\"materials\":[" );
                bool first_row_mat = true;
                for( const part_material &mat : row.materials ) {
                    std::fprintf( out, "%s[\"%s\",%.4f,%d]", first_row_mat ? "" : ",",
                                  harness_escape( mat.id.str() ).c_str(),
                                  static_cast<double>( mat.thickness ), mat.cover );
                    first_row_mat = false;
                }
                std::fprintf( out, "]}" );
                first_row = false;
            }
        }
        std::fprintf( out, "],\"flags\":[" );
        bool first_flag = true;
        for( const flag_id &flag : proto->get_flags() ) {
            std::fprintf( out, "%s\"%s\"", first_flag ? "" : ",",
                          harness_escape( flag.str() ).c_str() );
            first_flag = false;
        }
        // What a fire needs to know beyond materials: whether it goes off, and whether its ammo
        // cooks off, which is `item::will_explode_in_fire` (`src/item.cpp:13314-13325`).
        // What eating it does, for the comestibles (`itype.cpp:151-159` and the ingest rules).
        std::fprintf( out, "],\"nutr\":%d,\"quench\":%d,\"charges_to_use\":%d,"
                      "\"explode_in_fire\":%s,\"cookoff\":%s,\"special_cookoff\":%s}",
                      proto->comestible ? proto->comestible->get_default_nutr() : 0,
                      proto->comestible ? proto->comestible->quench : 0,
                      proto->charges_to_use(),
                      proto->explode_in_fire ? "true" : "false",
                      proto->ammo && proto->ammo->cookoff ? "true" : "false",
                      proto->ammo && proto->ammo->special_cookoff ? "true" : "false" );
        first = false;
    }
    std::fprintf( out, "]}\n" );
    std::fclose( out );
}

bool start_headless_game( const std::string &world_name, const std::string &start_loc,
                          std::string &error )
{
    // The reference's own tests stop before this point and build a flat synthetic
    // world instead (tests/test_main.cpp:88-168), so this follows the real new-game
    // path as far as it can go without input; see docs/parts/headless-start.md.
    // `avatar::create` writes a character template, so the directory has to
    // exist before it runs; without it the reference fails writing the file and
    // opens an error prompt (newcharacter.cpp:870 -> save_template).
    assure_dir_exist( PATH_INFO::templatedir() );

    world_generator->set_active_world( nullptr );
    world_generator->init();
    std::vector<mod_id> mods;
    mods.emplace_back( "dda" );
    WORLD *world = world_generator->make_new_world( world_name, mods );
    if( world == nullptr ) {
        error = "could not create world " + world_name;
        return false;
    }
    world_generator->set_active_world( world );

    // Calendar settings must follow the options before any turn happens.
    calendar::set_eternal_season( get_option<bool>( "ETERNAL_SEASON" ) );
    calendar::set_season_length( get_option<int>( "SEASON_LENGTH" ) );

    g->setup();
    get_avatar() = avatar();
    get_avatar().create( character_type::NOW );
    // The id is assigned by start_game itself; assigning one here makes the
    // reference complain that the character already has one.

    // Fixed start location, so the avatar's birth square is not a random draw.
    if( !start_loc.empty() ) {
        const start_location_id loc( start_loc );
        if( loc.is_valid() ) {
            get_avatar().random_start_location = false;
            get_avatar().start_location = loc;
        } else {
            error = "unknown start location " + start_loc;
            return false;
        }
    }

    if( !g->harness_start_game() ) {
        error = "start_game refused";
        return false;
    }
    return true;
}

void probe_mapgen( const std::string &oter_id )
{
    map &here = get_map();
    const string_id<oter_t> wanted( oter_id );
    const std::string mapgen_id = wanted->get_mapgen_id();
    // A real overmap position, because a dummy mapgendata builds nothing (see docs/11).
    const tripoint_abs_omt omt = project_to<coords::omt>( get_player_character().pos_abs() );
    const auto actual = overmap_buffer.ter( omt );
    std::fprintf( stderr, "probe: oter=%s mapgen=%s at_omt=%s\n", oter_id.c_str(),
                  mapgen_id.c_str(), actual->get_mapgen_id().c_str() );
    mapgendata dat( omt, here, 0.0f, calendar::turn, nullptr );
    if( !run_mapgen_func( mapgen_id, dat ) ) {
        std::fprintf( stderr, "probe: run_mapgen_func returned false\n" );
        return;
    }
    std::fprintf( stderr, "probe: row 0:" );
    for( int x = 0; x < 24; x++ ) {
        std::fprintf( stderr, " %s", here.ter( tripoint_bub_ms( x, 0, 0 ) ).id().str().c_str() );
    }
    std::fprintf( stderr, "\n" );
}

void dump_frame( const std::string &path, int radius )
{
    std::FILE *out = std::fopen( path.c_str(), "wb" );
    if( out == nullptr ) {
        std::fprintf( stderr, "harness: cannot write %s\n", path.c_str() );
        return;
    }
    const avatar &u = get_avatar();
    const tripoint_bub_ms centre = u.pos_bub();
    map &here = get_map();
    std::fprintf( out, "{\"harness\":\"frame\",\"version\":1,\"radius\":%d,\n", radius );
    std::fprintf( out, "\"centre\":[%d,%d],\"squares\":[\n", centre.x(), centre.y() );
    bool first = true;
    for( int dy = -radius; dy <= radius; dy++ ) {
        for( int dx = -radius; dx <= radius; dx++ ) {
            const tripoint_bub_ms p( centre.x() + dx, centre.y() + dy, centre.z() );
            const ter_t &terrain = here.ter( p ).obj();
            const furn_t &furniture = here.furn( p ).obj();
            // What a player sees is the terrain, or the furniture standing on it, or the item lying
            // there, or the field - in that order, which is the order the interface draws them.
            std::string glyph = utf32_to_utf8( terrain.symbol() );
            if( !furniture.id.is_null() && furniture.symbol() != 0 ) {
                glyph = furniture.symbol();
            }
            const map_stack items = here.i_at( p );
            if( !items.empty() && !items.begin()->symbol().empty() ) {
                glyph = items.begin()->symbol();
            }
            std::fprintf( out,
                          "%s{\"x\":%d,\"y\":%d,\"glyph\":\"%s\",\"ter\":\"%s\","
                          "\"furn\":\"%s\",\"items\":%d,\"light\":%.6f}",
                          first ? "" : ",\n", p.x(), p.y(), harness_escape( glyph ).c_str(),
                          harness_escape( terrain.id.str() ).c_str(),
                          harness_escape( furniture.id.str() ).c_str(),
                          static_cast<int>( items.size() ),
                          static_cast<double>( here.ambient_light_at( p ) ) );
            first = false;
        }
    }
    std::fprintf( out, "\n]}\n" );
    std::fclose( out );
}

void dump_state( const std::string &path )
{
    std::FILE *out = std::fopen( path.c_str(), "wb" );
    if( out == nullptr ) {
        std::fprintf( stderr, "harness: cannot write %s\n", path.c_str() );
        return;
    }
    const avatar &u = get_avatar();
    const time_point now = calendar::turn;

    cata_default_random_engine probe = rng_get_engine();
    std::fprintf( out, "{\"harness\":\"state\",\"version\":1,\n" );
    std::fprintf( out, "\"rng_next\":[" );
    for( int i = 0; i < 4; i++ ) {
        std::fprintf( out, "%s%llu", i == 0 ? "" : ",",
                      static_cast<unsigned long long>( probe() ) );
    }
    std::fprintf( out, "],\n" );

    std::fprintf( out,
                  "\"turn\":%d,\"season\":%d,\"day_of_season\":%d,\"hour\":%d,\n",
                  to_turn<int>( now ), static_cast<int>( season_of_year( now ) ),
                  day_of_season<int>( now ), hour_of_day<int>( now ) );

    const tripoint_abs_ms pos = u.pos_abs();
    std::fprintf( out,
                  "\"avatar\":{\"pos_abs\":[%d,%d,%d],\"str\":%d,\"dex\":%d,\"int\":%d,"
                  "\"per\":%d,\"hp\":%d,\"moves\":%d},\n",
                  pos.x(), pos.y(), pos.z(), u.str_max, u.dex_max, u.int_max, u.per_max,
                  static_cast<int>( u.get_hp() ), static_cast<int>( u.get_moves() ) );

    // The reality bubble, hashed tile by tile: terrain, furniture, and how many
    // items lie on the square. This is the first digest that compares worlds
    // rather than environments.
    std::vector<std::string> rows;
    const tripoint_bub_ms center = u.pos_bub();
    for( const tripoint_bub_ms &p : g->harness_map().points_in_radius( center, 60 ) ) {
        std::string row = string_format( "%d,%d,%d|%s|%s|%zu", p.x(), p.y(), p.z(),
                                         g->harness_map().ter( p ).id().str(), g->harness_map().furn( p ).id().str(),
                                         g->harness_map().i_at( p ).size() );
        rows.push_back( std::move( row ) );
    }
    std::fprintf( out,
                  "\"bubble\":{\"radius\":60,\"tiles\":%zu,\"digest\":\"%016llx\"},\n",
                  rows.size(),
                  static_cast<unsigned long long>( digest_strings( rows ) ) );

    std::size_t creatures = 0;
    for( Creature &critter : g->all_creatures() ) {
        static_cast<void>( critter );
        creatures++;
    }
    std::fprintf( out, "\"creatures\":%zu}\n", creatures );
    std::fclose( out );
}

} // namespace harness

namespace harness
{

// Every resolved monster type, with the fields the port loads first.
void dump_monsters( const std::string &path )
{
    std::FILE *out = std::fopen( path.c_str(), "wb" );
    if( out == nullptr ) {
        std::fprintf( stderr, "harness: cannot open %s\n", path.c_str() );
        return;
    }
    std::fprintf( out, "{\"harness\":\"monsters\",\"version\":1,\n\"prototypes\":[\n" );
    bool first = true;
    for( const mtype &proto : MonsterGenerator::generator().get_all_mtypes() ) {
        std::fprintf( out, "%s\n{\"id\":\"%s\",\"name\":\"%s\",\"symbol\":\"%s\",\"color\":\"%s\","
                      "\"bodytype\":\"%s\",\"hp\":%d,\"speed\":%d,\"diff\":%d,\"melee_skill\":%d,"
                      "\"dodge\":%d,\"morale\":%d,\"aggression\":%d,\"weight\":%lld,\"volume\":%lld,"
                      "\"species\":[",
                      first ? "" : ",", harness_escape( proto.id.str() ).c_str(),
                      harness_escape( proto.nname( 1 ) ).c_str(),
                      harness_escape( proto.sym ).c_str(),
                      string_from_color( proto.color ).c_str(),
                      harness_escape( proto.bodytype ).c_str(),
                      proto.hp, proto.speed, proto.difficulty_base, proto.melee_skill,
                      proto.sk_dodge, proto.morale, proto.agro,
                      static_cast<long long>( units::to_gram( proto.weight ) ),
                      static_cast<long long>( units::to_milliliter( proto.volume ) ) );
        bool first_species = true;
        for( const species_id &entry : proto.species ) {
            std::fprintf( out, "%s\"%s\"", first_species ? "" : ",",
                          harness_escape( entry.str() ).c_str() );
            first_species = false;
        }
        std::fprintf( out, "],\"categories\":[" );
        bool first_category = true;
        for( const std::string &entry : proto.categories ) {
            std::fprintf( out, "%s\"%s\"", first_category ? "" : ",",
                          harness_escape( entry ).c_str() );
            first_category = false;
        }
        std::fprintf( out, "],\"materials\":[" );
        bool first_material = true;
        for( const std::pair<const material_id, int> &entry : proto.mat ) {
            std::fprintf( out, "%s[\"%s\",%d]", first_material ? "" : ",",
                          harness_escape( entry.first.str() ).c_str(), entry.second );
            first_material = false;
        }
        // `creature_size` has no `enum_to_string` specialisation linked into this build, so the size is named
        // here rather than guessed at with a cast.
        const char *size_name = "medium";
        switch( proto.size ) {
            case creature_size::tiny:  size_name = "tiny";  break;
            case creature_size::small: size_name = "small"; break;
            case creature_size::medium: size_name = "medium"; break;
            case creature_size::large: size_name = "large"; break;
            case creature_size::huge:  size_name = "huge";  break;
            default: break;
        }
        std::fprintf( out, "],\"harvest\":\"%s\",\"dissect\":\"%s\",\"size\":\"%s\"}",
                      harness_escape( proto.harvest.str() ).c_str(),
                      harness_escape( proto.dissect.str() ).c_str(),
                      size_name );
        first = false;
    }
    std::fprintf( out, "\n]}\n" );
    std::fclose( out );
}

}  // namespace harness
