//! The numbers a trade's price is moved by.
//!
//! Reference: `npc_trading::net_price_adjustment` (`npctrade.cpp:143-160`) and `price_adjustment` (`skill.cpp:655-660`).

#![allow(clippy::expect_used)]

/// The barter curve, at the values a player can reach in one character: `0.045 * skill² - 0.025 * skill + 1` on the skill capped at five, reciprocal when it favours the buyer.
///
/// The cap is the whole subtlety: a difference of **five** and a difference of **fifty** answer the same number, and the sign of the difference decides whether the value is
/// used or inverted.
#[test]
fn the_barter_curve_rises_then_caps_and_inverts() {
    let close = |a: f64, b: f64| (a - b).abs() < 1e-9;
    assert!(
        close(cdda_sim::trade::price_adjustment(0), 1.0),
        "**no difference, no change**"
    );
    assert!(
        close(cdda_sim::trade::price_adjustment(1), 0.045 - 0.025 + 1.0),
        "**one point of barter is worth 1.02**"
    );
    assert!(
        close(
            cdda_sim::trade::price_adjustment(5),
            0.045 * 25.0 - 0.125 + 1.0
        ),
        "**five points cap the curve at 1.0**"
    );
    assert!(
        close(
            cdda_sim::trade::price_adjustment(50),
            cdda_sim::trade::price_adjustment(5)
        ),
        "**fifty is the same as five, because the skill is capped**"
    );
    assert!(
        close(
            cdda_sim::trade::price_adjustment(-3),
            1.0 / cdda_sim::trade::price_adjustment(3)
        ),
        "**a buyer three better inverts the curve**"
    );
}

/// The whole adjustment: a clever seller asks more, a buyer who is the person gets the inverse sign.
#[test]
fn the_clever_side_moves_the_price_and_the_person_side_inverts_it() {
    let close = |a: f64, b: f64| (a - b).abs() < 1e-9;
    let seller = cdda_sim::trade::Trader {
        intelligence: 8,
        barter: 0,
        is_npc: true,
    };
    let buyer = cdda_sim::trade::Trader {
        intelligence: 8,
        barter: 0,
        is_npc: false,
    };
    assert!(
        close(cdda_sim::trade::net_price_adjustment(buyer, seller), 1.0),
        "**evenly matched, the price is value**"
    );
    // **Four points of intelligence is 1.20**, the reference's `1 + 0.05 * min( 19, |diff| )`.
    let dull_buyer = cdda_sim::trade::Trader {
        intelligence: 4,
        ..buyer
    };
    assert!(
        close(
            cdda_sim::trade::net_price_adjustment(dull_buyer, seller),
            1.2
        ),
        "**a seller four cleverer asks 1.2 of value**"
    );
    // **And when the buyer is the person, the sign inverts**: `-1 / adjust`.
    let npc_buyer = cdda_sim::trade::Trader {
        is_npc: true,
        ..buyer
    };
    let person_seller = cdda_sim::trade::Trader {
        is_npc: false,
        ..seller
    };
    assert!(
        close(
            cdda_sim::trade::net_price_adjustment(npc_buyer, person_seller),
            -1.0
        ),
        "**an evenly matched NPC buyer pays the same, negated**"
    );
    // **Barter rides on top of intelligence**: three points of seller barter is `1.33` before the intelligence term.
    let chatty_seller = cdda_sim::trade::Trader {
        barter: 3,
        ..seller
    };
    assert!(
        close(
            cdda_sim::trade::net_price_adjustment(buyer, chatty_seller),
            cdda_sim::trade::price_adjustment(3)
        ),
        "**a seller three better at barter asks the curve's worth more**"
    );
}
