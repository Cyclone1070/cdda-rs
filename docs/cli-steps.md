# The playtest grammar

Every line of a scenario is one of these. `#` starts a comment; blank lines are ignored. A line that is not in this table is an error that names the line and the word.

Each line is a **real** scenario line with concrete values rather than a template, because the gate below parses these exact words - a document of placeholders would prove only
that the parser rejects them.

This file is part of the test surface: `crates/cli` has a gate that **parses every line below**, so a grammar arm that is removed or changed fails the suite. Adding a step means adding
a line here - the same way adding a `Step` variant means adding an arm in `parse`.

```
world 10 bare
world 10 flammable
seed 1234
scent 5 0
fire 3 3 2 0
turn 10
date 180000
data
survivor 0 0
items
monsters
place field
give crowbar 1
wield crowbar
attack 1 0
throw rock at 5 0
shot my-scenario.txt
goal 0 0
monster mon_zombie 1 0
craft makeshift_knife 10
expect effect winded
expect changed 2
expect fire 3 3
expect no fire 3 3
expect place field
expect item rock at 5 0
expect hurt 3
expect monster within 2
expect no monster at 1 0
expect monster effect spooked at 5 5
expect monsters 2 with effect spooked
expect floor rock 1
expect carried rock 1
```

shoot ruger_mini at 4 0 2
