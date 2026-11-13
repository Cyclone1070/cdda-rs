//! The five counts that move together, in one place.
//!
//! The dialogue censuses measure one body of data from four directions: how many programs the port can read, how many of those it can run, how many
//! members refused, how many programs are one member away from running, and how many of the data's effects it can carry out. Porting **one** effect
//! moves all of them, so a count hardcoded beside each assertion turns one feature into a dozen edits and a dozen gate runs.
//!
//! The counts live here and the censuses read them. Moving a count is one line; the censuses keep their invariants - everything is either runnable or
//! refused, every refusal names something real, every program is readable or explains why not - which is what a census is for.

/// What the dialogue census counted this time round.
///
/// Reference for what each one means: `docs/13-test-inventory.md` and the header of `eoc_programs.rs`.
#[derive(Debug, Clone, Copy)]
pub struct Census {
    /// Programs the data names.
    pub defined: usize,
    /// Programs the reader can understand whole.
    pub readable: usize,
    /// Readable programs that also ran to completion in a fresh world.
    pub ran: usize,
    /// Of those, the ones that changed nothing the world can see: entities, lines said, variables written.
    pub silent: usize,
    /// Of the silent ones, how many the data gives no effects at all.
    pub empty_programs: usize,
    /// Readable programs that ran in a world with terrain, an overmap and a player who answers - what a game has.
    pub ran_in_a_built_world: usize,
    /// Readable programs that refused at run time.
    pub refused_at_run: usize,
    /// Unreadable members across every program.
    pub leaves: usize,
    /// Programs exactly one member short of readable.
    pub one_gap: usize,
    /// Effect entries the reader can carry out.
    pub effects_runnable: usize,
    /// Effect entries the data holds.
    pub effects_defined: usize,
    /// Programs whose first unreadable member is their effect list.
    pub effect_blamed: usize,
    /// Programs whose first unreadable member is a math expression.
    /// Conditions the reader can answer.
    pub conditions_readable: usize,
}

