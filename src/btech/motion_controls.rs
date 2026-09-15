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

/// Towing refusal precedes sprint refusal; salvage equipment bypasses only the towing guard.
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
    ensure!(
        !super::sprint::enabled(world, id)?,
        "You can not backup while sprinting!"
    );
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

/// Host-aware throttle uses one policy for load and sprint conversion.
pub(crate) fn throttle_configured(
    world: &World,
    id: ObjectId,
    policy: super::SpeedPolicy,
) -> Result<f64> {
    if super::sprint::enabled(world, id)? {
        let base = world
            .btech
            .constructed_units()
            .get(&id)
            .map(|unit| unit.mobility().maximum_speed)
            .or_else(|| {
                world
                    .btech
                    .vehicles()
                    .get(&id)
                    .map(|unit| unit.maximum_speed())
            })
            .context("Unit is unavailable")?;
        let loaded = super::load::movement_maximum(world, id, base, policy.tsm_tow_bonus)?;
        return super::sprint::maximum(world, id, loaded, policy.tsm_sprint_bonus);
    }
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
