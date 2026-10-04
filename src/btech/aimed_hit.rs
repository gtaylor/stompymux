//! Anatomical hit preference shares launch preparation and per-packet selection across target classes.
use super::*;
use crate::{ObjectId, World};
use anyhow::{Context, Result};

/// An admitted conventional launch; its target-side preparation precedes defense and damage rolls.
pub(super) struct AimedLaunch {
    pub shooter: ObjectId,
    pub target: ObjectId,
    pub index: usize,
    pub weapon: BattleWeapon,
    pub fire_mode: BattleFireMode,
    pub ammunition: BattleAmmunitionMode,
    pub launched: bool,
    pub hit: bool,
    pub in_range: bool,
}

/// One launch's anatomical policy, reused by all its material packets.
#[derive(Clone, Copy)]
pub(super) struct AimedShot {
    selection: BattleAimSelection,
    immobile_hit: bool,
    computer: bool,
}

/// Missiles consume immobile-target preparation even on misses, but do not direct their hit locations.
pub(super) fn prepare(world: &mut World, request: AimedLaunch) -> Result<Option<AimedShot>> {
    let missile = request.weapon.gunnery_skill(true) == "Gunnery-Missile";
    if !request.launched
        || !request.in_range
        || (!request.hit && !missile)
        || request.weapon.is_artillery()
    {
        return Ok(None);
    }
    let Some(selection) = super::aimed_target::aimed_section(world, request.shooter)? else {
        return Ok(None);
    };
    let immobile_hit = if super::aimed_target::immobile(world, request.target)? {
        (6..=8).contains(&super::dice::unit_dice_mut(world, request.target)?.generic_roll())
    } else {
        false
    };
    let computer = !missile
        && request.ammunition != BattleAmmunitionMode::Cluster
        && selection.matches(world, request.target)
        && computer_assists(world, request.shooter, request.index, request.ammunition)?;
    Ok(Some(AimedShot {
        selection,
        immobile_hit: immobile_hit
            && !missile
            && request.fire_mode.rounds_per_cycle() == 1
            && request.ammunition != BattleAmmunitionMode::Cluster,
        computer,
    }))
}

/// Resolve hardware with the same mount/controller predicate used by ordinary aiming.
fn computer_assists(
    world: &World,
    shooter: ObjectId,
    index: usize,
    ammunition: BattleAmmunitionMode,
) -> Result<bool> {
    super::with_unit!(
        world
            .btech
            .unit(shooter)
            .context("Unit construction state is unavailable")?,
        |unit| {
            let loadout = unit.loadout()?;
            let mount = loadout
                .weapons
                .get(index)
                .context("Weapon index out of bounds")?;
            Ok(mount.computer_assists(
                ammunition,
                loadout
                    .systems
                    .iter()
                    .filter(|part| part.system == BattleSystem::TargetingComputer)
                    .map(|part| !unit.critical_unavailable(part.location)),
            ))
        }
    )
}

