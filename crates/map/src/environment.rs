//! Battlefield-wide conditions a map can set: light, weather and wind. Gravity, temperature
//! and the rule flags live on [`MapAsset`](crate::MapAsset) directly; this module also holds
//! the Tactical Operations modifiers those conditions impose on attacks, piloting, movement,
//! heat and how far units can see.
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
    /// `physical`. `illumination` says what lights the target, and `target_heat` is the heat
    /// of a target that tracks heat; every full step of heat for this level takes one off a
    /// weapon attack's modifier. A searchlight offsets darkness as the night conditions say
    /// (Tactical Operations p. 56); fire takes up to two off a weapon attack and cancels the
    /// physical modifier (p. 55).
    pub fn aim_modifier(
        self,
        physical: bool,
        illumination: Illumination,
        target_heat: Option<u16>,
    ) -> i16 {
        let (weapon, unarmed, lit_weapon) = match self {
            Self::Day => return 0,
            Self::Dawn | Self::Dusk => (1, 0, 1),
            Self::FullMoonNight => (2, 0, 0),
            Self::MoonlessNight => (3, 1, 0),
            Self::PitchBlack => (4, 2, 1),
        };
        let heat_step = self.heat_step().unwrap_or(u16::MAX);
        let darkness = match illumination {
            Illumination::Searchlight if self.is_night() => {
                if physical {
                    return 0;
                }
                lit_weapon
            }
            Illumination::Fire => {
                if physical {
                    return 0;
                }
                (weapon - 2).max(0)
            }
            Illumination::None | Illumination::Searchlight => {
                if physical {
                    return unarmed;
                }
                weapon
            }
        };
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

/// What lights a target in the dark, which decides how much of the darkness modifier an
/// attack on it keeps.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Illumination {
    /// Nothing but the ambient light.
    None,
    /// A burning hex or burning unit beside it, which cuts darkness penalties by up to two.
    Fire,
    /// A searchlight beam, its own lamp or scenario lighting, which offsets night as the
    /// light conditions say.
    Searchlight,
}

/// How far a ground unit can see in daylight under clear skies, in hexes (Tactical
/// Operations p. 189); no condition lets anything see farther.
pub const MAX_SIGHT: u8 = 60;

/// How far a ground unit sees at night with its own searchlight on, in hexes: it stands in for
/// the light's row of the Visual Range Table, and the weather still limits it (Tactical
/// Operations p. 189).
pub const SEARCHLIGHT_SIGHT: u8 = 30;

/// How far a ground unit sees at night a target that something else lights, in hexes; like
/// [`SEARCHLIGHT_SIGHT`], the weather still limits it.
pub const LIT_TARGET_SIGHT: u8 = 45;

impl Light {
    /// How far a ground unit can see under this light alone, in hexes (Tactical Operations
    /// Visual Range Table, p. 189).
    pub const fn visual_range(self) -> u8 {
        match self {
            Self::Day => MAX_SIGHT,
            Self::Dawn | Self::Dusk => 15,
            Self::FullMoonNight => 10,
            Self::MoonlessNight => 5,
            Self::PitchBlack => 3,
        }
    }

    /// Whether a searchlight stretches sight under this light: every level but daylight.
    pub const fn searchlights_help_sight(self) -> bool {
        !matches!(self, Self::Day)
    }
}

/// Battlefield weather, after the weather conditions of Tactical Operations (pp. 55–60).
/// Weather shortens sight, makes some attacks harder, slows movement through fog and adds
/// to piloting and driving rolls.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Weather {
    Clear,
    LightFog,
    HeavyFog,
    LightRain,
    ModerateRain,
    HeavyRain,
    GustingRain,
    TorrentialDownpour,
    LightSnow,
    ModerateSnow,
    HeavySnow,
    SnowFlurries,
    Sleet,
    LightHail,
    HeavyHail,
    IceStorm,
    BlowingSand,
    LightningStorm,
}

impl Weather {
    /// Every weather condition, in storage order.
    pub const ALL: [Self; 18] = [
        Self::Clear,
        Self::LightFog,
        Self::HeavyFog,
        Self::LightRain,
        Self::ModerateRain,
        Self::HeavyRain,
        Self::GustingRain,
        Self::TorrentialDownpour,
        Self::LightSnow,
        Self::ModerateSnow,
        Self::HeavySnow,
        Self::SnowFlurries,
        Self::Sleet,
        Self::LightHail,
        Self::HeavyHail,
        Self::IceStorm,
        Self::BlowingSand,
        Self::LightningStorm,
    ];

