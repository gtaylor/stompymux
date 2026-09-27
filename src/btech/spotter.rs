//! Shared observer links and target validation for Mech and vehicle indirect fire.
use super::*;
use crate::{Flag, ObjectId, World};
use anyhow::{Context, Result, ensure};

/// A validated spotter and its currently acquired unit target; no dice or derived aim is stored.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BattleSpotterTarget {
    pub spotter: ObjectId,
    pub target: ObjectId,
}

impl BattleUnit {
    /// Self-selection declares this unit a spotter; another ID selects a forward observer.
    pub fn spotter(&self) -> Option<ObjectId> {
        self.spotter
    }

    /// Read pending radio connections and periodic checks without advancing their clocks.
    pub fn spotter_events(&self) -> &super::BattleSpotterEvents {
        &self.spotter_events
    }

    /// Spotting prohibits firing; a selected observer restricts fire to indirect-capable weapons.
    pub(super) fn check_spotter_fire(&self, id: ObjectId, index: usize) -> Result<()> {
        check_fire(
            self.spotter,
            id,
            self.weapon_readiness(index)?.weapon,
            self.ammunition_mode(index)?,
        )
    }
}

impl BattleVehicle {
    /// Self-selection declares this unit a spotter; another ID selects its observer.
    pub fn spotter(&self) -> Option<ObjectId> {
        self.spotter
    }

    /// Read pending radio connections and periodic checks without advancing their clocks.
    pub fn spotter_events(&self) -> &super::BattleSpotterEvents {
        &self.spotter_events
    }

    /// Vehicle weapons obey the same spotting restrictions as Mech weapons.
    pub(super) fn check_spotter_fire(&self, id: ObjectId, index: usize) -> Result<()> {
        check_fire(
            self.spotter,
            id,
            self.weapon_readiness(index)?.weapon,
            self.ammunition_mode(index)?,
        )
    }
}

/// Fire restrictions depend on the observer role, not the shooter's anatomy.
fn check_fire(
    spotter: Option<ObjectId>,
    id: ObjectId,
    weapon: BattleWeapon,
    ammunition: BattleAmmunitionMode,
) -> Result<()> {
    check_role(spotter, id)?;
    ensure!(
        spotter.is_none()
            || weapon.supports_indirect_ammunition(ammunition)
            || weapon.is_artillery(),
        "Remove your spotter to fire non-IDF weapons"
    );
    Ok(())
}

/// Self-spotting rejects a firing attempt before weapon lookup or target parsing.
pub(super) fn check_firing_role(world: &World, id: ObjectId) -> Result<()> {
    check_role(selected(world, id), id)
}

/// Role admission is independent of the later weapon-family restriction.
fn check_role(spotter: Option<ObjectId>, id: ObjectId) -> Result<()> {
    ensure!(spotter != Some(id), "You cannot fire while spotting.");
    Ok(())
}

/// Read the selected link from either owned construction store.
pub(super) fn selected(world: &World, id: ObjectId) -> Option<ObjectId> {
    world
        .btech
        .vehicles()
        .get(&id)
        .and_then(BattleVehicle::spotter)
        .or_else(|| {
            world
                .btech
                .constructed_units()
                .get(&id)
                .and_then(BattleUnit::spotter)
        })
}

/// Shared perception and movement contributions for both shooters and observer types.
pub(super) fn indirect_aim(
    world: &World,
    source: super::fire_target::TargetSource,
    target: ObjectId,
    index: usize,
    fasa_turning: bool,
) -> Result<Option<BattleIndirectAim>> {
    let Some(link) = indirect_target_for_source(world, source, index)? else {
        return Ok(None);
    };
    ensure!(
        link.target == target,
        "Target differs from the spotter's selected target"
    );
    observer_aim(world, link.spotter, fasa_turning).map(Some)
}

/// Movement, skill and lock costs are shared by unit and empty-coordinate indirect shots.
pub(super) fn observer_aim(
    world: &World,
    spotter: ObjectId,
    fasa_turning: bool,
) -> Result<BattleIndirectAim> {
    let movement = if let Some(unit) = world.btech.vehicles().get(&spotter) {
        unit.attacker_movement_modifier(fasa_turning)
    } else {
        world.btech.constructed_units()[&spotter].attacker_movement_modifier(fasa_turning)
    };
    Ok(BattleIndirectAim {
        spotter,
        spotting: super::skills::unit_spotting_target(world, spotter)?,
        movement,
        target_lock: if super::targeting::selection(world, spotter)
            .is_some_and(|lock| lock.remaining() > 0)
        {
            2
        } else {
            0
        },
    })
}

