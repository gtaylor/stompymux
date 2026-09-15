//! Atomic tactical cockpit injuries, unconsciousness and scenario pilot loss.
use super::{BattleConsciousnessCheck, BattleNotice};
use crate::{Flag, ObjectId, World};
use anyhow::{Context, Result, ensure};
use serde::Serialize;
use std::sync::Arc;

/// Tactical injury outcome; notices are published by the enclosing attack after commit.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BattlePilotInjury {
    pub injuries: u8,
    pub killed: bool,
    pub consciousness: Option<BattleConsciousnessCheck>,
}

impl BattlePilotInjury {
    /// Optional occupant notice for a newly failed consciousness check or scenario death.
    pub fn notice(&self, unit: ObjectId) -> Option<BattleNotice> {
        let text = if self.killed {
            "The pilot is killed from personal injuries!"
        } else if self.consciousness.is_some_and(|check| !check.conscious) {
            "The pilot loses consciousness!"
        } else {
            return None;
        };
        Some(BattleNotice {
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
) -> Result<BattlePilotInjury> {
    let mut candidate = world.clone();
    let report = injure_tactical_pilot_in_candidate(&mut candidate, id, hits, toughness)?;
    *world = candidate;
    Ok(report)
}

/// Mutate an enclosing attack candidate; its owner discards all changes on failure.
pub(super) fn injure_tactical_pilot_in_candidate(
    world: &mut World,
    id: ObjectId,
    hits: u8,
    toughness: bool,
) -> Result<BattlePilotInjury> {
    injure_tactical_crew(world, id, hits, toughness, false)
}

/// A terminal blast injures the unit-owned crew after destruction has released its pilot.
/// Personal character health is independent once the cockpit assignment is gone.
pub(super) fn injure_terminal_crew(
    world: &mut World,
    id: ObjectId,
    hits: u8,
) -> Result<BattlePilotInjury> {
    let (pilot, destroyed, previous) = if let Some(unit) = world.btech.vehicles().get(&id) {
        (
            unit.pilot(),
            unit.is_destroyed(),
            unit.character_pilot_status(),
        )
    } else {
        let unit = world
            .btech
            .constructed_units()
            .get(&id)
            .context("Unit is unavailable")?;
        (
            unit.pilot(),
            unit.is_destroyed(),
            unit.character_pilot_status(),
        )
    };
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
fn injure_tactical_crew(
    world: &mut World,
    id: ObjectId,
    hits: u8,
    toughness: bool,
    terminal: bool,
) -> Result<BattlePilotInjury> {
    let object = world.objects.get(&id).context("Unit is unavailable")?;
    ensure!(!object.flags.contains(Flag::Going), "Unit is unavailable");
    let character = object.flags.contains(Flag::InCharacter);
    let (pilot, old_injuries, destroyed) = if let Some(vehicle) = world.btech.vehicles().get(&id) {
        (
            vehicle.pilot(),
            vehicle.pilot_injuries(),
            vehicle.is_destroyed(),
        )
    } else {
        let unit = world
            .btech
            .constructed_units()
            .get(&id)
            .context("Unit construction state is unavailable")?;
        (unit.pilot(), unit.pilot_injuries(), unit.is_destroyed())
    };
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
    let mut report = BattlePilotInjury {
        injuries,
        killed: injuries >= 6,
        consciousness: None,
    };
    if hits == 0 {
        return Ok(report);
    }
    super::pilot_health::set_count(world, id, injuries);
    if let Some(vehicle) = Arc::make_mut(&mut world.btech.vehicles).get_mut(&id) {
        vehicle.pilot_killed = report.killed;
        vehicle.reconcile_crew_loss();
    } else {
        let unit = Arc::make_mut(&mut world.btech.constructed)
            .get_mut(&id)
            .unwrap();
        unit.pilot_killed = report.killed;
        if report.killed {
            unit.pilot = None;
            unit.reconcile_damage();
        }
    }
    if let Some(pilot) = pilot {
        if report.killed {
            if let Some(recovery) = Arc::make_mut(&mut world.btech.recoveries).get_mut(&pilot) {
                recovery.remaining = 0;
            }
        } else {
            report.consciousness =
                super::recovery::check_tactical_consciousness(world, pilot, injuries, toughness)?;
        }
    } else if !report.killed {
        report.consciousness = super::crew_recovery::check(world, id, injuries, toughness)?;
    }
    world.btech.validate(world)?;
    Ok(report)
}

/// Health routing shared by chassis-specific damage resolvers.
pub(super) enum PilotInjury {
    Tactical(BattlePilotInjury),
    Character(super::BattleCharacterPilotInjury),
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
pub struct BattleTacticalImpact {
    pub impact: super::BattleImpactReport,
    pub pilot_injuries: Vec<BattlePilotInjury>,
    /// Private balance feedback ordered within this impact cascade.
    pub pilot_notices: Vec<super::BattlePilotNotice>,
    pub notices: Vec<BattleNotice>,
    /// Ordered immediate balance checks and any completed falls.
    pub balance: Vec<super::BattleBalanceReport>,
    pub flooding: Vec<super::BattleSectionExposureReport>,
}

/// Compose a conventional impact and surviving-pilot injuries in one candidate world.
/// Crew effects and dry-ground falls run between criticals; section notices and unsupported casualties remain explicit.
pub fn resolve_tactical_impact(
    world: &mut World,
    id: ObjectId,
    hit: super::BattleHit,
    damage: u16,
    rules: super::BattleFallRules,
) -> Result<BattleTacticalImpact> {
    let mut candidate = world.clone();
    let report =
        resolve_tactical_impact_in_candidate(&mut candidate, id, hit, damage, rules, None)?;
    *world = candidate;
    Ok(report)
}

/// Reuse the enclosing salvo checkpoint instead of cloning the world for every damage group.
pub(super) fn resolve_tactical_impact_in_candidate(
    world: &mut World,
    id: ObjectId,
    hit: super::BattleHit,
    damage: u16,
    rules: super::BattleFallRules,
    weapon_effect: Option<super::impact::WeaponEffect>,
) -> Result<BattleTacticalImpact> {
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
) -> Result<Option<BattleNotice>> {
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
    let recovery = if let Some(unit) = Arc::make_mut(&mut world.btech.constructed).get_mut(&id) {
        &mut unit.crew_recovery
    } else {
        let unit = Arc::make_mut(&mut world.btech.vehicles)
            .get_mut(&id)
            .unwrap();
        &mut unit.crew_recovery
    };
    recovery.edit_tactical_injuries(injuries);
    if let Some(pilot) = pilot
        && let Some(recovery) = Arc::make_mut(&mut world.btech.recoveries).get_mut(&pilot)
    {
        recovery.edit_tactical_injuries(injuries);
    }
    Ok(None)
}