    /// Decode the persisted map column, in the order of [`Weather::ALL`].
    pub fn from_stored(value: i64) -> Result<Self> {
        usize::try_from(value)
            .ok()
            .and_then(|index| Self::ALL.get(index).copied())
            .map_or_else(|| bail!("Map weather must be 0 to 17"), Ok)
    }

    /// Encode this condition for the persisted map column.
    pub fn stored(self) -> i64 {
        Self::ALL
            .iter()
            .position(|weather| *weather == self)
            .unwrap_or_default() as i64
    }

    /// Lowercase name used by map files, operator commands and Lua.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Clear => "clear",
            Self::LightFog => "light_fog",
            Self::HeavyFog => "heavy_fog",
            Self::LightRain => "light_rain",
            Self::ModerateRain => "moderate_rain",
            Self::HeavyRain => "heavy_rain",
            Self::GustingRain => "gusting_rain",
            Self::TorrentialDownpour => "torrential_downpour",
            Self::LightSnow => "light_snow",
            Self::ModerateSnow => "moderate_snow",
            Self::HeavySnow => "heavy_snow",
            Self::SnowFlurries => "snow_flurries",
            Self::Sleet => "sleet",
            Self::LightHail => "light_hail",
            Self::HeavyHail => "heavy_hail",
            Self::IceStorm => "ice_storm",
            Self::BlowingSand => "blowing_sand",
            Self::LightningStorm => "lightning_storm",
        }
    }

    /// Display name for players and editors.
    pub const fn label(self) -> &'static str {
        match self {
            Self::Clear => "Clear",
            Self::LightFog => "Light fog",
            Self::HeavyFog => "Heavy fog",
            Self::LightRain => "Light rain",
            Self::ModerateRain => "Moderate rain",
            Self::HeavyRain => "Heavy rain",
            Self::GustingRain => "Gusting rain",
            Self::TorrentialDownpour => "Torrential downpour",
            Self::LightSnow => "Light snow",
            Self::ModerateSnow => "Moderate snow",
            Self::HeavySnow => "Heavy snow",
            Self::SnowFlurries => "Snow flurries",
            Self::Sleet => "Sleet",
            Self::LightHail => "Light hail",
            Self::HeavyHail => "Heavy hail",
            Self::IceStorm => "Ice storm",
            Self::BlowingSand => "Blowing sand",
            Self::LightningStorm => "Lightning storm",
        }
    }

    /// How far a ground unit can see through this weather alone, in hexes (Tactical
    /// Operations Visual Range Table, p. 189).
    pub const fn visual_range(self) -> u8 {
        match self {
            Self::Clear | Self::LightningStorm => MAX_SIGHT,
            Self::LightFog | Self::LightRain | Self::LightSnow | Self::LightHail => 30,
            Self::ModerateRain | Self::ModerateSnow => 20,
            Self::HeavyRain | Self::SnowFlurries => 15,
            Self::GustingRain
            | Self::TorrentialDownpour
            | Self::HeavySnow
            | Self::Sleet
            | Self::HeavyHail
            | Self::IceStorm
            | Self::BlowingSand => 10,
            Self::HeavyFog => 5,
        }
    }

    /// To-hit modifier this weather gives an attack of `kind`, from the Expanded Movement
    /// Costs and Planetary Conditions Table (Tactical Operations p. 34). Pulse weapons count
    /// as direct-fire energy weapons.
    pub const fn aim_modifier(self, kind: AttackKind) -> i16 {
        match (self, kind) {
            (Self::HeavyFog, AttackKind::Energy) => 1,
            (Self::HeavyRain | Self::ModerateRain | Self::HeavySnow | Self::ModerateSnow, _) => 1,
            (Self::TorrentialDownpour, _) => 2,
            (Self::GustingRain, AttackKind::Ballistic) => 2,
            (Self::GustingRain, AttackKind::Missile) => 3,
            (Self::SnowFlurries | Self::Sleet | Self::LightningStorm, AttackKind::Missile) => 2,
            (Self::SnowFlurries | Self::Sleet | Self::LightningStorm, _) => 1,
            (Self::LightHail | Self::IceStorm, AttackKind::Missile) => 1,
            (Self::HeavyHail, AttackKind::Ballistic) => 1,
            (Self::HeavyHail, AttackKind::Missile) => 2,
            (Self::BlowingSand, AttackKind::Energy | AttackKind::Missile) => 1,
            _ => 0,
        }
    }

    /// Movement points this weather adds to every hex a ground unit enters: fog slows
    /// everything that walks or drives (Tactical Operations p. 55).
    pub const fn movement_cost(self) -> u8 {
        match self {
            Self::LightFog => 1,
            Self::HeavyFog => 2,
            _ => 0,
        }
    }

    /// Piloting or driving roll modifier for a unit moving as `movement`, with `rotorcraft`
    /// marking a VTOL, which the table groups with hovercraft and wheels.
    pub const fn piloting_modifier(self, movement: GroundMovement, rotorcraft: bool) -> i16 {
        let light_chassis =
            rotorcraft || matches!(movement, GroundMovement::Hover | GroundMovement::Wheeled);
        // Legged and tracked units share a column; hovercraft, wheels and rotors take the other.
        let (heavy, light) = match self {
            Self::HeavyRain | Self::HeavySnow => (1, 1),
            Self::TorrentialDownpour => (2, 2),
            Self::GustingRain => (2, 3),
            Self::HeavyHail => (1, 2),
            Self::IceStorm
            | Self::SnowFlurries
            | Self::Sleet
            | Self::LightHail
            | Self::LightningStorm
            | Self::BlowingSand => (0, 1),
            _ => (0, 0),
        };
        if light_chassis { light } else { heavy }
    }
}

