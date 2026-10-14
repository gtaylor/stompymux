//! Conventional heat accounting at the committed one-second simulation boundary.
use super::{Mech, Notice, Power, System};
use crate::{Flag, World};
use anyhow::{Context, Result, bail, ensure};
use serde::{Deserialize, Serialize};

/// Weapon heat and the last sampled excess.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct Heat {
    pub stored: f64,
    pub excess: f64,
}

impl Heat {
    /// Stored and excess heat are both finite and nonnegative.
    pub(crate) fn validate(self) -> Result<()> {
        ensure!(
            self.stored.is_finite()
                && self.stored >= 0.0
                && self.excess.is_finite()
                && self.excess >= 0.0,
            "Invalid unit heat"
        );
        Ok(())
    }

    /// Conventional weapon accuracy penalty at the most recent heat sample.
    pub fn to_hit_modifier(self) -> u8 {
        match self.excess {
            heat if heat >= 24.0 => 4,
            heat if heat >= 17.0 => 3,
            heat if heat >= 13.0 => 2,
            heat if heat >= 8.0 => 1,
            _ => 0,
        }
    }

    /// Scale the throttle by remaining walking MP, then round running MP upward.
    /// Engine/actuator limits remain the basis for acceleration and throttle validation.
    pub(crate) fn speed_multiplier(self, maximum: f64) -> f64 {
        if maximum <= 0.0 {
            return 0.0;
        }
        if self.excess < 5.0 {
            return 1.0;
        }
        let penalty = (self.excess / 5.0).floor().min(5.0);
        let walking_mp = (maximum / 1.5 / 10.75).round_ties_even();
        let running_mp = ((walking_mp - penalty) * 1.5).ceil().max(0.0);
        running_mp * 10.75 / maximum
    }

    /// Sample excess before reducing stored heat by one thirtieth of net cooling.
    fn advance(&mut self, rates: HeatRates) {
        self.excess = (self.stored + rates.production - rates.dissipation).max(0.0);
        self.stored = (self.stored - (rates.dissipation - rates.production) / 30.0).max(0.0);
    }
}

/// Continuous production and single-sink cooling, including the current map environment.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct HeatRates {
    pub production: f64,
    pub dissipation: f64,
}

impl Mech {
    /// Persisted thermal state; firing adds stored heat and the heartbeat samples excess.
    pub fn heat(&self) -> Heat {
        self.heat
    }

    /// Apply a named thermal edit without advancing cooling, heat hazards or random state.
    pub(super) fn set_thermal_field(&mut self, field: &str, value: &str) -> Result<()> {
        let value = f64::from(
            value
                .trim()
                .parse::<f32>()
                .context("Expected a finite thermal value")?,
        );
        ensure!(value.is_finite(), "Expected a finite thermal value");
        match field {
            "heat" => self.heat_sample.production = value,
            "dissheat" => self.heat_sample.dissipation = value,
            "overheat" => {
                let mut heat = self.heat;
                heat.excess = value;
                heat.validate()?;
                self.heat = heat;
            }
            _ => bail!("Unknown thermal field"),
        }
        Ok(())
    }

    /// Last committed thermal sample, including stored weapon heat in production.
    pub fn sampled_heat_rates(&self) -> HeatRates {
        self.heat_sample
    }

    /// Add heat from an outside source such as inferno missiles or a burning hex left behind
    /// (Total Warfare p. 159, Outside Heat Sources). Outside heat is cumulative to at most
    /// fifteen points a turn; the points that fit are added to the Heat Scale and returned.
    pub(super) fn absorb_outside_heat(&mut self, points: u8) -> u8 {
        let applied = self.overheat_clock.absorb_outside_heat(points);
        self.heat.stored += f64::from(applied);
        applied
    }

    /// Derive rates from surviving equipment, motion and the occupied map tile.
    pub fn heat_rates(&self, world: &World) -> HeatRates {
        self.heat_rates_with_disabled(world, self.disabled_cooling())
    }

