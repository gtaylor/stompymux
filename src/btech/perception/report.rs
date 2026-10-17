//! The read-only perception summary printed by the `sensor` command and returned to Lua.
use super::{PerceptionProfile, PerceptionStatus, SensorMode, perception_profile};
use crate::btech::Power;
use crate::{ObjectId, World};
use anyhow::{Context, Result};
use serde::Serialize;

/// A unit's perception systems together with the text the `sensor` command prints.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PerceptionReport {
    #[serde(flatten)]
    pub profile: PerceptionProfile,
    /// Whether the unit is running; stopped units perceive nothing.
    pub running: bool,
    pub text: String,
}

/// Summarize how far and by what means a placed unit can currently perceive.
pub fn perception_report(world: &World, id: ObjectId) -> Result<PerceptionReport> {
    let unit =
        crate::btech::scanner::scanner_unit(world, id).context("Enter a constructed unit first")?;
    unit.position.context("Unit is not on a battlefield")?;
    let running = unit.power == Power::Running;
    let profile = perception_profile(world, id)?;
    let mut lines = vec![
        sight_line(&profile),
        sensors_line(&profile),
        probe_line(&profile),
    ];
    if !running {
        lines.push("Your unit is shut down; nothing is perceived until it starts.".into());
    }
    Ok(PerceptionReport {
        profile,
        running,
        text: lines.join("\r\n"),
    })
}

/// Explain sight: its reach under the conditions, what darkness costs to hit, and how far a
/// lit target can be seen.
fn sight_line(profile: &PerceptionProfile) -> String {
    let light = profile.light;
    let conditions = format!(
        "{}, {}",
        light.label().to_lowercase(),
        profile.weather.label().to_lowercase()
    );
    let penalty = light.aim_modifier(false, super::super::Illumination::None, None);
    if light == crate::btech::Light::Day {
        return format!("Sight:   {} hexes ({conditions})", profile.sight_range);
    }
    let lit = light.aim_modifier(false, super::super::Illumination::Searchlight, None);
    if !light.searchlights_help_sight() {
        return format!(
            "Sight:   {} hexes ({conditions}): +{penalty} to hit",
            profile.sight_range
        );
    }
    format!(
        "Sight:   {} hexes ({conditions}): +{penalty} to hit, +{lit} if lit; lit targets seen to {}",
        profile.sight_range, profile.lit_sight_range
    )
}

/// Explain the built-in sensors: each system's brackets and this turn's sweep.
fn sensors_line(profile: &PerceptionProfile) -> String {
    let sensor = &profile.sensor;
    match sensor.status {
        PerceptionStatus::Absent => return "Sensors: none".into(),
        PerceptionStatus::Damaged => return "Sensors: destroyed".into(),
        // Electromagnetic interference silences only active probes, never the built-in sensors.
        PerceptionStatus::Ready | PerceptionStatus::Jammed | PerceptionStatus::Interference => {}
    }
    let systems = sensor
        .systems
        .iter()
        .filter(|system| system.mode != SensorMode::Probe)
        .map(|system| {
            format!(
                "{} {}/{}/{}",
                system.mode.label(),
                system.bands.short,
                system.bands.medium,
                system.bands.long
            )
        })
        .collect::<Vec<_>>()
        .join(", ");
    let sweep = match (sensor.sweep, sensor.band) {
        (None, _) => "sweep pending".to_owned(),
        (Some(roll), Some(band)) => format!("sweep {roll} finds the {} band", band.label()),
        (Some(roll), None) => format!("sweep {roll} finds nothing this turn"),
    };
    let jammed = if sensor.jammed {
        ", radar jammed by ECM"
    } else {
        ""
    };
    format!("Sensors: {systems} hexes; {sweep}{jammed}")
}

/// Name the probe, the brackets it sweeps with and how far it lets attacks see into woods
/// (Tactical Operations: Advanced Rules p. 97), or the reason it cannot.
fn probe_line(profile: &PerceptionProfile) -> String {
    let Some(probe) = profile.probe else {
        return "Probe:   none".into();
    };
    let sweeps = profile
        .sensor
        .systems
        .iter()
        .find(|system| system.mode == SensorMode::Probe)
        .map(|system| {
            format!(
                "sweeps {}/{}/{} hexes and ",
                system.bands.short, system.bands.medium, system.bands.long
            )
        })
        .unwrap_or_default();
    format!(
        "Probe:   {}, {sweeps}sees into woods within {} hexes{}",
        probe.kind.installed_name(probe.clan),
        probe.range,
        status_suffix(probe.status)
    )
}

