#include "harness.h"

#include <algorithm>

#include "calendar.h"
#include "options.h"
#include "rng.h"
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

} // namespace harness
