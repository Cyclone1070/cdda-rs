// Generates the randomness corpus the port's generator must reproduce.
//
// The reference draws from the standard library's generator and distributions
// (`src/rng.h:30` names `std::minstd_rand0`; `src/rng.cpp:16-36` draws through
// default-constructed `std::uniform_int_distribution` and friends). Those
// distributions are implementation-defined, so the corpus has to come from the
// same standard library the reference is built with — which is what this probe
// uses, and nothing else.
//
// Build:  clang++ -std=c++17 -O1 rng_corpus.cpp -o rng_corpus
// Run:    ./rng_corpus > ../../corpus/rng/engine.csv

#include <cstdint>
#include <cstdio>
#include <random>

namespace
{
constexpr int draws_per_case = 16;
const unsigned int seeds[] = { 1u, 42u, 12345u, 4294967295u };

std::minstd_rand0 fresh( unsigned int seed )
{
    return std::minstd_rand0( seed );
}

void emit_header( const char *name, unsigned int seed )
{
    std::printf( "%s,%u", name, seed );
}
} // namespace

int main()
{
    std::printf( "# engine=minstd_rand0 draws_per_case=%d columns=case,seed,draw...\n",
                 draws_per_case );

    for( unsigned int seed : seeds ) {
        // Raw engine output: minstd_rand0::operator().
        {
            emit_header( "engine_raw", seed );
            std::minstd_rand0 eng = fresh( seed );
            for( int i = 0; i < draws_per_case; i++ ) {
                std::printf( ",%llu", static_cast<unsigned long long>( eng() ) );
            }
            std::printf( "\n" );
        }
        // std::uniform_int_distribution<int>( lo, hi ), as src/rng.cpp:26 uses it.
        {
            const int lo = -10;
            const int hi = 10;
            emit_header( "uniform_int_-10_10", seed );
            std::minstd_rand0 eng = fresh( seed );
            std::uniform_int_distribution<int> dist;
            const std::uniform_int_distribution<int>::param_type param( lo, hi );
            for( int i = 0; i < draws_per_case; i++ ) {
                std::printf( ",%d", dist( eng, param ) );
            }
            std::printf( "\n" );
        }
        // std::uniform_int_distribution<unsigned int>() over its full range,
        // as src/rng.cpp:16 uses it.
        {
            emit_header( "uniform_uint_full", seed );
            std::minstd_rand0 eng = fresh( seed );
            std::uniform_int_distribution<unsigned int> dist;
            for( int i = 0; i < draws_per_case; i++ ) {
                std::printf( ",%u", dist( eng ) );
            }
            std::printf( "\n" );
        }
        // std::uniform_real_distribution<double>( 0, 1 ), src/rng.cpp:31.
        {
            emit_header( "uniform_real_0_1", seed );
            std::minstd_rand0 eng = fresh( seed );
            std::uniform_real_distribution<double> dist;
            for( int i = 0; i < draws_per_case; i++ ) {
                std::printf( ",%.17g", dist( eng ) );
            }
            std::printf( "\n" );
        }
        // std::normal_distribution<>( 0, 1 ) — note the reference caches the
        // second value of each pair, so the stream alternates between computing
        // and returning a stored value.
        {
            emit_header( "normal_0_1", seed );
            std::minstd_rand0 eng = fresh( seed );
            std::normal_distribution<> dist;
            for( int i = 0; i < draws_per_case; i++ ) {
                std::printf( ",%.17g", dist( eng ) );
            }
            std::printf( "\n" );
        }
    }
    return 0;
}
