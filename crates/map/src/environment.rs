//! Battlefield-wide conditions a map can set: light and wind. Gravity, temperature and the
//! rule flags live on [`MapAsset`](crate::MapAsset) directly; this module also holds the
//! Tactical Operations modifiers those conditions impose on attacks, piloting and heat.
use crate::GroundMovement;
use anyhow::{Result, bail, ensure};
use serde::{Deserialize, Serialize};

/// Normal gravity, as the percentage a map's gravity is stored in.
pub const STANDARD_GRAVITY: u8 = 100;

/// To-hit modifier gravity gives an attack of `kind`: missiles and direct-fire ballistic
/// weapons take one point for every full 0.2 G the map's `gravity` (a percentage of normal)
/// is away from 1 G, since their rounds fly on arcs set for standard gravity.
pub const fn gravity_aim_modifier(gravity: u8, kind: AttackKind) -> i16 {
    if !matches!(kind, AttackKind::Missile | AttackKind::Ballistic) {
        return 0;
    }
    (gravity.abs_diff(STANDARD_GRAVITY) / 20) as i16
}

/// Full or started steps of ten degrees a map's `temperature` lies outside Tactical
/// Operations' comfortable band of −30 °C to 50 °C: positive when hotter, negative when
/// colder, zero inside the band. Each step costs a heat-tracking unit one point of cooling
/// (or gives it one) and a vehicle one cruising MP.
pub const fn extreme_temperature_steps(temperature: i8) -> i16 {
    let temperature = temperature as i16;
    if temperature > 50 {
        return (temperature - 50 + 9) / 10;
    }
    if temperature < -30 {
        return -((-30 - temperature + 9) / 10);
    }
    0
}

/// Battlefield light, after the light conditions of Tactical Operations (p. 56). Darkness
/// makes weapon attacks harder, and at night physical attacks too; searchlights and hot
/// targets offset it. Glare plays as a full moon night and a solar flare as a moonless one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Light {
    Day,
    Dawn,
    Dusk,
    FullMoonNight,
    MoonlessNight,
    PitchBlack,
}

impl Light {
    /// Every light level, brightest first.
    pub const ALL: [Self; 6] = [
        Self::Day,
        Self::Dawn,
        Self::Dusk,
        Self::FullMoonNight,
        Self::MoonlessNight,
        Self::PitchBlack,
    ];

    /// Decode the persisted map column: zero is daylight and five is pitch black, in the order
    /// of [`Light::ALL`].
    pub fn from_stored(value: i64) -> Result<Self> {
        usize::try_from(value)
            .ok()
            .and_then(|index| Self::ALL.get(index).copied())
            .map_or_else(|| bail!("Map light must be 0 to 5"), Ok)
    }

    /// Encode this level for the persisted map column.
    pub fn stored(self) -> i64 {
        Self::ALL
            .iter()
            .position(|light| *light == self)
            .unwrap_or_default() as i64
    }

    /// Lowercase name used by map files, operator commands and Lua.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Day => "day",
            Self::Dawn => "dawn",
            Self::Dusk => "dusk",
            Self::FullMoonNight => "full_moon_night",
            Self::MoonlessNight => "moonless_night",
            Self::PitchBlack => "pitch_black",
        }
    }

    /// Display name for players and editors.
    pub const fn label(self) -> &'static str {
        match self {
            Self::Day => "Day",
            Self::Dawn => "Dawn",
            Self::Dusk => "Dusk",
            Self::FullMoonNight => "Full moon night",
            Self::MoonlessNight => "Moonless night",
            Self::PitchBlack => "Pitch black",
        }
    }

    /// Whether this is one of the night levels, where sight shrinks to what is lit and
    /// searchlights offset the darkness.
    pub const fn is_night(self) -> bool {
        matches!(
            self,
            Self::FullMoonNight | Self::MoonlessNight | Self::PitchBlack
        )
    }

    /// To-hit modifier for a weapon attack against a unit, or for a physical attack with
    /// `physical`. `lit` says the target is illuminated or has its own searchlight on, and
    /// `target_heat` is the heat of a target that tracks heat; every full step of heat for
    /// this level takes one off a weapon attack's modifier.
    pub fn aim_modifier(self, physical: bool, lit: bool, target_heat: Option<u16>) -> i16 {
        let (weapon, unarmed, lit_weapon) = match self {
            Self::Day => return 0,
            Self::Dawn | Self::Dusk => (1, 0, 1),
            Self::FullMoonNight => (2, 0, 0),
            Self::MoonlessNight => (3, 1, 0),
            Self::PitchBlack => (4, 2, 1),
        };
        let heat_step = self.heat_step().unwrap_or(u16::MAX);
        let searchlight = lit && self.is_night();
        if physical {
            return if searchlight { 0 } else { unarmed };
        }
        let darkness = if searchlight { lit_weapon } else { weapon };
        darkness - target_heat.map_or(0, |heat| (heat / heat_step) as i16)
    }

    /// Heat a target needs for each point taken off a weapon attack's darkness modifier, or
    /// `None` in daylight, where there is no modifier to offset.
    pub const fn heat_step(self) -> Option<u16> {
        match self {
            Self::Day => None,
            Self::Dawn | Self::Dusk => Some(25),
            Self::FullMoonNight => Some(20),
            Self::MoonlessNight => Some(15),
            Self::PitchBlack => Some(10),
        }
    }
}