/// Select an acquired friendly observer, declare self spotting, or clear either role.
pub fn select_spotter(
    world: &mut World,
    id: ObjectId,
    pilot: ObjectId,
    selected: Option<ObjectId>,
) -> Result<Vec<BattleNotice>> {
    if world.btech.vehicles().contains_key(&id) {
        super::vehicle_power::controlled(world, id, pilot)?;
    } else {
        power::controlled_unit(world, id, pilot)?;
    }
    let unit = super::scanner::scanner_unit(world, id).context("Unit is unavailable")?;
    ensure!(unit.power == BattlePower::Running, "Start the unit first");
    let text = match selected {
        None => {
            if self::selected(world, id) == Some(id) {
                "You spot no longer.".into()
            } else {
                "You disable the datalink to spotter.".into()
            }
        }
        Some(target) if target == id => {
            let recycling = if let Some(vehicle) = world.btech.vehicles().get(&id) {
                vehicle
                    .weapon_recycle()
                    .values()
                    .any(|seconds| *seconds > 0)
            } else {
                let unit = &world.btech.constructed_units()[&id];
                unit.weapon_recycle().values().any(|seconds| *seconds > 0)
                    || unit.limb_recycle().values().any(|seconds| *seconds > 0)
            };
            ensure!(!recycling, "You have weapons recycling!");
            "You are now set as a spotter.".into()
        }
        Some(target) => {
            let observer = super::scanner::scanner_unit(world, target)
                .context("That target does not exist!")?;
            ensure!(
                observer.signature.team == unit.signature.team,
                "That target does not exist!"
            );
            ensure!(
                self::selected(world, target) == Some(target),
                "That unit is not set up as spotter!"
            );
            let visible = visible_contact(world, id, target)?.is_some();
            if !visible && artillery_available(world, id)? {
                let range = super::unit_range(world, id, target)?.spatial;
                let maximum = 2.0 * f64::from(super::unit_radio_capabilities(world, target)?.range);
                let mut notices = vec![
                    BattleNotice {
                        unit: target,
                        text: "Someone is trying to establish a data link with you!".into(),
                    },
                    BattleNotice {
                        unit: id,
                        text: "You attempt to establish a data link..... please stand by.".into(),
                    },
                ];
                if range > maximum {
                    notices.push(BattleNotice {
                        unit: id,
                        text: "That target is our of data link range!".into(),
                    });
                } else {
                    super::spotter_events::connect(world, id, target, range)?;
                }
                return Ok(notices);
            }
            ensure!(visible, "You do not have LOS to that target!");
            format!("#{0} set as spotter.", target.0)
        }
    };
    super::artillery_adjustment::spotter_change(world, id, selected);
    if let Some(unit) = world.btech.vehicles.get_mut(&id) {
        unit.spotter = selected;
    } else {
        world.btech.constructed.get_mut(&id).unwrap().spotter = selected;
    }
    Ok(vec![BattleNotice { unit: id, text }])
}

/// Resolve a selected observer's unit target without requiring that the firer see the target.
/// The initial firer-to-observer contact is only required when establishing the direct link.
pub fn spotter_target(world: &World, firer: ObjectId) -> Result<BattleSpotterTarget> {
    let spotter = active_observer(world, firer)?;
    let target = match super::targeting::selection(world, spotter) {
        Some(BattleTargetSelection::Unit(lock)) => lock.target,
        Some(BattleTargetSelection::Hex(lock)) => super::hex_occupant(world, firer, lock.hex)?
            .context("Your spotter's hex is empty; no unit target is available")?,
        None => anyhow::bail!("Your spotter has no target set!"),
    };
    ensure!(
        visible_contact(world, spotter, target)?.is_some(),
        "Your spotter does not have a target in LOS!"
    );
    Ok(BattleSpotterTarget { spotter, target })
}

/// Coordinate spotting omits the observer's terrain aim contribution and spotting awards.
pub(super) fn coordinate_target(world: &World, observer: ObjectId) -> bool {
    matches!(
        super::targeting::selection(world, observer),
        Some(BattleTargetSelection::Hex(_))
    )
}

