//! Electronic sensors after the Tactical Operations double-blind rules (pp. 188–191): each
//! unit runs one sensor system at a time, sweeps it with 2d6 once a turn, and that sweep
//! decides which range band it detects targets in this turn. Terrain, water, the target's
//! heat or mass, and ECM and stealth systems all adjust what the sweep finds.
use super::ActiveProbe;
use crate::btech::TerrainLos;
use serde::{Deserialize, Serialize};

/// The sensor system a unit has selected. Every 'Mech and vehicle carries infrared, magnetic
/// anomaly, radar and seismic sensors; a probe mode needs an installed active probe.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum SensorMode {
    #[default]
    Infrared,
    Magscan,
    Radar,
    Seismic,
    Probe,
}

impl SensorMode {
    /// Every mode, in the order a unit's suite sweeps with them.
    pub const ALL: [Self; 5] = [
        Self::Infrared,
        Self::Magscan,
        Self::Radar,
        Self::Seismic,
        Self::Probe,
    ];

    /// Lowercase name used in reports and Lua.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Infrared => "infrared",
            Self::Magscan => "magscan",
            Self::Radar => "radar",
            Self::Seismic => "seismic",
            Self::Probe => "probe",
        }
    }

    /// Display name for the `sensor` report.
    pub const fn label(self) -> &'static str {
        match self {
            Self::Infrared => "infrared",
            Self::Magscan => "magnetic anomaly",
            Self::Radar => "radar",
            Self::Seismic => "seismic",
            Self::Probe => "active probe",
        }
    }

    /// Whether an intervening hill blocks this sensor: everything but seismic sensors,
    /// magnetic anomaly detectors and active probes.
    pub const fn blocked_by_hills(self) -> bool {
        matches!(self, Self::Infrared | Self::Radar)
    }

    /// Whether an intervening building hex blocks this sensor outright.
    pub const fn blocked_by_buildings(self) -> bool {
        matches!(self, Self::Radar | Self::Magscan)
    }

    /// Whether heavy woods, jungle and buildings along the line shrink the range brackets.
    pub const fn shrunk_by_cover(self) -> bool {
        matches!(self, Self::Infrared | Self::Radar)
    }

    /// Whether the sensor passes through the water's surface; only magscan does.
    pub const fn crosses_water_surface(self) -> bool {
        matches!(self, Self::Magscan)
    }
}

/// The range brackets of one sensor: the farthest hex of its short, medium and long bands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct SensorBands {
    pub short: u8,
    pub medium: u8,
    pub long: u8,
}

impl SensorBands {
    const fn new(short: u8, medium: u8, long: u8) -> Self {
        Self {
            short,
            medium,
            long,
        }
    }

    /// Range brackets from the Tactical Operations Sensor Range Table (p. 190) for one mode
    /// on a 'Mech or a combat vehicle; a probe mode uses the installed probe's brackets, and a
    /// Beagle that is itself Clan technology (`clan`, from its own slots) the Clan active probe's.
    pub fn of(mode: SensorMode, vehicle: bool, probe: Option<ActiveProbe>, clan: bool) -> Self {
        match (mode, vehicle) {
            (SensorMode::Infrared | SensorMode::Magscan, false) => Self::new(10, 20, 30),
            (SensorMode::Infrared | SensorMode::Magscan, true) => Self::new(9, 18, 27),
            (SensorMode::Radar, false) => Self::new(8, 16, 24),
            (SensorMode::Radar, true) => Self::new(6, 12, 18),
            (SensorMode::Seismic, false) => Self::new(2, 4, 6),
            (SensorMode::Seismic, true) => Self::new(1, 2, 3),
            (SensorMode::Probe, _) => match probe {
                Some(ActiveProbe::Beagle) if clan => Self::new(15, 30, 45),
                Some(ActiveProbe::Beagle) => Self::new(12, 24, 36),
                Some(ActiveProbe::Bloodhound) => Self::new(16, 32, 48),
                Some(ActiveProbe::Light | ActiveProbe::Watchdog) => Self::new(9, 18, 27),
                None => Self::new(0, 0, 0),
            },
        }
    }

