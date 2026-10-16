//! Durable per-unit hardware overrides that scripts and administrators assign at runtime.
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};

/// Largest sensor reach, in hexes, a unit may be assigned.
pub const MAX_SENSOR_RANGE: u8 = 127;

/// One of the three sensor displays whose reach a unit carries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SensorDisplay {
    /// The tactical map display.
    Tactical,
    /// The long-range map display.
    LongRange,
    /// Detailed `scan` inspection of units, buildings and mines.
    Scan,
}

/// Absent settings keep the default hardware; an explicit zero disables the corresponding range.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(super) struct HardwareSettings {
    #[serde(default)]
    pub targeting_mode: u8,
    pub tactical: Option<RangeSetting>,
    pub long_range: Option<RangeSetting>,
    pub scan: Option<RangeSetting>,
    pub radio_range: Option<u16>,
    pub radio_configuration: Option<u8>,
}

impl HardwareSettings {
    /// Reject snapshots whose ranges exceed the supported hardware representation.
    pub(super) fn validate(self) -> Result<()> {
        ensure!(
            [self.tactical, self.long_range, self.scan]
                .into_iter()
                .flatten()
                .all(|v| v.value <= MAX_SENSOR_RANGE && v.sensor_hits <= 2),
            "Invalid sensor range"
        );
        ensure!(
            self.radio_range.is_none_or(|v| v <= 32767),
            "Invalid radio range"
        );
        Ok(())
    }

    /// Assign one display's current reach on a unit that has taken `sensor_hits` sensor
    /// criticals, so only later criticals degrade it.
    pub(super) fn set_sensor_range(
        &mut self,
        display: SensorDisplay,
        range: u8,
        sensor_hits: u8,
    ) -> Result<()> {
        ensure!(range <= MAX_SENSOR_RANGE, "Invalid sensor range");
        let setting = Some(RangeSetting {
            value: range,
            sensor_hits: sensor_hits.min(2),
        });
        match display {
            SensorDisplay::Tactical => self.tactical = setting,
            SensorDisplay::LongRange => self.long_range = setting,
            SensorDisplay::Scan => self.scan = setting,
        }
        Ok(())
    }

    /// Parse a named field into a candidate so invalid edits cannot partially change hardware.
    pub(super) fn set(&mut self, field: &str, value: &str, sensor_hits: u8) -> Result<()> {
        let mut candidate = *self;
        match field {
            "radiorange" => candidate.radio_range = Some(value.trim().parse()?),
            "radiotype" => candidate.radio_configuration = Some(value.trim().parse()?),
            _ => {
                let display = match field {
                    "tacrange" => SensorDisplay::Tactical,
                    "lrsrange" => SensorDisplay::LongRange,
                    "scanrange" => SensorDisplay::Scan,
                    _ => anyhow::bail!("Unknown hardware field"),
                };
                candidate.set_sensor_range(display, value.trim().parse()?, sensor_hits)?;
            }
        }
        candidate.validate()?;
        *self = candidate;
        Ok(())
    }
}

/// A current range with the damage baseline at which it was assigned.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub(super) struct RangeSetting {
    value: u8,
    sensor_hits: u8,
}

impl RangeSetting {
    /// Undamaged hardware of the given reach, which every sensor critical degrades.
    pub(super) const fn installed(value: u8) -> Self {
        Self {
            value,
            sensor_hits: 0,
        }
    }

    /// New sensor criticals degrade an assigned range; prior damage has already been accounted for.
    /// The first new critical halves the range and a second leaves none.
    pub(super) fn at_hits(self, hits: u8) -> u8 {
        if hits <= self.sensor_hits {
            return self.value;
        }
        if hits >= 2 {
            return 0;
        }
        self.value / 2
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn assigned_ranges_preserve_current_values_and_take_subsequent_damage() {
        let mut hardware = HardwareSettings::default();
        hardware.set("scanrange", "25", 0).unwrap();
        let range = hardware.scan.unwrap();
        assert_eq!(range.at_hits(0), 25);
        assert_eq!(range.at_hits(1), 12);
        assert_eq!(range.at_hits(2), 0);
        hardware.set("scanrange", "17", 1).unwrap();
        assert_eq!(hardware.scan.unwrap().at_hits(1), 17);
        assert_eq!(hardware.scan.unwrap().at_hits(2), 0);
        hardware.set("scanrange", "0", 0).unwrap();
        assert_eq!(hardware.scan.unwrap().at_hits(0), 0);
    }

    #[test]
    fn typed_sensor_ranges_reach_their_own_display_and_reject_oversized_values() {
        let mut hardware = HardwareSettings::default();
        hardware
            .set_sensor_range(SensorDisplay::Tactical, 9, 0)
            .unwrap();
        hardware
            .set_sensor_range(SensorDisplay::LongRange, 40, 1)
            .unwrap();
        assert_eq!(hardware.tactical.unwrap().at_hits(0), 9);
        assert_eq!(hardware.long_range.unwrap().at_hits(1), 40);
        assert_eq!(hardware.scan, None);
        assert!(
            hardware
                .set_sensor_range(SensorDisplay::Scan, MAX_SENSOR_RANGE + 1, 0)
                .is_err()
        );
        assert_eq!(hardware.scan, None);
        assert!(hardware.set("tacrange", "128", 0).is_err());
        assert_eq!(hardware.tactical.unwrap().at_hits(0), 9);
    }
}
