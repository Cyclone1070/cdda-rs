// The reference's engine and distributions, one case per line, so the port can be held to the numbers the **oracle's** standard library produces.
//
// The standard leaves `uniform_int_distribution`, `uniform_real_distribution` and `normal_distribution` implementation-defined, and libstdc++ and libc++ disagree on
// most draws (`wrappers-libcxx.csv` against `wrappers-gcc.csv`, 64 of 98). The oracle is a gcc build, so this program is built with gcc:
//
//   c++ -std=c++17 -O0 corpus/rng/sequences.cpp -o /tmp/sequences && /tmp/sequences > corpus/rng/engine.csv
//
// Each line is `case,seed,draw,draw,...` with sixteen draws.

#include <cstdio>
#include <random>

using engine = std::minstd_rand0;

template <typename Distribution, typename... Args>
static void emit( const char *name, Distribution &dist, Args... args )
{
    for( unsigned long seed : { 1ul, 42ul, 12345ul, 4294967295ul } ) {
        std::printf( "%s,%lu", name, seed );
        engine eng( seed );
        for( int i = 0; i < 16; i++ ) {
            std::printf( ",%.17g", static_cast<double>( dist( eng, typename Distribution::param_type( args... ) ) ) );
        }
        std::printf( "\n" );
    }
}

int main()
{
    std::printf( "# engine=minstd_rand0 draws_per_case=16 columns=case,seed,draw...\n" );
    for( unsigned long seed : { 1ul, 42ul, 12345ul, 4294967295ul } ) {
        std::printf( "engine_raw,%lu", seed );
        engine eng( seed );
        for( int i = 0; i < 16; i++ ) {
            std::printf( ",%.17g", static_cast<double>( eng() ) );
        }
        std::printf( "\n" );
    }
    std::uniform_int_distribution<int> int_dist;
    emit( "uniform_int_-10_10", int_dist, -10, 10 );
    std::uniform_int_distribution<unsigned int> uint_dist;
    emit( "uniform_uint_full", uint_dist );
    std::uniform_real_distribution<double> real_dist;
    emit( "uniform_real_0_1", real_dist );
    std::normal_distribution<double> normal_dist;
    emit( "normal_0_1", normal_dist, 0.0, 1.0 );
    return 0;
}
