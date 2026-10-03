//! Passive Triple Strength Myomer installation and distinct runtime speed calculations.
use super::{BattleTemplate, BattleUnit};

/// One extra walking MP, rounded back to whole running MP with ties-to-even walking conversion.
fn boost(maximum: f64) -> f64 {
    if maximum <= 0.0 {
        return 0.0;
    }
    (((maximum / 1.5 / 10.75).round_ties_even() + 1.0) * 1.5).ceil() * 10.75
}

impl BattleTemplate {
    /// Installed slots determine TSM technology; loss or flooding does not remove passive myomer.
    pub fn has_triple_myomer(&self) -> bool {
        self.sections
            .values()
            .flat_map(|section| section.criticals.values())
            .filter(|part| part.equipment.eq_ignore_ascii_case("TripleStrengthMyomer"))
            .take(6)
            .count()
            >= 6
    }
}

impl BattleUnit {
    /// TSM activates at the last sampled excess heat of nine, independent of stored weapon heat.
    pub fn triple_myomer_active(&self) -> bool {
        self.definition().has_triple_myomer() && self.heat().excess >= 9.0
    }

    /// Current throttle ceiling; mechanical mobility remains a separate damage-only quantity.
    pub fn movement_maximum_speed(&self) -> f64 {
        self.movement_maximum_at(self.mobility().maximum_speed)
    }

    /// Apply movement equipment to an externally loaded base ceiling.
    pub(super) fn movement_maximum_at(&self, base: f64) -> f64 {
        let maximum = base * self.booster_multiplier();
        if self.triple_myomer_active() {
            boost(maximum)
        } else {
            maximum
        }
    }

    /// Saved controls can retain an equipment or gravity ceiling after those inputs change.
    pub(super) fn motion_speed_limit(&self, maximum: f64) -> f64 {
        let triple_myomer = self.definition().has_triple_myomer();
        let retained = super::speed_bonus::saved_limit(
            maximum,
            self.masc_installed().unwrap_or(false),
            self.supercharger_installed(),
            triple_myomer,
        );
        let envelope = self.booster_speed_envelope();
        if !triple_myomer {
            return retained.max(maximum * envelope);
        }
        // TSM rounding can occur before, between or after the two independent toggles.
        let bound = boost(maximum * envelope).max(boost(maximum) * envelope);
        if envelope > 1.0 {
            return retained
                .max(bound.max(boost(maximum * (4.0 / 3.0)) * (envelope / (4.0 / 3.0))));
        }
        retained.max(bound)
    }

    /// Speed and movement-heat updates apply their additional walking-MP conversion to the effective ceiling.
    pub(super) fn update_maximum_speed(&self) -> f64 {
        self.update_maximum_at(self.mobility().maximum_speed)
    }

    /// Movement-update conversion shared by unloaded and towing units.
    pub(super) fn update_maximum_at(&self, base: f64) -> f64 {
        self.update_from_maximum(self.movement_maximum_at(base))
    }

    /// Movement-update conversion after the world has applied load, equipment and gravity.
    pub(super) fn update_from_maximum(&self, maximum: f64) -> f64 {
        // Hot TSM replaces the movement-update booster adjustment with its own conversion.
        if self.triple_myomer_active() {
            return boost(maximum);
        }
        if self.masc_active() && self.supercharger_active() {
            return ((maximum / 1.5).round_ties_even() / 10.75 * 2.5).ceil() * 10.75;
        }
        if self.masc_active() || self.supercharger_active() {
            return maximum * (4.0 / 3.0);
        }
        maximum
    }

    /// Turning keeps its own hot-myomer adjustment after the shared effective ceiling.
    pub(super) fn turning_from_maximum(&self, maximum: f64) -> f64 {
        if self.triple_myomer_active() && maximum > 0.0 {
            maximum + 1.5 * 10.75
        } else {
            maximum
        }
    }

    /// At nine heat TSM suppresses the first heat penalty; higher heat bands still apply normally.
    pub(super) fn movement_heat_multiplier(&self, maximum: f64) -> f64 {
        if self.triple_myomer_active() && self.heat().excess < 10.0 {
            return 1.0;
        }
        self.heat().speed_multiplier(maximum)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Control and update speed conversions retain their distinct integer rounding.
    #[test]
    fn myomer_speed_rounding_and_zero_mobility() {
        for (input, expected) in [
            (0.0, 0.0),
            (118.25, 129.0),
            (129.0, 150.5),
            (64.5, 86.0),
            (59.125, 86.0),
        ] {
            assert_eq!(boost(input), expected);
        }
    }
}