/// The census as of the mapgen-update aiming port.
///
/// Every one of these was measured, not estimated: `CDDA_SHOW_MAPGEN`-style prints and the census's own `println!` report them at run time, and these
/// constants are the numbers those prints last said.
pub const NOW: Census = Census {
    defined: 692,
    // 584 after round 1049: `offer_mission` is a reader the data's programs were waiting on, so the readable count rises with the leaf count falling.
    // **598 after round 1430 read `u_activate_trait`**, which joins the mutation-activation rule to the program runner: one program stops refusing at its first entry, and the
    // effect census moves with it (1,580 -> 1,581) because the same reader is counted on both sides.
    // **599 after round 1432 read `u_emit`**, which is the same rule a spell already used, reached through a location variable.
    // **600 after round 1439 read `reveal_map`**, whose radius is a `dbl_or_var` the port already had a reader for: one program gained. **601 after round 1484** taught
    // `search_data` the `condition` member and the alternative semantics, which is the beggar program that had been blamed on `false_eocs`.
    // **602 after round 1487 read `trigger_event`**, which closes the census's `args+trigger_event` bucket: the effect reads a type's name and its `args`, and the
    // argument order is the dumped table's.
    // **603 after round 1489 read `has_ammo`**, the second of the dialogue census's five refusals to close.
    // **604 after round 1490 read `u_sell_item`**, the third of the census's refusals to close.
    // **605 after round 1491 read `u_map_run_item_eocs`**: the census's last effect refusal closes, leaving only the `u_has_camp` condition.
    // **606 after round 1492 read `u_has_camp`**, the last of the dialogue census's refusals: **zero programs now refuse at their first member**.
    // **608 after round 1493 answered `u_is_on_terrain` and `npc_is_character`**, two conditions the census had one member from.
    // **611 after round 1494 answered `u_has_species` and `npc_allies`/`npc_allies_global`**.
    // **692 of 692 after round 1498 wrote `transform_line`**: every program in the data reads whole, and the refusal map is empty. (`talk_effect_t::load_effect`): the last member a program was one gap from is `npc_is_underwater`.
    readable: 692,
    ran: 481,
    // Of those, how many answered `Done` without changing anything the world can see: four in five of them, because a program whose condition is
    // false for a fresh world legitimately does nothing - and a program whose condition could not be READ does nothing too, and used to count as a run.
    silent: 360,
    // Silent because the data gives the program no effects at all - a stub, not a rule the port is missing.
    empty_programs: 23,
    // The same programs run in a world that has terrain, an overmap and a player who answers, which is what a game has.
    // 404 after round 1493.
    // 407 after round 1494.
    // 411 after round 1495.
    ran_in_a_built_world: 486,
    // 205 after round 1439: the program `reveal_map` unblocked reads but refuses at run, because a radius the data draws needs dice a fresh world has not got.
    // 206 after round 1484: the beggar program reads now and refuses when it runs, because the condition it filters on asks a vitamin the item's own table has not got.
    // 207 after round 1489: the `has_ammo` program reads now and refuses at run, because the item it asks about is one a fresh world has not got in hand.
    // 208 after round 1491: the map-item program reads now and refuses at run, because its only data use is a menu.
    // 209 after round 1493.
    // 206 after round 1495: four programs the stored-condition path used to stop now run.
    refused_at_run: 211,
    // 74 leaves and 1,547 effects after round 1048 read the `npc_mutate_category` spelling; **73 and 1,548** after round 1049 read `offer_mission`.
    // Each reader that lands moves three numbers, and one of them may rise - see the runs' own comments.
    // 31 after round 1430: the `u_activate_trait` reader took one leaf with it.
    // 21 after round 1432: the `u_emit` reader took nine leaves with it, because the effect is used by nine programs' worth of entries.
    // 20 and 12 after round 1439: `reveal_map`'s radius was the last leaf in its program and the only gap in one more.
    // 19 after round 1484: the beggar program's `search_data` reads, so its last unreadable member is gone.
    // 18 after round 1487: the trigger_event reader took one leaf with it.
    // 17 after round 1489: the `has_ammo` reader took one leaf with it.
    // 15 after round 1490: `u_sell_item` and the `count` it reads are two more members the reader knows.
    // 14 after round 1491.
    // 13 after round 1492.
    // 11 after round 1493.
    // 8 after round 1494.
    // 7 after round 1495.
    // 6 after round 1496: `npc_is_underwater` was the last member a program was one gap from.
    leaves: 0,
    // 11 after round 1484: the program that stopped refusing at its `search_data` was one member short of reading, so the gap closed with it.
    // 2 after round 1494: `get_condition` and `npc_is_underwater`.
    // **0 after round 1496**: no program in the data is one member from reading whole.
    one_gap: 0,
    // 1,585 after round 1487: `trigger_event` is one more entry the reader can carry out, and the entry it unblocked is another.
    // 1,586 after round 1489: the `has_ammo` condition is one more entry the reader can carry out.
    // 1,587 after round 1490: `u_sell_item` is one more entry the reader can carry out.
    // 1,589 after round 1491: the map-item search is one more entry the reader can carry out.
    // 1,590 after round 1492: the `if` whose condition was `u_has_camp` is an entry the reader can now carry out.
    effects_runnable: 1_590,
    effects_defined: 1_612,
    // 85 after round 1439: the same program no longer blames an effect, because its first refusal is now past the effect list.
    // 84 after round 1484: same program, one fewer entry refused at its effect list.
    // 83 after round 1487: one fewer program blames its effect list, because that list now reads.
    // 82 after round 1489: one fewer program blames its effect list.
    // 81 after round 1490.
    // 80 after round 1491.
    // 79 after round 1492.
    // 80 at round 1496: programs whose first unreadable member is inside `effect`, which is the frontier the census now points at.
    effect_blamed: 0,
    // 3,228 after round 1489: `has_ammo` is one more condition every one of the data's condition sites can now read.
    // 3,240 after round 1492: `u_has_camp` is one more condition every condition site can read.
    // 3,242 after round 1493: two more conditions every condition site can read.
    // 3,269 after round 1494: `u_has_species` and the two ally counts are read wherever the data asks them.
    // 3,270 after round 1495: `get_condition` is one more condition the data's sites can read.
    // 3,272 after round 1496: `u_is_underwater` and its `npc_` pair are two more condition sites.
    conditions_readable: 3_272,
};

impl Census {
    /// The invariant every census shares, whatever the numbers are: nothing is unaccounted for.
    ///
    /// A count that fails this is not out of date, it is **wrong** - and the difference matters, because an out-of-date count means "port more", while a
    /// broken invariant means "the census is measuring something other than what it says".
    pub fn accounts_for_every_program(self) {
        assert_eq!(
            self.ran + self.refused_at_run,
            self.readable,
            "every readable program either ran or refused: {self:?}"
        );
        assert!(
            self.readable <= self.defined,
            "a program cannot be readable before it is defined: {self:?}"
        );
        assert!(
            self.ran <= self.readable,
            "a program cannot run before it can be read: {self:?}"
        );
        assert!(
            self.one_gap <= self.defined,
            "one-gap programs are programs: {self:?}"
        );
        assert!(
            self.effects_runnable <= self.effects_defined,
            "an effect cannot be carried out before it is defined: {self:?}"
        );
    }
}
