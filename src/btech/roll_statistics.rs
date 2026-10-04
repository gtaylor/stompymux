//! Generic BattleTech roll accounting and reference-format diagnostics, independent of random streams.
use anyhow::{Result, ensure};
use std::fmt::Write;

/// Counts of explicitly classified generic checks; direct character dice are not automatically counted.
/// Own this at the simulation lifetime boundary, so removing a unit cannot discard history.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RollStatistics {
    counts: [u64; 11],
}

impl RollStatistics {
    /// Merge committed counts, validating the aggregate before changing any bucket.
    pub fn merge(&mut self, other: &Self) -> Result<()> {
        ensure!(
            self.total().checked_add(other.total()).is_some(),
            "Roll statistics overflow"
        );
        for (target, source) in self.counts.iter_mut().zip(other.counts) {
            *target += source;
        }
        Ok(())
    }

    /// Record one completed generic 2d6 check, rejecting invalid or overflowing updates atomically.
    pub fn record(&mut self, roll: u8) -> Result<()> {
        ensure!(
            (2..=12).contains(&roll),
            "Generic roll must be between 2 and 12"
        );
        ensure!(self.total() < u64::MAX, "Roll statistics overflow");
        self.counts[usize::from(roll - 2)] += 1;
        Ok(())
    }

    /// Exact count of recorded checks, derived without a second mutable total.
    pub fn total(&self) -> u64 {
        self.counts.iter().sum()
    }

    /// Inspect counts in sum order, from two through twelve, without exposing a generator.
    pub fn counts(&self) -> &[u64; 11] {
        &self.counts
    }

    /// Render the wizard report; admission and committed roll collection belong to the host.
    pub fn render(&self) -> String {
        let total = self.total();
        if total == 0 {
            return "No rolls to show statistics for!".into();
        }
        const WEIGHTS: [u8; 11] = [1, 2, 3, 4, 5, 6, 5, 4, 3, 2, 1];
        let mut output = String::from(
            "#    Rolls %Current  Optimal Rolls %Optimal  %Hit Chance  %Miss Chance\n",
        );
        for (index, (&count, &weight)) in self.counts.iter().zip(&WEIGHTS).enumerate() {
            let optimal = f32::from(weight) * 100.0 / 36.0;
            let current = count as f32 * 100.0 / total as f32;
            let successes: u8 = WEIGHTS[index..].iter().sum();
            let hit = f32::from(successes) / 36.0 * 100.0;
            let expected = (optimal / 100.0 * total as f32) as u64;
            writeln!(
                output,
                "{:<3} {:6} {:8.3} {:14} {:8.3} {:12.3} {:13.3}",
                index + 2,
                count,
                current,
                expected,
                optimal,
                hit,
                100.0_f32 - hit,
            )
            .expect("writing to a String cannot fail");
        }
        write!(output, "Total rolls: {total}").expect("writing to a String cannot fail");
        output
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Edge targets have exact cumulative endpoints; rendering never changes the histogram.
    #[test]
    fn reference_rows_and_empty_report() {
        let mut statistics = RollStatistics::default();
        assert_eq!(statistics.render(), "No rolls to show statistics for!");
        statistics.record(7).unwrap();
        let before = statistics.clone();
        let report = statistics.render();
        let rows: Vec<_> = report.lines().collect();
        assert_eq!(rows.len(), 13);
        assert_eq!(
            rows[0],
            "#    Rolls %Current  Optimal Rolls %Optimal  %Hit Chance  %Miss Chance"
        );
        assert_eq!(
            rows[1],
            "2        0    0.000              0    2.778      100.000         0.000"
        );
        assert_eq!(
            rows[6],
            "7        1  100.000              0   16.667       58.333        41.667"
        );
        assert_eq!(
            rows[11],
            "12       0    0.000              0    2.778        2.778        97.222"
        );
        assert_eq!(rows[12], "Total rolls: 1");
        assert_eq!(statistics, before);
    }

    /// Each valid sum owns one bucket; invalid values and overflow do not partially change counts.
    #[test]
    fn bounded_accounting_and_independent_candidates() {
        let mut statistics = RollStatistics::default();
        for roll in 2..=12 {
            statistics.record(roll).unwrap();
        }
        assert_eq!(statistics.counts(), &[1; 11]);
        assert_eq!(statistics.total(), 11);
        let before = statistics.clone();
        for roll in [0, 1, 13, 255] {
            assert!(statistics.record(roll).is_err());
            assert_eq!(statistics, before);
        }
        let mut candidate = statistics.clone();
        candidate.record(7).unwrap();
        assert_eq!(statistics, before);
        let mut full = RollStatistics { counts: [0; 11] };
        full.counts[0] = u64::MAX - 1;
        full.record(12).unwrap();
        let before = full.clone();
        assert!(full.record(7).is_err());
        assert_eq!(full, before);
        assert!(full.merge(&statistics).is_err());
        assert_eq!(full, before);
        let mut combined = statistics.clone();
        combined.merge(&statistics).unwrap();
        assert_eq!(combined.counts(), &[2; 11]);
    }
}
