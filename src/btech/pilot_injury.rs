//! Atomic tactical cockpit injuries, unconsciousness and scenario pilot loss.
use super::{ConsciousnessCheck, Notice};
use crate::{Flag, ObjectId, World};
use anyhow::{Context, Result, ensure};
use serde::Serialize;

/// Tactical injury outcome; notices are published by the enclosing attack after commit.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TacticalPilotInjury {
    pub injuries: u8,
    pub killed: bool,
    /// The check that decided the warrior's state: the first failed Consciousness Roll, or the
    /// last one passed. `None` when no roll was made (no damage, already unconscious, killed).
    pub consciousness: Option<ConsciousnessCheck>,
}

impl TacticalPilotInjury {
    /// Optional occupant notice for a newly failed consciousness check or scenario death.
    pub fn notice(&self, unit: ObjectId) -> Option<Notice> {
        let text = if self.killed {
            "The pilot is killed from personal injuries!"
        } else if self.consciousness.is_some_and(|check| !check.conscious) {
            "The pilot loses consciousness!"
        } else {
            return None;
        };
        Some(Notice {
            unit,
            text: text.to_owned(),
        })
    }
}

/// Apply tactical crew injury, publishing health, dice and unit loss with or without a player.
/// Assigned in-character pilots use RPG health and require the separate casualty lifecycle.
pub fn injure_tactical_pilot(
    world: &mut World,
    id: ObjectId,
    hits: u8,
    toughness: bool,
) -> Result<TacticalPilotInjury> {
    world.attempt(|world| {
        let report = injure_tactical_pilot_in_candidate(world, id, hits, toughness)?;
        Ok(report)
    })
}

/// Mutate an enclosing attack candidate; its owner discards all changes on failure.
pub(super) fn injure_tactical_pilot_in_candidate(
    world: &mut World,
    id: ObjectId,
    hits: u8,
    toughness: bool,
) -> Result<TacticalPilotInjury> {
    injure_tactical_crew(world, id, hits, toughness, false)
}

/// A terminal blast injures the unit-owned crew after destruction has released its pilot.
/// Personal character health is independent once the cockpit assignment is gone.
pub(super) fn injure_terminal_crew(
    world: &mut World,
    id: ObjectId,
    hits: u8,
) -> Result<TacticalPilotInjury> {
    let (pilot, destroyed, previous) = super::with_unit!(
        world.btech.unit(id).context("Unit is unavailable")?,
        |unit| {
            (
                unit.pilot(),
                unit.is_destroyed(),
                unit.character_pilot_status(),
            )
        }
    );
    ensure!(
        destroyed && pilot.is_none(),
        "Terminal crew injury requires an unassigned wreck"
    );
    // Character-mode cockpit hits still contribute when terminal damage returns to tactical accounting.
    if let Some(previous) = previous {
        let current = world
            .btech
            .vehicles()
            .get(&id)
            .map(|unit| unit.pilot_injuries())
            .unwrap_or_else(|| world.btech.constructed_units()[&id].pilot_injuries());
        super::pilot_health::set_count(
            world,
            id,
            current.max(super::pilot_health::bounded(previous.injuries)),
        );
    }
    injure_tactical_crew(world, id, hits, false, true)
}

/// Share injury accumulation, consciousness dice and saved recovery for live and terminal crews.
/// Every point of damage taken calls for its own Consciousness Roll against the number for the
/// warrior's new total (Total Warfare p. 41); the Toughness advantage rolls the best two of 3d6.
fn injure_tactical_crew(
    world: &mut World,
    id: ObjectId,
    hits: u8,
    toughness: bool,
    terminal: bool,
) -> Result<TacticalPilotInjury> {
    let object = world.objects.get(&id).context("Unit is unavailable")?;
    ensure!(!object.flags.contains(Flag::Going), "Unit is unavailable");
    let character = object.flags.contains(Flag::InCharacter);
    let (pilot, old_injuries, destroyed) = crate::btech::with_unit!(
        world
            .btech
            .unit(id)
            .context("Unit construction state is unavailable")?,
        |unit| { (unit.pilot(), unit.pilot_injuries(), unit.is_destroyed()) }
    );
    ensure!(
        !character || pilot.is_none(),
        "In-character pilot injury requires character casualty handling"
    );
    ensure!(terminal || !destroyed, "Unit is already destroyed");
    if let Some(pilot) = pilot {
        ensure!(
            world.objects.get(&pilot).is_some_and(
                |player| player.location == Some(id) && !player.flags.contains(Flag::Going)
            ),
            "Pilot must be present"
        );
    }
    let injuries = super::pilot_health::bounded(u16::from(old_injuries) + u16::from(hits));
    let mut report = TacticalPilotInjury {
        injuries,
        killed: injuries >= 6,
        consciousness: None,
    };
    if hits == 0 {
        return Ok(report);
    }
    super::pilot_health::set_count(world, id, injuries);
    if let Some(vehicle) = world.btech.vehicles.get_mut(&id) {
        vehicle.pilot_killed = report.killed;
        vehicle.reconcile_crew_loss();
    } else {
        let unit = world.btech.constructed.get_mut(&id).unwrap();
        unit.pilot_killed = report.killed;
        if report.killed {
            unit.pilot = None;
            unit.reconcile_damage();
        }
    }
    if report.killed {
        if let Some(recovery) = pilot.and_then(|pilot| world.btech.recoveries.get_mut(&pilot)) {
            recovery.remaining = 0;
        }
        world.btech.validate_action(world)?;
        return Ok(report);
    }
    // The crew of a wreck is counted but makes no Consciousness Rolls: nothing it could wake
    // up to do remains, and its recovery clock stops.
    if destroyed {
        if let Some(recovery) = pilot.and_then(|pilot| world.btech.recoveries.get_mut(&pilot)) {
            recovery.remaining = 0;
        }
        if pilot.is_none() {
            super::crew_recovery::clear(world, id);
        }
        world.btech.validate_action(world)?;
        return Ok(report);
    }
    for point in 1..=hits {
        let total = super::pilot_health::bounded(u16::from(old_injuries) + u16::from(point));
        let check = match pilot {
            Some(pilot) => {
                super::recovery::check_tactical_consciousness(world, pilot, total, toughness)?
            }
            None => super::crew_recovery::check(world, id, total, toughness)?,
        };
        if check.is_some() {
            report.consciousness = check;
        }
    }
    world.btech.validate_action(world)?;
    Ok(report)
}

