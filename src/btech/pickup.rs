//! Shared pickup admission using current cockpit, movement, equipment and relationship state.
use super::{
    BattleMechChassis, BattlePosture, BattlePower, BattleSection, BattleSystem,
    BattleVehicleMovement, BattleVtolFlightPhase,
};
use crate::{Flag, Kind, ObjectId, World};
use anyhow::{Context, Result, ensure};

/// Read the explicit scenario permission for towing an out-of-character target.
pub fn unit_towable(world: &World, id: ObjectId) -> Result<bool> {
    if let Some(unit) = world.btech.vehicles().get(&id) {
        return Ok(unit.towable);
    }
    Ok(world
        .btech
        .constructed_units()
        .get(&id)
        .context("Unit construction is unavailable")?
        .towable)
}

/// Trusted scenario edit; the caller owns administrative authority and persistence.
pub fn set_towable(world: &mut World, id: ObjectId, enabled: bool) -> Result<()> {
    live(world, id)?;
    if let Some(unit) = world.btech.vehicles.get_mut(&id) {
        unit.towable = enabled;
        return Ok(());
    }
    world
        .btech
        .constructed
        .get_mut(&id)
        .context("Unit construction is unavailable")?
        .towable = enabled;
    Ok(())
}

/// Pickup accepts only live object identities; destroyed material remains a tow candidate.
fn live(world: &World, id: ObjectId) -> Result<()> {
    ensure!(world.objects.get(&id).is_some_and(|object| object.kind == Kind::Thing && !object.flags.contains(Flag::Going)), "Pickup unit is unavailable");
    Ok(())
}

/// Read installed towing technology without duplicating its gameplay meaning across chassis.
fn special(world: &World, id: ObjectId, name: &str) -> bool {
    world.btech.vehicles().get(&id).map_or_else(
        || {
            world.btech.constructed_units()[&id]
                .definition()
                .has_special(name)
        },
        |unit| unit.definition().has_special(name),
    )
}

/// Check pickup eligibility without stopping, preparing or attaching either participant.
/// The eventual pickup transaction must recheck this immediately before its mutations.
/// Friendly running targets may pass: shutting them down belongs to pickup preparation.
pub fn pickup_admission(
    world: &World,
    carrier: ObjectId,
    pilot: ObjectId,
    target: ObjectId,
) -> Result<()> {
    super::targeting::controlled(world, carrier, pilot)?;
    live(world, carrier)?;
    live(world, target)?;
    super::fortification::require_mobile(world, carrier)?;
    super::fortification::require_mobile(world, target)?;
    ensure!(carrier != target, "A unit cannot pick itself up");
    let source = super::scanner::scanner_unit(world, carrier).context("Carrier is unavailable")?;
    let victim = super::scanner::scanner_unit(world, target).context("Target is unavailable")?;
    let position = source
        .position
        .context("Place the carrier on a battlefield first")?;
    ensure!(
        victim.position == Some(position),
        "You need to be in the same hex!"
    );
    ensure!(
        super::visible_contact(world, carrier, target)?.is_some()
            && !super::unit_terrain_los(world, carrier, target)?.blocked,
        "That target is not in your line of sight."
    );
    ensure!(
        !special(world, target, "Carrier_Tech") || special(world, carrier, "Carrier_Tech"),
        "You cannot handle the mass on that carrier."
    );
    ensure!(
        !world.btech.tows().contains_key(&carrier),
        "You are already carrying a tow"
    );
    ensure!(
        world.btech.towed_by(carrier).is_none() && world.btech.towed_by(target).is_none(),
        "That unit is already being towed"
    );
    ensure!(!victim.signature.hidden, "You cannot pickup hiding targets");
    let tons = if let Some(unit) = world.btech.vehicles().get(&carrier) {
        ensure!(unit.crew_recovery().remaining == 0, "You are unconscious");
        ensure!(
            special(world, carrier, "SalvageTech"),
            "This vehicle requires salvage equipment to pick up a unit"
        );
        ensure!(
            !unit.rotor_destroyed()
                && (unit.free_fall().is_none() && unit.orbital_drop().is_none()),
            "You are in no position to pick anything up"
        );
        if let Some(flight) = unit.vtol_flight() {
            ensure!(
                matches!(
                    flight.phase,
                    BattleVtolFlightPhase::Landed | BattleVtolFlightPhase::Airborne
                ),
                "Finish the flight transition before pickup"
            );
            ensure!(
                flight.vertical_speed.abs() <= 1.0,
                "You are moving too fast to attempt a pickup."
            );
        }
        unit.definition().tons
    } else {
        let unit = &world.btech.constructed_units()[&carrier];
        ensure!(unit.crew_recovery().remaining == 0, "You are unconscious");
        ensure!(
            unit.chassis() != BattleMechChassis::Quad,
            "Quads cannot pick up units"
        );
        ensure!(
            unit.posture() == BattlePosture::Standing,
            "Stand before attempting pickup"
        );
        ensure!(
            unit.flight().is_none()
                && (unit.free_fall().is_none() && unit.orbital_drop().is_none()),
            "Finish airborne movement before pickup"
        );
        ensure!(
            unit.carried_club().is_none(),
            "Put down the club before pickup"
        );
        let arms = [BattleSection::LeftArm, BattleSection::RightArm];
        ensure!(
            arms.iter().all(|arm| unit.sections()[arm].internal > 0),
            "Both arms must survive to pick up a unit"
        );
        let mut functioning = false;
        for arm in arms {
            functioning |= super::physical::actuator(unit, arm, 0, BattleSystem::ShoulderOrHip)?
                && super::physical::actuator(unit, arm, 3, BattleSystem::HandOrFootActuator)?;
        }
        ensure!(functioning, "You need a functioning arm to pick things up");
        unit.definition().tons
    };
    ensure!(
        tons >= 5
            && (world.objects[&target].flags.contains(Flag::InCharacter)
                || unit_towable(world, target)?),
        "You can't tow that!"
    );
    ensure!(
        source.speed.abs() <= 1.0,
        "You are moving too fast to attempt a pickup."
    );
    if let Some(unit) = world.btech.vehicles().get(&target) {
        ensure!(
            unit.definition().movement != BattleVehicleMovement::Stationary,
            "That target is immobile"
        );
        ensure!(
            unit.burning_sections().is_empty(),
            "You can't tow a burning unit!"
        );
        ensure!(
            (unit.free_fall().is_none() && unit.orbital_drop().is_none()),
            "Wait until the falling target lands"
        );
    } else {
        let unit = &world.btech.constructed_units()[&target];
        ensure!(
            unit.flight().is_none()
                && (unit.free_fall().is_none() && unit.orbital_drop().is_none()),
            "Wait until the airborne target lands"
        );
    }
    let source_height =
        super::unit_elevation(world, carrier)?.context("Carrier height unavailable")?;
    let target_height =
        super::unit_elevation(world, target)?.context("Target height unavailable")?;
    let difference = i64::from(source_height) - i64::from(target_height);
    ensure!(
        (-2..=3).contains(&difference),
        "You are too far above or below the target"
    );
    let tile =
        world.btech.maps()[&position.map].hex(i64::from(position.x), i64::from(position.y))?;
    ensure!(
        !(tile.terrain() == super::Terrain::Bridge && target_height <= 0 && source_height > 0),
        "You need to be under the bridge to pick up this unit."
    );
    if source.signature.team != victim.signature.team {
        ensure!(
            victim.power != BattlePower::Running,
            "You cannot pick up a running enemy unit"
        );
        ensure!(
            victim.destroyed || victim.signature.team == 0,
            "You cannot pick up an intact enemy unit"
        );
    }
    Ok(())
}

