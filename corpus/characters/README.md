# Character recordings

Dumps that describe a **character** rather than a piece of content.

## `needs.json` — the arithmetic behind the metabolic rates

`--dump-needs` walks 72 character states — six hunger figures from engorged to past famished, three calorie stores from full to half, four activity levels — and for each
one records the state it read (`hunger`, `stored_kcal`, `kcal_percent`, `bodyweight_grams`, `metabolism_base`, `average_activity`, `max_exertion`) and the answer of
**each** of the four functions separately: `get_starvation`, `metabolic_rate`, `base_bmr`, `get_bmr`, plus the assembled `calc_needs_rates`.

`hunger` is the figure the dump **asked** for, not the figure the character had: `set_hunger` caps hunger at 300, so a case that asked for 600 is a character at 300 and
a metabolic multiplier of exactly 1. `bodyweight_grams` is recorded per case because a starving character loses weight, which moves `base_bmr` with it.

## `hunger-effect.json` — which hunger effect a body wears

`--dump-hunger-effect` walks 126 states — seven fillings of a 250 mL stomach from empty to full, three ages of the last bite either side of the ten-minute and
three-hour lines, and six body-fat levels from emaciated to obese — and records, per state, the reference's own answers for the body-fat figure, the calorie deficit, the
two volume tests and the effect that ended up installed.

Two of the nine hunger effects never appear, and not because the grid is thin: the fullness ratio is an **integer** division, so `hunger_full` is shadowed by
`hunger_engorged`, and `hunger_starving` is shadowed by `hunger_near_starving`. The grid is built to reach every rung that *is* reachable.
