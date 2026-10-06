//! One committed turn phase shared by all units, independent of startup and wall-clock jumps.
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};

/// A bounded global phase preserves the reference's odd-tick rounding without an unbounded counter.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "u8", into = "u8")]
pub(crate) struct TurnClock(u8);

impl TryFrom<u8> for TurnClock {
    type Error = anyhow::Error;
    fn try_from(phase: u8) -> Result<Self> {
        ensure!(phase < 30, "Invalid BattleTech turn phase");
        Ok(Self(phase))
    }
}
impl From<TurnClock> for u8 {
    fn from(clock: TurnClock) -> Self {
        clock.0
    }
}
impl TurnClock {
    /// Called once per committed simulation second, even on otherwise idle worlds.
    pub(crate) fn advance(&mut self) {
        self.0 = (self.0 + 1) % 30;
    }
    /// Both the last odd tick and the following even tick belong to the turn boundary.
    pub(super) fn due(self) -> bool {
        self.0 == 29 || self.0 == 0
    }
    /// Whether this is the first phase of a turn, for effects that happen once a turn.
    pub(super) fn starts_turn(self) -> bool {
        self.0 == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Odd/even turn boundaries recur together, and saved phases cannot exceed one turn.
    #[test]
    fn phase_wrap_and_rounding_are_bounded() {
        let mut clock = TurnClock::default();
        for elapsed in 1..=90 {
            clock.advance();
            assert_eq!(clock.due(), matches!(elapsed % 30, 0 | 29));
            assert_eq!(
                serde_json::from_value::<TurnClock>(serde_json::to_value(clock).unwrap()).unwrap(),
                clock
            );
        }
        assert!(serde_json::from_value::<TurnClock>(30.into()).is_err());
        assert!(serde_json::from_value::<TurnClock>((-1).into()).is_err());
    }
}