    /// Heat points the shut-off heat sinks would otherwise dissipate this turn.
    fn disabled_cooling(&self) -> u16 {
        self.heat_cutoff
            .disabled_points(self.heat_sink_points_each(), self.cooling_capacity())
    }

    /// Surviving physical cooling before intentionally disabling any heat points.
    pub fn cooling_capacity(&self) -> u16 {
        self.reconstructed_cooling
            .unwrap_or(self.definition().heat_sinks)
            .saturating_sub(u16::from(self.system_hits(System::HeatSink)))
    }

    /// Derive environment cooling from the capacity left after the shut-off sinks.
    fn heat_rates_with_disabled(&self, world: &World, disabled: u16) -> HeatRates {
        let mut production = 10.0
            * f64::from(u8::from(self.stealth().enabled) + u8::from(self.null_signature().enabled));
        if self.power() == Power::Running {
            production += 5.0 * f64::from(self.system_hits(System::Engine));
            // Jumping heat is charged once per jump at takeoff (Total Warfare p. 53), not
            // as a rate while airborne.
            if let Some(motion) = self.motion().filter(|motion| motion.speed != 0.0) {
                production +=
                    if motion.desired_speed > self.update_maximum_speed() * 2.0 / 3.0 + 0.1 {
                        2.0
                    } else {
                        1.0
                    };
            }
        }
        let mut rates = HeatRates {
            production,
            dissipation: f64::from(self.cooling_capacity().saturating_sub(disabled)),
        };
        let Some(position) = self.position() else {
            return rates;
        };
        let Some(map) = world.btech.maps().get(&position.map) else {
            return rates;
        };
        let tile = map
            .hex(i64::from(position.x), i64::from(position.y))
            .expect("validated placed unit terrain");
        let elevation = self.elevation_level(tile);
        // Fire heat arrives once a turn through `absorb_outside_heat` (see `advance_heat`);
        // magma heat comes from the hex itself and no cap limits it.
        rates.production += f64::from(tile.ground_heat(elevation));
        if tile.immerses(elevation) {
            // Heat sinks under water dissipate twice as much, up to six extra points a turn
            // (Total Warfare p. 159). Wading puts only the leg sinks under; deeper water, or
            // lying in it, puts them all under.
            let wading = elevation == i32::from(tile.water_line()) - 1;
            let points_each = if self.definition().has_double_heat_sinks() {
                2.0
            } else {
                1.0
            };
            let submerged_points = if wading && self.posture() != super::Posture::Prone {
                self.leg_heat_sinks() as f64 * points_each
            } else {
                rates.dissipation
            };
            rates.dissipation += submerged_points.clamp(0.0, 6.0);
        }
        if tile.chills(elevation) && self.leg_heat_sinks() > 0 {
            rates.dissipation += 1.0;
        }
        rates.dissipation -= f64::from(super::extreme_temperature_steps(map.temperature()));
        rates
    }

    /// Working heat sinks mounted in the legs, which water and snow around the feet cool.
    fn leg_heat_sinks(&self) -> usize {
        self.loadout()
            .expect("validated unit loadout")
            .systems
            .iter()
            .filter(|part| {
                part.system == System::HeatSink
                    && self.chassis().is_leg(part.location.section)
                    && !self.critical_unavailable(part.location)
            })
            .count()
    }

    /// Whether the Mech stands in the flames of a burning hex and so absorbs five heat points
    /// at the turn boundary (Tactical Operations: Advanced Rules p. 42, Effects of Fire).
    pub fn in_flames(&self, world: &World) -> bool {
        let Some(position) = self.position() else {
            return false;
        };
        let Some(map) = world.btech.maps().get(&position.map) else {
            return false;
        };
        let tile = map
            .hex(i64::from(position.x), i64::from(position.y))
            .expect("validated placed unit terrain");
        tile.is_burning() && self.reaches_flames(tile, self.elevation_level(tile))
    }

