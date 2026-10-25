# Character recordings

Dumps that describe a **character** rather than a piece of content.

## `needs.json` — the arithmetic behind the metabolic rates

`--dump-needs` walks 72 character states — six hunger figures from engorged to past famished, three calorie stores from full to half, four activity levels — and for each
one records the state it read (`hunger`, `stored_kcal`, `kcal_percent`, `bodyweight_grams`, `metabolism_base`, `average_activity`, `max_exertion`) and the answer of
**each** of the four functions separately: `get_starvation`, `metabolic_rate`, `base_bmr`, `get_bmr`, plus the assembled `calc_needs_rates`.

`hunger` is the figure the dump **asked** for, not the figure the character had: `set_hunger` caps hunger at 300, so a case that asked for 600 is a character at 300 and
a metabolic multiplier of exactly 1. `bodyweight_grams` is recorded per case because a starving character loses weight, which moves `base_bmr` with it.