/// Parenthetical condition for optional equipment; working equipment needs none.
fn status_suffix(status: PerceptionStatus) -> &'static str {
    match status {
        PerceptionStatus::Ready | PerceptionStatus::Absent => "",
        PerceptionStatus::Jammed => " (jammed by ECM)",
        PerceptionStatus::Damaged => " (destroyed)",
        PerceptionStatus::Interference => " (useless under electromagnetic interference)",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::btech::{
        ActiveProbe, Light, ProbeProfile, SensorBands, SensorProfile, SensorSystem, Weather,
    };

    fn system(mode: SensorMode, short: u8, medium: u8, long: u8) -> SensorSystem {
        SensorSystem {
            mode,
            bands: SensorBands {
                short,
                medium,
                long,
            },
        }
    }

    fn profile() -> PerceptionProfile {
        PerceptionProfile {
            light: Light::MoonlessNight,
            weather: Weather::Clear,
            sight_range: 5,
            lit_sight_range: 45,
            sensor: SensorProfile {
                systems: vec![
                    system(SensorMode::Infrared, 10, 20, 30),
                    system(SensorMode::Magscan, 10, 20, 30),
                    system(SensorMode::Radar, 8, 16, 24),
                    system(SensorMode::Seismic, 2, 4, 6),
                    system(SensorMode::Probe, 12, 24, 36),
                ],
                status: PerceptionStatus::Ready,
                jammed: false,
                sweep: Some(6),
                band: Some(super::super::SensorBand::Medium),
                probe: Some(ActiveProbe::Beagle),
                probe_clan: false,
            },
            probe: Some(ProbeProfile {
                kind: ActiveProbe::Beagle,
                clan: false,
                range: 4,
                status: PerceptionStatus::Ready,
            }),
            clairvoyant: false,
            cloud_base: 0,
            level: 0,
            team: 0,
            vehicle: false,
            underwater: false,
            disturbed: (false, false),
        }
    }

    /// Each system gets one line that names its reach or the reason it is silent.
    #[test]
    fn lines_describe_reach_and_conditions() {
        let profile = profile();
        assert_eq!(
            sight_line(&profile),
            "Sight:   5 hexes (moonless night, clear): +3 to hit, +0 if lit; lit targets seen to 45"
        );
        assert_eq!(
            sensors_line(&profile),
            "Sensors: infrared 10/20/30, magnetic anomaly 10/20/30, radar 8/16/24, seismic 2/4/6 hexes; sweep 6 finds the medium band"
        );
        assert_eq!(
            probe_line(&profile),
            "Probe:   Beagle Active Probe, sweeps 12/24/36 hexes and sees into woods within 4 hexes"
        );
        let day = PerceptionProfile {
            light: Light::Day,
            weather: Weather::HeavyFog,
            sight_range: 5,
            sensor: SensorProfile {
                systems: profile.sensor.systems[..4].to_vec(),
                jammed: true,
                sweep: Some(11),
                band: None,
                ..profile.sensor.clone()
            },
            probe: Some(ProbeProfile {
                kind: ActiveProbe::Beagle,
                clan: false,
                range: 4,
                status: PerceptionStatus::Damaged,
            }),
            ..profile.clone()
        };
        assert_eq!(sight_line(&day), "Sight:   5 hexes (day, heavy fog)");
        assert_eq!(
            sensors_line(&day),
            "Sensors: infrared 10/20/30, magnetic anomaly 10/20/30, radar 8/16/24, seismic 2/4/6 hexes; sweep 11 finds nothing this turn, radar jammed by ECM"
        );
        assert_eq!(
            probe_line(&day),
            "Probe:   Beagle Active Probe, sees into woods within 4 hexes (destroyed)"
        );
        let pending = PerceptionProfile {
            sensor: SensorProfile {
                sweep: None,
                band: None,
                ..profile.sensor.clone()
            },
            probe: None,
            ..profile.clone()
        };
        assert!(sensors_line(&pending).ends_with("hexes; sweep pending"));
        assert_eq!(probe_line(&pending), "Probe:   none");
        // Electromagnetic interference leaves the probe installed but useless: no sweep.
        let interfered = PerceptionProfile {
            sensor: SensorProfile {
                systems: profile.sensor.systems[..4].to_vec(),
                ..profile.sensor.clone()
            },
            probe: Some(ProbeProfile {
                kind: ActiveProbe::Beagle,
                clan: false,
                range: 4,
                status: PerceptionStatus::Interference,
            }),
            ..profile.clone()
        };
        assert_eq!(
            probe_line(&interfered),
            "Probe:   Beagle Active Probe, sees into woods within 4 hexes (useless under electromagnetic interference)"
        );
        let silent = PerceptionProfile {
            sensor: SensorProfile {
                systems: Vec::new(),
                status: PerceptionStatus::Damaged,
                ..profile.sensor.clone()
            },
            ..profile
        };
        assert_eq!(sensors_line(&silent), "Sensors: destroyed");
    }
}
