//! Shared cockpit readouts and named speed request interpretation.
use crate::{ObjectId, World};
use anyhow::{Context, Result, ensure};

/// Cockpit confirmation truncates the accepted speed; simulation keeps its full precision.
pub(super) fn speed_confirmation(speed: f64) -> String {
    format!("Desired speed changed to {} KPH.", speed as i32)
}

/// Read actual and requested motion without changing state or requiring assignment to the controls.
pub fn motion_readout(world: &World, unit: ObjectId, viewer: ObjectId) -> Result<super::Motion> {
    if world.btech.vehicles().contains_key(&unit) {
        return super::vehicle_driving::readout(world, unit, viewer);
    }
    super::brief::cockpit_access(world, unit, viewer)?;
    let record = &world.btech.constructed_units()[&unit];
    ensure!(
        record.power() == super::Power::Running && !record.is_destroyed(),
        "Start the unit first"
    );
    record.motion().context("Unit is not placed")
}

/// Resolve named speeds or clamp a finite numeric request to the current throttle envelope.
/// A climbing or descending VTOL's named speeds are what its vertical speed leaves of the
/// cruising and flanking budgets.
pub(crate) fn speed_request(
    world: &World,
    unit: ObjectId,
    argument: &str,
    policy: super::SpeedPolicy,
) -> Result<f64> {
    let mut envelope = throttle_envelope(world, unit, policy)?;
    if let Some(flight) = world
        .btech
        .vehicles()
        .get(&unit)
        .and_then(|vehicle| vehicle.vtol_flight())
    {
        envelope = super::vtol_speed::horizontal_envelope(envelope, flight.vertical_speed)?;
    }
    let speed = match argument.trim().to_ascii_lowercase().as_str() {
        "walk" | "cruise" => envelope.walking,
        "run" | "flank" => envelope.running,
        "stop" => 0.0,
        "back" => -envelope.walking,
        value => value
            .parse::<f64>()
            .context("Usage: speed <kph|stop|walk|run|back|cruise|flank>")?,
    };
    ensure!(speed.is_finite(), "Invalid speed");
    Ok(envelope.clamp(speed))
}

/// Reversing while towing requires salvage equipment.
pub(super) fn require_reverse_allowed(world: &World, id: ObjectId, speed: f64) -> Result<()> {
    if speed >= 0.0 {
        return Ok(());
    }
    if world.btech.tows().contains_key(&id) {
        let salvage = crate::btech::with_unit!(
            world
                .btech
                .unit(id)
                .context("Unit construction is unavailable")?,
            |unit| { unit.definition().has_special("SalvageTech") }
        );
        ensure!(salvage, "You can not backup while towing!");
    }
    Ok(())
}

/// World-aware throttle ceiling shared by command parsing, admission and display.
/// Load applies before Mech equipment bonuses; VTOL vertical allocation is a later control check.
pub fn throttle_maximum(world: &World, id: ObjectId, tsm_tow_bonus: bool) -> Result<f64> {
    throttle_configured(world, id, super::SpeedPolicy { tsm_tow_bonus })
}

/// Host-aware throttle uses one policy for load accounting.
pub(crate) fn throttle_configured(
    world: &World,
    id: ObjectId,
    policy: super::SpeedPolicy,
) -> Result<f64> {
    Ok(throttle_envelope(world, id, policy)?.running)
}

/// A unit's live walking (cruising) and running (flanking) ceilings. A Mech's come from
/// damage-only mobility with hot myomer, heat and boosters applied in whole MP; a vehicle's
/// from its damage-adjusted cruising MP less extreme temperature. Either is then scaled for
/// towed load, and map gravity applies last.
pub(crate) fn throttle_envelope(
    world: &World,
    id: ObjectId,
    policy: super::SpeedPolicy,
) -> Result<super::SpeedEnvelope> {
    if let Some(vehicle) = world.btech.vehicles().get(&id) {
        let position = vehicle.position();
        let base =
            super::speed_bonus::vehicle_temperature(world, position, vehicle.speed_envelope());
        let loaded = super::load::movement_maximum(world, id, base.running, policy.tsm_tow_bonus)?;
        let factor = if base.running > 0.0 {
            loaded / base.running
        } else {
            0.0
        };
        return super::speed_bonus::envelope_on_map(world, position, base.scaled(factor));
    }
    let unit = world
        .btech
        .constructed_units()
        .get(&id)
        .context("Enter a constructed unit first")?;
    let base = unit.mobility().envelope();
    let loaded = super::load::movement_maximum(world, id, base.running, policy.tsm_tow_bonus)?;
    let factor = if base.running > 0.0 {
        loaded / base.running
    } else {
        0.0
    };
    // Equipment and heat work in whole MP on the damaged figure; towed load and gravity
    // then scale the result continuously.
    let envelope = unit.equip_envelope(base).scaled(factor);
    super::speed_bonus::envelope_on_map(world, unit.position(), envelope)
}

/// Cut the throttle target while a heading change is under way: 80 % of the request with
/// less than 31 degrees still to turn, one tenth less for every further 30 degrees, down to
/// 30 % for a full reversal. Together with the rate-limited turn this stands in for the
/// 1 MP per hexside that Total Warfare p. 50 charges for facing changes.
pub(super) fn turning_throttle(target: f64, remaining: f64) -> f64 {
    let remaining = remaining.abs();
    if remaining < 1.0 {
        return target;
    }
    target * (8.0 - ((remaining - 1.0) / 30.0).floor()) / 10.0
}

/// The running ceiling that sets a Mech's acceleration: its live envelope without the heat
/// penalty, so a hot unit is capped lower but closes on that cap as fast as a cool one.
pub(crate) fn acceleration_ceiling(
    world: &World,
    id: ObjectId,
    policy: super::SpeedPolicy,
) -> Result<f64> {
    let unit = world
        .btech
        .constructed_units()
        .get(&id)
        .context("Enter a constructed unit first")?;
    let base = unit.mobility().envelope();
    let loaded = super::load::movement_maximum(world, id, base.running, policy.tsm_tow_bonus)?;
    let factor = if base.running > 0.0 {
        loaded / base.running
    } else {
        0.0
    };
    let envelope = unit.equip_envelope_without_heat(base).scaled(factor);
    Ok(super::speed_bonus::envelope_on_map(world, unit.position(), envelope)?.running)
}
