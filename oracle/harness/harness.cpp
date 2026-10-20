#include "harness.h"

#include <algorithm>

#include "avatar.h"
#include "calendar.h"
#include "do_turn.h"
#include "game.h"
#include "map.h"
#include "enums.h"
#include "item_factory.h"
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
                      "\"latent_heat\":%.6g}",
                      first ? "" : ",", harness_escape( material.id.str() ).c_str(),
                      harness_escape( material.name() ).c_str(),
                      static_cast<double>( material.density() ), material.chip_resist(),
                      material.breathability(),
                      static_cast<double>( material.specific_heat_liquid() ),
                      static_cast<double>( material.specific_heat_solid() ),
                      static_cast<double>( material.latent_heat() ) );
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
        std::fprintf( out, "%s\n{\"id\":\"%s\",\"priority\":%d,\"levels\":[",
                      first ? "" : ",", harness_escape( field.id.str() ).c_str(), field.priority );
        bool first_level = true;
        for( const field_intensity_level &level : field.intensity_levels ) {
            std::fprintf( out, "%s\n{\"name\":\"%s\",\"sym\":%u,\"color\":\"%s\","
                          "\"dangerous\":%s,\"transparent\":%s,\"move_cost\":%d,"
                          "\"concentration\":%d,\"light\":%.6g}",
                          first_level ? "" : ",", harness_escape( level.name.translated() ).c_str(),
                          level.symbol, string_from_color( level.color ).c_str(),
                          level.dangerous ? "true" : "false",
                          level.transparent ? "true" : "false", level.move_cost,
                          level.concentration, static_cast<double>( level.light_emitted ) );
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
        std::fprintf( out, "]}" );
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
        std::fprintf( out, "]}" );
        first = false;
    }
    std::fprintf( out, "\n]}\n" );
    std::fclose( out );
}

}  // namespace harness
