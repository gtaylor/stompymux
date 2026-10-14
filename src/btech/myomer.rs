//! Triple Strength Myomer activation and the live speed envelope it, heat and the boosters
//! produce from damage-only mobility.
use super::{Mech, SpeedEnvelope};

impl Mech {
    /// TSM activates at the last sampled excess heat of nine, independent of stored weapon heat.
    pub fn triple_myomer_active(&self) -> bool {
        self.definition().has_triple_myomer() && self.heat().excess >= 9.0
    }

    /// Apply equipment and heat to a damage-adjusted, load-scaled envelope. Hot TSM adds
    /// two walking MP (Total Warfare p. 139), heat takes one walking MP per five points
    /// (TW p. 159), running is refigured as walking x 1.5 rounded up after each, and an
    /// active MASC or supercharger then runs at twice the walking figure left (TW p. 137).
    pub(super) fn equip_envelope(&self, base: SpeedEnvelope) -> SpeedEnvelope {
        self.equipped(base, self.heat().walking_mp_penalty())
    }

    /// The envelope equipment alone would give, as if the unit were cool.
    pub(super) fn equip_envelope_without_heat(&self, base: SpeedEnvelope) -> SpeedEnvelope {
        self.equipped(base, 0.0)
    }

    /// Apply hot myomer, a walking-MP heat penalty and the active boosters, in that order.
    fn equipped(&self, base: SpeedEnvelope, heat_penalty: f64) -> SpeedEnvelope {
        if base.is_stopped() {
            return base;
        }
        let mut envelope = base;
        if self.triple_myomer_active() {
            envelope = envelope.with_walking_mp_delta(2.0);
        }
        if heat_penalty > 0.0 {
            envelope = envelope.with_walking_mp_delta(-heat_penalty);
        }
        envelope.with_boosters(u8::from(self.masc_active()) + u8::from(self.supercharger_active()))
    }

    /// The throttle envelope of a unit nothing tows on an ordinary map.
    pub fn speed_envelope(&self) -> SpeedEnvelope {
        self.equip_envelope(self.mobility().envelope())
    }

    /// Current running ceiling; mechanical mobility remains a separate damage-only quantity.
    pub fn movement_maximum_speed(&self) -> f64 {
        self.speed_envelope().running
    }

    /// Current walking ceiling, the threshold above which the unit is running.
    pub fn walking_speed(&self) -> f64 {
        self.speed_envelope().walking
    }

    /// The widest envelope saved controls may retain: every installed booster and hot
    /// myomer applied to `base`, under the lightest gravity the rules allow.
    pub(super) fn motion_speed_limit(&self, base: SpeedEnvelope) -> SpeedEnvelope {
        let mut envelope = base;
        if self.definition().has_triple_myomer() {
            envelope = envelope.with_walking_mp_delta(2.0);
        }
        let boosters = u8::from(self.masc_installed().unwrap_or(false))
            + u8::from(self.supercharger_installed());
        envelope.with_boosters(boosters).scaled(2.0)
    }
}