impl std::fmt::Display for Weather {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.label())
    }
}

impl std::str::FromStr for Weather {
    type Err = anyhow::Error;

    /// Parse named weather conditions used by operator commands and Lua; spaces and hyphens
    /// may stand in for underscores.
    fn from_str(value: &str) -> Result<Self> {
        let name = value.trim().to_ascii_lowercase().replace([' ', '-'], "_");
        Self::ALL
            .into_iter()
            .find(|weather| weather.name() == name)
            .map_or_else(
                || bail!("Expected a weather condition such as clear, heavy_fog or light_rain"),
                Ok,
            )
    }
}

/// How far a ground unit can see under `light` and `weather` together: the more restrictive
/// of the two conditions (Tactical Operations p. 189).
pub const fn visual_range(light: Light, weather: Weather) -> u8 {
    let by_light = light.visual_range();
    let by_weather = weather.visual_range();
    if by_light < by_weather {
        by_light
    } else {
        by_weather
    }
}

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

    /// Weather names, storage and the visual range table agree, and the more restrictive of
    /// light and weather sets how far a unit sees.
    #[test]
    fn weather_round_trips_and_shortens_sight() {
        for (index, weather) in Weather::ALL.into_iter().enumerate() {
            assert_eq!(weather.stored(), index as i64);
            assert_eq!(Weather::from_stored(index as i64).unwrap(), weather);
            assert_eq!(weather.name().parse::<Weather>().unwrap(), weather);
            assert_eq!(weather.label().parse::<Weather>().unwrap(), weather);
            assert_eq!(
                serde_json::to_value(weather).unwrap(),
                serde_json::json!(weather.name())
            );
        }
        assert!(Weather::from_stored(18).is_err());
        assert!("drizzle".parse::<Weather>().is_err());
        assert_eq!(visual_range(Light::Day, Weather::Clear), 60);
        assert_eq!(visual_range(Light::Day, Weather::HeavyFog), 5);
        assert_eq!(visual_range(Light::MoonlessNight, Weather::LightRain), 5);
        assert_eq!(visual_range(Light::PitchBlack, Weather::Clear), 3);
        assert_eq!(visual_range(Light::Dusk, Weather::ModerateSnow), 15);
        assert_eq!(Weather::HeavyFog.aim_modifier(AttackKind::Energy), 1);
        assert_eq!(Weather::HeavyFog.aim_modifier(AttackKind::Ballistic), 0);
        assert_eq!(Weather::GustingRain.aim_modifier(AttackKind::Missile), 3);
        assert_eq!(
            Weather::TorrentialDownpour.aim_modifier(AttackKind::Other),
            2
        );
        assert_eq!(Weather::HeavyFog.movement_cost(), 2);
        assert_eq!(
            Weather::GustingRain.piloting_modifier(GroundMovement::Hover, false),
            3
        );
        assert_eq!(
            Weather::Sleet.piloting_modifier(GroundMovement::Tracked, true),
            1
        );
        assert_eq!(
            Weather::Sleet.piloting_modifier(GroundMovement::Legged, false),
            0
        );
    }

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
        use Illumination::{Fire, None as Dark, Searchlight};
        for (light, [weapon, physical, lit_weapon, lit_physical]) in rows {
            assert_eq!(light.aim_modifier(false, Dark, None), weapon, "{light:?}");
            assert_eq!(light.aim_modifier(true, Dark, None), physical, "{light:?}");
            assert_eq!(
                light.aim_modifier(false, Searchlight, None),
                lit_weapon,
                "{light:?}"
            );
            assert_eq!(
                light.aim_modifier(true, Searchlight, None),
                lit_physical,
                "{light:?}"
            );
            // Fire takes up to two off a weapon attack and cancels the physical penalty.
            assert_eq!(
                light.aim_modifier(false, Fire, None),
                (weapon - 2).max(0),
                "{light:?}"
            );
            assert_eq!(light.aim_modifier(true, Fire, None), 0, "{light:?}");
        }
        assert_eq!(Dusk.aim_modifier(false, Dark, Some(24)), 1);
        assert_eq!(Dusk.aim_modifier(false, Dark, Some(25)), 0);
        assert_eq!(FullMoonNight.aim_modifier(false, Dark, Some(40)), 0);
        assert_eq!(MoonlessNight.aim_modifier(false, Dark, Some(30)), 1);
        assert_eq!(PitchBlack.aim_modifier(false, Dark, Some(39)), 1);
        assert_eq!(PitchBlack.aim_modifier(true, Dark, Some(50)), 2);
        assert_eq!(Day.aim_modifier(false, Dark, Some(50)), 0);
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

    /// Weather modifies attacks by weapon class, slows walkers and drivers only in fog and
    /// adds to control rolls by chassis, as the Planetary Conditions table lays out.
    #[test]
    fn weather_follows_the_planetary_conditions_table() {
        use GroundMovement::{Hover, Legged, Tracked, Wheeled};
        assert_eq!(Weather::Clear.aim_modifier(AttackKind::Missile), 0);
        assert_eq!(Weather::HeavyFog.aim_modifier(AttackKind::Energy), 1);
        assert_eq!(Weather::HeavyFog.aim_modifier(AttackKind::Ballistic), 0);
        assert_eq!(Weather::ModerateRain.aim_modifier(AttackKind::Other), 1);
        assert_eq!(
            Weather::TorrentialDownpour.aim_modifier(AttackKind::Energy),
            2
        );
        assert_eq!(Weather::GustingRain.aim_modifier(AttackKind::Ballistic), 2);
        assert_eq!(Weather::GustingRain.aim_modifier(AttackKind::Missile), 3);
        assert_eq!(Weather::GustingRain.aim_modifier(AttackKind::Energy), 0);
        assert_eq!(Weather::Sleet.aim_modifier(AttackKind::Missile), 2);
        assert_eq!(Weather::Sleet.aim_modifier(AttackKind::Ballistic), 1);
        assert_eq!(Weather::HeavyHail.aim_modifier(AttackKind::Ballistic), 1);
        assert_eq!(Weather::HeavyHail.aim_modifier(AttackKind::Missile), 2);
        assert_eq!(Weather::BlowingSand.aim_modifier(AttackKind::Ballistic), 0);
        assert_eq!(Weather::BlowingSand.aim_modifier(AttackKind::Energy), 1);
        assert_eq!(Weather::LightFog.movement_cost(), 1);
        assert_eq!(Weather::HeavyFog.movement_cost(), 2);
        assert_eq!(Weather::HeavyRain.movement_cost(), 0);
        assert_eq!(Weather::Clear.piloting_modifier(Legged, false), 0);
        assert_eq!(Weather::HeavyRain.piloting_modifier(Tracked, false), 1);
        assert_eq!(
            Weather::TorrentialDownpour.piloting_modifier(Legged, false),
            2
        );
        assert_eq!(Weather::GustingRain.piloting_modifier(Legged, false), 2);
        assert_eq!(Weather::GustingRain.piloting_modifier(Tracked, false), 2);
        assert_eq!(Weather::GustingRain.piloting_modifier(Wheeled, false), 3);
        assert_eq!(Weather::GustingRain.piloting_modifier(Tracked, true), 3);
        assert_eq!(Weather::HeavyHail.piloting_modifier(Legged, false), 1);
        assert_eq!(Weather::HeavyHail.piloting_modifier(Hover, false), 2);
        assert_eq!(Weather::IceStorm.piloting_modifier(Legged, false), 0);
        assert_eq!(Weather::IceStorm.piloting_modifier(Tracked, false), 0);
        assert_eq!(Weather::IceStorm.piloting_modifier(Wheeled, false), 1);
        assert_eq!(Weather::LightningStorm.piloting_modifier(Hover, false), 1);
    }
}
