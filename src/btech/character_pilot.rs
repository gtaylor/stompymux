//! In-character cockpit injuries, persistent health and fatal unit loss.
use super::{BattleCharacterInjury, BattleConsciousnessCheck};
use crate::{Flag, ObjectId, World};
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};

/// Character-mode pilot damage is independent of the six-hit tactical injury limit.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BattleCharacterPilotStatus {
    pub injuries: u16,
    pub killed: bool,
}

/// Health and consciousness outcomes; the action owner publishes fatal evacuation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BattleCharacterPilotInjury {
    pub player: ObjectId,
    pub injury: BattleCharacterInjury,
    pub consciousness: Option<BattleConsciousnessCheck>,
}

/// Apply health, consciousness and unit consequences atomically to a present in-character pilot.
pub fn injure_character_pilot(
    world: &mut World,
    unit: ObjectId,
    hits: u8,
    toughness: bool,
) -> Result<BattleCharacterPilotInjury> {
    let object = world.objects.get(&unit).context("Unit is unavailable")?;
    ensure!(
        object.flags.contains(Flag::InCharacter) && !object.flags.contains(Flag::Going),
        "Character injury requires a live in-character unit"
    );
    let (destroyed, pilot, status) = if let Some(vehicle) = world.btech.vehicles().get(&unit) {
        (
            vehicle.is_destroyed(),
            vehicle.pilot(),
            vehicle.character_pilot_status(),
        )
    } else {
        let state = world
            .btech
            .constructed_units()
            .get(&unit)
            .context("Unit construction state is unavailable")?;
        (
            state.is_destroyed(),
            state.pilot(),
            state.character_pilot_status(),
        )
    };
    ensure!(!destroyed, "Unit is already destroyed");
    let pilot = pilot.context("Unit has no assigned pilot")?;
    ensure!(
        world.objects.get(&pilot).is_some_and(
            |object| object.location == Some(unit) && !object.flags.contains(Flag::Going)
        ),
        "Pilot must be present"
    );
    let mut status = status.unwrap_or_default();
    let mut candidate = world.clone();
    let injury = super::injure_character(&mut candidate, pilot, hits)?;
    let mut report = BattleCharacterPilotInjury {
        player: pilot,
        injury,
        consciousness: None,
    };
    if hits == 0 {
        return Ok(report);
    }
    if injury.fatal {
        status.killed = true;
        store_status(&mut candidate, unit, status);
        if let Some(recovery) = candidate.btech.recoveries.get_mut(&pilot) {
            recovery.remaining = 0;
        }
    } else {
        let pain = candidate
            .btech
            .character_values()
            .get(&pilot)
            .is_some_and(|values| super::advantages::enabled(values, "Pain_Resistance"));
        report.consciousness =
            super::check_character_consciousness(&mut candidate, pilot, pain, toughness)?;
        status.injuries = u16::from(super::pilot_health::bounded(
            status.injuries.saturating_add(u16::from(hits)),
        ));
        store_status(&mut candidate, unit, status);
    }
    candidate.btech.validate_action(&candidate)?;
    *world = candidate;
    Ok(report)
}

/// Keep the health calculation shared while each chassis performs its own wreck cleanup.
fn store_status(world: &mut World, id: ObjectId, status: BattleCharacterPilotStatus) {
    let injuries = super::pilot_health::bounded(status.injuries);
    let status = BattleCharacterPilotStatus {
        injuries: injuries.into(),
        ..status
    };
    super::pilot_health::set_count(world, id, injuries);
    if let Some(vehicle) = world.btech.vehicles.get_mut(&id) {
        vehicle.character_pilot = Some(status);
        if status.killed {
            vehicle.reconcile_crew_loss();
        }
        return;
    }
    let unit = world.btech.constructed.get_mut(&id).unwrap();
    unit.character_pilot = Some(status);
    if status.killed {
        unit.pilot = None;
        unit.reconcile_damage();
    }
}

/// Publish initial consciousness feedback only to the affected pilot; no second roll is performed.
pub(super) fn notify_injury(
    scripts: &crate::Scripts,
    report: &BattleCharacterPilotInjury,
) -> Result<()> {
    let Some(check) = report.consciousness else {
        return Ok(());
    };
    let recipient = super::BattleMessageTarget::Player(report.player);
    super::notify_message(scripts, recipient, "You attempt to keep consciousness!")?;
    super::notify_message(
        scripts,
        recipient,
        &format!(
            "Retain Conciousness on: {}  \tRoll: {}",
            check.target, check.roll
        ),
    )?;
    if !check.conscious {
        super::notify_message(
            scripts,
            recipient,
            "Consciousness slips away from you as you enter a sea of darkness...",
        )?;
    }
    Ok(())
}
