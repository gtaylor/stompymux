//! Explicit weapon-operator admission, independent of movement and cockpit ownership.
use crate::{ObjectId, World};
use anyhow::{Context, Result, ensure};

/// A current operator and the physical equipment and targeting state that operator controls.
#[derive(Clone, Copy)]
pub(super) struct CombatOperator {
    pub source: super::fire_target::TargetSource,
    pub station: Option<super::BattleGunnerContext>,
}

/// Revalidate weapon authority at each mechanical boundary without substituting the parent pilot.
pub(super) fn controlled(world: &World, unit: ObjectId, actor: ObjectId) -> Result<CombatOperator> {
    let location = world.objects.get(&actor).and_then(|object| object.location);
    if let Some(station) = location.filter(|id| world.btech.gunner_stations().contains_key(id)) {
        let context = super::gunner_context(world, station, actor)?;
        ensure!(context.parent == unit, "Station does not control this unit");
        super::power::control_health(world, unit, actor)?;
        return Ok(CombatOperator {
            source: context.target_source(world)?,
            station: Some(context),
        });
    }
    super::radio::controlled(world, unit, actor)?;
    Ok(CombatOperator {
        source: unit.into(),
        station: None,
    })
}

/// Mech-only entry points retain their anatomy guard before accessing constructed storage.
pub(super) fn controlled_mech(
    world: &World,
    unit: ObjectId,
    actor: ObjectId,
) -> Result<CombatOperator> {
    ensure!(
        world.btech.constructed_units().contains_key(&unit),
        "Unit construction state is unavailable"
    );
    controlled(world, unit, actor)
}

/// Resolve an owning cockpit or station without imposing operation-specific power or weapon gates.
pub(super) fn for_owner(world: &World, owner: ObjectId, actor: ObjectId) -> Result<CombatOperator> {
    let unit = if world.btech.gunner_stations().contains_key(&owner) {
        super::gunner_context(world, owner, actor)?.parent
    } else {
        owner
    };
    controlled(world, unit, actor)
}

/// Resolve the equipment owner and admit running controls without requiring permission to fire.
pub(super) fn admit_running(
    world: &World,
    owner: ObjectId,
    actor: ObjectId,
) -> Result<CombatOperator> {
    let operator = for_owner(world, owner, actor)?;
    super::power::require_running_unit(world, operator.source.unit)?;
    Ok(operator)
}

/// Running equipment and weapons hold precede argument decoding and cover loss.
pub(super) fn admit(world: &World, owner: ObjectId, actor: ObjectId) -> Result<CombatOperator> {
    let operator = admit_running(world, owner, actor)?;
    super::weapons_hold::check(world, operator.source.unit)?;
    Ok(operator)
}

impl CombatOperator {
    /// Apply only this station's override; a cockpit retains the supplied ordinary policy.
    pub fn aim_rules(self, mut rules: super::BattleAimRules) -> super::BattleAimRules {
        if let Some(station) = self.station {
            rules.override_weapon_arcs = station.arcs != 0;
        }
        rules
    }

    /// Share chassis/family skills while keeping the gunner distinct from the parent pilot.
    pub fn gunnery(self, world: &World, index: usize, extended: bool) -> Result<i16> {
        if let Some(station) = self.station {
            return station.gunnery_target(world, index, extended);
        }
        super::unit_gunnery_target(world, self.source.unit, index, extended)
    }

    /// A nonzero station mask admits only its assigned hull/torso or turret directions.
    /// Zero retains the ordinary mounting checks in the enclosing shot.
    pub fn check_arc(self, world: &World, target: ObjectId) -> Result<()> {
        if self.station.is_none_or(|station| station.arcs == 0) {
            return Ok(());
        }
        let bearing = super::unit_range(world, self.source.unit, target)?
            .bearing
            .unwrap_or(180.0);
        self.check_bearing(world, bearing)
    }

    /// Terrain and unit targets share the same directional ownership check.
    pub fn check_point_arc(self, world: &World, target: super::BattlePoint) -> Result<()> {
        if self.station.is_none_or(|station| station.arcs == 0) {
            return Ok(());
        }
        let point = super::scanner::scanner_unit(world, self.source.unit)
            .and_then(|unit| unit.point)
            .context("Shooter is not placed")?;
        self.check_bearing(world, point.bearing(target)?.unwrap_or(180.0))
    }

    /// Apply the assigned mask after geometry has supplied a continuous bearing.
    fn check_bearing(self, world: &World, bearing: f64) -> Result<()> {
        let station = self.station.context("Station is unavailable")?;
        let source = super::scanner::scanner_unit(world, self.source.unit)
            .context("Shooter is unavailable")?;
        let heading = source.heading.context("Shooter is not placed")?;
        let mut mask = match source.facing.contact_arc(heading, bearing)? {
            super::BattleContactArc::Front => 1,
            super::BattleContactArc::Left => 2,
            super::BattleContactArc::Right => 4,
            super::BattleContactArc::Rear => 8,
        };
        if let Some(turret) = world
            .btech
            .vehicles()
            .get(&self.source.unit)
            .and_then(|unit| unit.turret_heading())
        {
            let angle = (bearing.round() - turret.trunc()).rem_euclid(360.0);
            if angle <= 30.0 || angle >= 330.0 {
                mask |= 16;
            }
        }
        ensure!(
            station.arcs & mask != 0,
            "You do not control that firing arc"
        );
        Ok(())
    }
}
