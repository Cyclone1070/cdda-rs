# Item corpora

Two dumps and a session, all recorded from the instrumented reference.

| File | Flag | What it holds |
|---|---|---|
| `measures.json` | `--dump-item-measures` | For each of the 9,912 types, an item constructed and **asked** its weight, integral weight, volume and integral volume through `item::weight` and `item::volume` — the rules' answers, not the data's numbers |
| `pockets.json` | `--dump-pockets` | Every type's pockets: kind, capacity, `watertight`, `airtight`, `rigid` — the facts `can_contain` refuses on |
| `pocket-session.json` | `--dump-pocket-session` | A scripted sequence of `item::put_in` calls: what fits in what, why a refusal happened, and what the container weighs, takes up and has room for after every step |

## Recipes

```sh
./oracle/build/src/cataclysm --userdir oracle/userdir --datadir "$PWD/oracle/src/data/" \
  --world harness-fixed --dump-item-measures corpus/items/measures.json
./oracle/build/src/cataclysm --userdir oracle/userdir --datadir "$PWD/oracle/src/data/" \
  --world harness-fixed --dump-pockets corpus/items/pockets.json
./oracle/build/src/cataclysm --userdir oracle/userdir --datadir "$PWD/oracle/src/data/" \
  --world harness-fixed --dump-pocket-session corpus/items/pocket-session.json
```

Both write after the data loads (`check_mod_data`), so the answers are about the real content and not about an empty registry — the first attempt at
`measures.json` came out empty for exactly that reason.

## What a gate may and may not assume

- A **prototype** dump says what the data declares; a **measures** dump says what the rules make of it. A port that copies numbers passes the first and
  fails the second.
- **Zero is a value.** `integral_weight: 0` on a gunmod and `integral_volume: 0` on a pseudo magazine are deliberate; the loaders copy the ordinary number
  only when the field is still unset (`src/item_factory.cpp:394-400`).
- A pocket session records the **decision and its consequence together**. The backpack cannot hold liquid and refuses a bottle with a reason
  (`can_contain`'s phase rule); the port reproduces that refusal from `pockets.json`, and the backpack's capacity is the **sum of its four pockets**
  (25,000 + 510 + 5,000 + 5,000 ml), which is what the session's own room numbers add up to — so the test checks the data against the session rather than
  believing a number it chose itself.