/// Prepare an admitted target on the caller's unpublished pickup candidate.
/// The enclosing action must release any tow owned by the target, then attach,
/// synchronize height, apply terrain consequences and publish returned notices.
/// This helper deliberately does not establish or release tow ownership.
pub(super) fn prepare_target(
    world: &mut World,
    target: ObjectId,
    rules: super::BattleFallRules,
) -> Result<Vec<super::BattleNotice>> {
    let power = super::scanner::scanner_unit(world, target)
        .context("Target is unavailable")?
        .power;
    // Pickup arrests translation before shutdown, avoiding a shutdown-induced moving fall.
    if let Some(unit) = world.btech.vehicles.get_mut(&target) {
        unit.halt();
        unit.dig = super::BattleDigState::default();
        unit.building_entry = None;
        if let Some(flight) = &mut unit.vtol_flight {
            *flight = super::BattleVtolFlight {
                altitude: flight.altitude,
                ..Default::default()
            };
        }
    } else {
        let unit = world
            .btech
            .constructed
            .get_mut(&target)
            .context("Target is unavailable")?;
        if let Some(motion) = &mut unit.motion {
            motion.stop_translation();
            motion.desired_heading = motion.heading;
        }
        unit.posture = BattlePosture::Prone;
        unit.facing.torso = super::BattleTorso::Center;
        unit.facing.arms_flipped = false;
        unit.stand_timer = None;
        unit.hull_down = Default::default();
        unit.building_entry = None;
    }
    if power == BattlePower::Off {
        return Ok(Vec::new());
    }
    super::power::stop_admitted(world, target, rules)
}

/// Admit and prepare a pickup target atomically, without attaching tow cables.
/// This is a domain preparation operation for an enclosing pickup transaction,
/// not a complete player action: attachment, terrain effects and publication follow.
pub fn prepare_pickup(
    world: &mut World,
    carrier: ObjectId,
    pilot: ObjectId,
    target: ObjectId,
    rules: super::BattleFallRules,
) -> Result<Vec<super::BattleNotice>> {
    pickup_admission(world, carrier, pilot, target)?;
    world.attempt(|world| {
        let notices = prepare_target(world, target, rules)?;
        world.btech.validate_action(world)?;
        Ok(notices)
    })
}