impl std::str::FromStr for Light {
    type Err = anyhow::Error;

    /// Parse named light levels used by operator commands and Lua; spaces and hyphens may
    /// stand in for underscores.
    fn from_str(value: &str) -> Result<Self> {
        let name = value.trim().to_ascii_lowercase().replace([' ', '-'], "_");
        Self::ALL
            .into_iter()
            .find(|light| light.name() == name)
            .map_or_else(
                || {
                    bail!(
                        "Expected day, dawn, dusk, full_moon_night, moonless_night or pitch_black"
                    )
                },
                Ok,
            )
    }
}

/// The farthest weather visibility a map can set, in hexes.
pub const MAX_VISIBILITY: u8 = 60;

/// How an attack reaches its target, which decides what wind and gravity do to it. Tactical
/// Operations groups weapons as missile, direct-fire ballistic and direct-fire energy (pulse
/// weapons included); artillery and physical attacks are none of these.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AttackKind {
    Missile,
    Ballistic,
    Energy,
    Other,
}

/// Wind strength categories from Tactical Operations' wind weather conditions (p. 59), which
/// a map's wind speed falls into.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum WindStrength {
    Calm,
    LightGale,
    ModerateGale,
    StrongGale,
    Storm,
    /// An F1 to F3 tornado.
    Tornado,
    /// An F4 or stronger tornado.
    SevereTornado,
}

impl WindStrength {
    /// Display name for players and operators.
    pub const fn label(self) -> &'static str {
        match self {
            Self::Calm => "Calm",
            Self::LightGale => "Light gale",
            Self::ModerateGale => "Moderate gale",
            Self::StrongGale => "Strong gale",
            Self::Storm => "Storm",
            Self::Tornado => "Tornado (F1-F3)",
            Self::SevereTornado => "Tornado (F4+)",
        }
    }

    /// To-hit modifier this wind gives an attack of `kind`, or `None` when the wind makes the
    /// attack impossible: no missiles fly in a tornado, and only energy weapons fire in an
    /// F4 or stronger one.
    pub const fn aim_modifier(self, kind: AttackKind) -> Option<i16> {
        use AttackKind::{Ballistic, Energy, Missile};
        Some(match (self, kind) {
            (Self::ModerateGale, Missile) => 1,
            (Self::StrongGale, Missile) => 2,
            (Self::StrongGale, Ballistic) => 1,
            (Self::Storm, Missile) => 3,
            (Self::Storm, Ballistic) => 2,
            (Self::Tornado, Missile) | (Self::SevereTornado, Missile | Ballistic) => return None,
            (Self::Tornado, Ballistic) | (Self::SevereTornado, Energy) => 3,
            (Self::Tornado, Energy) => 2,
            _ => 0,
        })
    }

    /// Piloting or driving modifier the wind adds for a ground unit moving by `movement`.
    /// Gales and storms push 'Mechs and hovercraft around but not tracked or wheeled
    /// vehicles; a tornado troubles everything.
    pub const fn piloting_modifier(self, movement: GroundMovement) -> i16 {
        use GroundMovement::{Hover, Legged};
        match (self, movement) {
            (Self::StrongGale, Legged) => 1,
            (Self::StrongGale, Hover) => 2,
            (Self::Storm, Legged | Hover) | (Self::Tornado, _) => 3,
            (Self::SevereTornado, _) => 5,
            _ => 0,
        }
    }
}