/// Health routing shared by chassis-specific damage resolvers.
pub(super) enum PilotInjury {
    Tactical(TacticalPilotInjury),
    Character(super::CharacterPilotInjury),
}

/// Select character health only for an assigned in-character pilot; virtual crews remain tactical.
pub(super) fn injure_in_candidate(
    world: &mut World,
    id: ObjectId,
    hits: u8,
    toughness: bool,
) -> Result<PilotInjury> {
    let pilot = world
        .btech
        .vehicles()
        .get(&id)
        .and_then(|unit| unit.pilot())
        .or_else(|| {
            world
                .btech
                .constructed_units()
                .get(&id)
                .and_then(|unit| unit.pilot())
        });
    if pilot.is_some()
        && world
            .objects
            .get(&id)
            .is_some_and(|object| object.flags.contains(Flag::InCharacter))
    {
        return super::injure_character_pilot(world, id, hits, toughness)
            .map(PilotInjury::Character);
    }
    injure_tactical_pilot_in_candidate(world, id, hits, toughness).map(PilotInjury::Tactical)
}

/// Material damage plus applied tactical pilot injuries; unresolved effects remain explicit.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TacticalImpact {
    pub impact: super::ImpactReport,
    pub pilot_injuries: Vec<TacticalPilotInjury>,
    /// Private balance feedback ordered within this impact cascade.
    pub pilot_notices: Vec<super::PilotNotice>,
    pub notices: Vec<Notice>,
    /// Ordered immediate balance checks and any completed falls.
    pub balance: Vec<super::BalanceReport>,
    pub flooding: Vec<super::SectionExposureReport>,
}

/// Compose a conventional impact and surviving-pilot injuries in one candidate world.
/// Crew effects and dry-ground falls run between criticals; section notices and unsupported casualties remain explicit.
pub fn resolve_tactical_impact(
    world: &mut World,
    id: ObjectId,
    hit: super::Hit,
    damage: u16,
    rules: super::FallRules,
) -> Result<TacticalImpact> {
    world.attempt(|world| {
        let report = resolve_tactical_impact_in_candidate(world, id, hit, damage, rules, None)?;
        Ok(report)
    })
}

/// Reuse the enclosing salvo checkpoint instead of cloning the world for every damage group.
pub(super) fn resolve_tactical_impact_in_candidate(
    world: &mut World,
    id: ObjectId,
    hit: super::Hit,
    damage: u16,
    rules: super::FallRules,
    weapon_effect: Option<super::impact::WeaponEffect>,
) -> Result<TacticalImpact> {
    ensure!(
        world
            .objects
            .get(&id)
            .is_some_and(|object| !object.flags.contains(Flag::InCharacter)),
        "Tactical impact requires a non-character unit"
    );
    ensure!(
        world
            .objects
            .get(&id)
            .is_some_and(|object| !object.flags.contains(Flag::Going)),
        "Unit is unavailable"
    );
    super::impact::resolve_in_candidate(world, id, hit, damage, Some(rules), weapon_effect)
}

/// Administrative tactical injury edits retain recovery timing; the enclosing field action owns rollback.
pub(super) fn set_administrative_injuries(
    world: &mut World,
    id: ObjectId,
    value: &str,
) -> Result<Option<Notice>> {
    let injuries = value
        .trim()
        .parse::<u8>()
        .context("Expected a tactical injury count from 0 to 6")?;
    ensure!(
        injuries <= 6,
        "Expected a tactical injury count from 0 to 6"
    );
    let (pilot, previous, destroyed) = if let Some(unit) = world.btech.constructed_units().get(&id)
    {
        (unit.pilot(), unit.pilot_injuries(), unit.is_destroyed())
    } else {
        let unit = world
            .btech
            .vehicles()
            .get(&id)
            .context("Unit is unavailable")?;
        (unit.pilot(), unit.pilot_injuries(), unit.is_destroyed())
    };
    ensure!(
        !world.objects[&id].flags.contains(Flag::InCharacter) || pilot.is_none(),
        "In-character pilot injury requires character casualty handling"
    );
    if injuries == previous {
        return Ok(None);
    }
    ensure!(
        !destroyed,
        "Cannot edit tactical injuries on a destroyed unit"
    );
    if injuries == 6 {
        // The operator requested an exact terminal count, even if startup
        // projected a larger count that has not yet caused a casualty.
        super::pilot_health::set_count(world, id, 5);
        let report = injure_tactical_pilot_in_candidate(world, id, 1, false)?;
        return Ok(report.notice(id));
    }
    super::pilot_health::set_count(world, id, injuries);
    let recovery = super::with_unit_mut!(world.btech.unit_mut(id).unwrap(), |unit| {
        &mut unit.crew_recovery
    });
    recovery.edit_tactical_injuries(injuries);
    if let Some(pilot) = pilot
        && let Some(recovery) = world.btech.recoveries.get_mut(&pilot)
    {
        recovery.edit_tactical_injuries(injuries);
    }
    Ok(None)
}