    /// The brackets after cover along the line shrinks them, never below zero.
    const fn adjusted(self, shrink: u8) -> Self {
        Self::new(
            self.short.saturating_sub(shrink),
            self.medium.saturating_sub(shrink),
            self.long.saturating_sub(shrink),
        )
    }

    /// The hexes `band` covers: from one past the previous band's reach to its own reach,
    /// widened by `expand` hexes at both ends for a hot or massive target (Tactical
    /// Operations p. 190: a 'Mech three hexes short of the medium bracket is found by a
    /// medium-band sweep when it carries three heat penalties).
    fn covers(self, band: SensorBand, distance: u64, expand: u8) -> bool {
        let (lower, upper) = match band {
            SensorBand::Short => (0, self.short),
            SensorBand::Medium => (self.short.saturating_add(1), self.medium),
            SensorBand::Long => (self.medium.saturating_add(1), self.long),
        };
        let lower = u64::from(lower.saturating_sub(expand));
        let upper = u64::from(upper.saturating_add(expand));
        distance >= lower && distance <= upper
    }
}

/// Which range band a sweep detects targets in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SensorBand {
    Short,
    Medium,
    Long,
}

impl SensorBand {
    /// The band a modified 2d6 sweep result detects in: 7 or 8 the short band, 5 or 6 the
    /// medium band, 2 to 4 the long band, and 9 or more nothing at all.
    pub const fn of_result(result: i16) -> Option<Self> {
        match result {
            7 | 8 => Some(Self::Short),
            5 | 6 => Some(Self::Medium),
            2..=4 => Some(Self::Long),
            _ => None,
        }
    }

    /// Display name for the `sensor` report.
    pub const fn label(self) -> &'static str {
        match self {
            Self::Short => "short",
            Self::Medium => "medium",
            Self::Long => "long",
        }
    }
}

/// ECM and stealth systems around or on a target that make it harder for a sweep to find
/// (Tactical Operations ECM/Stealth Modifier Table, p. 190).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TargetSystems {
    /// A hostile Angel ECM suite covers the spotter or the target.
    pub angel: bool,
    /// A hostile Guardian, Clan or Watchdog ECM suite covers the spotter or the target.
    pub guardian: bool,
    /// The target's null signature system is engaged.
    pub null_signature: bool,
    /// The target's stealth armor is engaged.
    pub stealth: bool,
}

impl TargetSystems {
    /// Whether any system is active, which is when a failed sweep reports jamming.
    pub const fn any(self) -> bool {
        self.angel || self.guardian || self.null_signature || self.stealth
    }

    /// Whether any system that announces itself through jamming is active: every ECM suite
    /// and stealth armor, which needs one; a null signature stays silent.
    pub const fn jams(self) -> bool {
        self.angel || self.guardian || self.stealth
    }
}

