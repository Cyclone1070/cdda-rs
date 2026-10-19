#include "harness.h"

#include <algorithm>

#include "avatar.h"
#include "calendar.h"
#include "game.h"
#include "map.h"
#include "enums.h"
#include "options.h"
#include "rng.h"
#include "start_location.h"
#include "string_id.h"
#include "worldfactory.h"
#include <cstdio>
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

std::string json_escape( const std::string &value )
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
                      first ? "" : ",\n", json_escape( entry.first ).c_str(),
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
                      json_escape( entry.first ).c_str() );
        bool first_id = true;
        for( const std::string &id : entry.second ) {
            std::fprintf( out, "%s\"%s\"", first_id ? "" : ",", json_escape( id ).c_str() );
            first_id = false;
        }
        std::fprintf( out, "]}" );
        first_registry = false;
    }
    std::fprintf( out, "\n]}\n" );
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

bool start_headless_game( const std::string &world_name, const std::string &start_loc,
                          std::string &error )
{
    // The reference's own tests stop before this point and build a flat synthetic
    // world instead (tests/test_main.cpp:88-168), so this follows the real new-game
    // path as far as it can go without input; see docs/parts/headless-start.md.
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
    get_avatar().setID( g->assign_npc_id(), false );

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
