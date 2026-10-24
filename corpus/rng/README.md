# The standard library's shuffle

`shuffle.json` is 36 permutations: nine run lengths (1 to 9) against four engine seeds (1, 2, 12345, 99991), written by
`--dump-shuffle` from the instrumented reference.

It exists because `put_into_container` shuffles the items it is about to offer a container with **`std::shuffle` over the
game's own engine** (`src/item_group.cpp:127-129`) — not through `rng()`, so no `rng(lo, hi)` hook ever records a die for
it. A port that wants the same permutations has to reproduce libc++'s `__shuffle` **and** its
`uniform_int_distribution<ptrdiff_t>`, which on a 64-bit platform means the `uint64_t` instantiation with its
`__independent_bits_engine`. A permutation is the only observable that pins both at once.

The short runs matter: a run of one is not shuffled at all (the distribution is never asked), and a run of two asks it
exactly once, so the degenerate cases are where an off-by-one in the loop shows up.

Reproduce with:

    ./oracle/build/src/cataclysm --userdir oracle/userdir --datadir "$PWD/oracle/src/data/" \
        --world harness-fixed --seed 12345 --dump-shuffle "$PWD/corpus/rng/shuffle.json"
