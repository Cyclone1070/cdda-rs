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
    // **600 after round 1439 read `reveal_map`**, whose radius is a `dbl_or_var` the port already had a reader for: one program gained.
    readable: 600,
    ran: 395,
    // Of those, how many answered `Done` without changing anything the world can see: four in five of them, because a program whose condition is
    // false for a fresh world legitimately does nothing - and a program whose condition could not be READ does nothing too, and used to count as a run.
    silent: 289,
    // Silent because the data gives the program no effects at all - a stub, not a rule the port is missing.
    empty_programs: 22,
    // The same programs run in a world that has terrain, an overmap and a player who answers, which is what a game has.
    ran_in_a_built_world: 400,
    // 205 after round 1439: the program `reveal_map` unblocked reads but refuses at run, because a radius the data draws needs dice a fresh world has not got.
    refused_at_run: 205,
    // 74 leaves and 1,547 effects after round 1048 read the `npc_mutate_category` spelling; **73 and 1,548** after round 1049 read `offer_mission`.
    // Each reader that lands moves three numbers, and one of them may rise - see the runs' own comments.
    // 31 after round 1430: the `u_activate_trait` reader took one leaf with it.
    // 21 after round 1432: the `u_emit` reader took nine leaves with it, because the effect is used by nine programs' worth of entries.
    // 20 and 12 after round 1439: `reveal_map`'s radius was the last leaf in its program and the only gap in one more.
    leaves: 20,
    one_gap: 12,
    effects_runnable: 1_583,
    effects_defined: 1_612,
    // 85 after round 1439: the same program no longer blames an effect, because its first refusal is now past the effect list.
    effect_blamed: 85,
    conditions_readable: 3_227,
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
