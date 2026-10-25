# Map and mapgen recordings

## `examine-actions.json` — what examining a square does

`--dump-examine-actions` writes one entry per terrain or furniture prototype whose `"examine_action"` is not empty:

```sh
oracle/build/src/cataclysm --userdir oracle/userdir --datadir "$PWD/oracle/src/data/" \
    --world harness-fixed --seed 12345 --dump-examine-actions "$PWD/corpus/maps/examine-actions.json"
```

73 of the 77 names in the content are **bare strings** — a hardcoded function with no data — and 4 are **objects** whose members the loader reads. The name of a bare
one is recovered by **pointer identity** against the reference's own name table, because the reference keeps no name beside its function pointer; the harness gained a
one-line accessor for that table rather than a second copy of it.
