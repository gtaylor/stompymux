//! Carried trees: acquisition, release and arm availability shared by physical commands.
use super::*;
use crate::{ObjectId, World};
use anyhow::{Context, Result, ensure};

impl Mech {
    /// The arm holding a tree, independent of physical recovery.
    pub fn carried_club(&self) -> Option<Arm> {
        self.carried_club
    }
}

/// Check the arm, shoulder and hand used when grabbing a tree.
fn usable(unit: &Mech, arm: Arm) -> Result<bool> {
    let section = arm.section();
    if unit.sections()[&section].internal == 0 {
        return Ok(false);
    }
    let loadout = unit.loadout()?;
    for (slot, system) in [(0, System::ShoulderOrHip), (3, System::HandOrFootActuator)] {
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
    Ok(true)
}

/// Messages for a carried tree released by the pilot or by a shutdown/damage transition.
pub(super) fn dropped_notices(world: &World, id: ObjectId) -> Vec<Notice> {
    let mut notices = vec![Notice {
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
/// A 'Mech may not fire weapons or make physical attacks in the turn it picks up a club
/// (Total Warfare p. 146): a pickup is refused within thirty seconds of either, and ties up
/// every section for the next thirty. It consumes no dice or terrain.
pub fn grab_club(
    world: &mut World,
    id: ObjectId,
    pilot: ObjectId,
    selection: Option<&str>,
) -> Result<Vec<Notice>> {
    super::power::controlled_unit(world, id, pilot)?;
    let unit = &world.btech.constructed_units()[&id];
    ensure!(
        unit.power() == Power::Running && !unit.is_destroyed(),
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
        unit.chassis() != MechChassis::Quad,
        "Quads can't carry a club."
    );
    ensure!(
        unit.posture() != Posture::Prone,
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
    ensure!(
        !unit.fired_this_turn(world.btech.simulation_time()),
        "You fired weapons this turn and cannot also pick up a club."
    );
    ensure!(
        unit.limb_recycle().is_empty(),
        "Your limbs are still recovering from another attack"
    );
    for kind in ArmAttack::HAND_WEAPONS
        .into_iter()
        .filter(|kind| kind.needs_hand())
    {
        for arm in [Arm::Left, Arm::Right] {
            ensure!(
                !kind.available(unit, arm.section())?,
                "You cannot grab a club while carrying a {}",
                kind.name()
            );
        }
    }
    let arm = match selection.map(str::to_ascii_lowercase).as_deref() {
        None if usable(unit, Arm::Left)? => Arm::Left,
        None => Arm::Right,
        Some("l" | "left") => Arm::Left,
        Some("r" | "right") => Arm::Right,
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
        unit.tie_up_every_section();
        world.btech.validate_action(world)?;
        let mut notices = super::broadcast::observer_notices(
            world,
            id,
            "reaches down and yanks a tree out of the ground!",
        );
        notices.push(Notice {
            unit: id,
            text: format!(
                "You reach down and yank a tree out of the ground with your {}.",
                arm.section().name().replace('_', " ")
            ),
        });
        Ok(notices)
    })
}
