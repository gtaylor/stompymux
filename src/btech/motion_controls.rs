//! Shared cockpit readouts and named speed request interpretation.
use crate::{ObjectId, World};
use anyhow::{Context, Result, ensure};

/// Cockpit confirmation truncates the accepted speed; simulation keeps its full precision.
pub(super) fn speed_confirmation(speed: f64) -> String {
    format!("Desired speed changed to {} KPH.", speed as i32)
}

/// Read actual and requested motion without changing state or requiring assignment to the controls.
pub fn motion_readout(
    world: &World,
    unit: ObjectId,
    viewer: ObjectId,
) -> Result<super::BattleMotion> {
    if world.btech.vehicles().contains_key(&unit) {
        return super::vehicle_driving::readout(world, unit, viewer);
    }
    super::brief::cockpit_access(world, unit, viewer)?;
    let record = &world.btech.constructed_units()[&unit];
    ensure!(
        record.power() == super::BattlePower::Running && !record.is_destroyed(),
        "Start the unit first"
    );
    record.motion().context("Unit is not placed")
}

/// Resolve named speeds or clamp a finite numeric request to the current throttle envelope.
pub(crate) fn speed_request(
    world: &World,
    unit: ObjectId,
    argument: &str,
    policy: super::SpeedPolicy,
) -> Result<f64> {
    let maximum = throttle_configured(world, unit, policy)?;
    let walking = maximum * 2.0 / 3.0;
    let speed = match argument.trim().to_ascii_lowercase().as_str() {
        "walk" | "cruise" => walking,
        "run" | "flank" => maximum,
        "stop" => 0.0,
        "back" => -walking,
        value => value
            .parse::<f64>()
            .context("Usage: speed <kph|stop|walk|run|back|cruise|flank>")?,
    };
    ensure!(speed.is_finite(), "Invalid speed");
    Ok(speed.clamp(-walking, maximum))
}

/// Reversing while towing requires salvage equipment.
pub(super) fn require_reverse_allowed(world: &World, id: ObjectId, speed: f64) -> Result<()> {
    if speed >= 0.0 {
        return Ok(());
    }
    if world.btech.tows().contains_key(&id) {
        let salvage = if let Some(unit) = world.btech.vehicles().get(&id) {
            unit.definition().has_special("SalvageTech")
        } else {
            world
                .btech
                .constructed_units()
                .get(&id)
                .context("Unit construction is unavailable")?
                .definition()
                .has_special("SalvageTech")
        };
        ensure!(salvage, "You can not backup while towing!");
    }
    Ok(())
}

/// World-aware throttle ceiling shared by command parsing, admission and display.
/// Load applies before Mech equipment bonuses; VTOL vertical allocation is a later control check.
pub fn throttle_maximum(world: &World, id: ObjectId, tsm_tow_bonus: bool) -> Result<f64> {
    throttle_configured(
        world,
        id,
        super::SpeedPolicy {
            tsm_tow_bonus,
            ..super::SpeedPolicy::STANDARD
        },
    )
}

/// Host-aware throttle uses one policy for load accounting.
pub(crate) fn throttle_configured(
    world: &World,
    id: ObjectId,
    policy: super::SpeedPolicy,
) -> Result<f64> {
    if let Some(unit) = world.btech.vehicles().get(&id) {
        let maximum =
            super::load::movement_maximum(world, id, unit.maximum_speed(), policy.tsm_tow_bonus)?;
        return super::speed_bonus::on_map(world, unit.position(), maximum);
    }
    let unit = world
        .btech
        .constructed_units()
        .get(&id)
        .context("Enter a constructed unit first")?;
    let base = super::load::movement_maximum(
        world,
        id,
        unit.mobility().maximum_speed,
        policy.tsm_tow_bonus,
    )?;
    super::speed_bonus::on_map(world, unit.position(), unit.movement_maximum_at(base))
}

/// Apply the configured speed reduction during an ordinary heading change.
pub(super) fn turning_throttle(target: f64, remaining: f64, slowdown: i64) -> f64 {
    let remaining = remaining.abs();
    let factor = match slowdown {
        1 if remaining != 0.0 => 2.0 / 3.0,
        1 => 0.75,
        2 if remaining >= 1.0 => (8.0 - ((remaining - 1.0) / 30.0).floor()) / 10.0,
        _ => 1.0,
    };
    target * factor
}
