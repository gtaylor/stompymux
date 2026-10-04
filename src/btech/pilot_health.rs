//! Startup projects personal health into cockpit injury state without resolving a death or save.
use crate::{Flag, ObjectId, World};

/// Positive pilot damage is bounded by the reference's signed-byte storage.
pub(super) fn bounded(count: u16) -> u8 {
    count.min(127) as u8
}

/// Health-derived cockpit damage uses the reference fallback for zero or excessive Build.
fn injuries(profile: Option<super::BattleCharacter>) -> u8 {
    let Some(profile) = profile else {
        return 0;
    };
    let doubled = u16::from(profile.build) * 2;
    let divisor = if (1..=100).contains(&doubled) {
        doubled
    } else {
        10
    };
    bounded((u16::from(profile.bruise) + u16::from(profile.lethal)) / divisor)
}

/// Keep the cockpit scalar and optional character report in agreement for every injury source.
pub(super) fn set_count(world: &mut World, id: ObjectId, injuries: u8) {
    let (count, character) = super::with_unit_mut!(world.btech.unit_mut(id).unwrap(), |unit| {
        (&mut unit.pilot_injuries, &mut unit.character_pilot)
    });
    *count = injuries;
    if let Some(character) = character {
        character.injuries = injuries.into();
    }
}

/// Called only after startup admission; no dice, health, timer or assignment changes occur.
pub(super) fn synchronize(world: &mut World, id: ObjectId, player: ObjectId) {
    let injuries = injuries(world.btech.characters().get(&player).copied());
    let character = (injuries > 0 && world.objects[&id].flags.contains(Flag::InCharacter))
        .then_some(super::BattleCharacterPilotStatus {
            injuries: injuries.into(),
            killed: false,
        });
    super::with_unit_mut!(world.btech.unit_mut(id).unwrap(), |unit| {
        unit.character_pilot = character;
        unit.crew_recovery.edit_tactical_injuries(injuries);
    });
    set_count(world, id, injuries);
    if let Some(recovery) = world.btech.recoveries.get_mut(&player) {
        recovery.edit_tactical_injuries(injuries);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Preserve truncation, invalid-Build fallback and byte-sized health extremes.
    #[test]
    fn startup_injury_arithmetic() {
        assert_eq!(injuries(None), 0);
        for (build, bruise, lethal, expected) in [
            (5, 19, 0, 1),
            (5, 19, 1, 2),
            (0, 19, 1, 2),
            (51, 19, 1, 2),
            (50, 255, 255, 5),
            (1, 255, 255, 127),
        ] {
            assert_eq!(
                injuries(Some(super::super::BattleCharacter {
                    build,
                    bruise,
                    lethal,
                    reflexes: 1,
                    intuition: 1,
                    learn: 1,
                    charisma: 1
                })),
                expected
            );
        }
    }
}
