# Zone types, as the reference holds them

`all.json` is `--dump-zone-types`: every `zone_type` the game holds after loading the dda set — 64 of them — with the name, description, display field and the two flags
the interface and the ownership rules read.

The registry is JSON, but its tag does not read the way it looks: the data writes zone types with `"type": "LOOT_ZONE"` (`src/init.cpp:370`), not `"zone_type"`, so a
loader that looks for its own name finds nothing and returns an empty registry without an error.

| Range | Value |
|---|---|
| Zone types | 64 |
| In `data/json/zones.json` | 21 |
| The rest | other data files and the mods the dda set pulls in |
| May be owned by a character | 42 |
| Hidden from the interface | 1 (`LOOT_ITEM_GROUP`) |

Regenerate with:

    ./oracle/build/src/cataclysm --datadir oracle/src/data/ --userdir oracle/userdir \
        --seed 12345 --dump-zone-types corpus/zones/all.json < /dev/null
