//! Vehicle ammunition criticals collect a whole-vehicle cascade before internal damage resolution.
use super::{BattleAmmunitionDraw, BattleVehicle, BattleVehicleSection};
use crate::{Flag, ObjectId, World};
use anyhow::{Context, Result, ensure};
use serde::Serialize;
use std::sync::Arc;

/// Consumed ammunition and the internal damage still owed by the enclosing critical action.
/// Zero damage requires the weapon-destruction fallback instead of an explosion.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[must_use = "Resolve cascade damage or the weapon-destruction fallback before committing the attack"]
pub struct BattleVehicleAmmunitionCascade {
    pub section: BattleVehicleSection,
    pub ammunition: Vec<BattleAmmunitionDraw>,
    pub damage: u32,
}

impl BattleVehicle {
    /// Inspect live non-Gauss bins across every section without spending ammunition or dice.
    /// This critical uses the bin projectile damage, independent of launcher selection,
    /// and includes plasma rounds despite their exemption from ordinary bin explosions.
    pub fn ammunition_cascade(
        &self,
        section: BattleVehicleSection,
    ) -> Result<BattleVehicleAmmunitionCascade> {
        ensure!(!self.is_destroyed(), "Vehicle is destroyed");
        ensure!(
            self.sections()
                .get(&section)
                .is_some_and(|state| state.internal > 0),
            "Vehicle section is unavailable"
        );
        let loadout = self.loadout()?;
        let mut report = BattleVehicleAmmunitionCascade {
            section,
            ammunition: Vec::new(),
            damage: 0,
        };
        for (index, bin) in loadout.ammunition.iter().enumerate() {
            let rounds = self.ammunition()[index];
            if rounds == 0
                || self.critical_unavailable(bin.location)
                || bin.weapon.weapon_explosion_damage() > 0
            {
                continue;
            }
            let profile = bin.weapon.profile_for_ammunition(bin.mode);
            let damage =
                u32::from(rounds) * u32::from(profile.damage) * u32::from(profile.missiles.max(1));
            report.damage = report
                .damage
                .checked_add(damage)
                .context("Vehicle ammunition cascade exceeds damage limit")?;
            report.ammunition.push(BattleAmmunitionDraw {
                bin_index: index,
                rounds,
            });
        }
        Ok(report)
    }
}

/// Empty the cascade's bins atomically in a caller-owned attack candidate.
/// The returned damage and all further critical, casualty and visibility effects must
/// be resolved before that candidate is committed. This is not a standalone attack.
pub fn discharge_vehicle_ammunition_cascade(
    world: &mut World,
    id: ObjectId,
    section: BattleVehicleSection,
) -> Result<BattleVehicleAmmunitionCascade> {
    ensure!(
        world
            .objects
            .get(&id)
            .is_some_and(|object| !object.flags.contains(Flag::Going)),
        "Vehicle is unavailable"
    );
    let mut vehicle = world
        .btech
        .vehicles()
        .get(&id)
        .context("Vehicle is unavailable")?
        .clone();
    let report = vehicle.ammunition_cascade(section)?;
    for draw in &report.ammunition {
        vehicle.expend_ammunition(draw.bin_index, draw.rounds)?;
    }
    Arc::make_mut(&mut world.btech.vehicles).insert(id, vehicle);
    Ok(report)
}
