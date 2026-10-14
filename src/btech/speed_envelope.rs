//! A BattleMech's walking and running ceilings in km/h, and the movement-point arithmetic
//! Total Warfare applies to them: walking MP is the template's own figure, running MP is
//! walking x 1.5 rounded up (TW p. 53), and damage, heat, myomer and boosters change the
//! walking figure before running is recomputed from it.
use anyhow::{Result, ensure};

/// Kilometres per hour of one movement point: one hex a turn (Total Warfare p. 48).
pub const MP_KPH: f64 = 10.75;

/// Walking and running ceilings in km/h after whatever modifiers the caller has applied.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpeedEnvelope {
    /// Walking (cruising) ceiling; backward travel is limited to it (TW p. 49).
    pub walking: f64,
    /// Running ceiling, normally walking x 1.5 rounded up to whole MP.
    pub running: f64,
}

/// Running MP for a walking figure: x 1.5, rounded up (Total Warfare p. 53). Fractional
/// walking figures come from load and gravity scaling and round the same way.
fn running_mp(walking_mp: f64) -> f64 {
    (walking_mp * 1.5 - 1e-9).ceil().max(0.0)
}

/// Walking MP recovered from a running ceiling: the largest whole figure whose running
/// MP is not above it, so a 5/8 walks at 5 and a 3/5 at 3.
fn walking_mp_of_running(running_kph: f64) -> f64 {
    let running_mp = running_kph / MP_KPH;
    if running_mp <= 1.0 {
        // A ceiling of one MP or less is a walk the unit cannot better (a lost leg).
        return running_mp.max(0.0);
    }
    (running_mp / 1.5 + 1e-6).floor()
}

impl SpeedEnvelope {
    /// No movement at all.
    pub const STOPPED: Self = Self {
        walking: 0.0,
        running: 0.0,
    };

    /// The envelope of a whole-MP walking figure.
    pub fn from_walking_mp(walking_mp: f64) -> Self {
        let walking_mp = walking_mp.max(0.0);
        Self {
            walking: walking_mp * MP_KPH,
            running: running_mp(walking_mp) * MP_KPH,
        }
    }

    /// The envelope a running ceiling implies: templates and administrative edits store
    /// the running figure, from which the whole-MP walking figure is recovered. A running
    /// ceiling that is not a whole number of recomputed MP keeps its excess as an offset.
    pub fn from_running(running_kph: f64) -> Self {
        let running_kph = running_kph.max(0.0);
        Self {
            walking: (walking_mp_of_running(running_kph) * MP_KPH).min(running_kph),
            running: running_kph,
        }
    }

    /// Walking ceiling in movement points.
    pub fn walking_mp(self) -> f64 {
        self.walking / MP_KPH
    }

    /// Whether the unit cannot move at all.
    pub fn is_stopped(self) -> bool {
        self.running <= 0.0
    }

    /// Scale both ceilings, for load and gravity.
    pub fn scaled(self, factor: f64) -> Self {
        Self {
            walking: (self.walking * factor).max(0.0),
            running: (self.running * factor).max(0.0),
        }
    }

    /// Change the walking figure by `delta` MP and recompute running as the new walking
    /// x 1.5 rounded up, keeping any offset the running ceiling already carried (a
    /// hardened-armor running penalty or an administrative edit).
    pub fn with_walking_mp_delta(self, delta: f64) -> Self {
        let walking_mp = (self.walking_mp() + delta).max(0.0);
        let offset = self.running - running_mp(self.walking_mp()) * MP_KPH;
        let running = (running_mp(walking_mp) * MP_KPH + offset).max(0.0);
        let walking = walking_mp * MP_KPH;
        Self {
            walking,
            running: running.max(walking),
        }
    }

    /// Running under MASC or a supercharger is twice the current walking MP (TW p. 137,
    /// Tactical Operations: Advanced Units and Equipment p. 345); under both it is 2.5
    /// times, rounded up.
    pub fn with_boosters(self, boosters: u8) -> Self {
        let walking_mp = self.walking_mp();
        let running_mp = match boosters {
            0 => return self,
            1 => (walking_mp * 2.0 - 1e-9).ceil(),
            _ => (walking_mp * 2.5 - 1e-9).ceil(),
        };
        Self {
            walking: self.walking,
            running: (running_mp * MP_KPH).max(self.walking),
        }
    }

