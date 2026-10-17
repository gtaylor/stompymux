//! Triple Strength Myomer activation and the live speed envelope it, heat, load, gravity and
//! the boosters produce from damage-only mobility.
use super::{LoadEffect, Mech, SpeedEnvelope};

impl Mech {
    /// TSM activates at the last sampled excess heat of nine, independent of stored weapon heat.
    pub fn triple_myomer_active(&self) -> bool {
        self.definition().has_triple_myomer() && self.heat().excess >= 9.0
    }

    /// Apply equipment and heat to a damage-adjusted envelope. Hot TSM adds two walking MP
    /// (Total Warfare p. 139), heat takes one walking MP per five points (TW p. 159), running
    /// is refigured as walking x 1.5 rounded up after each, and an active MASC or
    /// supercharger then runs at twice the walking figure left (TW p. 137).
    pub(super) fn equip_envelope(&self, base: SpeedEnvelope) -> SpeedEnvelope {
        self.live_envelope(
            base,
            self.heat().walking_mp_penalty(),
            LoadEffect::NONE,
            None,
        )
    }

    /// The template's envelope with myomer, heat, load and gravity applied but no damage: the
    /// figure effective-speed queries read, which a lost leg leaves alone.
    pub(super) fn template_envelope(
        &self,
        base: SpeedEnvelope,
        load: LoadEffect,
        gravity: Option<i64>,
    ) -> SpeedEnvelope {
        self.envelope_with(base, self.heat().walking_mp_penalty(), load, gravity, false)
    }

    /// The live envelope from a damage-adjusted `base`: hot myomer, a walking-MP heat
    /// penalty, the walking MP cargo in lift hoists takes off (Total Warfare p. 136), towed
    /// `load` (a factor on both ceilings) and the boosters, on a map of `gravity` percent of
    /// standard. Hoisted cargo that brings the walking MP to zero or less leaves 1 MP and no
    /// running, boosters included (ibid.). Away from standard gravity the walking MP left is
    /// divided by the G-rating and rounded to the nearest whole MP, halves down, after every
    /// other modifier, and running is then figured from it, boosters included (Tactical
    /// Operations: Advanced Rules p. 53). A 'Mech that has lost a leg walks at 1 MP whatever
    /// would raise it, and cannot run (TW p. 122).
    pub(super) fn live_envelope(
        &self,
        base: SpeedEnvelope,
        heat_penalty: f64,
        load: LoadEffect,
        gravity: Option<i64>,
    ) -> SpeedEnvelope {
        self.envelope_with(base, heat_penalty, load, gravity, true)
    }

    /// Shared envelope arithmetic; `damaged` applies the lost-leg override to a `base` that
    /// already carries the unit's damage.
    fn envelope_with(
        &self,
        base: SpeedEnvelope,
        heat_penalty: f64,
        load: LoadEffect,
        gravity: Option<i64>,
        damaged: bool,
    ) -> SpeedEnvelope {
        if base.is_stopped() {
            return base;
        }
        let crippled = damaged && self.leg_loss_fixes_walking();
        let mut envelope = base;
        if self.triple_myomer_active() && !crippled {
            envelope = envelope.with_walking_mp_delta(2.0);
        }
        if heat_penalty > 0.0 {
            envelope = envelope.with_walking_mp_delta(-heat_penalty);
        }
        let (envelope, crawling) =
            super::lift_hoist::hoisted_envelope(envelope, load.hoist_penalty);
        let walking_only = crippled || crawling;
        let boosters = if walking_only {
            0
        } else {
            u8::from(self.masc_active()) + u8::from(self.supercharger_active())
        };
        let mut envelope = match gravity.filter(|gravity| *gravity != 100) {
            None => envelope.with_boosters(boosters).scaled(load.factor),
            Some(gravity) => {
                super::speed_bonus::under_gravity(envelope.scaled(load.factor), gravity)
                    .with_boosters(boosters)
            }
        };
        if walking_only {
            envelope.running = envelope.walking;
        }
        envelope
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
        let envelope = envelope.with_boosters(boosters);
        SpeedEnvelope {
            walking: super::speed_bonus::saved_limit(envelope.walking),
            running: super::speed_bonus::saved_limit(envelope.running),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Hot triple-strength myomer adds two walking MP to an intact 'Mech, but one that has
    /// lost a leg walks at 1 MP regardless and cannot run (Total Warfare p. 122).
    #[test]
    fn a_lost_leg_overrides_hot_myomer() {
        let template = super::super::MechTemplate::parse(
            "CN9-D3",
            include_str!("../../game/units/CN9-D3.toml"),
        )
        .unwrap();
        let mut unit = Mech::from_template(template).unwrap();
        unit.heat.excess = 9.0;
        assert!(unit.triple_myomer_active());
        let mp = |unit: &Mech| {
            let envelope =
                unit.live_envelope(unit.mobility().envelope(), 0.0, LoadEffect::NONE, None);
            (
                envelope.walking_mp(),
                envelope.running / super::super::MP_KPH,
            )
        };
        let walking = unit.mobility().envelope().walking_mp();
        assert_eq!(mp(&unit).0, walking + 2.0);
        unit.sections
            .get_mut(&super::super::MechSection::LeftLeg)
            .unwrap()
            .internal = 0;
        assert!(unit.leg_loss_fixes_walking());
        assert_eq!(mp(&unit), (1.0, 1.0));
    }
}
