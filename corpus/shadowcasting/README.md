# Shadow-casting fixtures

`expected.txt` is printed by the reference's own implementation and is safe to gate
against. It is the output of `extract.cpp` in this directory, which is
`tests/shadowcasting_test.cpp:33-90` (`oldCastLight`) - the copy the reference keeps of
its own caster so it can check the newer one against it - driven over five 11x11 grids.

Three things had to be right, and the first attempt got two of them wrong:

1. **The radius is a parameter.** The reference casts 60 cells and its real map is
   wider than that, so the cast never leaves the buffer. An 11x11 grid is not, so the
   extraction uses 5 and every write is bounds-checked: an overrun aborts the run
   instead of overwriting whatever is next in memory. The first attempt printed light
   the algorithm never placed, and the output still looked plausible.
2. **Eight segments, not four.** `src/shadowcasting.cpp:545-580` casts four for one
   level and four mirrored (`cast_horizontal_zlight_segment`). Four leaves two
   quadrants dark, which is what the first fixtures showed.
3. **The origin does not light itself.** The walk starts one step out, so the centre
   cell is dark. That is the reference's behaviour, not a gap in the port.

The Rust port reads `expected.txt` instead of restating it, so there is one copy of
the expected output and a typo cannot pass as a fixture. It agrees on all five
scenarios: empty field, single pillar, two walls, and a five-cell block.

Regenerate with:

    clang++ -std=c++17 -O0 extract.cpp -o extract && ./extract > expected.txt
