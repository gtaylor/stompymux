//! Computer failure selection is independent of heartbeat admission, state effects and recovery scheduling.
use super::BattleDice;
use anyhow::{Result, ensure};
use serde::Serialize;

/// The available displays and target are sampled before a failure is selected.
#[derive(Debug, Clone, Copy)]
pub struct BattleComputerFailureInput {
    pub parts_enabled: bool,
    /// Zero uses the default quality five; extended catalogue indices retain their failure thresholds.
    pub quality: u8,
    pub has_target: bool,
    pub tactical_range: u8,
    pub long_range: u8,
    pub scanner_range: u8,
}

/// State transitions owed by the enclosing computer-failure action.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BattleComputerFailure {
    LoseTarget,
    Tactical,
    LongRange,
    Scanner,
    AllDisplays,
    Shutdown,
}

/// Choose a failure without changing unit state; callers own eligibility and application.
/// Disabled parts consume no dice. A six is rerolled once, and unavailable effects are discarded.
pub fn select_computer_failure(
    dice: &mut BattleDice,
    input: BattleComputerFailureInput,
) -> Result<Option<BattleComputerFailure>> {
    select_with(|sides| dice.die(sides), input)
}

/// Keep conditional random draws explicit so failed admission cannot schedule replacement rolls.
fn select_with(
    mut draw: impl FnMut(u16) -> Result<u16>,
    input: BattleComputerFailureInput,
) -> Result<Option<BattleComputerFailure>> {
    if !input.parts_enabled {
        return Ok(None);
    }
    let quality = if input.quality == 0 {
        5
    } else {
        usize::from(input.quality)
    };
    let brand = (24 + quality - 1) * 5 / 6;
    ensure!(
        brand < 30,
        "Computer quality has no failure catalogue entry"
    );
    let reliability = [80, 90, 95, 100, 101][brand % 5];
    if draw(5000)? != 42 || draw(100)? <= reliability {
        return Ok(None);
    }
    let roll = draw(6)? as u8;
    let roll = if roll == 6 { draw(6)? as u8 } else { roll };
    Ok(admitted_effect(input, roll))
}

/// Selection guards preserve the distinction between a single display and the shared scanner gate.
fn admitted_effect(input: BattleComputerFailureInput, roll: u8) -> Option<BattleComputerFailure> {
    use BattleComputerFailure::*;
    let all = input.tactical_range > 0 && input.long_range > 0 && input.scanner_range > 0;
    match roll {
        1 if input.has_target => Some(LoseTarget),
        2 if input.tactical_range > 0 => Some(Tactical),
        3 if input.long_range > 0 => Some(LongRange),
        4 if all => Some(Scanner),
        5 if all => Some(AllDisplays),
        6 => Some(Shutdown),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const INPUT: BattleComputerFailureInput = BattleComputerFailureInput {
        parts_enabled: true,
        quality: 1,
        has_target: true,
        tactical_range: 20,
        long_range: 40,
        scanner_range: 20,
    };

    /// Every effect requires its actual display/target prerequisites; shutdown has no display gate.
    #[test]
    fn effect_admission_preserves_shared_scanner_requirement() {
        use BattleComputerFailure::*;
        for (roll, effect) in [
            (1, LoseTarget),
            (2, Tactical),
            (3, LongRange),
            (4, Scanner),
            (5, AllDisplays),
            (6, Shutdown),
        ] {
            assert_eq!(admitted_effect(INPUT, roll), Some(effect));
        }
        assert_eq!(
            admitted_effect(
                BattleComputerFailureInput {
                    has_target: false,
                    ..INPUT
                },
                1
            ),
            None
        );
        for field in 0..3 {
            let mut input = INPUT;
            match field {
                0 => input.tactical_range = 0,
                1 => input.long_range = 0,
                _ => input.scanner_range = 0,
            }
            assert_eq!(admitted_effect(input, 4), None);
            assert_eq!(admitted_effect(input, 5), None);
            assert_eq!(admitted_effect(input, 6), Some(Shutdown));
        }
    }

    /// Catalogue quality boundaries retain their integer-index mapping and one-time six reroll.
    #[test]
    fn selection_preserves_quality_and_draw_order() {
        for (quality, reliability) in [
            (0, 100),
            (1, 80),
            (2, 80),
            (3, 90),
            (4, 95),
            (5, 100),
            (6, 101),
            (7, 80),
            (8, 80),
            (9, 90),
            (10, 95),
            (11, 100),
            (12, 101),
        ] {
            for quality_roll in [80, 81, 90, 91, 95, 96, 100] {
                for selection in 1..=6 {
                    let fails = quality_roll > reliability;
                    let mut draws = vec![(5000, 42), (100, quality_roll)];
                    if fails {
                        draws.push((6, selection));
                        if selection == 6 {
                            draws.push((6, 6));
                        }
                    }
                    let mut draws = draws.into_iter();
                    let result = select_with(
                        |sides| {
                            let (expected_sides, value) =
                                draws.next().expect("unexpected random draw");
                            assert_eq!(sides, expected_sides);
                            Ok(value)
                        },
                        BattleComputerFailureInput { quality, ..INPUT },
                    )
                    .unwrap();
                    assert!(draws.next().is_none());
                    assert_eq!(
                        result,
                        if fails {
                            admitted_effect(INPUT, selection as u8)
                        } else {
                            None
                        }
                    );
                }
            }
        }
        let mut draws = vec![(5000, 42), (100, 100), (6, 6), (6, 2)].into_iter();
        assert_eq!(
            select_with(
                |sides| {
                    let (expected, value) = draws.next().unwrap();
                    assert_eq!(sides, expected);
                    Ok(value)
                },
                INPUT
            )
            .unwrap(),
            Some(BattleComputerFailure::Tactical)
        );
        assert!(draws.next().is_none());
        let mut draws = vec![(5000, 41)].into_iter();
        assert_eq!(
            select_with(
                |sides| {
                    let (expected, value) = draws.next().expect("rare gate must stop selection");
                    assert_eq!(sides, expected);
                    Ok(value)
                },
                INPUT
            )
            .unwrap(),
            None
        );
    }

    /// Disabled hardware and invalid catalogue ratings leave the private stream untouched.
    #[test]
    fn disabled_and_invalid_inputs_do_not_draw() {
        let mut dice = BattleDice::seeded([7; 32]);
        let before = dice.clone();
        assert_eq!(
            select_computer_failure(
                &mut dice,
                BattleComputerFailureInput {
                    parts_enabled: false,
                    ..INPUT
                }
            )
            .unwrap(),
            None
        );
        assert_eq!(dice, before);
        assert!(
            select_computer_failure(
                &mut dice,
                BattleComputerFailureInput {
                    quality: 255,
                    ..INPUT
                }
            )
            .is_err()
        );
        assert_eq!(dice, before);
    }
}