    /// Whether a Mech at `elevation` in a burning hex stands in the flames, which burn on the
    /// hex's topmost surface: water, a bridge deck, a roof or the ground. A Mech under the
    /// water, beneath a bridge deck or flying above the flames is clear of them.
    fn reaches_flames(&self, tile: super::Hex, elevation: i32) -> bool {
        let surface = i32::from(tile.top_height());
        let height = if self.posture() == super::Posture::Prone {
            1
        } else {
            2
        };
        elevation <= surface && elevation + height > surface
    }

    /// Predict a committed thermal sample without mutating inspection state or consuming dice.
    /// The returned cutoff is the setting after this second: a pending sink request takes
    /// effect when the unit's turn clock wraps. Idle units (reactor off, no heat, no request,
    /// not in flames) have no sample and their turn clock stands still.
    fn thermal_sample(&self, world: &World) -> (super::HeatCutoff, Option<HeatRates>) {
        let mut cutoff = self.heat_cutoff;
        cutoff.clamp(self.working_heat_sinks());
        if self.power() != Power::Running
            && self.heat == Heat::default()
            && !cutoff.pending()
            && !self.in_flames(world)
        {
            return (cutoff, None);
        }
        let rates = self.heat_rates_with_disabled(world, self.disabled_cooling());
        if self.overheat_clock.phase == 29 {
            cutoff.apply();
        }
        (cutoff, Some(rates))
    }

    /// Include idle transitions, pending sink requests and fire exposure in heartbeat admission.
    pub fn heat_active(&self, world: &World) -> bool {
        let (cutoff, rates) = self.thermal_sample(world);
        let Some(rates) = rates else {
            return cutoff != self.heat_cutoff;
        };
        if cutoff != self.heat_cutoff || cutoff.pending() || self.in_flames(world) {
            return true;
        }
        let mut next = self.heat;
        next.advance(rates);
        next != self.heat
    }
}

/// Update thermal accounting, including cooling after shutdown, in the caller's world transaction.
/// The same transaction must call advance_overheat after this sample to apply due thermal hazards.
///
/// At each unit's turn boundary a pending heat-sink request takes effect (Total Warfare
/// p. 159) and a Mech standing in flames absorbs five points of outside heat, counted against
/// the fifteen-point outside-heat cap like infernos and heat-causing weapons (Total Warfare
/// p. 159; Tactical Operations: Advanced Rules p. 42).
pub fn advance_heat(world: &mut World) -> Vec<Notice> {
    let ids: Vec<_> = world
        .btech
        .constructed_units()
        .iter()
        .filter(|(id, unit)| {
            (unit.heat_active(world) || unit.overheat_active())
                && world
                    .objects
                    .get(id)
                    .is_some_and(|object| !object.flags.contains(Flag::Going))
        })
        .map(|(&id, unit)| (id, unit.thermal_sample(world), unit.in_flames(world)))
        .collect();
    let mut notices = Vec::new();
    for (id, (cutoff, rates), in_flames) in ids {
        let unit = world.btech.constructed.get_mut(&id).unwrap();
        let Some(rates) = rates else {
            unit.heat_cutoff = cutoff;
            continue;
        };
        let previous = indicator(unit.heat.excess);
        unit.heat_sample = HeatRates {
            production: unit.heat.stored + rates.production,
            dissipation: rates.dissipation,
        };
        unit.heat.advance(rates);
        let turn_boundary = !unit.is_destroyed() && {
            unit.overheat_clock.tick();
            unit.overheat_clock.phase == 0
        };
        if cutoff.shut_off != unit.heat_cutoff.shut_off {
            notices.push(Notice {
                unit: id,
                text: if cutoff.shut_off == 0 {
                    "[fg=green]All heat sinks are back on.[reset]".to_owned()
                } else {
                    format!(
                        "[fg=yellow]{} heat sinks are now shut off.[reset]",
                        cutoff.shut_off
                    )
                },
            });
        }
        unit.heat_cutoff = cutoff;
        if in_flames && turn_boundary && unit.absorb_outside_heat(5) > 0 {
            notices.push(Notice {
                unit: id,
                text: "The flames around you heat your 'Mech.".to_owned(),
            });
        }
        let current = indicator(unit.heat.excess);
        if previous != current {
            let text = match current {
                2 => "Your Excess Heat indicator turns RED!",
                1 => "Your Excess Heat indicator turns YELLOW",
                _ => "Your Excess Heat indicator turns GREEN",
            };
            notices.push(Notice {
                unit: id,
                text: text.to_owned(),
            });
        }
    }
    notices
}

