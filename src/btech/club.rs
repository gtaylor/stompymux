//! Carried trees: acquisition, release and arm availability shared by physical commands.
use super::*;
use crate::{ObjectId, World};
use anyhow::{Context, Result, ensure};

impl BattleUnit {
    /// The arm holding a tree, independent of physical recovery.
    pub fn carried_club(&self) -> Option<BattleArm> {
        self.carried_club
    }
}

/// Check the arm, shoulder, hand and ranged weapon recovery used when grabbing a tree.
fn usable(unit: &BattleUnit, arm: BattleArm) -> Result<bool> {
    let section = arm.section();
    if unit.sections()[&section].internal == 0 {
        return Ok(false);
    }
    let loadout = unit.loadout()?;
    for (slot, system) in [
        (0, BattleSystem::ShoulderOrHip),
        (3, BattleSystem::HandOrFootActuator),
    ] {
        let location = CriticalLocation { section, slot };
        if unit.critical_unavailable(location)
            || !loadout
                .systems
                .iter()
                .any(|part| part.location == location && part.system == system)
        {
            return Ok(false);
        }
    }
    Ok(!loadout.weapons.iter().enumerate().any(|(index, mount)| {
        unit.weapon_recycle().contains_key(&index)
            && mount.criticals.iter().any(|part| part.section == section)
    }))
}

/// Messages for a carried tree released by the pilot or by a shutdown/damage transition.
pub(super) fn dropped_notices(world: &World, id: ObjectId) -> Vec<BattleNotice> {
    let mut notices = vec![BattleNotice {
        unit: id,
        text: "Your club falls to the ground and shatters.".into(),
    }];
    notices.extend(super::broadcast::observer_notices(
        world,
        id,
        "'s club falls to the ground and shatters.",
    ));
    notices
}

/// Grab a tree in a selected arm (left first by default), or release it with `-`.
/// Acquisition resets that arm's recovery to sixty seconds without consuming dice or terrain.
pub fn grab_club(
    world: &mut World,
    id: ObjectId,
    pilot: ObjectId,
    selection: Option<&str>,
) -> Result<Vec<BattleNotice>> {
    super::power::controlled_unit(world, id, pilot)?;
    let unit = &world.btech.constructed_units()[&id];
    ensure!(
        unit.power() == BattlePower::Running && !unit.is_destroyed(),
        "Start the unit first"
    );
    if selection == Some("-") {
        ensure!(
            unit.carried_club.is_some(),
            "You aren't currently carrying a club."
        );
        world.btech.constructed.get_mut(&id).unwrap().carried_club = None;
        return Ok(dropped_notices(world, id));
    }
    ensure!(
        unit.chassis() != BattleMechChassis::Quad,
        "Quads can't carry a club."
    );
    ensure!(
        unit.posture() != BattlePosture::Prone,
        "You can't grab a club while lying flat on your face."
    );
    ensure!(
        !unit.airborne() && unit.free_fall().is_none(),
        "You can't grab a club while in the air!"
    );
    ensure!(
        unit.unjam().is_none(),
        "You are too busy unjamming a weapon!"
    );
    for kind in BattleArmAttack::HAND_WEAPONS
        .into_iter()
        .filter(|kind| kind.needs_hand())
    {
        for arm in [BattleArm::Left, BattleArm::Right] {
            ensure!(
                !kind.available(unit, arm.section())?,
                "You cannot grab a club while carrying a {}",
                kind.name()
            );
        }
    }
    let arm = match selection.map(str::to_ascii_lowercase).as_deref() {
        None if usable(unit, BattleArm::Left)? => BattleArm::Left,
        None => BattleArm::Right,
        Some("l" | "left") => BattleArm::Left,
        Some("r" | "right") => BattleArm::Right,
        _ => anyhow::bail!("Choose left, right, or - to drop the club"),
    };
    ensure!(
        usable(unit, arm)?,
        "You don't have a free arm with a working hand and shoulder!"
    );
    ensure!(
        unit.carried_club.is_none(),
        "You're already carrying a club."
    );
    let position = unit.position().context("Unit is not on a battlefield")?;
    let tile =
        world.btech.maps()[&position.map].hex(i64::from(position.x), i64::from(position.y))?;
    ensure!(
        tile.is_woods(),
        "There don't appear to be any trees within grabbing distance."
    );
    world.attempt(|world| {
        let unit = world.btech.constructed.get_mut(&id).unwrap();
        unit.carried_club = Some(arm);
        unit.limb_recycle.insert(arm.section(), 60);
        world.btech.validate_action(world)?;
        let mut notices = super::broadcast::observer_notices(
            world,
            id,
            "reaches down and yanks a tree out of the ground!",
        );
        notices.push(BattleNotice {
            unit: id,
            text: format!(
                "You reach down and yank a tree out of the ground with your {}.",
                arm.section().name().replace('_', " ")
            ),
        });
        Ok(notices)
    })
}
