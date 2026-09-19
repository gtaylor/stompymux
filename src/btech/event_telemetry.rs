//! Process-local event timing used by the reference lag percentage.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct BattleEventTelemetry {
    pub process_start: i64,
    pub ticks: u64,
}

impl BattleEventTelemetry {
    pub fn lag(self, now: i64) -> i32 {
        if self.ticks == 0 {
            return 0;
        }
        let value = (i128::from(now.saturating_sub(self.process_start)) * 100
            / i128::from(self.ticks))
            - 100;
        value.clamp(i128::from(i32::MIN), i128::from(i32::MAX)) as i32
    }
}