/// Prevailing wind, which carries smoke, spreads fire and, at gale force and above, spoils
/// aim and footing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Wind {
    /// Bearing the wind blows from, in degrees, 0 to 359.
    pub direction: u16,
    /// Wind speed in kilometres per hour; zero is calm.
    pub speed: u16,
}

impl Wind {
    /// The Tactical Operations wind category a speed in km/h falls into, using the Beaufort
    /// scale for the gales and storm and the Fujita scale for tornadoes: a light gale from
    /// 50 km/h, moderate from 62, strong from 75, a storm from 89, a tornado from 117 and an
    /// F4 tornado from 333.
    pub const fn strength_of(speed: u16) -> WindStrength {
        match speed {
            0..50 => WindStrength::Calm,
            50..62 => WindStrength::LightGale,
            62..75 => WindStrength::ModerateGale,
            75..89 => WindStrength::StrongGale,
            89..117 => WindStrength::Storm,
            117..333 => WindStrength::Tornado,
            _ => WindStrength::SevereTornado,
        }
    }

    /// This wind's Tactical Operations category.
    pub const fn strength(self) -> WindStrength {
        Self::strength_of(self.speed)
    }
    /// Require a bearing below 360 and a speed the map record can hold.
    pub fn validate(self) -> Result<()> {
        ensure!(self.direction < 360, "Wind direction must be 0 to 359");
        ensure!(
            i16::try_from(self.speed).is_ok(),
            "Wind speed must be at most {}",
            i16::MAX
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Stored and named forms round-trip for every level.
    #[test]
    fn light_levels_round_trip_through_storage_and_names() {
        for light in Light::ALL {
            assert_eq!(Light::from_stored(light.stored()).unwrap(), light);
            assert_eq!(light.name().parse::<Light>().unwrap(), light);
        }
        assert_eq!(Light::Day.stored(), 0);
        assert_eq!(Light::PitchBlack.stored(), 5);
        assert_eq!(
            " Full Moon Night ".parse::<Light>().unwrap(),
            Light::FullMoonNight
        );
        assert_eq!(
            "moonless-night".parse::<Light>().unwrap(),
            Light::MoonlessNight
        );
        assert!(Light::from_stored(6).is_err());
        assert!(Light::from_stored(-1).is_err());
        assert!("twilight".parse::<Light>().is_err());
    }

    /// The Tactical Operations light table: weapon and physical penalties, what searchlights
    /// offset, and the heat steps that make hot targets easier to hit.
    #[test]
    fn light_aim_follows_tactical_operations() {
        use Light::*;
        let rows = [
            (Day, [0, 0, 0, 0]),
            (Dawn, [1, 0, 1, 0]),
            (Dusk, [1, 0, 1, 0]),
            (FullMoonNight, [2, 0, 0, 0]),
            (MoonlessNight, [3, 1, 0, 0]),
            (PitchBlack, [4, 2, 1, 0]),
        ];
        for (light, [weapon, physical, lit_weapon, lit_physical]) in rows {
            assert_eq!(light.aim_modifier(false, false, None), weapon, "{light:?}");
            assert_eq!(light.aim_modifier(true, false, None), physical, "{light:?}");
            assert_eq!(
                light.aim_modifier(false, true, None),
                lit_weapon,
                "{light:?}"
            );
            assert_eq!(
                light.aim_modifier(true, true, None),
                lit_physical,
                "{light:?}"
            );
        }
        assert_eq!(Dusk.aim_modifier(false, false, Some(24)), 1);
        assert_eq!(Dusk.aim_modifier(false, false, Some(25)), 0);
        assert_eq!(FullMoonNight.aim_modifier(false, false, Some(40)), 0);
        assert_eq!(MoonlessNight.aim_modifier(false, false, Some(30)), 1);
        assert_eq!(PitchBlack.aim_modifier(false, false, Some(39)), 1);
        assert_eq!(PitchBlack.aim_modifier(true, false, Some(50)), 2);
        assert_eq!(Day.aim_modifier(false, false, Some(50)), 0);
        assert!(!Dusk.is_night() && FullMoonNight.is_night() && PitchBlack.is_night());
    }

    /// Wind speeds fall into the Tactical Operations categories at the Beaufort and Fujita
    /// boundaries.
    #[test]
    fn wind_speeds_fall_into_tactical_operations_categories() {
        use WindStrength::*;
        for (speed, strength) in [
            (0, Calm),
            (49, Calm),
            (50, LightGale),
            (62, ModerateGale),
            (74, ModerateGale),
            (75, StrongGale),
            (89, Storm),
            (116, Storm),
            (117, Tornado),
            (332, Tornado),
            (333, SevereTornado),
            (u16::MAX, SevereTornado),
        ] {
            assert_eq!(Wind::strength_of(speed), strength, "{speed} km/h");
        }
    }

    /// The wind weather table: missiles suffer first, tornadoes ground missiles entirely and
    /// the worst leave only energy weapons.
    #[test]
    fn wind_spoils_aim_by_weapon_class() {
        use AttackKind::*;
        use WindStrength::*;
        let rows = [
            (Calm, [Some(0), Some(0), Some(0)]),
            (LightGale, [Some(0), Some(0), Some(0)]),
            (ModerateGale, [Some(1), Some(0), Some(0)]),
            (StrongGale, [Some(2), Some(1), Some(0)]),
            (Storm, [Some(3), Some(2), Some(0)]),
            (Tornado, [None, Some(3), Some(2)]),
            (SevereTornado, [None, None, Some(3)]),
        ];
        for (wind, [missile, ballistic, energy]) in rows {
            assert_eq!(wind.aim_modifier(Missile), missile, "{wind:?}");
            assert_eq!(wind.aim_modifier(Ballistic), ballistic, "{wind:?}");
            assert_eq!(wind.aim_modifier(Energy), energy, "{wind:?}");
        }
        assert_eq!(Storm.aim_modifier(Other), Some(0));
    }

    /// Strong winds trouble 'Mechs and hovercraft before tracked and wheeled vehicles.
    #[test]
    fn wind_spoils_footing_by_movement() {
        use GroundMovement::*;
        use WindStrength::*;
        assert_eq!(ModerateGale.piloting_modifier(Legged), 0);
        assert_eq!(StrongGale.piloting_modifier(Legged), 1);
        assert_eq!(StrongGale.piloting_modifier(Hover), 2);
        assert_eq!(StrongGale.piloting_modifier(Tracked), 0);
        assert_eq!(Storm.piloting_modifier(Legged), 3);
        assert_eq!(Storm.piloting_modifier(Wheeled), 0);
        assert_eq!(Tornado.piloting_modifier(Wheeled), 3);
        assert_eq!(SevereTornado.piloting_modifier(Legged), 5);
    }

    /// Missiles and ballistics take one point per full 0.2 G off standard gravity.
    #[test]
    fn gravity_spoils_missile_and_ballistic_aim() {
        assert_eq!(gravity_aim_modifier(100, AttackKind::Missile), 0);
        assert_eq!(gravity_aim_modifier(81, AttackKind::Missile), 0);
        assert_eq!(gravity_aim_modifier(80, AttackKind::Ballistic), 1);
        assert_eq!(gravity_aim_modifier(140, AttackKind::Missile), 2);
        assert_eq!(gravity_aim_modifier(20, AttackKind::Missile), 4);
        assert_eq!(gravity_aim_modifier(20, AttackKind::Energy), 0);
        assert_eq!(gravity_aim_modifier(250, AttackKind::Other), 0);
    }

    /// Every started ten degrees beyond −30 °C or 50 °C counts as a step.
    #[test]
    fn extreme_temperatures_count_started_steps() {
        for (temperature, steps) in [
            (20, 0),
            (50, 0),
            (51, 1),
            (60, 1),
            (61, 2),
            (127, 8),
            (-30, 0),
            (-31, -1),
            (-40, -1),
            (-41, -2),
            (-128, -10),
        ] {
            assert_eq!(
                extreme_temperature_steps(temperature),
                steps,
                "{temperature} °C"
            );
        }
    }

    #[test]
    fn wind_bearings_stay_below_a_full_turn() {
        Wind {
            direction: 359,
            speed: 40,
        }
        .validate()
        .unwrap();
        assert!(
            Wind {
                direction: 360,
                speed: 0
            }
            .validate()
            .is_err()
        );
        assert!(
            Wind {
                direction: 0,
                speed: u16::MAX
            }
            .validate()
            .is_err()
        );
    }
}
