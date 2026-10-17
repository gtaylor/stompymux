//! Tactical Operations planetary-condition modifiers a unit's surroundings impose on it: the
//! piloting modifier its hex, the wind and the weather add to every control roll, the to-hit
//! modifier wind, weather and gravity give each kind of weapon, and electromagnetic
//! interference (Tactical Operations: Advanced Rules p. 53).
use super::{AttackKind, GroundMovement, MapFlag, StoredMap, VehicleMovement, Weapon, Wind};
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

    /// Whether electromagnetic interference covers the battlefield (Tactical Operations:
    /// Advanced Rules p. 53). It covers the whole map: every unit on it operates inside the
    /// affected area.
    pub fn electromagnetic_interference(&self) -> bool {
        self.has_flag(MapFlag::ElectromagneticInterference)
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

/// To-hit modifier electromagnetic interference gives every ranged weapon attack made by a unit
/// operating in the affected area (Tactical Operations: Advanced Rules p. 53). Physical
/// attacks do not take it.
pub const INTERFERENCE_AIM_MODIFIER: u8 = 2;

/// What electromagnetic interference takes from every roll on the Cluster Hits Table made by a
/// unit operating in the affected area, whatever the weapon system; the modified roll never
/// goes below the table's 2 (Tactical Operations: Advanced Rules p. 53).
pub const INTERFERENCE_CLUSTER_PENALTY: u8 = 2;

/// The battlefield a unit is placed on, if any.
fn unit_map(world: &World, id: ObjectId) -> Option<&StoredMap> {
    let position = super::scanner::scanner_unit(world, id)?.position?;
    world.btech.maps().get(&position.map)
}

/// Whether `id` operates under electromagnetic interference: it is placed on a battlefield the
/// condition covers. Unplaced units are not affected.
pub(super) fn under_interference(world: &World, id: ObjectId) -> bool {
    unit_map(world, id).is_some_and(StoredMap::electromagnetic_interference)
}

/// The electromagnetic-interference term of a ranged weapon attack by `shooter`.
pub(super) fn interference_aim_modifier(world: &World, shooter: ObjectId) -> u8 {
    if under_interference(world, shooter) {
        INTERFERENCE_AIM_MODIFIER
    } else {
        0
    }
}

/// A Cluster Hits Table roll after electromagnetic interference, never below 2.
pub(super) fn interfered_cluster_roll(roll: u8, interference: bool) -> u8 {
    if interference {
        roll.saturating_sub(INTERFERENCE_CLUSTER_PENALTY).max(2)
    } else {
        roll
    }
}

/// To-hit modifier wind, weather and gravity on the shooter's battlefield give `weapon`, or
/// `None` when the wind is too strong for the weapon to fire at all.
pub(super) fn aim_modifier(world: &World, shooter: ObjectId, weapon: Weapon) -> Option<i16> {
    let Some(map) = unit_map(world, shooter) else {
        return Some(0);
    };
    let kind = attack_kind(weapon);
    let wind = map.wind().strength().aim_modifier(kind)?;
    let weather = map
        .weather_condition()
        .map_or(0, |weather| weather.aim_modifier(kind));
    Some(wind + weather + super::gravity_aim_modifier(map.gravity_percent(), kind))
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

/// The hex a placed unit stands in, its level there and how it moves over the ground, plus the
/// map's wind; `None` for an unplaced unit.
fn footing(world: &World, id: ObjectId) -> Option<(super::Hex, i32, GroundMovement, Wind)> {
    if let Some(unit) = world.btech.constructed_units().get(&id) {
        let position = unit.position()?;
        let map = world.btech.maps().get(&position.map)?;
        let tile = map
            .base_hex(i64::from(position.x), i64::from(position.y))
            .ok()?;
        return Some((
            tile,
            unit.elevation_level(tile),
            GroundMovement::Legged,
            map.wind(),
        ));
    }
    let vehicle = world.btech.vehicles().get(&id)?;
    let position = vehicle.position()?;
    let map = world.btech.maps().get(&position.map)?;
    let tile = map
        .base_hex(i64::from(position.x), i64::from(position.y))
        .ok()?;
    Some((
        tile,
        vehicle.elevation_level(tile),
        super::vehicle_motion::ground_movement(vehicle.definition().movement),
        map.wind(),
    ))
}

/// What the terrain under a unit, the wind and the weather add to every piloting or driving
/// roll it makes: the hex's Tactical Operations PSR modifier for where the unit stands, plus
/// gale, storm and precipitation penalties. Unplaced units take none.
pub(super) fn piloting_modifier(world: &World, id: ObjectId) -> i16 {
    let Some((tile, level, terrain, wind)) = footing(world, id) else {
        return 0;
    };
    // Rotorcraft catch the wind like hovercraft do, and the weather table groups them with
    // hovercraft and wheels.
    let rotorcraft = world
        .btech
        .vehicles()
        .get(&id)
        .is_some_and(|vehicle| vehicle.definition().movement == VehicleMovement::Vtol);
    let buffeted = if rotorcraft {
        GroundMovement::Hover
    } else {
        terrain
    };
    let weather = unit_map(world, id)
        .and_then(|map| map.weather_condition().ok())
        .map_or(0, |weather| weather.piloting_modifier(terrain, rotorcraft));
    tile.piloting_modifier(terrain, level) + wind.strength().piloting_modifier(buffeted) + weather
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

    /// Electromagnetic interference takes two from a Cluster Hits Table roll and never takes it
    /// below the table's 2 (Tactical Operations: Advanced Rules p. 53).
    #[test]
    fn interference_lowers_cluster_rolls_to_the_table_floor() {
        for (roll, expected) in [(12, 10), (7, 5), (5, 3), (4, 2), (3, 2), (2, 2)] {
            assert_eq!(interfered_cluster_roll(roll, true), expected, "roll {roll}");
            assert_eq!(interfered_cluster_roll(roll, false), roll);
        }
        assert_eq!(INTERFERENCE_AIM_MODIFIER, 2);
    }

    /// The condition is the map's electromagnetic interference flag and nothing else.
    #[test]
    fn interference_reads_the_map_flag() {
        let mut map: StoredMap = serde_json::from_value(serde_json::json!({
            "name": "emi", "width": 1, "height": 1, "gravity": 100, "temperature": 20,
            "flags": 0, "light": 0, "weather": 0, "cloud_base": 200
        }))
        .unwrap();
        assert!(!map.electromagnetic_interference());
        map.flags = MapFlag::ElectromagneticInterference.apply(map.flags, true);
        assert!(map.electromagnetic_interference());
        map.flags = MapFlag::Vacuum.apply(0, true);
        assert!(!map.electromagnetic_interference());
    }
}
