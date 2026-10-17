//! Effective running speed from shared load accounting and chassis-specific equipment and
//! map rules, for gunnery experience, building entry and the inspection report.
use super::{Mech, StoredMap};
use anyhow::Result;

impl Mech {
    /// Maximum used by classic gunnery XP, derived from current mass and template speed.
    /// This unit-local query excludes external load; world load queries include towing.
    /// Damage-only mobility and throttle limits remain separate.
    /// Gravity applies only on maps enabling special environmental rules.
    pub fn effective_maximum_speed(&self, map: Option<&StoredMap>) -> Result<f64> {
        self.effective_speed_with_load(
            map,
            super::UnitLoad {
                nominal_tons: self.definition().tons,
                material_mass: self.effective_mass()?,
                carried_mass: 0,
                destroyed: self.is_destroyed(),
            },
            self.definition().max_speed,
        )
    }

    /// Apply the same myomer, booster and map rules to either local or world-owned load.
    pub(super) fn effective_speed_with_load(
        &self,
        map: Option<&StoredMap>,
        load: super::UnitLoad,
        maximum: f64,
    ) -> Result<f64> {
        let envelope = super::SpeedEnvelope::from_running(maximum);
        let loaded = load.maximum_speed(maximum)?;
        let factor = if maximum > 0.0 { loaded / maximum } else { 0.0 };
        let running = self.equip_envelope(envelope).scaled(factor).running;
        super::speed_bonus::gravity(
            running as f32,
            map.filter(|map| map.gravity != 100).map(|map| map.gravity),
        )
    }
}

/// Effective transfer speed includes live external load without caching chassis state.
/// Mechs retain their mass, booster, myomer and environmental conversions; vehicles
/// use their live flanking ceiling after motive damage, extreme temperature and the shared
/// external-load penalty.
/// Both chassis families apply map gravity after load.
pub fn unit_effective_maximum_speed(
    world: &crate::World,
    id: crate::ObjectId,
    tsm_tow_bonus: bool,
) -> Result<f64> {
    configured(world, id, super::speed_bonus::SpeedPolicy { tsm_tow_bonus })
}

/// Host-aware effective speed keeps the towing policy consistent through every transfer recheck.
pub(crate) fn configured(
    world: &crate::World,
    id: crate::ObjectId,
    policy: super::speed_bonus::SpeedPolicy,
) -> Result<f64> {
    if world.btech.vehicles().contains_key(&id) {
        return super::motion_controls::throttle_configured(world, id, policy);
    }
    let unit = world
        .btech
        .constructed_units()
        .get(&id)
        .ok_or_else(|| anyhow::anyhow!("Unit construction is unavailable"))?;
    let map = unit
        .position()
        .and_then(|position| world.btech.maps().get(&position.map));
    unit.effective_speed_with_load(
        map,
        super::unit_load(world, id, policy.tsm_tow_bonus)?,
        unit.definition().max_speed,
    )
}

#[cfg(test)]
mod tests {
    /// Exercise the shared load calculator with no external load.
    fn mass_speed(tons: u16, mass: u32, maximum: f64, destroyed: bool) -> f32 {
        super::super::UnitLoad {
            nominal_tons: tons,
            material_mass: mass,
            carried_mass: 0,
            destroyed,
        }
        .maximum_speed(maximum)
        .unwrap() as f32
    }

    /// Underweight designs cannot gain speed, and material above the nominal tonnage is not a
    /// speed penalty: construction refuses overweight designs (TechManual p. 97).
    #[test]
    fn construction_weight_does_not_change_unloaded_speed() {
        for mass in [0, 1, 1024, 35 * 1024, 60 * 1024, u32::MAX] {
            assert_eq!(mass_speed(35, mass, 118.25, false), 118.25);
        }
        assert_eq!(mass_speed(35, 35 * 1024, 118.25, true), 0.0);
    }
}
