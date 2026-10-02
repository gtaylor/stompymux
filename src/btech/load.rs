//! Shared fixed-mass load accounting and propulsion penalties for battlefield units.
use crate::{ObjectId, World};
use anyhow::{Context, Result, ensure};
use serde::Serialize;

/// Construction mass for authored cargo space, shared by Mechs and vehicles.
/// This measures the cargo installation; loose stock is accounted for separately.
pub(super) fn cargo_space_mass(space: Option<&str>, carrier: bool, cargo: bool) -> Result<u32> {
    let space = space
        .map(|value| value.parse::<u32>().context("Invalid cargo space"))
        .transpose()?
        .unwrap_or(0);
    let divisor = if carrier {
        1000.0
    } else if cargo {
        100.0
    } else {
        500.0
    };
    let mass = (space as f32 / divisor * 1024.0).trunc();
    ensure!(
        f64::from(mass) <= f64::from(u32::MAX),
        "Cargo space mass overflow"
    );
    Ok(mass as u32)
}

/// A derived load snapshot in 1/1024-ton mass units; never stored or cached in the world.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct BattleUnitLoad {
    pub nominal_tons: u16,
    /// Current gameplay mass, including any temporary administrative correction.
    pub material_mass: u32,
    /// Effective cargo and tow load after their separate equipment discounts.
    pub carried_mass: u64,
    pub destroyed: bool,
}

impl BattleUnitLoad {
    /// Apply load and construction-weight penalties before boosters and environmental rules.
    /// Underweight material cannot increase speed; more than three adjusted nominal masses stops motion.
    pub fn maximum_speed(self, maximum: f64) -> Result<f64> {
        ensure!(
            self.nominal_tons > 0,
            "Load calculation requires positive tonnage"
        );
        ensure!(
            maximum.is_finite() && maximum >= 0.0,
            "Invalid unloaded maximum speed"
        );
        if self.destroyed {
            return Ok(0.0);
        }
        let nominal = u64::from(self.nominal_tons) * 1024;
        let material = u64::from(self.material_mass);
        let adjusted = match material {
            1 => nominal,
            mass if mass > nominal => mass + (mass - nominal) / 2,
            mass => mass + (nominal - mass) / 3,
        };
        let total = adjusted
            .checked_add(self.carried_mass)
            .context("Unit load overflow")?;
        if total > 3 * nominal {
            return Ok(0.0);
        }
        let denominator = (nominal + self.carried_mass / 3).max(total).max(1024);
        let speed = maximum as f32 * f32::from(self.nominal_tons) * 1024.0 / denominator as f32;
        ensure!(speed.is_finite(), "Loaded speed overflow");
        Ok(f64::from(speed))
    }
}

/// Resolve current mass, ownership and towing discounts for any admitted chassis.
/// Loose stock and external towing share this projection without sharing their equipment discounts.
/// The flag supplies the world's configured hot-myomer towing bonus.
pub fn unit_load(world: &World, id: ObjectId, tsm_tow_bonus: bool) -> Result<BattleUnitLoad> {
    let (nominal_tons, material_mass, destroyed, salvage, carrier, hot_myomer, cargo_multiplier) =
        if let Some(unit) = world.btech.vehicles().get(&id) {
            (
                unit.definition().tons,
                unit.effective_mass()?,
                unit.is_destroyed(),
                unit.definition().has_special("SalvageTech"),
                unit.definition().has_special("Carrier_Tech"),
                false,
                if unit.definition().has_special("CargoTech") {
                    1_u64
                } else {
                    2
                },
            )
        } else {
            let unit = world
                .btech
                .constructed_units()
                .get(&id)
                .context("Unit construction is unavailable")?;
            (
                unit.definition().tons,
                unit.effective_mass()?,
                unit.is_destroyed(),
                unit.definition().has_special("SalvageTech"),
                unit.definition().has_special("Carrier_Tech"),
                unit.triple_myomer_active(),
                if unit.definition().has_special("CargoTech") {
                    2_u64
                } else {
                    4
                },
            )
        };
    let carried_mass = if let Some(target) = world.btech.tows().get(&id) {
        let mass = if let Some(unit) = world.btech.vehicles().get(target) {
            unit.effective_mass()?
        } else {
            world
                .btech
                .constructed_units()
                .get(target)
                .context("Tow target is unavailable")?
                .effective_mass()?
        };
        towing_mass(mass, salvage, tsm_tow_bonus && hot_myomer, carrier)
    } else {
        0
    };
    let extra_fuel = world
        .btech
        .vehicles()
        .get(&id)
        .map_or(0, |unit| unit.auxiliary_fuel_mass());
    let cargo = super::inventory_mass(world, id)?
        .checked_add(extra_fuel)
        .context("Unit fuel load overflow")?
        .checked_mul(cargo_multiplier)
        .context("Unit cargo load overflow")?
        / 2;
    let carried_mass = carried_mass
        .checked_add(cargo)
        .context("Unit load overflow")?;
    Ok(BattleUnitLoad {
        nominal_tons,
        material_mass,
        carried_mass,
        destroyed,
    })
}

/// Each enabled discount halves the remaining integral load, in gameplay order.
fn towing_mass(material: u32, salvage: bool, hot_myomer: bool, carrier: bool) -> u64 {
    let mut mass = u64::from(material) * 2;
    for enabled in [salvage, hot_myomer, carrier] {
        if enabled {
            mass /= 2;
        }
    }
    mass
}