    /// Clamp a signed throttle request to the envelope: backward at most the walking
    /// ceiling, forward at most the running ceiling.
    pub fn clamp(self, speed: f64) -> f64 {
        speed.clamp(-self.walking, self.running)
    }

    /// Whether a signed speed is a running speed: faster forward than the walking ceiling,
    /// with a small tolerance for accumulated floating-point error.
    pub fn is_running(self, speed: f64) -> bool {
        speed > self.walking + 0.1
    }

    /// Both ceilings must be finite, non-negative and ordered.
    pub fn validate(self) -> Result<()> {
        ensure!(
            self.walking.is_finite()
                && self.running.is_finite()
                && self.walking >= 0.0
                && self.running >= self.walking,
            "Invalid speed envelope"
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Running MP is walking x 1.5 rounded up, and the template's running figure gives
    /// back its odd walking figure.
    #[test]
    fn walking_and_running_round_trip() {
        for (walk, run) in [
            (0.0, 0.0),
            (1.0, 2.0),
            (3.0, 5.0),
            (4.0, 6.0),
            (5.0, 8.0),
            (7.0, 11.0),
        ] {
            let envelope = SpeedEnvelope::from_walking_mp(walk);
            assert_eq!(envelope.walking, walk * MP_KPH);
            assert_eq!(envelope.running, run * MP_KPH);
            let recovered = SpeedEnvelope::from_running(run * MP_KPH);
            assert_eq!(recovered, envelope);
        }
        // A 5/8 walks at 53.75, not two thirds of its 86 km/h run.
        assert_eq!(SpeedEnvelope::from_running(86.0).walking, 53.75);
        assert_eq!(SpeedEnvelope::from_running(32.25).walking, 21.5);
    }

    /// TSM adds two walking MP and heat takes them away again before running is refigured
    /// (Total Warfare pp. 139 and 159): a 7/11 at heat 10 moves 8/12.
    #[test]
    fn walking_deltas_refigure_running() {
        let base = SpeedEnvelope::from_walking_mp(7.0);
        let hot = base.with_walking_mp_delta(2.0);
        assert_eq!(hot, SpeedEnvelope::from_walking_mp(9.0));
        assert_eq!(
            hot.with_walking_mp_delta(-1.0),
            SpeedEnvelope::from_walking_mp(8.0)
        );
        assert_eq!(hot.with_walking_mp_delta(-1.0).running, 129.0);
        assert_eq!(
            base.with_walking_mp_delta(-10.0),
            SpeedEnvelope::from_walking_mp(0.0)
        );
        // A hardened-armor running penalty survives the refiguring.
        let hardened = SpeedEnvelope {
            walking: 43.0,
            running: 53.75,
        };
        assert_eq!(hardened.with_walking_mp_delta(-1.0).running, 43.0);
        assert_eq!(hardened.with_walking_mp_delta(-1.0).walking, 32.25);
    }

    /// Boosters double the current walking MP, or multiply it by 2.5 together.
    #[test]
    fn boosters_multiply_walking() {
        let base = SpeedEnvelope::from_walking_mp(5.0);
        assert_eq!(base.with_boosters(0), base);
        assert_eq!(base.with_boosters(1).running, 107.5);
        assert_eq!(base.with_boosters(2).running, 139.75);
        assert_eq!(base.with_boosters(2).walking, 53.75);
        // Heat reduces walking before the booster multiplies it.
        assert_eq!(
            base.with_walking_mp_delta(-1.0).with_boosters(1).running,
            86.0
        );
    }

    /// Clamping limits reverse to walking and forward to running.
    #[test]
    fn clamp_and_running_threshold() {
        let envelope = SpeedEnvelope::from_walking_mp(4.0);
        assert_eq!(envelope.clamp(-100.0), -43.0);
        assert_eq!(envelope.clamp(100.0), 64.5);
        assert_eq!(envelope.clamp(50.0), 50.0);
        assert!(!envelope.is_running(43.0));
        assert!(envelope.is_running(43.2));
        assert!(envelope.scaled(0.5).validate().is_ok());
        assert!(
            SpeedEnvelope {
                walking: 10.0,
                running: 5.0
            }
            .validate()
            .is_err()
        );
    }
}
