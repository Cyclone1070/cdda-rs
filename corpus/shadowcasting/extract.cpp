// Prints shadow-casting fixtures from the reference's own caster.
//
// Source: `tests/shadowcasting_test.cpp:33-90` (`oldCastLight`), the copy the
// reference keeps of its own algorithm so it can check the newer one against it.
//
// Two things are deliberately different from that copy, and both are corrections
// after the first attempt produced fixtures that were wrong:
//
//  * the radius is a parameter (5) instead of the reference's 60, because its real
//    map is wider than 60 cells and an 11x11 grid is not;
//  * every write is bounds-checked and aborts the run, so a cast that leaves the
//    grid fails loudly instead of overwriting whatever is next in memory.
//    The first attempt did neither and printed light the algorithm never placed.
//
// Build and run: clang++ -std=c++17 -O0 extract.cpp -o extract && ./extract
#include <cstdio>
#include <cstdlib>
#include <cmath>
#include <algorithm>

static constexpr float SOLID = 0.0f;
static constexpr int N = 11;
static constexpr int C = N / 2;
static constexpr int RADIUS = 5;

static float input[N][N];   // [x][y], as the reference indexes its caches
static float out[N][N];

static int rl_dist( int x, int y )
{
    return std::max( std::abs( x ), std::abs( y ) );
}

static void put( int x, int y )
{
    if( x < 0 || y < 0 || x >= N || y >= N ) {
        std::fprintf( stderr, "extract: cast left the grid at %d,%d\n", x, y );
        std::exit( 1 );
    }
    out[x][y] = 1.0f;
}

static void cast( int xx, int xy, int yx, int yy, int ox, int oy, int od,
                  int row, float start, float end )
{
    float newStart = 0.0f;
    const float radius = static_cast<float>( RADIUS - od );
    if( start < end ) {
        return;
    }
    bool blocked = false;
    for( int distance = row; static_cast<float>( distance ) <= radius && !blocked; distance++ ) {
        const int dy = -distance;
        for( int dx = -distance; dx <= 0; dx++ ) {
            const int cx = ox + dx * xx + dy * xy;
            const int cy = oy + dx * yx + dy * yy;
            const float leftSlope = ( static_cast<float>( dx ) - 0.5f ) /
                                    ( static_cast<float>( dy ) + 0.5f );
            const float rightSlope = ( static_cast<float>( dx ) + 0.5f ) /
                                     ( static_cast<float>( dy ) - 0.5f );
            if( start < rightSlope ) {
                continue;
            } else if( end > leftSlope ) {
                break;
            }
            if( rl_dist( dx, dy ) <= RADIUS ) {
                put( cx, cy );
            }
            if( blocked ) {
                if( input[cx][cy] == SOLID ) {
                    newStart = rightSlope;
                } else {
                    blocked = false;
                    start = newStart;
                }
            } else if( input[cx][cy] == SOLID && distance < RADIUS ) {
                blocked = true;
                cast( xx, xy, yx, yy, ox, oy, od, distance + 1, start, leftSlope );
                newStart = rightSlope;
            }
        }
    }
}

static void scenario( const char *name )
{
    for( int x = 0; x < N; x++ ) {
        for( int y = 0; y < N; y++ ) {
            out[x][y] = 0.0f;
        }
    }
    // Eight segments, not four: the reference casts four for one level and four
    // mirrored for the other (`src/shadowcasting.cpp:545-580`,
    // `cast_horizontal_zlight_segment`). The four I used first left two quadrants
    // dark, which is how the earlier fixtures went wrong a second way.
    const int transforms[8][4] = {
        { 0, 1, 1, 0 }, { 1, 0, 0, 1 }, { 0, -1, 1, 0 }, { -1, 0, 0, 1 },
        { 0, 1, -1, 0 }, { 1, 0, 0, -1 }, { 0, -1, -1, 0 }, { -1, 0, 0, -1 },
    };
    for( const auto &tr : transforms ) {
        cast( tr[0], tr[1], tr[2], tr[3], C, C, 0, 1, 1.0f, 0.0f );
    }
    std::printf( "scenario %s\n", name );
    for( int y = 0; y < N; y++ ) {
        for( int x = 0; x < N; x++ ) {
            const char c = input[x][y] == SOLID ? 'X' : ( out[x][y] > 0 ? '#' : '.' );
            std::putchar( c );
        }
        std::putchar( '\n' );
    }
}

static void reset()
{
    for( int x = 0; x < N; x++ ) {
        for( int y = 0; y < N; y++ ) {
            input[x][y] = 1.0f;
        }
    }
}

int main()
{
    reset();
    scenario( "empty" );

    reset();
    input[C + 2][C] = SOLID;
    scenario( "pillar_east" );

    reset();
    for( int y = 0; y < N; y++ ) {
        input[C + 2][y] = SOLID;
    }
    scenario( "wall_east" );

    reset();
    for( int x = 0; x < N; x++ ) {
        input[x][C + 2] = SOLID;
    }
    scenario( "wall_south" );

    reset();
    for( int d = -2; d <= 2; d++ ) {
        input[C + d][C + 2] = SOLID;
    }
    scenario( "block_near" );
    return 0;
}
