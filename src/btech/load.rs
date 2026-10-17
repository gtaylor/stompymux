//! Shared fixed-mass load accounting and propulsion penalties for battlefield units.
use crate::{ObjectId, World};
use anyhow::{Context, Result, ensure};
use serde::Serialize;

/// A derived load snapshot in 1/1024-ton mass units; never stored or cached in the world.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct UnitLoad {
    pub nominal_tons: u16,
    /// Current gameplay mass, including any temporary administrative correction.
    pub material_mass: u32,
    /// Effective cargo and tow load after their separate equipment discounts. Stock a unit
    /// holds in its lift hoists is not part of it: that costs walking MP instead.
    pub carried_mass: u64,
    /// Walking (Cruising) MP that the stock the unit holds in its lift hoists takes off:
    /// none up to half its tonnage, one above that and two above its full tonnage (Total
    /// Warfare p. 136).
    pub hoist_penalty: u8,
    pub destroyed: bool,
}

/// How carried load changes a live speed envelope: the carried-load formula's factor on
/// both ceilings, and the walking MP that cargo held in lift hoists takes off before it
/// (Total Warfare p. 136). The two never stack on the same cargo: hoisted stock is left out
/// of the formula.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LoadEffect {
    pub factor: f64,
    pub hoist_penalty: u8,
}

impl LoadEffect {
    /// No load at all.
    pub const NONE: Self = Self {
        factor: 1.0,
        hoist_penalty: 0,
    };
}

