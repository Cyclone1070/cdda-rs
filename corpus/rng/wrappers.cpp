// Reproduces the reference's random-number wrappers with the standard library
// the reference links against (libc++), so the port can be held to the same
// numbers. Build and run:
//
//   clang++ -std=c++17 -O0 corpus/rng/wrappers.cpp -o /tmp/wrappers
//   /tmp/wrappers > corpus/rng/wrappers.csv
//
// Every line is `name,value`. The engine is `std::minstd_rand0`, seeded with
// 12345, and the distributions are the default-constructed ones `src/rng.cpp`
// keeps in function-local statics.

#include <algorithm>
#include <cmath>
#include <cstdio>
#include <random>
#include <string>
#include <vector>

using engine = std::minstd_rand0;

static engine &rng_get_engine()
{
    static engine eng( 12345 );
    return eng;
}

static int rng( int lo, int hi )
{
    static std::uniform_int_distribution<int> dist;
    if( lo > hi ) {
        std::swap( lo, hi );
    }
    return dist( rng_get_engine(), std::uniform_int_distribution<>::param_type( lo, hi ) );
}

static double rng_float( double lo, double hi )
{
    static std::uniform_real_distribution<double> dist;
    if( lo > hi ) {
        std::swap( lo, hi );
    }
    return dist( rng_get_engine(), std::uniform_real_distribution<>::param_type( lo, hi ) );
}

static double normal_roll( double mean, double stddev )
{
    static std::normal_distribution<double> dist;
    return dist( rng_get_engine(), std::normal_distribution<>::param_type( mean, stddev ) );
}

static double rng_normal( double lo, double hi )
{
    if( lo > hi ) {
        std::swap( lo, hi );
    }
    const double range = ( hi - lo ) / 4;
    if( range == 0.0 ) {
        return hi;
    }
    const double val = normal_roll( ( hi + lo ) / 2, range );
    return std::max( lo, std::min( hi, val ) );
}

static unsigned int rng_bits()
{
    static std::uniform_int_distribution<unsigned int> dist;
    return dist( rng_get_engine() );
}

static bool one_in( int chance )
{
    return chance <= 1 || rng( 0, chance - 1 ) == 0;
}

static bool x_in_y( double x, double y )
{
    return rng_float( 0.0, 1.0 ) <= x / y;
}

static int dice( int number, int sides )
{
    int ret = 0;
    for( int i = 0; i < number; i++ ) {
        ret += rng( 1, sides );
    }
    return ret;
}

static int roll_remainder( double value )
{
    double integ;
    const double frac = modf( value, &integ );
    if( value > 0.0 && value > integ && x_in_y( frac, 1.0 ) ) {
        integ++;
    } else if( value < 0.0 && value < integ && x_in_y( -frac, 1.0 ) ) {
        integ--;
    }
    return static_cast<int>( integ );
}

static std::vector<int> rng_sequence( size_t count, int lo, int hi, int seed )
{
    if( lo > hi ) {
        std::swap( lo, hi );
    }
    std::vector<int> result;
    engine eng( seed );
    std::uniform_int_distribution<int> dist;
    const std::uniform_int_distribution<int>::param_type param( lo, hi );
    for( size_t i = 0; i < count; i++ ) {
        result.push_back( dist( eng, param ) );
    }
    return result;
}

static void print( const char *name, long long value )
{
    std::printf( "%s,%lld\n", name, value );
}

static void print_real( const char *name, double value )
{
    // Twenty significant digits: enough to catch a difference in the last bit.
    std::printf( "%s,%.20g\n", name, value );
}

int main()
{
    // Each call is made once, in this order, against one shared engine, so the
    // file fixes the whole stream rather than isolated values.
    for( int i = 0; i < 8; i++ ) {
        print( "rng.0.100", rng( 0, 100 ) );
    }
    for( int i = 0; i < 4; i++ ) {
        print( "rng.5.5", rng( 5, 5 ) );
    }
    // The reference swaps reversed bounds rather than refusing them.
    print( "rng.reversed", rng( 100, 0 ) );
    print( "rng.negative", rng( -50, -10 ) );
    for( int i = 0; i < 4; i++ ) {
        print_real( "rng_float", rng_float( 0.0, 1.0 ) );
    }
    print_real( "rng_float.offset", rng_float( -2.5, 7.5 ) );
    for( int i = 0; i < 6; i++ ) {
        print_real( "normal_roll.0.1", normal_roll( 0.0, 1.0 ) );
    }
    print_real( "normal_roll.10.2", normal_roll( 10.0, 2.0 ) );
    for( int i = 0; i < 6; i++ ) {
        print_real( "rng_normal.0.10", rng_normal( 0.0, 10.0 ) );
    }
    print_real( "rng_normal.equal", rng_normal( 4.0, 4.0 ) );
    print_real( "rng_normal.reversed", rng_normal( 10.0, 0.0 ) );
    for( int i = 0; i < 4; i++ ) {
        print( "rng_bits", static_cast<long long>( rng_bits() ) );
    }
    for( int i = 0; i < 12; i++ ) {
        print( "one_in.4", one_in( 4 ) );
    }
    print( "one_in.1", one_in( 1 ) );
    print( "one_in.0", one_in( 0 ) );
    for( int i = 0; i < 8; i++ ) {
        print( "x_in_y.1.3", x_in_y( 1.0, 3.0 ) );
    }
    print( "x_in_y.zero", x_in_y( 0.0, 1.0 ) );
    print( "x_in_y.all", x_in_y( 5.0, 5.0 ) );
    for( int i = 0; i < 6; i++ ) {
        print( "dice.3.6", dice( 3, 6 ) );
    }
    print( "dice.0.6", dice( 0, 6 ) );
    print( "roll_remainder.1.3", roll_remainder( 1.3 ) );
    print( "roll_remainder.-2.7", roll_remainder( -2.7 ) );
    print( "roll_remainder.5.0", roll_remainder( 5.0 ) );

    // A sequence from its own engine, leaving the shared one alone.
    for( int value : rng_sequence( 8, 0, 9, 42 ) ) {
        print( "rng_sequence.42", value );
    }
    for( int value : rng_sequence( 4, -3, 3, 7 ) ) {
        print( "rng_sequence.7.reversed", -value );
    }
    return 0;
}
