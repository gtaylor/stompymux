//! Fractional conventional range brackets and explicit unsupported-range outcomes.
use stompymux_rs::{RangeBracket as Bracket, Weapon};

#[test]
fn fractional_minimum_and_bracket_boundaries_follow_separate_rounding_rules() {
    for (distance, bracket, modifier) in [
        (0.0, Bracket::Minimum, 7),
        (1.2, Bracket::Minimum, 5),
        (5.0, Bracket::Minimum, 2),
        (5.001, Bracket::Minimum, 1),
        (6.0, Bracket::Minimum, 1),
        (6.049, Bracket::Minimum, 1),
        (6.051, Bracket::Short, 0),
        (7.049, Bracket::Short, 0),
        (7.051, Bracket::Medium, 2),
        (14.049, Bracket::Medium, 2),
        (14.051, Bracket::Long, 4),
        (21.0, Bracket::Long, 4),
    ] {
        let range = Weapon::Lrm20
            .range_modifier(distance, false)
            .unwrap()
            .unwrap();
        assert_eq!((range.bracket, range.modifier), (bracket, modifier));
    }
    assert!(
        Weapon::Lrm20
            .range_modifier(21.001, false)
            .unwrap()
            .is_none()
    );
    assert_eq!(
        Weapon::MediumLaser
            .range_modifier(0.0, false)
            .unwrap()
            .unwrap()
            .modifier,
        0
    );
}

#[test]
fn physical_maximum_is_strict_and_extended_range_uses_twice_medium_range() {
    for weapon in [
        Weapon::MediumLaser,
        Weapon::Srm4,
        Weapon::Srm6,
        Weapon::Ac20,
    ] {
        assert_eq!(
            weapon
                .range_modifier(3.049, false)
                .unwrap()
                .unwrap()
                .modifier,
            0
        );
        assert_eq!(
            weapon
                .range_modifier(3.051, false)
                .unwrap()
                .unwrap()
                .modifier,
            2
        );
        assert_eq!(
            weapon
                .range_modifier(6.049, false)
                .unwrap()
                .unwrap()
                .modifier,
            2
        );
        assert_eq!(
            weapon
                .range_modifier(6.051, false)
                .unwrap()
                .unwrap()
                .modifier,
            4
        );
        assert!(weapon.range_modifier(9.001, false).unwrap().is_none());
        assert_eq!(
            weapon.range_modifier(9.001, true).unwrap().unwrap().bracket,
            Bracket::Long
        );
        assert_eq!(
            weapon
                .range_modifier(9.051, true)
                .unwrap()
                .unwrap()
                .modifier,
            8
        );
        assert_eq!(
            weapon.range_modifier(12.0, true).unwrap().unwrap().bracket,
            Bracket::Extreme
        );
        assert!(weapon.range_modifier(12.001, true).unwrap().is_none());
    }
    assert_eq!(
        Weapon::Lrm20
            .range_modifier(28.0, true)
            .unwrap()
            .unwrap()
            .modifier,
        8
    );
    assert!(
        Weapon::Lrm20
            .range_modifier(28.001, true)
            .unwrap()
            .is_none()
    );
    for distance in [-0.1, f64::NAN, f64::INFINITY] {
        assert!(Weapon::Srm4.range_modifier(distance, false).is_err());
    }
}