/// Revalidate an established observer link independently of whether its target is a unit or hex.
pub(super) fn active_observer(world: &World, firer: ObjectId) -> Result<ObjectId> {
    let unit =
        super::scanner::scanner_unit(world, firer).context("Firing unit is not constructed")?;
    let spotter = selected(world, firer).context("No spotter selected")?;
    ensure!(spotter != firer, "You cannot fire while spotting.");
    for id in [firer, spotter] {
        ensure!(
            world
                .objects
                .get(&id)
                .is_some_and(|object| !object.flags.contains(Flag::Going)),
            "Spotter link is unavailable"
        );
    }
    let observer =
        super::scanner::scanner_unit(world, spotter).context("Spotter is unavailable")?;
    ensure!(
        selected(world, spotter) == Some(spotter),
        "You do not have a spotter!"
    );
    ensure!(
        observer.power == BattlePower::Running && !observer.destroyed,
        "Spotter is unavailable"
    );
    ensure!(
        unit.position.is_some() && unit.position.map(|p| p.map) == observer.position.map(|p| p.map),
        "Spotter is on another battlefield"
    );
    ensure!(
        observer.signature.team == unit.signature.team,
        "Spotter is not friendly"
    );
    ensure!(
        !world
            .btech
            .vehicles()
            .get(&spotter)
            .and_then(BattleVehicle::pilot)
            .or_else(|| world
                .btech
                .constructed_units()
                .get(&spotter)
                .and_then(BattleUnit::pilot))
            .is_some_and(|pilot| world.btech.unconscious(pilot)),
        "Your spotter is unconscious!"
    );
    Ok(spotter)
}

/// Resolve observer participation using the unit's lock and datalink.
pub(super) fn indirect_target_for_source(
    world: &World,
    source: super::fire_target::TargetSource,
    index: usize,
) -> Result<Option<BattleSpotterTarget>> {
    let firer = source.unit;
    ensure!(
        super::scanner::scanner_unit(world, firer).is_some(),
        "Firing unit is not constructed"
    );
    if !uses_observer(world, source, index)? {
        return Ok(None);
    }
    spotter_target(world, firer).map(Some)
}

/// A direct unit lock overrides a selected observer for conventional weapons.
fn uses_observer(
    world: &World,
    source: super::fire_target::TargetSource,
    index: usize,
) -> Result<bool> {
    let firer = source.unit;
    let (weapon, ammunition) = installation(world, firer, index)?;
    Ok(weapon.supports_indirect_ammunition(ammunition)
        && !weapon.is_artillery()
        && selected(world, firer).is_some()
        && !matches!(
            source.selection(world),
            Some(BattleTargetSelection::Unit(_))
        ))
}

/// Resolve observer participation using the unit's lock and datalink.
pub(super) fn indirect_hex_for_source(
    world: &World,
    source: super::fire_target::TargetSource,
    index: usize,
) -> Result<Option<(ObjectId, BattleHexCoordinate)>> {
    let firer = source.unit;
    if !uses_observer(world, source, index)? {
        return Ok(None);
    }
    let observer = active_observer(world, firer)?;
    let Some(BattleTargetSelection::Hex(lock)) = super::targeting::selection(world, observer)
    else {
        return Ok(None);
    };
    if super::hex_occupant(world, firer, lock.hex)?.is_some() {
        return Ok(None);
    }
    ensure!(
        super::hex_visible(world, observer, lock.hex)?,
        "That target is not in your spotters line of sight!"
    );
    Ok(Some((observer, lock.hex)))
}

/// Indirect fire cannot cross from a surface shooter into a submerged target's water layer.
pub(super) fn check_indirect_water(
    world: &World,
    shooter: ObjectId,
    target: ObjectId,
) -> Result<()> {
    let elevation = |id| super::unit_elevation(world, id)?.context("Unit is not placed");
    ensure!(
        elevation(target)? >= 0 || elevation(shooter)? < 0,
        "You can't fire into water with that weapon from here."
    );
    Ok(())
}

