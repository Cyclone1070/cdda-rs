//! The two numbers a trade's price is moved by: a barter skill's curve, and the balance of wits and words between buyer and seller.
//!
//! Reference: `npc_trading::net_price_adjustment` (`npctrade.cpp:143-160`) and `price_adjustment` (`skill.cpp:655-660`). The pair is the **whole** of the reference's opinion of a
//! trade that is not a faction's price rule: the seller's intelligence against the buyer's, the two barter skills against each other, and a reciprocal when the **buyer** is the
//! NPC rather than the seller.
//!
//! **The numbers are arguments, not fields.** The reference reads `int_cur` and `skill_speech` off two `Character`s; the port has no character stats yet, so the rule takes a
//! [`Trader`] and a [`Trader`] and is testable before the stats exist. When the stats do, they fill this in - the rule does not change.

/// What a trade needs to know about one side of it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Trader {
    /// Their intelligence (`int_cur`).
    pub intelligence: i32,
    /// Their speech skill (`skill_speech`), the reference's barter skill.
    pub barter: i32,
    /// Whether this side is a **person** rather than the player.
    pub is_npc: bool,
}

/// The curve a barter-skill difference makes.
///
/// Reference: `price_adjustment( int barter_skill )` (`skill.cpp:655-660`): `val = 0.045 * skill² - 0.025 * skill + 1` on the skill capped at **five**, and the reciprocal when the
/// difference favours the buyer - so a seller better at barter than the buyer asks more, and a better buyer pays less.
#[must_use]
pub fn price_adjustment(barter_skill: i32) -> f64 {
    let skill = f64::from(barter_skill.abs().min(5));
    let val = 0.045 * skill * skill - 0.025 * skill + 1.0;
    if barter_skill >= 0 { val } else { 1.0 / val }
}

/// How far the price swings from value, given both sides.
///
/// Reference: `npc_trading::net_price_adjustment( buyer, seller )` (`npctrade.cpp:143-160`):
/// `int_adj = 1 + 0.05 * min( 19, |seller.int - buyer.int| )`, `soc_adj = price_adjustment( round( seller.barter - buyer.barter ) )`, and
/// `adjust = int_adj * soc_adj` when the **seller** is the cleverer (or equally clever) side, `soc_adj / int_adj` when they are not. The answer is **negated and inverted** when
/// the buyer is the person, because the reference's caller multiplies by `1 + 0.25 * adjust` and a buyer's advantage must move the price the other way.
#[must_use]
pub fn net_price_adjustment(buyer: Trader, seller: Trader) -> f64 {
    let int_diff = seller.intelligence - buyer.intelligence;
    let int_adj = 1.0 + 0.05 * f64::from(int_diff.abs().min(19));
    let soc_adj = price_adjustment(seller.barter - buyer.barter);
    let adjust = if int_diff >= 0 {
        int_adj * soc_adj
    } else {
        soc_adj / int_adj
    };
    if seller.is_npc { adjust } else { -1.0 / adjust }
}
