//! Analog frequency search using receiver-owned dice and the enclosing transmission transaction.
use super::Notice;
use crate::{ObjectId, World};
use anyhow::Result;
use serde::Serialize;

/// One detected transmission moves a scanning channel toward its source frequency.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FrequencyScan {
    pub receiver: ObjectId,
    pub channel: u8,
    pub previous: u32,
    pub frequency: u32,
}

/// Inspect unmatched analog traffic; distance, power, muting and ECM do not gate a search.
/// Zero-frequency broadcasts are excluded. An already exact frequency needs no search.
pub(super) fn scan(
    world: &mut World,
    receiver: ObjectId,
    frequency: u32,
    message: &str,
) -> Result<(Vec<FrequencyScan>, Vec<Notice>)> {
    let unit = super::radio::unit(world, receiver)?;
    if frequency == 0 || !unit.radio_capabilities().scan {
        return Ok((Vec::new(), Vec::new()));
    }
    let channels: Vec<_> = unit
        .radio_channels()
        .iter()
        .enumerate()
        .filter(|(_, c)| c.mode.scan && c.frequency != frequency)
        .map(|(i, c)| (i, c.frequency))
        .collect();
    let unit = super::radio::storage(world, receiver)?;
    let mut scans = Vec::new();
    let mut notices = Vec::new();
    for (channel, previous) in channels {
        if usize::from(unit.dice.die(100)?) > message.len().min(80) {
            continue;
        }
        if scans.is_empty() {
            notices.push(Notice {
                unit: receiver,
                text: "You notice a unknown transmission your scanner.. ".into(),
            });
        }
        let difference = frequency.abs_diff(previous);
        let fraction = u32::from(unit.dice.die(message.len().min(99) as u16)?);
        let step = (fraction * difference / 100).max(1);
        let progress = step * 100 / difference;
        let precision = match progress {
            0..30 => "somewhat",
            30..60 => "fairly well",
            60..95 => "precisely",
            _ => "exactly",
        };
        let next = if frequency > previous {
            previous + step
        } else {
            previous - step
        };
        unit.radio[channel].frequency = next;
        scans.push(FrequencyScan {
            receiver,
            channel: channel as u8,
            previous,
            frequency: next,
        });
        notices.push(Notice {
            unit: receiver,
            text: format!(
                "Your systems manage to zero on it {precision} on channel {}.",
                char::from(b'A' + channel as u8)
            ),
        });
    }
    Ok((scans, notices))
}
