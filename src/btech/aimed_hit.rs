//! Anatomical hit preference shares launch preparation and per-packet selection across target classes.
use super::*;
use crate::{ObjectId, World};
use anyhow::{Context, Result};

/// An admitted conventional launch; its target-side preparation precedes defense and damage rolls.
pub(super) struct AimedLaunch {
    pub shooter: ObjectId,
    pub target: ObjectId,
    pub index: usize,
    pub weapon: Weapon,
    pub fire_mode: FireMode,
    pub ammunition: AmmunitionMode,
    pub launched: bool,
    pub hit: bool,
    pub in_range: bool,
}

/// One launch's anatomical policy, reused by all its material packets.
#[derive(Clone, Copy)]
pub(super) struct AimedShot {
    selection: AimSelection,
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
        && request.ammunition != AmmunitionMode::Cluster
        && selection.matches(world, request.target)
        && computer_assists(world, request.shooter, request.index, request.ammunition)?;
    Ok(Some(AimedShot {
        selection,
        immobile_hit: immobile_hit
            && !missile
            && request.fire_mode.rounds_per_cycle() == 1
            && request.ammunition != AmmunitionMode::Cluster,
        computer,
    }))
}

/// Resolve hardware with the same mount/controller predicate used by ordinary aiming.
fn computer_assists(
    world: &World,
    shooter: ObjectId,
    index: usize,
    ammunition: AmmunitionMode,
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
                    .filter(|part| part.system == System::TargetingComputer)
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
        arc: HitArc,
        partial_cover: bool,
    ) -> Result<Option<UnitSection>> {
        if !self.immobile_hit
            && (!self.computer || super::dice::unit_dice_mut(world, target)?.d6() < 3)
        {
            return Ok(None);
        }
        let slot = self.selection.slot();
        if world.btech.constructed_units().contains_key(&target) {
            let Some(section) = MechSection::ALL.get(slot).copied() else {
                return Ok(None);
            };
            let allowed = match section {
                MechSection::LeftArm | MechSection::LeftTorso => arc != HitArc::Right,
                MechSection::RightArm | MechSection::RightTorso => arc != HitArc::Left,
                MechSection::LeftLeg => arc != HitArc::Right && !partial_cover,
                MechSection::RightLeg => arc != HitArc::Left && !partial_cover,
                MechSection::CenterTorso => true,
                MechSection::Head => {
                    self.immobile_hit || super::aimed_target::immobile(world, target)?
                }
            };
            return Ok(allowed.then_some(UnitSection::Mech(section)));
        }
        let section = match slot {
            0 if arc != HitArc::Right => VehicleSection::Left,
            1 if arc != HitArc::Left => VehicleSection::Right,
            2 if arc != HitArc::Rear => VehicleSection::Front,
            3 if arc != HitArc::Front => VehicleSection::Rear,
            4 => VehicleSection::Turret,
            _ => return Ok(None),
        };
        Ok(Some(UnitSection::Vehicle(section)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Bare constructed anatomy isolates hit selection from launch admission and map effects.
    fn target(world: &mut World, source: &str, running: bool) {
        let id = ObjectId(42);
        let power = if running { Power::Running } else { Power::Off };
        match UnitTemplate::parse("target", source).unwrap() {
            UnitTemplate::Mech(template) => {
                let mut unit = Mech::from_template(template).unwrap();
                unit.power = power;
                world.btech.constructed.insert(id, unit);
            }
            UnitTemplate::Vehicle(template) => {
                let mut unit = Vehicle::new(template).unwrap();
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
                AimSelection::Mech(MechSection::LeftArm),
                AimSelection::Mech(MechSection::RightArm),
                AimSelection::GroundVehicle(VehicleSection::Front),
                AimSelection::Vtol(VehicleSection::Rear),
                AimSelection::GroundVehicle(VehicleSection::Turret),
                AimSelection::Vtol(VehicleSection::Rotor),
                AimSelection::Mech(MechSection::RightLeg),
                AimSelection::Mech(MechSection::Head),
            ];
            for (selection, section) in selections.into_iter().zip(MechSection::ALL) {
                for arc in [HitArc::Front, HitArc::Rear, HitArc::Left, HitArc::Right] {
                    for cover in [false, true] {
                        let hidden = match section {
                            MechSection::LeftArm | MechSection::LeftTorso => arc == HitArc::Right,
                            MechSection::RightArm | MechSection::RightTorso => arc == HitArc::Left,
                            MechSection::LeftLeg => arc == HitArc::Right || cover,
                            MechSection::RightLeg => arc == HitArc::Left || cover,
                            MechSection::Head | MechSection::CenterTorso => false,
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
                            (!hidden).then_some(UnitSection::Mech(section))
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
                (MechSection::LeftArm, Some(VehicleSection::Left)),
                (MechSection::RightArm, Some(VehicleSection::Right)),
                (MechSection::LeftTorso, Some(VehicleSection::Front)),
                (MechSection::RightTorso, None), // Rear hidden from the front.
                (MechSection::CenterTorso, Some(VehicleSection::Turret)),
                (MechSection::LeftLeg, None), // Rotor is not a directed hit location.
                (MechSection::RightLeg, None),
                (MechSection::Head, None),
            ] {
                let policy = AimedShot {
                    selection: AimSelection::Mech(selected),
                    immobile_hit: true,
                    computer: false,
                };
                assert_eq!(
                    policy
                        .preferred(&mut world, ObjectId(42), HitArc::Front, false)
                        .unwrap(),
                    expected.map(UnitSection::Vehicle)
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
                    (MechSection::Head, HitArc::Front, false),
                    (MechSection::LeftArm, HitArc::Right, false),
                    (MechSection::LeftLeg, HitArc::Front, true),
                    (MechSection::CenterTorso, HitArc::Rear, false),
                ] {
                    let mut world = World::default();
                    target(
                        &mut world,
                        include_str!("../../game/mechs/JR7-D.toml"),
                        running,
                    );
                    let mut dice = Dice::seeded([byte; 32]);
                    world.btech.constructed.get_mut(&ObjectId(42)).unwrap().dice = dice.clone();
                    let success = dice.d6() >= 3
                        && (section == MechSection::CenterTorso
                            || (section == MechSection::Head && !running));
                    let policy = AimedShot {
                        selection: AimSelection::Mech(section),
                        immobile_hit: false,
                        computer: true,
                    };
                    assert_eq!(
                        policy
                            .preferred(&mut world, ObjectId(42), arc, cover)
                            .unwrap(),
                        success.then_some(UnitSection::Mech(section))
                    );
                    assert_eq!(world.btech.constructed_units()[&ObjectId(42)].dice, dice);
                }
            }
        }
    }
}