/// What a sweep of `mode` adds to its result for each of the target's systems: the Tactical
/// Operations ECM/Stealth Modifier Table (p. 191). A Beagle probe that is itself Clan technology
/// (`clan`, from its own slots rather than the chassis) is the Clan active probe and takes
/// that row. Passive infrared, magscan and seismic sensors
/// are not fooled by any of them.
pub fn ecm_modifier(
    mode: SensorMode,
    vehicle: bool,
    probe: Option<ActiveProbe>,
    clan: bool,
    systems: TargetSystems,
) -> i16 {
    // Columns: Angel ECM, null signature, Guardian/Clan/Watchdog ECM, stealth armor.
    let row: Option<[i16; 4]> = match mode {
        SensorMode::Infrared | SensorMode::Magscan | SensorMode::Seismic => None,
        SensorMode::Radar if vehicle => Some([7, 7, 6, 5]),
        SensorMode::Radar => Some([7, 6, 5, 4]),
        SensorMode::Probe => match probe {
            Some(ActiveProbe::Beagle) if clan => Some([5, 5, 4, 3]),
            Some(ActiveProbe::Beagle) => Some([6, 5, 4, 3]),
            Some(ActiveProbe::Bloodhound) => Some([4, 3, 2, 1]),
            Some(ActiveProbe::Light) => Some([6, 6, 5, 4]),
            Some(ActiveProbe::Watchdog) => Some([6, 5, 4, 3]),
            None => None,
        },
    };
    let Some(row) = row else {
        return 0;
    };
    let mut modifier = 0;
    for (active, penalty) in [
        (systems.angel, row[0]),
        (systems.null_signature, row[1]),
        (systems.guardian, row[2]),
        (systems.stealth, row[3]),
    ] {
        if active {
            modifier += penalty;
        }
    }
    modifier
}

/// Everything about one target a sweep needs besides the sensor itself.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SweepTarget<'a> {
    /// Hexes between the units.
    pub distance: u64,
    pub terrain: &'a TerrainLos,
    /// The observer and target lie on opposite sides of a water surface.
    pub across_water_surface: bool,
    /// Full heat movement penalties on a target that tracks heat (one per five heat).
    pub heat_penalties: u8,
    /// The target is burning with inferno fuel or standing in a burning hex.
    pub burning: bool,
    pub tons: u32,
    /// The target stands in a heavy industrial hex, which magscans cannot read.
    pub heavy_industrial: bool,
    /// The target is moving along the ground (not airborne, not swimming free of the bottom).
    pub moving_on_ground: bool,
    pub systems: TargetSystems,
}

/// What one sweep made of one target.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct SweepResult {
    /// The sensor system that made this sweep.
    pub mode: SensorMode,
    pub detected: bool,
    /// The band the modified sweep detects in, if any.
    pub band: Option<SensorBand>,
    /// The ECM and stealth modifier applied to the sweep against this target.
    pub modifier: i16,
    /// A system that announces itself kept the target hidden from a sweep that would
    /// otherwise have found it.
    pub jammed: bool,
}