/// Award one point to each eligible crew before indirect-shot damage changes the target.
/// Ordinary skill timing applies independently to the observer and firing unit.
pub(super) fn award_indirect_experience(
    world: &mut World,
    firer: ObjectId,
    link: BattleSpotterTarget,
    aim: &mut BattleAimModifiers,
) -> Result<Vec<BattleChannelMessage>> {
    if coordinate_target(world, link.spotter) {
        return Ok(Vec::new());
    }
    let mut messages = Vec::new();
    for (unit, skill, channel, description) in [
        (
            link.spotter,
            "Gunnery-Spotting",
            BattleChannel::Experience,
            "spotting XP",
        ),
        (
            firer,
            "Gunnery-Artillery",
            BattleChannel::AttackExperience,
            "1 artillery XP",
        ),
    ] {
        if unit == link.target
            || [unit, link.target].into_iter().any(|id| {
                world.objects.get(&id).is_none_or(|object| {
                    !object.flags.contains(Flag::InCharacter) || object.flags.contains(Flag::Going)
                })
            })
        {
            continue;
        }
        let attacker =
            super::scanner::scanner_unit(world, unit).context("Shooter is unavailable")?;
        let target =
            super::scanner::scanner_unit(world, link.target).context("Target is unavailable")?;
        if target.destroyed || attacker.signature.team == target.signature.team {
            continue;
        }
        let Some(pilot) = super::skills::active_pilot(world, unit)? else {
            continue;
        };
        if world.objects[&pilot].flags.contains(Flag::Going) {
            continue;
        }
        let award =
            award_skill_experience(world, pilot, skill, 1, crate::clock::wall_time(), false)?;
        if award.accepted {
            messages.push(BattleChannelMessage::new(
                channel,
                format!("{} gained {description}", world.objects[&pilot].name),
            ));
        }
    }
    // A newly earned observer level applies to the shot that earned it.
    if let Some(observer) = &mut aim.indirect {
        observer.spotting = super::skills::unit_spotting_target(world, observer.spotter)?;
    }
    Ok(messages)
}

/// Read the chosen installation and ammunition family across supported chassis.
pub(super) fn installation(
    world: &World,
    id: ObjectId,
    index: usize,
) -> Result<(BattleWeapon, BattleAmmunitionMode)> {
    if let Some(unit) = world.btech.vehicles().get(&id) {
        return Ok((
            unit.weapon_readiness(index)?.weapon,
            unit.ammunition_mode(index)?,
        ));
    }
    let unit = world
        .btech
        .constructed_units()
        .get(&id)
        .context("Unit is not constructed")?;
    Ok((
        unit.weapon_readiness(index)?.weapon,
        unit.ammunition_mode(index)?,
    ))
}

/// Numbered artillery lookup ignores ammunition but rejects first-slot loss and recycling.
fn artillery_available(world: &World, id: ObjectId) -> Result<bool> {
    if let Some(unit) = world.btech.vehicles().get(&id) {
        return Ok(unit
            .loadout()?
            .weapons
            .iter()
            .enumerate()
            .any(|(index, mount)| {
                mount.weapon.is_artillery()
                    && mount
                        .criticals
                        .first()
                        .is_some_and(|&first| !unit.critical_unavailable(first))
                    && unit.weapon_recycle().get(&index).copied().unwrap_or(0) == 0
            }));
    }
    let unit = &world.btech.constructed_units()[&id];
    Ok(unit
        .loadout()?
        .weapons
        .iter()
        .enumerate()
        .any(|(index, mount)| {
            mount.weapon.is_artillery()
                && mount.criticals.first().is_some_and(|&first| {
                    !unit.critical_unavailable(first)
                        && unit
                            .limb_recycle()
                            .get(&first.section)
                            .copied()
                            .unwrap_or(0)
                            == 0
                })
                && unit.weapon_recycle().get(&index).copied().unwrap_or(0) == 0
        }))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Rockets can use observer coordination without gaining switchable hotload or semi-guided modes.
    #[test]
    fn spotter_indirect_profiles_do_not_enable_rocket_ammunition_modes() {
        for weapon in [
            BattleWeapon::Rocket10,
            BattleWeapon::Rocket15,
            BattleWeapon::Rocket20,
        ] {
            assert!(weapon.supports_indirect_fire());
            assert!(!weapon.supports_hotload());
            assert!(!weapon.supports_semiguided());
        }
        assert!(!BattleWeapon::MediumLaser.supports_indirect_fire());
        assert!(BattleWeapon::Lrm5.supports_indirect_fire());
        assert!(BattleWeapon::Lrm5.supports_semiguided());
    }
}
