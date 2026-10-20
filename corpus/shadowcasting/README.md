# Shadow-casting fixtures: not yet usable

`expected.txt` was produced by extracting the reference's own caster
(`tests/shadowcasting_test.cpp:33-90`, `oldCastLight`) and printing four 11x11 grids.
**Do not gate anything against it yet.** It is wrong, and the reason is worth keeping.

## What went wrong

The reference's loop runs to `radius = 60 - offsetDistance`, and its real map is
larger than that radius, so the cast never leaves the buffer. My extraction used an
11x11 grid - far smaller than 60 cells - and the C++ side wrote past the end of the
array. C++ does not complain about that; it overwrites whatever is next, and the
printed grids show light where the algorithm never put it.

I caught it because a faithful Rust port disagreed with the fixtures on the simplest
possible case: an empty grid, where the reference's output says every cell is lit and
the port lights only the quadrant that stays inside the buffer. When a fixture
disagrees with a line-by-line port, the fixture is at least as likely to be wrong as
the port, and here it was.

## What to do next time

* Make the radius a parameter, not the reference's 60, and keep it smaller than the
  grid so no cast can leave it.
* Add bounds checking to the extraction program and make it *refuse to write* out of
  range, so an overrun fails loudly instead of inventing light.
* Regenerate `expected.txt`, then port.

The extraction recipe stays in this directory's history; the corrected one should
replace `expected.txt` wholesale rather than patch it.
