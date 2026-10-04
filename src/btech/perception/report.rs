//! The read-only perception summary printed by the `sensor` command and returned to Lua.
use super::{PerceptionProfile, PerceptionStatus, perception_profile};
use crate::btech::{Light, Power};
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
        sensors_line(&profile),
        sight_line(&profile),
        probe_line(&profile),
        radar_line(&profile),
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

/// Explain the all-conditions band, including why it is short or silent.
fn sensors_line(profile: &PerceptionProfile) -> String {
    let detail = match profile.sensors {
        PerceptionStatus::Ready => {
            format!("{} hexes in any light or weather", profile.sensor_range)
        }
        PerceptionStatus::Degraded => format!(
            "{} hexes in any light or weather (damaged)",
            profile.sensor_range
        ),
        PerceptionStatus::Jammed => "jammed by ECM, relying on sight".into(),
        PerceptionStatus::Damaged => "destroyed, relying on sight".into(),
        PerceptionStatus::Disabled => "disabled on this battlefield".into(),
        PerceptionStatus::Absent => "none".into(),
    };
    format!("Sensors: {detail}")
}

/// Explain weather reach and the night-time darkness rule.
fn sight_line(profile: &PerceptionProfile) -> String {
    match profile.light {
        Light::Day => format!("Sight:   {} hexes", profile.sight_range),
        Light::Twilight => format!("Sight:   {} hexes (twilight)", profile.sight_range),
        Light::Night => format!(
            "Sight:   {} hexes at night, +1 to hit unless the target is lit; lit targets to {}",
            profile.sight_range, profile.lit_sight_range
        ),
    }
}

/// Name the working probe, or the reason it cannot see.
fn probe_line(profile: &PerceptionProfile) -> String {
    let Some(probe) = profile.probe else {
        return "Probe:   none".into();
    };
    format!(
        "Probe:   {}, {} hexes{}",
        probe.kind.name(),
        probe.range,
        status_suffix(probe.status)
    )
}

/// Describe installed radar and its condition.
fn radar_line(profile: &PerceptionProfile) -> String {
    let Some(radar) = profile.radar else {
        return "Radar:   none".into();
    };
    format!(
        "Radar:   {} hexes against airborne targets{}",
        radar.range,
        status_suffix(radar.status)
    )
}

/// Parenthetical condition for optional equipment; working equipment needs none.
fn status_suffix(status: PerceptionStatus) -> &'static str {
    match status {
        PerceptionStatus::Ready | PerceptionStatus::Absent => "",
        PerceptionStatus::Degraded => " (damaged)",
        PerceptionStatus::Jammed => " (jammed by ECM)",
        PerceptionStatus::Damaged => " (destroyed)",
        PerceptionStatus::Disabled => " (disabled on this battlefield)",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::btech::{ActiveProbe, ProbeProfile, RadarProfile};

    fn profile() -> PerceptionProfile {
        PerceptionProfile {
            light: Light::Night,
            sight_range: 10,
            lit_sight_range: 30,
            sensor_range: 15,
            sensors: PerceptionStatus::Ready,
            probe: Some(ProbeProfile {
                kind: ActiveProbe::Beagle,
                range: 6,
                status: PerceptionStatus::Jammed,
            }),
            radar: None,
            clairvoyant: false,
            ceiling: 30,
            cloud_base: 0,
            level: 0,
        }
    }

    /// Each system gets one line that names its reach or the reason it is silent.
    #[test]
    fn lines_describe_reach_and_conditions() {
        let profile = profile();
        assert_eq!(
            sensors_line(&profile),
            "Sensors: 15 hexes in any light or weather"
        );
        assert_eq!(
            sight_line(&profile),
            "Sight:   10 hexes at night, +1 to hit unless the target is lit; lit targets to 30"
        );
        assert_eq!(
            probe_line(&profile),
            "Probe:   Beagle Active Probe, 6 hexes (jammed by ECM)"
        );
        assert_eq!(radar_line(&profile), "Radar:   none");
        let jammed = PerceptionProfile {
            sensors: PerceptionStatus::Jammed,
            sensor_range: 0,
            light: Light::Day,
            radar: Some(RadarProfile {
                range: 180,
                status: PerceptionStatus::Ready,
            }),
            ..profile
        };
        assert_eq!(
            sensors_line(&jammed),
            "Sensors: jammed by ECM, relying on sight"
        );
        assert_eq!(sight_line(&jammed), "Sight:   10 hexes");
        assert_eq!(
            radar_line(&jammed),
            "Radar:   180 hexes against airborne targets"
        );
    }
}
