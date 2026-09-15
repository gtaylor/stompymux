//! Effective speed from shared load accounting and chassis-specific propulsion and map rules.
use super::{BattleUnit, StoredBattleMap};
use anyhow::Result;

impl BattleUnit {
    /// Maximum used by classic gunnery XP, derived from current mass and template speed.
    /// This unit-local query excludes external load and pilot advantages; world load
    /// queries include towing and Speed Demon. Damage-only mobility and throttle limits remain separate.
    /// Gravity applies only on maps enabling special environmental rules.
    pub fn effective_maximum_speed(&self, map: Option<&StoredBattleMap>) -> Result<f64> {
        self.effective_speed_with_load(
            map,
            super::BattleUnitLoad {
                nominal_tons: self.definition().tons,
                material_mass: self.effective_mass()?,
                carried_mass: 0,
                destroyed: self.is_destroyed(),
            },
            self.definition().max_speed,
            false,
            true,
        )
    }

    /// Apply the same myomer, booster and map rules to either local or world-owned load.
    pub(super) fn effective_speed_with_load(
        &self,
        map: Option<&StoredBattleMap>,
        load: super::BattleUnitLoad,
        maximum: f64,
        speed_demon: bool,
        tsm_sprint_bonus: bool,
    ) -> Result<f64> {
        let speed = super::speed_bonus::SpeedBonuses {
            masc: self.masc_active(),
            supercharger: self.supercharger_active(),
            sprint: self.sprinting,
            hot_myomer: self.triple_myomer_active(),
            tsm_sprint_bonus,
            speed_demon,
        }
        .apply(load.maximum_speed(maximum)?)?;
        super::speed_bonus::gravity(
            speed,
            map.filter(|map| map.uses_special_rules() && map.gravity != 100)
                .map(|map| map.gravity),
        )
    }
}

/// Effective transfer speed includes live external load without caching chassis state.
/// Mechs retain their mass, booster, myomer and environmental conversions; vehicles
/// use the damage-adjusted movement ceiling with the shared external-load penalty.
/// Both chassis families apply pilot bonuses and enabled map gravity after load.
pub fn unit_effective_maximum_speed(
    world: &crate::World,
    id: crate::ObjectId,
    tsm_tow_bonus: bool,
) -> Result<f64> {
    configured(
        world,
        id,
        super::speed_bonus::SpeedPolicy {
            tsm_tow_bonus,
            ..super::speed_bonus::SpeedPolicy::STANDARD
        },
    )
}

/// Host-aware effective speed keeps both myomer policies together through every transfer recheck.
pub(crate) fn configured(
    world: &crate::World,
    id: crate::ObjectId,
    policy: super::speed_bonus::SpeedPolicy,
) -> Result<f64> {
    if let Some(unit) = world.btech.vehicles().get(&id) {
        let base =
            super::load::movement_maximum(world, id, unit.maximum_speed(), policy.tsm_tow_bonus)?;
        return super::sprint::maximum(world, id, base, policy.tsm_sprint_bonus);
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
        unit.pilot()
            .is_some_and(|pilot| super::skills::boolean_advantage(world, pilot, "Speed_Demon")),
        policy.tsm_sprint_bonus,
    )
}

#[cfg(test)]
mod tests {
    /// Exercise the shared load calculator with no external load.
    fn mass_speed(tons: u16, mass: u32, maximum: f64, destroyed: bool) -> f32 {
        super::super::BattleUnitLoad {
            nominal_tons: tons,
            material_mass: mass,
            carried_mass: 0,
            destroyed,
        }
        .maximum_speed(maximum)
        .unwrap() as f32
    }

    /// Underweight designs cannot gain speed; overweight designs pay a half-excess surcharge.
    #[test]
    fn construction_weight_and_overload_boundaries() {
        for mass in [0, 1, 1024, 35 * 1024] {
            assert_eq!(mass_speed(35, mass, 118.25, false), 118.25);
        }
        assert_eq!(mass_speed(40, 60 * 1024, 64.5, false), 64.5 * 40.0 / 70.0);
        // Exactly three nominal masses can move; the next fixed mass unit cannot.
        assert_eq!(mass_speed(3, 7 * 1024, 64.5, false), 21.5);
        assert_eq!(mass_speed(3, 7 * 1024 + 1, 64.5, false), 0.0);
        assert_eq!(mass_speed(35, u32::MAX, 118.25, false), 0.0);
        assert_eq!(mass_speed(35, 35 * 1024, 118.25, true), 0.0);
    }
}
