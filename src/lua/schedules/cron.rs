//! The C server's five-field UTC cron grammar, compiled into bounded field masks.
use anyhow::{Context, Result, ensure};
use chrono::{DateTime, Datelike, Timelike, Utc};

/// C SBUF_SIZE includes the trailing terminator.
const EXPRESSION_LIMIT: usize = 256;

/// A field retains whether its entire source was literally `*`, for C's day matching.
#[derive(Clone, Debug)]
struct Field {
    mask: u64,
    wildcard: bool,
}

impl Field {
    /// Validate every list element even when an earlier element already matches.
    fn parse(source: &str, minimum: u32, maximum: u32) -> Result<Self> {
        let mut mask = 0;
        for part in source.split(',') {
            let (range, step) = match part.split_once('/') {
                Some((range, step)) => (range, number(step)?),
                None => (part, 1),
            };
            ensure!(step > 0, "step must be positive");
            let (first, last) = if range == "*" {
                (u64::from(minimum), u64::from(maximum))
            } else if let Some((first, last)) = range.split_once('-') {
                (number(first)?, number(last)?)
            } else {
                let value = number(range)?;
                (value, value)
            };
            ensure!(
                first >= u64::from(minimum) && last <= u64::from(maximum) && first <= last,
                "range must be within {minimum}..{maximum}"
            );
            for value in first..=last {
                if (value - first) % step == 0 {
                    mask |= 1 << value;
                }
            }
        }
        Ok(Self {
            mask,
            wildcard: source == "*",
        })
    }

    /// Constant-time lookup after complete validation.
    fn matches(&self, value: u32) -> bool {
        self.mask & (1 << value) != 0
    }
}

/// Match C strtol's unsigned decimal syntax and signed 64-bit range.
fn number(source: &str) -> Result<u64> {
    ensure!(
        !source.is_empty() && source.bytes().all(|b| b.is_ascii_digit()),
        "expected decimal number"
    );
    let value: u64 = source.parse().context("number overflow")?;
    ensure!(value <= i64::MAX as u64, "number overflow");
    Ok(value)
}

/// Validated cron text plus minute, hour, day, month and weekday masks.
#[derive(Clone, Debug)]
pub struct Cron {
    source: String,
    fields: [Field; 5],
}

impl Cron {
    /// Compile only the legacy numeric five-field grammar; no seconds, names or macros.
    pub fn parse(source: &str) -> Result<Self> {
        ensure!(
            source.len() < EXPRESSION_LIMIT,
            "cron expression must be shorter than {EXPRESSION_LIMIT} bytes"
        );
        let fields: Vec<_> = source
            .split([' ', '\t'])
            .filter(|s| !s.is_empty())
            .collect();
        ensure!(fields.len() == 5, "cron requires five fields");
        let ranges = [(0, 59), (0, 23), (1, 31), (1, 12), (0, 6)];
        let parsed = fields
            .iter()
            .zip(ranges)
            .enumerate()
            .map(|(index, (text, (low, high)))| {
                Field::parse(text, low, high).with_context(|| format!("cron field {}", index + 1))
            })
            .collect::<Result<Vec<_>>>()?;
        Ok(Self {
            source: source.into(),
            fields: parsed.try_into().expect("five fields"),
        })
    }

    /// Preserve author spelling for contexts and inspection.
    pub fn source(&self) -> &str {
        &self.source
    }

    /// Day fields use OR only when neither source field was the literal wildcard.
    pub fn matches(&self, timestamp: i64) -> bool {
        let Some(date): Option<DateTime<Utc>> = DateTime::from_timestamp(timestamp, 0) else {
            return false;
        };
        let [minute, hour, day, month, weekday] = &self.fields;
        let days = (
            day.matches(date.day()),
            weekday.matches(date.weekday().num_days_from_sunday()),
        );
        minute.matches(date.minute())
            && hour.matches(date.hour())
            && month.matches(date.month())
            && if !day.wildcard && !weekday.wildcard {
                days.0 || days.1
            } else {
                days.0 && days.1
            }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    /// Test UTC matching without reading the machine clock.
    fn at(y: i32, m: u32, d: u32, h: u32, min: u32) -> i64 {
        Utc.with_ymd_and_hms(y, m, d, h, min, 0)
            .unwrap()
            .timestamp()
    }

    #[test]
    fn cron_c_grammar_calendar_and_complete_validation() {
        let sunday = at(2026, 9, 6, 12, 30);
        let monday = at(2026, 9, 7, 12, 30);
        for source in [
            "* * * * *",
            "30 12 6 9 0",
            "0,15,30 10-14/2 * * *",
            "*/15 * * * *",
            "\t30\t12  * 9 * ",
        ] {
            assert!(Cron::parse(source).unwrap().matches(sunday), "{source}");
        }
        for source in [
            "31 * * * *",
            "* 13 * * *",
            "* * * 10 *",
            "* * * * 7",
            "@hourly",
            "0 0 0 * * *",
            "0 0 * * MON",
            "*/0 * * * *",
            "10-5 * * * *",
            "*,bad * * * *",
            "*, * * * *",
            "0/1/2 * * * *",
            "0 * 0 * *",
            "0 * * 13 *",
            "0\n0 * * *",
            "0 * * * *\0",
        ] {
            if let Ok(cron) = Cron::parse(source) {
                assert!(!cron.matches(sunday), "{source}");
            }
        }
        for source in [
            "*,bad * * * *",
            "*, * * * *",
            "*/0 * * * *",
            "-1 * * * *",
            "+1 * * * *",
            "1-2-3 * * * *",
            "1/9223372036854775808 * * * *",
        ] {
            assert!(Cron::parse(source).is_err(), "{source}");
        }
        assert!(
            !Cron::parse("30/2 * * * *")
                .unwrap()
                .matches(at(2026, 9, 6, 12, 32))
        ); // a single value with a step remains a singleton
        assert!(Cron::parse("30 12 1 * 0").unwrap().matches(sunday)); // restricted days OR
        assert!(!Cron::parse("30 12 * * 0").unwrap().matches(monday)); // literal wildcard uses AND
        assert!(Cron::parse("30 12 */1 * 0").unwrap().matches(monday)); // */1 is deliberately not literal *
        assert!(
            Cron::parse("0 0 29 2 *")
                .unwrap()
                .matches(at(2024, 2, 29, 0, 0))
        );
        assert!(
            !Cron::parse("0 0 29 2 *")
                .unwrap()
                .matches(at(2025, 2, 28, 0, 0))
        );
        assert!(Cron::parse(&format!("{}* * * * *", " ".repeat(247))).is_err());
        assert!(Cron::parse(&format!("{}* * * * *", " ".repeat(246))).is_ok());
    }
}
