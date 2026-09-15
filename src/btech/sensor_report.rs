//! Read-only reference sensor descriptions, active slot layout and pending selections.
use super::{BattleSensorMode as Sensor, BattleSensorPair, BattleVehicleMovement};
use crate::{ObjectId, World};
use anyhow::{Context, Result};

/// Descriptions are presentation text, not a second source of detection rules.
struct Description {
    range: &'static str,
    blocked: &'static str,
    notes: &'static str,
}

/// Report the reference's published sensor specifications without recalculating live eligibility.
fn description(sensor: Sensor) -> Description {
    let (range, blocked, notes) = match sensor {
        Sensor::Visual => (
            "Visual",
            "Fire/Smoke/Obstacles, 3 pt woods, 5 underwater hexes",
            "Bad in night-fighting (BTH)",
        ),
        Sensor::LightAmplification => (
            "Visual (Dawn/Dusk), 2x Visual (Night)",
            "Fire/Smoke/Obstacles, 2 pt woods, any water",
            "Somewhat harder enemy detection (than vislight), bad in forests (BTH/range)",
        ),
        Sensor::Infrared => (
            "15",
            "Fire/Obstacles, 6 pt woods",
            "Easy to hit 'hot' targets, not very efficient in forests (BTH)",
        ),
        Sensor::Electromagnetic => (
            "16-24",
            "Mountains/Obstacles, 8 pt woods",
            "Easy to hit heavies, good in forests (BTH), overall unreliable (chances of detection/BTH)",
        ),
        Sensor::Seismic => (
            "4-8",
            "Nothing",
            "Easier heavy and/or moving object detection (although overall hard to detect with), somewhat unreliable(BTH)",
        ),
        Sensor::Radar => (
            "<=180",
            "Obstacles, enemy elevation (Enemy Z >= 10, range: 180, Enemy Z < 10, range: varies)",
            "Premier anti-aircraft sensor, partially negates partial cover(BTH), doesn't see targets that are too low for detection",
        ),
        Sensor::BeagleProbe => (
            "<=6",
            "Nothing (except range)",
            "Ultimate sensor in close-range detection (slightly varying BTH, but ignores partial/woods/water)",
        ),
        Sensor::LightProbe => (
            "<=3",
            "Nothing (except range)",
            "Short range, but ultimate sensor in close-range detection (slightly varying BTH, but ignores partial/woods/water)",
        ),
        Sensor::BloodhoundProbe => (
            "<=8",
            "Nothing (except range)",
            "Superior version of the Beagle Active Probe (slightly varying BTH, but ignores partial/woods/water)",
        ),
    };
    Description {
        range,
        blocked,
        notes,
    }
}

/// Only optical modes need an arc annotation; matching slots or fixed platforms cover 360 degrees.
fn mode(sensor: Sensor, full_arc: bool, verbose: bool) -> String {
    let name = if sensor == Sensor::Visual {
        "Vislight"
    } else {
        sensor.name()
    };
    let mut text = name.to_owned();
    if matches!(sensor, Sensor::Visual | Sensor::LightAmplification) {
        text.push_str(if full_arc {
            " in 360 degree scanning mode"
        } else {
            " in 120 degree scanning mode (Forward arc)"
        });
    }
    text.push(' ');
    let info = description(sensor);
    if verbose {
        text.push_str(&format!(
            "\r\n\tRange:      {}\r\n\tBlocked by: {}\r\n\tNotes:      {}",
            info.range, info.blocked, info.notes
        ));
    } else {
        text.push_str(&format!("(R:{})", info.range));
    }
    text
}

/// One matching-mode line or a two-slot block, retaining reference spacing and header widths.
fn block(title: &str, pair: BattleSensorPair, stationary: bool, verbose: bool) -> String {
    if pair.primary == pair.secondary {
        return format!("{title}: {}", mode(pair.primary, true, verbose));
    }
    format!(
        "{title}\r\n{}\r\nPrimary:   {}\r\nSecondary: {}",
        "-".repeat(title.len()),
        mode(pair.primary, stationary, verbose),
        mode(pair.secondary, stationary, verbose)
    )
}

/// Inspect active and pending sensor modes without changing timers, locks, contacts or dice.
pub fn sensor_report(world: &World, id: ObjectId, verbose: bool) -> Result<String> {
    let (selection, stationary) = if let Some(unit) = world.btech.constructed_units().get(&id) {
        (unit.sensor_selection(), false)
    } else {
        let unit = world
            .btech
            .vehicles()
            .get(&id)
            .context("Enter a constructed unit first")?;
        (
            unit.sensor_selection(),
            unit.definition().movement == BattleVehicleMovement::Stationary,
        )
    };
    let mut text = block("Sensors", selection.active, stationary, verbose);
    if let Some(change) = selection.pending {
        text.push_str("\r\n");
        text.push_str(&block("Wanted", change.wanted, stationary, false));
    }
    Ok(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every published sensor name and compact range uses its reference spelling and punctuation.
    #[test]
    fn compact_descriptions_cover_all_nine_modes() {
        for (sensor, expected) in [
            (
                Sensor::Visual,
                "Vislight in 360 degree scanning mode (R:Visual)",
            ),
            (
                Sensor::LightAmplification,
                "Light-amplification in 360 degree scanning mode (R:Visual (Dawn/Dusk), 2x Visual (Night))",
            ),
            (Sensor::Infrared, "Infrared (R:15)"),
            (Sensor::Electromagnetic, "Electromagnetic (R:16-24)"),
            (Sensor::Seismic, "Seismic (R:4-8)"),
            (Sensor::Radar, "Radar (R:<=180)"),
            (Sensor::BeagleProbe, "Beagle ActiveProbe (R:<=6)"),
            (Sensor::LightProbe, "Light Beagle ActiveProbe (R:<=3)"),
            (Sensor::BloodhoundProbe, "Bloodhound ActiveProbe (R:<=8)"),
        ] {
            let pair = BattleSensorPair {
                primary: sensor,
                secondary: sensor,
            };
            assert_eq!(
                block("Sensors", pair, false, false),
                format!("Sensors: {expected}")
            );
            assert_eq!(
                block("Wanted", pair, true, false),
                format!("Wanted: {expected}")
            );
            let verbose = block("Sensors", pair, false, true);
            assert_eq!(verbose.matches("\tRange:      ").count(), 1);
            assert_eq!(verbose.matches("\tBlocked by: ").count(), 1);
            assert_eq!(verbose.matches("\tNotes:      ").count(), 1);
            assert!(!verbose.contains("(R:"));
        }
    }
}