/// Indicator bands are distinct from weapon accuracy thresholds.
fn indicator(excess: f64) -> u8 {
    if excess >= 19.0 {
        return 2;
    }
    if excess >= 14.0 {
        return 1;
    }
    0
}

impl super::Vehicle {
    /// Surviving vehicle cooling capacity, independent of weapon heat eligibility.
    pub fn cooling_capacity(&self) -> Result<u16> {
        let lost = self
            .loadout()?
            .systems
            .iter()
            .filter(|part| {
                part.system == System::HeatSink && self.critical_destroyed(part.location)
            })
            .count() as u16;
        Ok(self.definition().heat_sink_capacity().saturating_sub(lost))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn heat_speed_uses_walking_mp_bands_and_never_reverses_a_throttle() {
        for (excess, expected) in [
            (4.999, 118.25),
            (5.0, 96.75),
            (9.999, 96.75),
            (10.0, 86.0),
            (15.0, 64.5),
            (20.0, 53.75),
            (25.0, 32.25),
            (100.0, 32.25),
        ] {
            let heat = Heat {
                stored: 0.0,
                excess,
            };
            assert!((118.25 * heat.speed_multiplier(118.25) - expected).abs() < 1e-10);
        }
        let heat = Heat {
            stored: 0.0,
            excess: 25.0,
        };
        assert_eq!(heat.speed_multiplier(64.5), 0.0);
        assert_eq!(heat.speed_multiplier(10.75), 0.0);
        assert_eq!(heat.speed_multiplier(0.0), 0.0);
        let heat = Heat {
            stored: 0.0,
            excess: 5.0,
        };
        // A hip-damaged Jenner derives its MP from the reduced engine limit.
        assert!((59.125 * heat.speed_multiplier(59.125) - 53.75).abs() < 1e-10);
    }

    #[test]
    fn thermal_sample_precedes_fractional_cooling() {
        let mut heat = Heat {
            stored: 30.0,
            excess: 0.0,
        };
        heat.advance(HeatRates {
            production: 2.0,
            dissipation: 10.0,
        });
        assert_eq!(heat.excess, 22.0);
        assert_eq!(heat.stored, 30.0 - 8.0 / 30.0);
        assert_eq!(heat.to_hit_modifier(), 3);
        heat.advance(HeatRates {
            production: 10.0,
            dissipation: 2.0,
        });
        assert_eq!(heat.stored, 30.0);
    }

    #[test]
    fn cooling_stops_at_zero_and_corrupt_heat_is_rejected() {
        let mut heat = Heat {
            stored: 0.1,
            excess: 3.0,
        };
        heat.advance(HeatRates {
            production: 0.0,
            dissipation: 10.0,
        });
        assert_eq!(heat, Heat::default());
        for stored in [f64::NEG_INFINITY, f64::NAN, f64::INFINITY, -1.0] {
            assert!(
                Heat {
                    stored,
                    excess: 0.0
                }
                .validate()
                .is_err()
            );
        }
        for (excess, modifier) in [
            (0.0, 0),
            (7.9, 0),
            (8.0, 1),
            (13.0, 2),
            (17.0, 3),
            (24.0, 4),
        ] {
            assert_eq!(
                Heat {
                    stored: 0.0,
                    excess
                }
                .to_hit_modifier(),
                modifier
            );
        }
    }
}
