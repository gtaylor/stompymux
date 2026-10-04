//! Saved anatomical aim and shared target immobility, independent of the firing chassis.
use super::*;
use crate::{ObjectId, World};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

/// Anatomical preference retains its selected target class across subsequent lock changes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "class", content = "section", rename_all = "snake_case")]
pub enum BattleAimSelection {
    Mech(BattleSection),
    GroundVehicle(BattleVehicleSection),
    Vtol(BattleVehicleSection),
}

impl BattleAimSelection {
    /// Ground anatomy has no rotor slot; saved preferences must remain selectable.
    pub(super) fn validate(self) -> Result<()> {
        anyhow::ensure!(
            self != Self::GroundVehicle(BattleVehicleSection::Rotor),
            "Invalid aimed section for ground vehicle"
        );
        Ok(())
    }

    /// Numeric anatomical identity used when an immobile target has a different class from the saved preference.
    pub(super) fn slot(self) -> usize {
        match self {
            Self::Mech(section) => BattleSection::ALL
                .iter()
                .position(|value| *value == section)
                .unwrap(),
            Self::GroundVehicle(section) | Self::Vtol(section) => match section {
                BattleVehicleSection::Left => 0,
                BattleVehicleSection::Right => 1,
                BattleVehicleSection::Front => 2,
                BattleVehicleSection::Rear => 3,
                BattleVehicleSection::Turret => 4,
                BattleVehicleSection::Rotor => 5,
            },
        }
    }

    /// Targeting computers require the saved unit class, independently of its current lock identity.
    pub(super) fn matches(self, world: &World, target: ObjectId) -> bool {
        match self {
            Self::Mech(_) => world.btech.constructed_units().contains_key(&target),
            Self::GroundVehicle(_) => world
                .btech
                .vehicles()
                .get(&target)
                .is_some_and(|unit| unit.definition().movement != BattleVehicleMovement::Vtol),
            Self::Vtol(_) => world
                .btech
                .vehicles()
                .get(&target)
                .is_some_and(|unit| unit.definition().movement == BattleVehicleMovement::Vtol),
        }
    }
}

/// Read the saved selection without requiring a current lock or a running unit.
pub fn aimed_section(world: &World, id: ObjectId) -> Result<Option<BattleAimSelection>> {
    let unit = world
        .btech
        .unit(id)
        .context("Unit construction state is unavailable")?;
    Ok(unit.aimed_section())
}

/// Select using the locked target's anatomy; clearing needs no current target.
pub fn set_aimed_section(
    world: &mut World,
    id: ObjectId,
    pilot: ObjectId,
    section: Option<&str>,
) -> Result<BattleNotice> {
    super::power::controlled_running_unit(world, id, pilot)?;
    let (selection, text) = if let Some(section) = section {
        let target = super::scanner::scanner_unit(world, id)
            .and_then(|unit| unit.selected)
            .context("Error: You need to be locked onto something to target its part!")?;
        if let Some(unit) = world.btech.vehicles().get(&target) {
            let section = BattleVehicleSection::parse_location(section)
                .map_err(|_| anyhow::anyhow!("Invalid location!"))?;
            let selection = if unit.definition().movement == BattleVehicleMovement::Vtol {
                BattleAimSelection::Vtol(section)
            } else {
                anyhow::ensure!(section != BattleVehicleSection::Rotor, "Invalid location!");
                BattleAimSelection::GroundVehicle(section)
            };
            (
                Some(selection),
                format!("{} targetted.", section.name().replace('_', " ")),
            )
        } else {
            let unit = world
                .btech
                .constructed_units()
                .get(&target)
                .context("Error: You need to be locked onto something to target its part!")?;
            let section = unit
                .chassis()
                .parse_location(section)
                .map_err(|_| anyhow::anyhow!("Invalid location!"))?;
            (
                Some(BattleAimSelection::Mech(section)),
                format!(
                    "{} targetted.",
                    unit.chassis().section_name(section).replace('_', " ")
                ),
            )
        }
    } else {
        (None, "Targetting disabled.".into())
    };
    crate::btech::with_unit_mut!(
        world
            .btech
            .unit_mut(id)
            .context("Unit construction state is unavailable")?,
        |unit| {
            unit.aimed_section = selection;
        }
    );
    Ok(BattleNotice { unit: id, text })
}

