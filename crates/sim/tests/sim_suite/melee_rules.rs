
/// How well a character aims a blow: their dexterity, their skills, and the weapon's own balance.
///
/// Reference: `Character::get_melee_hit_base` (`melee.cpp:375-386`) is `get_hit_base()` - which is `get_dex() / 4.0f` (`character.cpp:5705-5709`) - plus `get_hit_weapon`, which is
/// `skill / 3 + melee_skill / 2 + weap.get_to_hit()`. The three terms are added in tenths of an accuracy point in the port, which is the unit `MeleeAttack::accuracy_tenths` already
/// uses, so a fixture and a character's own skill are the same kind of number.
#[test]
fn how_well_a_character_aims_a_blow() {
    use cdda_sim::melee::{hit_base_tenths, hit_weapon_tenths};

    // Dexterity eight is two points of accuracy, which is twenty tenths.
    assert_eq!(hit_base_tenths(8), 20, "a dexterous character aims well");
    assert_eq!(hit_base_tenths(0), 0, "and one with no dexterity does not");
    assert_eq!(hit_base_tenths(12), 30, "twelve is three points");

    // Skills are thirds and halves: unarmed three is one point, melee four is two.
    assert_eq!(hit_weapon_tenths(3, 4, 0), 30, "one point of unarmed and two of melee");
    // The weapon's own balance is added last and can be negative - a badly balanced thing aims worse.
    assert_eq!(hit_weapon_tenths(0, 0, -2), -20, "a clumsy weapon takes accuracy away");
    assert_eq!(hit_weapon_tenths(0, 0, 3), 30, "a well balanced one gives it");
}