impl AimedShot {
    /// Prefer an exposed section, or let the existing hit table resolve the packet normally.
    /// Immobile success bypasses the computer roll, including when its requested location is hidden.
    pub(super) fn preferred(
        self,
        world: &mut World,
        target: ObjectId,
        arc: BattleHitArc,
        partial_cover: bool,
    ) -> Result<Option<BattleUnitSection>> {
        if !self.immobile_hit
            && (!self.computer || super::dice::unit_dice_mut(world, target)?.d6() < 3)
        {
            return Ok(None);
        }
        let slot = self.selection.slot();
        if world.btech.constructed_units().contains_key(&target) {
            let Some(section) = BattleSection::ALL.get(slot).copied() else {
                return Ok(None);
            };
            let allowed = match section {
                BattleSection::LeftArm | BattleSection::LeftTorso => arc != BattleHitArc::Right,
                BattleSection::RightArm | BattleSection::RightTorso => arc != BattleHitArc::Left,
                BattleSection::LeftLeg => arc != BattleHitArc::Right && !partial_cover,
                BattleSection::RightLeg => arc != BattleHitArc::Left && !partial_cover,
                BattleSection::CenterTorso => true,
                BattleSection::Head => {
                    self.immobile_hit || super::aimed_target::immobile(world, target)?
                }
            };
            return Ok(allowed.then_some(BattleUnitSection::Mech(section)));
        }
        let section = match slot {
            0 if arc != BattleHitArc::Right => BattleVehicleSection::Left,
            1 if arc != BattleHitArc::Left => BattleVehicleSection::Right,
            2 if arc != BattleHitArc::Rear => BattleVehicleSection::Front,
            3 if arc != BattleHitArc::Front => BattleVehicleSection::Rear,
            4 => BattleVehicleSection::Turret,
            _ => return Ok(None),
        };
        Ok(Some(BattleUnitSection::Vehicle(section)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Bare constructed anatomy isolates hit selection from launch admission and map effects.
    fn target(world: &mut World, source: &str, running: bool) {
        let id = ObjectId(42);
        let power = if running {
            BattlePower::Running
        } else {
            BattlePower::Off
        };
        match BattleUnitTemplate::parse("target", source).unwrap() {
            BattleUnitTemplate::Mech(template) => {
                let mut unit = BattleUnit::from_template(template).unwrap();
                unit.power = power;
                world.btech.constructed.insert(id, unit);
            }
            BattleUnitTemplate::Vehicle(template) => {
                let mut unit = BattleVehicle::new(template).unwrap();
                unit.power = power;
                world.btech.vehicles.insert(id, unit);
            }
        }
    }

    /// Successful immobile aim remaps stored anatomy numerically; cover and exposed sides still apply.
    #[test]
    fn immobile_aim_preserves_numeric_anatomy_and_cover() {
        for source in [
            include_str!("../../game/mechs/JR7-D.toml"),
            include_str!("../../game/mechs/GOL-1H.toml"),
        ] {
            let mut world = World::default();
            target(&mut world, source, false);
            let selections = [
                BattleAimSelection::Mech(BattleSection::LeftArm),
                BattleAimSelection::Mech(BattleSection::RightArm),
                BattleAimSelection::GroundVehicle(BattleVehicleSection::Front),
                BattleAimSelection::Vtol(BattleVehicleSection::Rear),
                BattleAimSelection::GroundVehicle(BattleVehicleSection::Turret),
                BattleAimSelection::Vtol(BattleVehicleSection::Rotor),
                BattleAimSelection::Mech(BattleSection::RightLeg),
                BattleAimSelection::Mech(BattleSection::Head),
            ];
            for (selection, section) in selections.into_iter().zip(BattleSection::ALL) {
                for arc in [
                    BattleHitArc::Front,
                    BattleHitArc::Rear,
                    BattleHitArc::Left,
                    BattleHitArc::Right,
                ] {
                    for cover in [false, true] {
                        let hidden = match section {
                            BattleSection::LeftArm | BattleSection::LeftTorso => {
                                arc == BattleHitArc::Right
                            }
                            BattleSection::RightArm | BattleSection::RightTorso => {
                                arc == BattleHitArc::Left
                            }
                            BattleSection::LeftLeg => arc == BattleHitArc::Right || cover,
                            BattleSection::RightLeg => arc == BattleHitArc::Left || cover,
                            BattleSection::Head | BattleSection::CenterTorso => false,
                        };
                        let before = world.btech.clone();
                        let policy = AimedShot {
                            selection,
                            immobile_hit: true,
                            computer: true,
                        };
                        assert_eq!(
                            policy
                                .preferred(&mut world, ObjectId(42), arc, cover)
                                .unwrap(),
                            (!hidden).then_some(BattleUnitSection::Mech(section))
                        );
                        assert_eq!(
                            world.btech, before,
                            "Immobile success must not roll a computer die"
                        );
                    }
                }
            }
        }
        for source in [
            include_str!("../../game/mechs/Demolisher.toml"),
            include_str!("../../game/mechs/Kestrel.toml"),
        ] {
            let mut world = World::default();
            target(&mut world, source, false);
            for (selected, expected) in [
                (BattleSection::LeftArm, Some(BattleVehicleSection::Left)),
                (BattleSection::RightArm, Some(BattleVehicleSection::Right)),
                (BattleSection::LeftTorso, Some(BattleVehicleSection::Front)),
                (BattleSection::RightTorso, None), // Rear hidden from the front.
                (
                    BattleSection::CenterTorso,
                    Some(BattleVehicleSection::Turret),
                ),
                (BattleSection::LeftLeg, None), // Rotor is not a directed hit location.
                (BattleSection::RightLeg, None),
                (BattleSection::Head, None),
            ] {
                let policy = AimedShot {
                    selection: BattleAimSelection::Mech(selected),
                    immobile_hit: true,
                    computer: false,
                };
                assert_eq!(
                    policy
                        .preferred(&mut world, ObjectId(42), BattleHitArc::Front, false)
                        .unwrap(),
                    expected.map(BattleUnitSection::Vehicle)
                );
            }
        }
    }

    /// Computer failures, hidden sections and mobile heads consume one die before ordinary routing.
    #[test]
    fn computer_roll_precedes_exposure_and_head_admission() {
        for running in [false, true] {
            for byte in 0..32 {
                for (section, arc, cover) in [
                    (BattleSection::Head, BattleHitArc::Front, false),
                    (BattleSection::LeftArm, BattleHitArc::Right, false),
                    (BattleSection::LeftLeg, BattleHitArc::Front, true),
                    (BattleSection::CenterTorso, BattleHitArc::Rear, false),
                ] {
                    let mut world = World::default();
                    target(
                        &mut world,
                        include_str!("../../game/mechs/JR7-D.toml"),
                        running,
                    );
                    let mut dice = BattleDice::seeded([byte; 32]);
                    world.btech.constructed.get_mut(&ObjectId(42)).unwrap().dice = dice.clone();
                    let success = dice.d6() >= 3
                        && (section == BattleSection::CenterTorso
                            || (section == BattleSection::Head && !running));
                    let policy = AimedShot {
                        selection: BattleAimSelection::Mech(section),
                        immobile_hit: false,
                        computer: true,
                    };
                    assert_eq!(
                        policy
                            .preferred(&mut world, ObjectId(42), arc, cover)
                            .unwrap(),
                        success.then_some(BattleUnitSection::Mech(section))
                    );
                    assert_eq!(world.btech.constructed_units()[&ObjectId(42)].dice, dice);
                }
            }
        }
    }
}