/// Resolve one sensor sweep against one target. `clan` marks a probe that is itself Clan
/// technology, whose Beagle slot is the Clan active probe.
pub fn sweep(
    mode: SensorMode,
    bands: SensorBands,
    vehicle: bool,
    probe: Option<ActiveProbe>,
    clan: bool,
    roll: u8,
    target: SweepTarget<'_>,
) -> SweepResult {
    let modifier = ecm_modifier(mode, vehicle, probe, clan, target.systems);
    let band = SensorBand::of_result(i16::from(roll) + modifier);
    let unmodified = SensorBand::of_result(i16::from(roll));
    let reaches = |band: Option<SensorBand>| {
        let Some(band) = band else {
            return false;
        };
        if target.across_water_surface && !mode.crosses_water_surface() {
            return false;
        }
        let terrain = target.terrain;
        // Any building hex blocks radar and magscan, hardened ones included (Tactical
        // Operations p. 190, Sensor Range Table).
        if (mode.blocked_by_hills() && terrain.hills)
            || (mode.blocked_by_buildings() && terrain.building_hexes() > 0)
        {
            return false;
        }
        match mode {
            SensorMode::Magscan if target.tons < 20 || target.heavy_industrial => return false,
            SensorMode::Seismic if !target.moving_on_ground => return false,
            _ => {}
        }
        // Heavy woods and ordinary buildings take a hex off every bracket, ultra-heavy woods
        // and hardened buildings two (Tactical Operations p. 190, Sensor Range Table).
        let shrink = if mode.shrunk_by_cover() {
            terrain
                .heavy_woods
                .saturating_add(terrain.ultra_woods.saturating_mul(2))
                .saturating_add(terrain.buildings)
                .saturating_add(terrain.hardened_buildings.saturating_mul(2))
        } else {
            0
        };
        let expand = match mode {
            SensorMode::Infrared => target
                .heat_penalties
                .saturating_add(u8::from(target.burning)),
            SensorMode::Magscan if target.tons > 100 => 2,
            SensorMode::Magscan if target.tons >= 80 => 1,
            _ => 0,
        };
        bands.adjusted(shrink).covers(band, target.distance, expand)
    };
    let detected = reaches(band);
    SweepResult {
        mode,
        detected,
        band,
        modifier,
        jammed: !detected && target.systems.jams() && reaches(unmodified),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn target(terrain: &TerrainLos, distance: u64) -> SweepTarget<'_> {
        SweepTarget {
            distance,
            terrain,
            across_water_surface: false,
            heat_penalties: 0,
            burning: false,
            tons: 50,
            heavy_industrial: false,
            moving_on_ground: true,
            systems: TargetSystems::default(),
        }
    }

    /// Sweep results pick the band and the brackets follow the Sensor Range Table.
    #[test]
    fn sweeps_detect_the_band_their_result_names() {
        let clear = TerrainLos::default();
        let bands = SensorBands::of(SensorMode::Infrared, false, None, false);
        assert_eq!(bands, SensorBands::new(10, 20, 30));
        for (roll, distance, detected) in [
            (7, 10, true),
            (8, 11, false),
            (5, 11, true),
            (6, 20, true),
            (6, 21, false),
            (2, 30, true),
            (4, 20, false),
            (9, 1, false),
            (12, 5, false),
        ] {
            let result = sweep(
                SensorMode::Infrared,
                bands,
                false,
                None,
                false,
                roll,
                target(&clear, distance),
            );
            assert_eq!(result.detected, detected, "roll {roll} at {distance}");
            assert!(!result.jammed);
        }
        assert_eq!(
            SensorBands::of(SensorMode::Radar, true, None, false),
            SensorBands::new(6, 12, 18)
        );
        assert_eq!(
            SensorBands::of(SensorMode::Seismic, false, None, false),
            SensorBands::new(2, 4, 6)
        );
        assert_eq!(
            SensorBands::of(
                SensorMode::Probe,
                false,
                Some(ActiveProbe::Bloodhound),
                false
            ),
            SensorBands::new(16, 32, 48)
        );
        assert_eq!(
            SensorBands::of(SensorMode::Probe, false, Some(ActiveProbe::Beagle), true),
            SensorBands::new(15, 30, 45)
        );
        assert_eq!(
            SensorBands::of(SensorMode::Probe, true, None, false),
            SensorBands::new(0, 0, 0)
        );
    }

    /// Hills stop infrared and radar but not magscan, seismic or probes; buildings stop radar
    /// and magscan; heavy woods and buildings shorten infrared and radar brackets.
    #[test]
    fn terrain_blocks_and_shrinks_by_sensor() {
        let hill = TerrainLos {
            hills: true,
            blocked: true,
            ..TerrainLos::default()
        };
        let building = TerrainLos {
            buildings: 1,
            blocked: true,
            ..TerrainLos::default()
        };
        let woods = TerrainLos {
            heavy_woods: 2,
            ultra_woods: 1,
            ..TerrainLos::default()
        };
        let detects = |mode, terrain: &TerrainLos, distance| {
            sweep(
                mode,
                SensorBands::of(mode, false, Some(ActiveProbe::Beagle), false),
                false,
                Some(ActiveProbe::Beagle),
                false,
                7,
                target(terrain, distance),
            )
            .detected
        };
        assert!(!detects(SensorMode::Infrared, &hill, 5));
        assert!(!detects(SensorMode::Radar, &hill, 5));
        assert!(detects(SensorMode::Magscan, &hill, 5));
        assert!(detects(SensorMode::Seismic, &hill, 2));
        assert!(detects(SensorMode::Probe, &hill, 5));
        assert!(!detects(SensorMode::Radar, &building, 5));
        assert!(!detects(SensorMode::Magscan, &building, 5));
        assert!(detects(SensorMode::Infrared, &building, 5));
        assert!(detects(SensorMode::Probe, &building, 5));
        // Two heavy and one ultra-heavy woods hex take four hexes off infrared brackets.
        assert!(detects(SensorMode::Infrared, &woods, 6));
        assert!(!detects(SensorMode::Infrared, &woods, 7));
        assert!(detects(SensorMode::Magscan, &woods, 10));
    }

    /// Tactical Operations p. 190, Sensor Range Table: a hardened building hex takes two hexes
    /// off infrared and radar brackets where an ordinary one takes one, and any building hex,
    /// hardened included, blocks radar and magscan outright.
    #[test]
    fn hardened_buildings_shrink_twice_and_block_radar_and_magscan() {
        let hardened = TerrainLos {
            hardened_buildings: 1,
            blocked: true,
            ..TerrainLos::default()
        };
        let ordinary = TerrainLos {
            buildings: 1,
            blocked: true,
            ..TerrainLos::default()
        };
        let detects = |mode, terrain: &TerrainLos, distance| {
            sweep(
                mode,
                SensorBands::of(mode, false, None, false),
                false,
                None,
                false,
                7,
                target(terrain, distance),
            )
            .detected
        };
        // The 'Mech infrared short bracket reaches ten hexes on open ground.
        assert!(detects(SensorMode::Infrared, &ordinary, 9));
        assert!(!detects(SensorMode::Infrared, &ordinary, 10));
        assert!(detects(SensorMode::Infrared, &hardened, 8));
        assert!(!detects(SensorMode::Infrared, &hardened, 9));
        for mode in [SensorMode::Radar, SensorMode::Magscan] {
            assert!(!detects(mode, &hardened, 1), "{mode:?}");
            assert!(!detects(mode, &ordinary, 1), "{mode:?}");
        }
    }

    /// Heat stretches infrared brackets toward the spotter, mass stretches magscan, light
    /// units and industrial hexes hide from magscan, and seismic needs ground movement.
    #[test]
    fn heat_mass_and_movement_change_what_is_found() {
        let clear = TerrainLos::default();
        let bands = SensorBands::of(SensorMode::Infrared, false, None, false);
        // A medium-band sweep misses eight hexes until the target carries fifteen heat.
        let mut cold = target(&clear, 8);
        assert!(!sweep(SensorMode::Infrared, bands, false, None, false, 5, cold).detected);
        cold.heat_penalties = 3;
        assert!(sweep(SensorMode::Infrared, bands, false, None, false, 5, cold).detected);
        let magscan = SensorBands::of(SensorMode::Magscan, false, None, false);
        let mut heavy = target(&clear, 11);
        heavy.tons = 85;
        assert!(sweep(SensorMode::Magscan, magscan, false, None, false, 7, heavy).detected);
        heavy.tons = 15;
        assert!(!sweep(SensorMode::Magscan, magscan, false, None, false, 7, heavy).detected);
        heavy.tons = 85;
        heavy.heavy_industrial = true;
        assert!(!sweep(SensorMode::Magscan, magscan, false, None, false, 7, heavy).detected);
        let seismic = SensorBands::of(SensorMode::Seismic, false, None, false);
        let mut still = target(&clear, 2);
        still.moving_on_ground = false;
        assert!(!sweep(SensorMode::Seismic, seismic, false, None, false, 7, still).detected);
        still.moving_on_ground = true;
        assert!(sweep(SensorMode::Seismic, seismic, false, None, false, 7, still).detected);
        let mut submerged = target(&clear, 5);
        submerged.across_water_surface = true;
        assert!(
            !sweep(
                SensorMode::Infrared,
                bands,
                false,
                None,
                false,
                7,
                submerged
            )
            .detected
        );
        assert!(
            sweep(
                SensorMode::Magscan,
                magscan,
                false,
                None,
                false,
                7,
                submerged
            )
            .detected
        );
    }

    /// Tactical Operations p. 191, ECM/Stealth Modifier Table: the Clan active probe takes
    /// +5 against Angel ECM and null signature, +4 against other ECM and +3 against stealth,
    /// where the Inner Sphere Beagle takes +6 against Angel. The Watchdog keeps its own row.
    #[test]
    fn clan_active_probe_takes_its_own_ecm_row() {
        let row = |probe, clan| {
            [
                TargetSystems {
                    angel: true,
                    ..TargetSystems::default()
                },
                TargetSystems {
                    null_signature: true,
                    ..TargetSystems::default()
                },
                TargetSystems {
                    guardian: true,
                    ..TargetSystems::default()
                },
                TargetSystems {
                    stealth: true,
                    ..TargetSystems::default()
                },
            ]
            .map(|systems| ecm_modifier(SensorMode::Probe, false, Some(probe), clan, systems))
        };
        assert_eq!(row(ActiveProbe::Beagle, true), [5, 5, 4, 3]);
        assert_eq!(row(ActiveProbe::Beagle, false), [6, 5, 4, 3]);
        assert_eq!(row(ActiveProbe::Watchdog, true), [6, 5, 4, 3]);
        assert_eq!(row(ActiveProbe::Light, true), [6, 6, 5, 4]);
    }

    /// The ECM/Stealth Modifier Table pushes radar and probe sweeps past the detecting
    /// results, reports jamming for suites that announce themselves, and leaves passive
    /// sensors alone.
    #[test]
    fn ecm_and_stealth_follow_the_modifier_table() {
        let guardian = TargetSystems {
            guardian: true,
            ..TargetSystems::default()
        };
        assert_eq!(
            ecm_modifier(SensorMode::Radar, false, None, false, guardian),
            5
        );
        assert_eq!(
            ecm_modifier(SensorMode::Radar, true, None, false, guardian),
            6
        );
        assert_eq!(
            ecm_modifier(
                SensorMode::Probe,
                false,
                Some(ActiveProbe::Beagle),
                false,
                guardian
            ),
            4
        );
        assert_eq!(
            ecm_modifier(
                SensorMode::Probe,
                false,
                Some(ActiveProbe::Bloodhound),
                false,
                TargetSystems {
                    angel: true,
                    stealth: true,
                    ..TargetSystems::default()
                }
            ),
            5
        );
        assert_eq!(
            ecm_modifier(SensorMode::Infrared, false, None, false, guardian),
            0
        );
        assert_eq!(
            ecm_modifier(SensorMode::Seismic, true, None, false, guardian),
            0
        );
        let clear = TerrainLos::default();
        let bands = SensorBands::of(SensorMode::Radar, false, None, false);
        let mut covered = target(&clear, 5);
        covered.systems = guardian;
        let result = sweep(SensorMode::Radar, bands, false, None, false, 7, covered);
        assert!(!result.detected && result.jammed && result.band.is_none());
        covered.systems = TargetSystems {
            null_signature: true,
            ..TargetSystems::default()
        };
        let quiet = sweep(SensorMode::Radar, bands, false, None, false, 7, covered);
        assert!(!quiet.detected && !quiet.jammed);
        // A low roll with ECM climbs into a detecting band.
        covered.systems = guardian;
        let climbed = sweep(SensorMode::Radar, bands, false, None, false, 2, covered);
        assert_eq!(climbed.band, Some(SensorBand::Short));
        assert!(climbed.detected);
    }

    /// Names and serialization agree, and infrared is the suite's first system.
    #[test]
    fn modes_serialize_by_name() {
        for mode in SensorMode::ALL {
            assert_eq!(
                serde_json::to_value(mode).unwrap(),
                serde_json::json!(mode.name())
            );
        }
        assert_eq!(SensorMode::default(), SensorMode::Infrared);
    }
}