/// Authored cargo capacity shared by load admission and administrative inspection.
pub(super) fn cargo_capacity(world: &World, id: ObjectId) -> u32 {
    let cargo_space = world
        .btech
        .constructed_units()
        .get(&id)
        .and_then(|unit| unit.definition().attributes.get("cargo_space"))
        .or_else(|| {
            world
                .btech
                .vehicles()
                .get(&id)
                .and_then(|unit| unit.definition().attributes.get("cargo_space"))
        });
    cargo_space
        .and_then(|space| space.parse::<u32>().ok())
        .unwrap_or(0)
}

/// Edit installation capacity without discarding stock; the caller owns transaction rollback.
pub(super) fn set_cargo_capacity(
    world: &mut World,
    id: ObjectId,
    value: &str,
    tsm_bonus: bool,
) -> Result<()> {
    let capacity = value
        .trim()
        .parse::<i32>()
        .context("Expected a nonnegative 32-bit cargo capacity")?;
    ensure!(capacity >= 0, "Cargo capacity cannot be negative");
    if let Some(unit) = world.btech.constructed.get_mut(&id) {
        unit.set_cargo_space(capacity as u32);
    } else {
        world
            .btech
            .vehicles
            .get_mut(&id)
            .context("Unit is unavailable")?
            .set_cargo_space(capacity as u32);
    }
    // Evaluate construction even when zero capacity bypasses ordinary load penalties.
    unit_load(world, id, tsm_bonus)?;
    reconcile(world, id, tsm_bonus)
}

/// Whether cargo installations, loose stock or towing require live propulsion limits.
pub(super) fn carries_load(world: &World, id: ObjectId) -> bool {
    cargo_capacity(world, id) > 0
        || world
            .btech
            .vehicles()
            .get(&id)
            .is_some_and(|unit| unit.auxiliary_fuel_mass() > 0)
        || world.btech.tows().contains_key(&id)
        || world.btech.inventories.get(&id).is_some_and(|entries| {
            entries.iter().any(|entry| {
                super::BattlePart::from_id(entry.part_id).is_some_and(|part| part.mass > 0)
            })
        })
}

/// Apply external load to a damage-adjusted propulsion ceiling before chassis movement bonuses.
pub(super) fn movement_maximum(
    world: &World,
    id: ObjectId,
    maximum: f64,
    tsm_bonus: bool,
) -> Result<f64> {
    if !carries_load(world, id) {
        return Ok(maximum);
    }
    Ok(unit_load(world, id, tsm_bonus)?
        .maximum_speed(maximum)?
        .min(maximum))
}

/// Reconcile a changed external load immediately without advancing position, time or dice.
pub(super) fn reconcile(world: &mut World, id: ObjectId, tsm_bonus: bool) -> Result<()> {
    let maximum = super::throttle_maximum(world, id, tsm_bonus)?;
    let road = super::scanner::scanner_unit(world, id)
        .and_then(|unit| unit.position)
        .and_then(|position| {
            world
                .btech
                .maps()
                .get(&position.map)
                .and_then(|map| map.hex(i64::from(position.x), i64::from(position.y)).ok())
        })
        .is_some_and(|hex| matches!(hex.terrain(), super::Terrain::Road | super::Terrain::Bridge));
    if let Some(unit) = world.btech.vehicles.get_mut(&id) {
        if let Some(mut motion) = unit.motion() {
            motion.limit_load(
                maximum,
                if road && maximum > 0.0 && !unit.definition().is_vtol() {
                    maximum + 10.75
                } else {
                    maximum
                },
            );
            unit.motion = Some(motion);
        }
        return Ok(());
    }
    let unit = world
        .btech
        .constructed
        .get_mut(&id)
        .context("Unit is unavailable")?;
    if let Some(mut motion) = unit.motion() {
        motion.limit_load(maximum, maximum);
        unit.motion = Some(motion);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn towing_discounts_truncate_in_fixed_units_without_overflow() {
        for salvage in [false, true] {
            for hot_myomer in [false, true] {
                for carrier in [false, true] {
                    let halves = u32::from(salvage) + u32::from(hot_myomer) + u32::from(carrier);
                    assert_eq!(towing_mass(7, salvage, hot_myomer, carrier), 14 >> halves);
                }
            }
        }
        assert_eq!(
            towing_mass(u32::MAX, false, false, false),
            u64::from(u32::MAX) * 2
        );
    }

    #[test]
    fn carried_load_uses_both_denominator_floors_and_exact_overload_boundary() {
        let mut load = BattleUnitLoad {
            nominal_tons: 3,
            material_mass: 3 * 1024,
            carried_mass: 6 * 1024,
            destroyed: false,
        };
        assert_eq!(load.maximum_speed(64.5).unwrap(), 21.5);
        load.carried_mass += 1;
        assert_eq!(load.maximum_speed(64.5).unwrap(), 0.0);
        load.material_mass = 0;
        load.carried_mass = 3;
        assert_eq!(
            load.maximum_speed(64.5).unwrap(),
            f64::from(64.5_f32 * 3072.0 / 3073.0)
        );
        load.carried_mass = u64::MAX;
        assert!(load.maximum_speed(64.5).is_err());
        load.carried_mass = 0;
        assert!(load.maximum_speed(f64::NAN).is_err());
        load.nominal_tons = 0;
        assert!(load.maximum_speed(64.5).is_err());
    }
}
