//! Durable administrative hardware overrides, distinct from template default requests.
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};

/// Absent settings retain installed hardware; explicit zero disables the corresponding range.
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
        ensure!(self.targeting_mode <= 4, "Invalid targeting mode");
        ensure!(
            [self.tactical, self.long_range, self.scan]
                .into_iter()
                .flatten()
                .all(|v| v.value <= 127 && v.sensor_hits <= 2),
            "Invalid sensor range"
        );
        ensure!(
            self.radio_range.is_none_or(|v| v <= 32767),
            "Invalid radio range"
        );
        Ok(())
    }

    /// Parse a named field into a candidate so invalid edits cannot partially change hardware.
    pub(super) fn set(&mut self, field: &str, value: &str, sensor_hits: u8) -> Result<()> {
        let mut candidate = *self;
        let range = || -> Result<RangeSetting> {
            Ok(RangeSetting {
                value: value.trim().parse()?,
                sensor_hits: sensor_hits.min(2),
            })
        };
        match field {
            "targcomp" => candidate.targeting_mode = value.trim().parse()?,
            "tacrange" => candidate.tactical = Some(range()?),
            "lrsrange" => candidate.long_range = Some(range()?),
            "scanrange" => candidate.scan = Some(range()?),
            "radiorange" => candidate.radio_range = Some(value.trim().parse()?),
            "radiotype" => candidate.radio_configuration = Some(value.trim().parse()?),
            _ => anyhow::bail!("Unknown hardware field"),
        }
        candidate.validate()?;
        *self = candidate;
        Ok(())
    }
}

/// A current range with the damage baseline at which an administrator assigned it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub(super) struct RangeSetting {
    value: u8,
    sensor_hits: u8,
}

impl RangeSetting {
    /// New sensor criticals degrade an assigned range; prior damage has already been accounted for.
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
}