impl UnitLoad {
    /// Apply carried load before boosters and environmental rules. Construction refuses
    /// overweight designs (TechManual p. 97), so material above the nominal tonnage adds no
    /// penalty; underweight material cannot increase speed; more than three adjusted nominal
    /// masses stops motion.
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
            mass if mass >= nominal => nominal,
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
/// Stock held in lift hoists is left to the hoist's walking-MP penalty instead (Total Warfare
/// p. 136). The flag supplies the world's configured hot-myomer towing bonus.
pub fn unit_load(world: &World, id: ObjectId, tsm_tow_bonus: bool) -> Result<UnitLoad> {
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
    let stock = super::inventory_mass(world, id)?;
    let hoisted = holds_stock_in_hoists(world, id)?;
    let hoist_penalty = if hoisted {
        super::lift_hoist::hoisted_walking_penalty(nominal_tons, stock)
    } else {
        0
    };
    let cargo = if hoisted { 0 } else { stock }
        .checked_add(extra_fuel)
        .context("Unit fuel load overflow")?
        .checked_mul(cargo_multiplier)
        .context("Unit cargo load overflow")?
        / 2;
    let carried_mass = carried_mass
        .checked_add(cargo)
        .context("Unit load overflow")?;
    Ok(UnitLoad {
        nominal_tons,
        material_mass,
        carried_mass,
        hoist_penalty,
        destroyed,
    })
}

/// Whether the unit holds its loose stock in lift hoists: it mounts one, working or not, and
/// has no cargo bay (`CargoTech`), so its stock went in through the hoists (Total Warfare
/// p. 136; see `help cargo`). Such cargo costs walking MP by the lift hoist rule rather than
/// entering the carried-load formula, and the unit may not jump while it carries any (Total
/// Warfare pp. 137, 261).
pub(super) fn holds_stock_in_hoists(world: &World, id: ObjectId) -> Result<bool> {
    let Some(unit) = world.btech.unit(id) else {
        return Ok(false);
    };
    super::with_unit!(unit, |unit| Ok(!unit.definition().has_special("CargoTech")
        && unit
            .loadout()?
            .systems
            .iter()
            .any(|part| part.system == super::System::LiftHoist)))
}

/// Whether the unit carries any stock in its lift hoists.
pub(super) fn carries_hoisted_cargo(world: &World, id: ObjectId) -> Result<bool> {
    Ok(holds_stock_in_hoists(world, id)? && super::inventory_mass(world, id)? > 0)
}

/// The load effect on a live envelope whose running ceiling is `running`: the carried-load
/// formula's factor and the lift hoists' walking-MP penalty.
pub(super) fn load_effect(
    world: &World,
    id: ObjectId,
    running: f64,
    tsm_bonus: bool,
) -> Result<LoadEffect> {
    if !carries_load(world, id) {
        return Ok(LoadEffect::NONE);
    }
    let load = unit_load(world, id, tsm_bonus)?;
    let loaded = load.maximum_speed(running)?.min(running);
    Ok(LoadEffect {
        factor: if running > 0.0 { loaded / running } else { 0.0 },
        hoist_penalty: load.hoist_penalty,
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

/// The cargo bay's capacity in hundredths of a ton, as the `cargospace` field states it, shared
/// by load admission and administrative inspection; zero without a bay.
pub(super) fn cargo_capacity(world: &World, id: ObjectId) -> u32 {
    let tons = world
        .btech
        .constructed_units()
        .get(&id)
        .and_then(|unit| unit.definition().attributes.get(super::CARGO_TONS))
        .or_else(|| {
            world
                .btech
                .vehicles()
                .get(&id)
                .and_then(|unit| unit.definition().attributes.get(super::CARGO_TONS))
        });
    // Half-ton steps of 512/1024 ton are exactly 50 hundredths each.
    super::cargo_mass(tons.map(String::as_str)).map_or(0, |mass| mass / 512 * 50)
}

/// The `cargo_tons` text for a cargo bay of `hundredths` of a ton. A bay is built in half-ton
/// steps (TechManual p. 239), so the capacity must be a multiple of 50.
pub(super) fn cargo_tons(hundredths: u32) -> Result<String> {
    ensure!(
        hundredths.is_multiple_of(50),
        "Cargo capacity is set in half tons: a multiple of 50 hundredths of a ton (TechManual p. 239)"
    );
    Ok((f64::from(hundredths) / 100.0).to_string())
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
    let tons = cargo_tons(capacity as u32)?;
    crate::btech::with_unit_mut!(
        world.btech.unit_mut(id).context("Unit is unavailable")?,
        |unit| {
            unit.set_cargo_tons(tons);
        }
    );
    // Evaluate the resized construction even when the unit carries nothing.
    unit_load(world, id, tsm_bonus)?;
    reconcile(world, id, tsm_bonus)
}

/// Whether loose stock, auxiliary fuel or a tow loads the unit, so live propulsion limits apply.
/// A cargo bay is construction mass, not load (TechManual p. 239): it holds nothing until
/// stock is loaded into it, so an empty bay leaves movement exactly as it is without one.
pub(super) fn carries_load(world: &World, id: ObjectId) -> bool {
    world
        .btech
        .vehicles()
        .get(&id)
        .is_some_and(|unit| unit.auxiliary_fuel_mass() > 0)
        || world.btech.tows().contains_key(&id)
        || world.btech.inventories.get(&id).is_some_and(|entries| {
            entries
                .iter()
                .any(|entry| super::Part::from_id(entry.part_id).is_some_and(|part| part.mass > 0))
        })
}

/// Reconcile a changed external load immediately without advancing position, time or dice.
pub(super) fn reconcile(world: &mut World, id: ObjectId, tsm_bonus: bool) -> Result<()> {
    let envelope = super::motion_controls::throttle_envelope(
        world,
        id,
        super::SpeedPolicy {
            tsm_tow_bonus: tsm_bonus,
        },
    )?;
    let tile = super::scanner::scanner_unit(world, id)
        .and_then(|unit| unit.position)
        .and_then(|position| {
            world
                .btech
                .maps()
                .get(&position.map)
                .and_then(|map| map.hex(i64::from(position.x), i64::from(position.y)).ok())
        });
    if let Some(unit) = world.btech.vehicles.get_mut(&id) {
        if let Some(mut motion) = unit.motion() {
            let movement = unit.definition().movement;
            motion.limit_load(
                envelope,
                tile.map_or(envelope, |tile| {
                    super::vehicle_speed::actual_speed_bounds(tile, movement, envelope)
                }),
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
        motion.limit_to(envelope);
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

    /// A cargo bay is construction, not load (TechManual p. 239): the Flatbed Truck's empty
    /// 6-ton bay carries nothing and leaves its movement unlimited by load until stock is
    /// loaded into it.
    #[test]
    fn an_empty_cargo_bay_is_not_load() {
        let config = crate::Config::load("tests/fixtures/game").unwrap();
        let mut world = World::default();
        let id = world.create(&config, "Truck".into(), crate::Kind::Thing);
        crate::create_battle_vehicle(
            &mut world,
            id,
            crate::VehicleTemplate::parse(
                "Flatbed_Truck",
                include_str!("../../game/units/Flatbed_Truck.toml"),
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(cargo_capacity(&world, id), 600);
        assert!(!carries_load(&world, id));
        assert_eq!(unit_load(&world, id, true).unwrap().carried_mass, 0);
        assert_eq!(
            load_effect(&world, id, 86.0, true).unwrap(),
            LoadEffect::NONE
        );
        let gold = super::super::Part::parse("Gold").unwrap().part_id;
        super::super::set_part_store_quantity(&mut world, id, gold, 30).unwrap();
        assert!(carries_load(&world, id));
        assert!(unit_load(&world, id, true).unwrap().carried_mass > 0);
        assert!(load_effect(&world, id, 86.0, true).unwrap().factor < 1.0);
    }

    #[test]
    fn carried_load_uses_both_denominator_floors_and_exact_overload_boundary() {
        let mut load = UnitLoad {
            nominal_tons: 3,
            material_mass: 3 * 1024,
            carried_mass: 6 * 1024,
            hoist_penalty: 0,
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