/// Mech immobility is distinct from standing still, falling or losing a leg.
pub(super) fn mech_immobile(world: &World, unit: &BattleUnit) -> bool {
    unit.fortified
        || unit.crew_recovery().remaining > 0
        || unit.power() != BattlePower::Running
        || unit
            .pilot()
            .is_some_and(|pilot| world.btech.unconscious(pilot))
}

/// Shared immobile-target predicate for movement bonuses, aimed fire and targeting-computer policy.
pub(super) fn immobile(world: &World, target: ObjectId) -> Result<bool> {
    if let Some(unit) = world.btech.vehicles().get(&target) {
        return Ok(unit.fortified
            || unit.crew_recovery().remaining > 0
            || unit.power() != BattlePower::Running
            || unit.immobilized()
            || unit.definition().movement == BattleVehicleMovement::Stationary
            || unit
                .pilot()
                .is_some_and(|pilot| world.btech.unconscious(pilot)));
    }
    Ok(mech_immobile(
        world,
        world
            .btech
            .constructed_units()
            .get(&target)
            .context("Target construction is unavailable")?,
    ))
}

/// Head selection overrides computer assistance; mobile directed computer fire costs three instead of granting one.
pub(super) fn apply_aim(
    world: &World,
    shooter: ObjectId,
    target: ObjectId,
    weapon: BattleWeapon,
    aim: &mut BattleAimModifiers,
) -> Result<()> {
    let Some(selection) = aimed_section(world, shooter)? else {
        return Ok(());
    };
    let immobile = immobile(world, target)?;
    if selection.slot() == 7
        && world.btech.constructed_units().contains_key(&target)
        && weapon.gunnery_skill(true) != "Gunnery-Missile"
    {
        aim.aimed_section = if immobile { 7 } else { 25 };
        aim.targeting_computer = 0;
    } else if aim.targeting_computer == -1 && !immobile {
        aim.targeting_computer = 3;
    }
    Ok(())
}

/// Host transaction publishes the selection notice only after validating the candidate world.
pub(crate) fn action(
    scripts: &crate::Scripts,
    config: &crate::Config,
    id: ObjectId,
    pilot: ObjectId,
    section: Option<&str>,
) -> Result<Option<BattleAimSelection>> {
    scripts.atomic(|_| {
        let notice = set_aimed_section(&mut scripts.world.borrow_mut(), id, pilot, section)?;
        scripts.world.borrow().validate_action(config)?;
        let selected = aimed_section(&scripts.world.borrow(), id)?;
        super::notify_unit(scripts, notice)?;
        Ok(selected)
    })
}

/// Cockpit authority precedes argument validation, matching ordinary targeting controls.
pub(crate) fn command(
    ctx: &crate::CommandContext<'_>,
    input: &crate::CommandInput,
) -> Result<crate::CommandAction> {
    let result = (|| {
        let id = ctx
            .scripts
            .world
            .borrow()
            .objects
            .get(&ctx.player)
            .and_then(|player| player.location)
            .context("Enter a unit first")?;
        super::power::controlled_running_unit(&ctx.scripts.world.borrow(), id, ctx.player)?;
        let args: Vec<_> = input.args.split_whitespace().collect();
        anyhow::ensure!(args.len() == 1, "Invalid number of arguments to function!");
        action(
            ctx.scripts,
            ctx.config,
            id,
            ctx.player,
            (args[0] != "-").then_some(args[0]),
        )
    })();
    Ok(match result {
        Ok(_) => crate::CommandAction::Continue,
        Err(error) => {
            crate::CommandAction::Report(crate::CommandReport::Reply(format!("{error:#}")))
        }
    })
}

/// Cockpit fire/sight messages expose the selected anatomy only for matching non-missile targets.
pub(super) fn suffix(
    world: &World,
    shooter: ObjectId,
    target: ObjectId,
    weapon: BattleWeapon,
) -> Result<String> {
    if weapon.gunnery_skill(true) == "Gunnery-Missile" {
        return Ok(String::new());
    }
    let Some(selection) =
        aimed_section(world, shooter)?.filter(|selection| selection.matches(world, target))
    else {
        return Ok(String::new());
    };
    let name = match selection {
        BattleAimSelection::Mech(section) => world.btech.constructed_units()[&target]
            .chassis()
            .section_name(section),
        BattleAimSelection::GroundVehicle(section) | BattleAimSelection::Vtol(section) => {
            section.name()
        }
    };
    Ok(format!("'s {}", name.replace('_', " ")))
}
