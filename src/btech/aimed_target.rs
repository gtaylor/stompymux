//! Saved anatomical aim and shared target immobility, independent of the firing chassis.
use super::*;
use crate::{ObjectId, World};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

/// Anatomical preference retains its selected target class across subsequent lock changes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "class", content = "section", rename_all = "snake_case")]
pub enum AimSelection {
    Mech(MechSection),
    GroundVehicle(VehicleSection),
    Vtol(VehicleSection),
}

impl AimSelection {
    /// Ground anatomy has no rotor slot; saved preferences must remain selectable.
    pub(super) fn validate(self) -> Result<()> {
        anyhow::ensure!(
            self != Self::GroundVehicle(VehicleSection::Rotor),
            "Invalid aimed section for ground vehicle"
        );
        Ok(())
    }

    /// Numeric anatomical identity used when an immobile target has a different class from the saved preference.
    pub(super) fn slot(self) -> usize {
        match self {
            Self::Mech(section) => MechSection::ALL
                .iter()
                .position(|value| *value == section)
                .unwrap(),
            Self::GroundVehicle(section) | Self::Vtol(section) => match section {
                VehicleSection::Left => 0,
                VehicleSection::Right => 1,
                VehicleSection::Front => 2,
                VehicleSection::Rear => 3,
                VehicleSection::Turret => 4,
                VehicleSection::Rotor => 5,
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
                .is_some_and(|unit| unit.definition().movement != VehicleMovement::Vtol),
            Self::Vtol(_) => world
                .btech
                .vehicles()
                .get(&target)
                .is_some_and(|unit| unit.definition().movement == VehicleMovement::Vtol),
        }
    }
}

/// Read the saved selection without requiring a current lock or a running unit.
pub fn aimed_section(world: &World, id: ObjectId) -> Result<Option<AimSelection>> {
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
) -> Result<Notice> {
    super::power::controlled_running_unit(world, id, pilot)?;
    let (selection, text) = if let Some(section) = section {
        let target = super::scanner::scanner_unit(world, id)
            .and_then(|unit| unit.selected)
            .context("Error: You need to be locked onto something to target its part!")?;
        if let Some(unit) = world.btech.vehicles().get(&target) {
            let section = VehicleSection::parse_location(section)
                .map_err(|_| anyhow::anyhow!("Invalid location!"))?;
            let selection = if unit.definition().movement == VehicleMovement::Vtol {
                AimSelection::Vtol(section)
            } else {
                anyhow::ensure!(section != VehicleSection::Rotor, "Invalid location!");
                AimSelection::GroundVehicle(section)
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
                Some(AimSelection::Mech(section)),
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
    Ok(Notice { unit: id, text })
}

/// Mech immobility is distinct from standing still, falling or losing a leg: a shut-down
/// Mech or one whose MechWarrior is unconscious is immobile (Total Warfare p. 110), and so is a
/// biped that has lost both legs and both arms, or a four-legged Mech that has lost all four
/// legs (p. 123).
pub(super) fn mech_immobile(world: &World, unit: &Mech) -> bool {
    unit.fortified
        || unit.crew_recovery().remaining > 0
        || unit.power() != Power::Running
        || unit
            .pilot()
            .is_some_and(|pilot| world.btech.unconscious(pilot))
        || limbless(unit)
}

/// Whether a Mech has lost the limbs it needs to move at all: both legs and both arms of a
/// biped, or all four legs of a quad (Total Warfare p. 123).
fn limbless(unit: &Mech) -> bool {
    let chassis = unit.chassis();
    if unit.unavailable_legs() < chassis.legs().len() {
        return false;
    }
    if chassis == MechChassis::Quad {
        return true;
    }
    [MechSection::LeftArm, MechSection::RightArm]
        .into_iter()
        .all(|arm| unit.sections()[&arm].internal == 0 || unit.section_disabled(arm))
}

/// Shared immobile-target predicate for movement bonuses, aimed fire and targeting-computer policy.
pub(super) fn immobile(world: &World, target: ObjectId) -> Result<bool> {
    if let Some(unit) = world.btech.vehicles().get(&target) {
        return Ok(unit.fortified
            || unit.crew_recovery().remaining > 0
            || unit.power() != Power::Running
            || unit.immobilized()
            || unit.definition().movement == VehicleMovement::Stationary
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

/// What an aimed shot would be made with: the weapon, how it is firing and whether the shot
/// is indirect or one gun of a machine gun array's volley.
#[derive(Debug, Clone, Copy)]
pub(super) struct AimedAttack {
    pub weapon: Weapon,
    pub fire_mode: FireMode,
    pub ammunition: AmmunitionMode,
    pub indirect: bool,
    pub volley: bool,
}

impl AimedAttack {
    /// Whether the attack may be aimed at an immobile target (Total Warfare p. 110): not with
    /// missile launchers, area-effect weapons, cluster or flak attacks (LB-X cluster rounds and
    /// machine gun array volleys among them), hyper-assault Gauss rifles, indirect fire, or a
    /// rapid-fire weapon firing more than one shot.
    pub(super) fn immobile_allowed(self) -> bool {
        self.weapon.gunnery_skill(true) != "Gunnery-Missile"
            && !self.weapon.is_artillery()
            && !self.weapon.is_hag()
            && self.ammunition.munition() != AmmunitionMode::Cluster
            && self.fire_mode.rounds_per_cycle() == 1
            && !self.indirect
            && !self.volley
    }

    /// Whether a targeting computer may make this an aimed shot at an active target (Total
    /// Warfare p. 143): every attack an immobile target admits, except pulse weapons. The
    /// computer itself must also assist the weapon.
    pub(super) fn computer_allowed(self) -> bool {
        self.immobile_allowed() && !self.weapon.is_pulse()
    }
}

/// Apply a saved aim to the to-hit number. An aim at an immobile Mech's head costs +3 in
/// place of the immobile -4 (Total Warfare p. 110) and can be made only without the targeting
/// computer; a head aim at a mobile Mech is impossible. A targeting computer's aimed shot at an
/// active target of the saved class costs +3 instead of the computer's -1 (p. 143). An attack
/// that may not be aimed is an ordinary shot.
pub(super) fn apply_aim(
    world: &World,
    shooter: ObjectId,
    target: ObjectId,
    attack: AimedAttack,
    aim: &mut AimModifiers,
) -> Result<()> {
    let Some(selection) = aimed_section(world, shooter)? else {
        return Ok(());
    };
    if !attack.immobile_allowed() {
        return Ok(());
    }
    let immobile = immobile(world, target)?;
    if selection == AimSelection::Mech(MechSection::Head)
        && world.btech.constructed_units().contains_key(&target)
    {
        aim.aimed_section = if immobile { 7 } else { 25 };
        aim.targeting_computer = 0;
        return Ok(());
    }
    if aim.targeting_computer == -1
        && !immobile
        && attack.computer_allowed()
        && selection.matches(world, target)
    {
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
) -> Result<Option<AimSelection>> {
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
    weapon: Weapon,
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
        AimSelection::Mech(section) => world.btech.constructed_units()[&target]
            .chassis()
            .section_name(section),
        AimSelection::GroundVehicle(section) | AimSelection::Vtol(section) => section.name(),
    };
    Ok(format!("'s {}", name.replace('_', " ")))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An attack with the given weapon, firing mode and ammunition, fired directly and alone.
    fn attack(weapon: Weapon, fire_mode: FireMode, ammunition: AmmunitionMode) -> AimedAttack {
        AimedAttack {
            weapon,
            fire_mode,
            ammunition,
            indirect: false,
            volley: false,
        }
    }

    /// Missile launchers, cluster rounds, hyper-assault Gauss rifles, indirect fire, machine gun
    /// array volleys and multi-shot rapid fire may not be aimed (Total Warfare p. 110); a
    /// targeting computer's aimed shot excludes pulse weapons as well (p. 143).
    #[test]
    fn aimed_shots_exclude_the_attacks_the_book_lists() {
        use AmmunitionMode as Ammo;
        use FireMode as Mode;
        let laser = attack(Weapon::MediumLaser, Mode::Normal, Ammo::Normal);
        assert!(laser.immobile_allowed() && laser.computer_allowed());
        let pulse = attack(Weapon::MediumPulseLaser, Mode::Normal, Ammo::Normal);
        assert!(pulse.immobile_allowed() && !pulse.computer_allowed());
        let single_ultra = attack(Weapon::UltraAc5, Mode::Normal, Ammo::Normal);
        assert!(single_ultra.immobile_allowed() && single_ultra.computer_allowed());
        for excluded in [
            attack(Weapon::Lrm10, Mode::Normal, Ammo::Normal),
            attack(Weapon::Lbx10, Mode::Normal, Ammo::Cluster),
            attack(Weapon::ClanHag20, Mode::Normal, Ammo::Normal),
            attack(Weapon::UltraAc5, Mode::Ultra, Ammo::Normal),
            attack(Weapon::RotaryAc5, Mode::Rotary4, Ammo::Normal),
            attack(Weapon::LongTom, Mode::Normal, Ammo::Normal),
            AimedAttack {
                volley: true,
                ..attack(Weapon::MachineGun, Mode::Normal, Ammo::Normal)
            },
            AimedAttack {
                indirect: true,
                ..laser
            },
        ] {
            assert!(!excluded.immobile_allowed(), "{excluded:?}");
            assert!(!excluded.computer_allowed(), "{excluded:?}");
        }
        let gun = attack(Weapon::MachineGun, Mode::Normal, Ammo::Normal);
        assert!(gun.immobile_allowed());
    }
}
