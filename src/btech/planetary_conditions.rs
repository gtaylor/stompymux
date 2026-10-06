//! Tactical Operations planetary-condition modifiers a unit's surroundings impose on it: the
//! piloting modifier its hex and the wind add to every control roll, and the to-hit modifier
//! wind and gravity give each kind of weapon.
use super::{AttackKind, GroundMovement, StoredMap, VehicleMovement, Weapon, Wind};
use crate::{ObjectId, World};
use anyhow::{Result, ensure};

impl StoredMap {
    /// Battlefield temperature in degrees Celsius, as validated for storage.
    pub fn temperature(&self) -> i8 {
        i8::try_from(self.temperature).unwrap_or(if self.temperature < 0 {
            i8::MIN
        } else {
            i8::MAX
        })
    }

    /// Battlefield gravity as a percentage of standard, as validated for storage.
    pub fn gravity_percent(&self) -> u8 {
        u8::try_from(self.gravity).unwrap_or(if self.gravity < 0 { 0 } else { u8::MAX })
    }

    /// Prevailing wind, with the stored speed in km/h.
    pub fn wind(&self) -> Wind {
        Wind {
            direction: u16::try_from(self.wind_direction).unwrap_or_default(),
            speed: u16::try_from(self.wind_speed).unwrap_or_default(),
        }
    }
}

/// The Tactical Operations weapon class wind and gravity read: missile, direct-fire
/// ballistic, direct-fire energy (lasers, PPCs, flamers and plasma), or neither for artillery.
pub(super) fn attack_kind(weapon: Weapon) -> AttackKind {
    if weapon.is_artillery() {
        return AttackKind::Other;
    }
    if weapon.is_energy() {
        return AttackKind::Energy;
    }
    match weapon.gunnery_skill(true) {
        "Gunnery-Missile" => AttackKind::Missile,
        "Gunnery-Ballistic" => AttackKind::Ballistic,
        "Gunnery-Laser" => AttackKind::Energy,
        _ => AttackKind::Other,
    }
}

/// The battlefield a unit is placed on, if any.
fn unit_map(world: &World, id: ObjectId) -> Option<&StoredMap> {
    let position = super::scanner::scanner_unit(world, id)?.position?;
    world.btech.maps().get(&position.map)
}

/// To-hit modifier wind and gravity on the shooter's battlefield give `weapon`, or `None`
/// when the wind is too strong for the weapon to fire at all.
pub(super) fn aim_modifier(world: &World, shooter: ObjectId, weapon: Weapon) -> Option<i16> {
    let Some(map) = unit_map(world, shooter) else {
        return Some(0);
    };
    let kind = attack_kind(weapon);
    let wind = map.wind().strength().aim_modifier(kind)?;
    Some(wind + super::gravity_aim_modifier(map.gravity_percent(), kind))
}

/// Refuse to fire a weapon the battlefield's wind keeps from flying: missiles in any tornado,
/// and everything but energy weapons in an F4 or stronger one.
pub(super) fn check_wind(world: &World, shooter: ObjectId, weapon: Weapon) -> Result<()> {
    let Some(map) = unit_map(world, shooter) else {
        return Ok(());
    };
    let strength = map.wind().strength();
    ensure!(
        strength.aim_modifier(attack_kind(weapon)).is_some(),
        "The {} is too fierce to fire that weapon.",
        strength.label().to_ascii_lowercase()
    );
    Ok(())
}

/// What the terrain under a unit and the wind add to every piloting or driving roll it makes:
/// the hex's Tactical Operations PSR modifier for where the unit stands, plus gale and storm
/// penalties. Unplaced units take none.
pub(super) fn piloting_modifier(world: &World, id: ObjectId) -> i16 {
    let (tile, level, terrain, wind) = if let Some(unit) = world.btech.constructed_units().get(&id)
    {
        let Some(position) = unit.position() else {
            return 0;
        };
        let Some(map) = world.btech.maps().get(&position.map) else {
            return 0;
        };
        let Ok(tile) = map.base_hex(i64::from(position.x), i64::from(position.y)) else {
            return 0;
        };
        (
            tile,
            unit.elevation_level(tile),
            GroundMovement::Legged,
            map.wind(),
        )
    } else if let Some(vehicle) = world.btech.vehicles().get(&id) {
        let Some(position) = vehicle.position() else {
            return 0;
        };
        let Some(map) = world.btech.maps().get(&position.map) else {
            return 0;
        };
        let Ok(tile) = map.base_hex(i64::from(position.x), i64::from(position.y)) else {
            return 0;
        };
        (
            tile,
            vehicle.elevation_level(tile),
            super::vehicle_motion::ground_movement(vehicle.definition().movement),
            map.wind(),
        )
    } else {
        return 0;
    };
    // Rotorcraft catch the wind like hovercraft do.
    let buffeted = match world.btech.vehicles().get(&id) {
        Some(vehicle) if vehicle.definition().movement == VehicleMovement::Vtol => {
            GroundMovement::Hover
        }
        _ => terrain,
    };
    tile.piloting_modifier(terrain, level) + wind.strength().piloting_modifier(buffeted)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Weapons fall into the classes wind and gravity distinguish.
    #[test]
    fn weapons_fall_into_attack_classes() {
        assert_eq!(attack_kind(Weapon::Lrm20), AttackKind::Missile);
        assert_eq!(attack_kind(Weapon::Srm6), AttackKind::Missile);
        assert_eq!(attack_kind(Weapon::Ac20), AttackKind::Ballistic);
        assert_eq!(attack_kind(Weapon::GaussRifle), AttackKind::Ballistic);
        assert_eq!(attack_kind(Weapon::MediumLaser), AttackKind::Energy);
        assert_eq!(attack_kind(Weapon::Ppc), AttackKind::Energy);
        assert_eq!(attack_kind(Weapon::Flamer), AttackKind::Energy);
        assert_eq!(attack_kind(Weapon::LongTom), AttackKind::Other);
    }
}
