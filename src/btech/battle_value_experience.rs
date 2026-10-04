//! Battle-value gunnery experience arithmetic and atomic skill awards without random gates.
use super::{GunneryAwardRequest, GunneryExperienceMode, Weapon};
use crate::{World, config::XpConfig};
use anyhow::{Result, ensure};
use serde::Serialize;

/// Pre-damage facts used by the battle-value formula; speeds are nominal maxima, not load-adjusted.
#[derive(Debug, Clone, Copy)]
pub struct BattleValueExperienceInput {
    pub attacker_value: f64,
    pub target_value: f64,
    pub attacker_speed: f64,
    pub target_speed: f64,
    pub attacker_pilot_modifier: f64,
    pub target_pilot_modifier: f64,
    pub weapon: Weapon,
    pub damage: u16,
    pub base_to_hit: i32,
    pub unit_modifier: f64,
}

/// Deterministic award calculation; undefined square-root arithmetic receives the minimum award.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct BattleValueExperienceCalculation {
    /// None explicitly records an out-of-domain pilot adjustment or nonfinite arithmetic.
    pub difficulty: Option<f64>,
    pub amount: u32,
}

/// Skill targets are lower for better pilots; values above seven retain their distinct slope.
pub fn pilot_battle_value_modifier(gunnery: i16, piloting: i16) -> f64 {
    let base = match gunnery {
        ..=-1 => 2.05 - f64::from(gunnery) * 0.20,
        0 => 2.05,
        1 => 1.85,
        2 => 1.65,
        3 => 1.45,
        4 => 1.25,
        5 => 1.15,
        6 => 1.05,
        7 => 0.95,
        _ => 0.95 - f64::from(gunnery) * 0.10,
    };
    base - f64::from(piloting) * 0.05
}

impl BattleValueExperienceInput {
    /// Apply configured multipliers literally, followed by the damage percentage and award cap.
    /// Zero multipliers still receive one XP; impossible and optionally trivial hits receive none.
    pub fn calculate(self, config: &XpConfig) -> Result<Option<BattleValueExperienceCalculation>> {
        self.calculate_with_settings(config, &super::WeaponSettings::default())
    }

    /// Apply the same formula with the current runtime catalogue overrides.
    pub fn calculate_with_settings(
        self,
        config: &XpConfig,
        settings: &super::WeaponSettings,
    ) -> Result<Option<BattleValueExperienceCalculation>> {
        if self.damage == 0 || self.base_to_hit > 12 || (config.bthmod != 0 && self.base_to_hit < 3)
        {
            return Ok(None);
        }
        ensure!(
            config.defaultweapdam > 0
                && config.defaultweapbv >= 0
                && config.modifier >= 0
                && config.missilemod >= 0
                && config.ammomod >= 0
                && (1..=i64::from(u32::MAX)).contains(&config.xpgain_cap),
            "Invalid battle-value XP configuration"
        );
        for value in [
            self.attacker_value,
            self.target_value,
            self.attacker_speed,
            self.target_speed,
            self.unit_modifier,
        ] {
            ensure!(
                value.is_finite() && value >= 0.0,
                "Invalid battle-value XP input"
            );
        }
        ensure!(
            self.attacker_pilot_modifier.is_finite() && self.target_pilot_modifier.is_finite(),
            "Invalid pilot BV modifier"
        );
        let (source, target) = if config.use_pilot_bv_mod != 0 {
            (
                self.attacker_value * self.attacker_pilot_modifier,
                self.target_value * self.target_pilot_modifier,
            )
        } else {
            (self.attacker_value, self.target_value)
        };
        let mut multiplier = config.modifier as f64;
        if config.bthmod != 0 {
            let failures = (1..=6)
                .flat_map(|first| (1..=6).map(move |second| first + second))
                .filter(|sum| *sum < self.base_to_hit)
                .count();
            multiplier = 2.0 * multiplier * failures as f64 / 36.0;
        }
        let weapon_modifier = match self.weapon.gunnery_skill(true) {
            "Gunnery-Missile" => config.missilemod as f64,
            "Gunnery-Ballistic" => config.ammomod as f64,
            _ => 1.0,
        };
        let recycle = f64::from(settings.recycle_seconds(self.weapon));
        let recycle_modifier = if config.vrtmod != 0 && recycle < 30.0 {
            (recycle / 30.0).sqrt()
        } else {
            1.0
        };
        let source_speed = (self.attacker_speed as f32 / 10.75).trunc() as f64 + 1.0;
        let target_speed = (self.target_speed as f32 / 10.75).trunc() as f64 + 1.0;
        let damage_divisor = if config.defaultweapdam > 1 {
            f64::from(self.damage)
        } else {
            1.0
        };
        let numerator = ((target + 1.0) * target_speed * config.defaultweapbv as f64
            / config.defaultweapdam as f64)
            .sqrt();
        let denominator =
            ((source + 1.0) * source_speed * f64::from(settings.battle_value(self.weapon))
                / damage_divisor)
                .sqrt();
        let mut difficulty =
            recycle_modifier * weapon_modifier * multiplier * numerator / denominator;
        if config.perunit_xpmod != 0 {
            difficulty *= self.unit_modifier;
        }
        let raw = difficulty * f64::from(self.damage) / 100.0;
        // Floating-to-integer conversion outside its domain is undefined in C. Define the
        // minimum award explicitly instead of poisoning reports or failing the whole shot.
        let difficulty = (difficulty.is_finite() && raw.is_finite()).then_some(difficulty);
        let amount = difficulty.map_or(1, |_| {
            (raw.max(0.0) as u32).clamp(1, config.xpgain_cap as u32)
        });
        Ok(Some(BattleValueExperienceCalculation {
            difficulty,
            amount,
        }))
    }
}

/// One deterministic battle-value award, including ordinary character skill rate limits.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct BattleValueExperienceAward {
    pub calculation: BattleValueExperienceCalculation,
    pub amount: u32,
    pub skill: &'static str,
    pub award: super::ExperienceAward,
}

/// Derive current pre-impact facts and apply XP atomically, consuming no dice.
pub fn award_battle_value_gunnery_experience(
    world: &mut World,
    request: GunneryAwardRequest,
    config: &XpConfig,
) -> Result<Option<BattleValueExperienceAward>> {
    if request.damage == 0
        || !super::gunnery_experience_eligible(
            world,
            request.attacker,
            request.pilot,
            request.target,
            request.base_to_hit,
            GunneryExperienceMode::BattleValue {
                difficulty_modifier: config.bthmod != 0,
            },
        )
    {
        return Ok(None);
    }
    let value = |id| -> Result<_> {
        let battle_value = super::battle_value::configured(world, id, request.speed_policy())?;
        crate::btech::with_unit!(world.btech.unit(id).unwrap(), |unit| {
            Ok((
                battle_value,
                unit.definition().max_speed,
                unit.experience_settings(),
            ))
        })
    };
    let (source_value, source_speed, source_settings) = value(request.attacker)?;
    let (target_value, target_speed, _) = value(request.target)?;
    let pilot_modifier = |id| -> Result<f64> {
        if config.use_pilot_bv_mod == 0 {
            return Ok(1.0);
        }
        Ok(pilot_battle_value_modifier(
            super::skills::unit_weapon_gunnery_target(
                world,
                id,
                request.weapon,
                request.extended_gunnery,
            )?,
            super::unit_piloting_target(world, id, request.extended_piloting)?,
        ))
    };
    let calculation = BattleValueExperienceInput {
        attacker_value: source_value.total,
        target_value: target_value.total,
        attacker_speed: source_speed,
        target_speed,
        attacker_pilot_modifier: pilot_modifier(request.attacker)?,
        target_pilot_modifier: pilot_modifier(request.target)?,
        weapon: request.weapon,
        damage: request.damage,
        base_to_hit: request.base_to_hit,
        unit_modifier: source_settings.multiplier,
    }
    .calculate_with_settings(config, world.btech.weapon_settings())?
    .expect("Eligible nonzero damage has battle-value difficulty");
    let skill = super::gunnery_experience::award_skill(world, request);
    world.attempt(|world| {
        let award = super::award_skill_experience(
            world,
            request.pilot,
            skill,
            calculation.amount,
            request.now,
            false,
        )?;
        Ok(Some(BattleValueExperienceAward {
            calculation,
            amount: calculation.amount,
            skill,
            award,
        }))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Equal units firing a medium laser isolate catalogue and damage scaling.
    fn input() -> BattleValueExperienceInput {
        BattleValueExperienceInput {
            attacker_value: 700.0,
            target_value: 700.0,
            attacker_speed: 64.5,
            target_speed: 64.5,
            attacker_pilot_modifier: 1.0,
            target_pilot_modifier: 1.0,
            weapon: Weapon::MediumLaser,
            damage: 5,
            base_to_hit: 7,
            unit_modifier: 1.0,
        }
    }

    /// Formula switches and nominal speed truncation have independent, observable effects.
    #[test]
    fn battle_value_formula_configuration() {
        let base = XpConfig::default();
        let calculate = |input: BattleValueExperienceInput, config: XpConfig| {
            input.calculate(&config).unwrap().unwrap()
        };
        assert_eq!(calculate(input(), base.clone()).amount, 8);
        assert_eq!(
            calculate(
                input(),
                XpConfig {
                    bthmod: 200,
                    ..base.clone()
                }
            )
            .amount,
            6
        );
        assert_eq!(
            calculate(
                input(),
                XpConfig {
                    vrtmod: 1,
                    ..base.clone()
                }
            )
            .amount,
            6
        );
        assert_eq!(
            calculate(
                input(),
                XpConfig {
                    bthmod: 1,
                    vrtmod: 1,
                    ..base.clone()
                }
            )
            .amount,
            5
        );
        assert_eq!(
            calculate(
                BattleValueExperienceInput {
                    target_speed: 64.49,
                    ..input()
                },
                base.clone()
            )
            .amount,
            7
        );
        assert_eq!(
            calculate(
                BattleValueExperienceInput {
                    unit_modifier: 2.0,
                    ..input()
                },
                XpConfig {
                    xpgain_cap: 50,
                    ..base.clone()
                }
            )
            .amount,
            16
        );
        assert_eq!(
            calculate(
                BattleValueExperienceInput {
                    unit_modifier: 0.0,
                    ..input()
                },
                base.clone()
            )
            .amount,
            1
        );
        assert_eq!(
            calculate(
                BattleValueExperienceInput {
                    unit_modifier: 0.0,
                    ..input()
                },
                XpConfig {
                    perunit_xpmod: 0,
                    ..base.clone()
                }
            )
            .amount,
            8
        );
        let missile = BattleValueExperienceInput {
            weapon: Weapon::Srm4,
            damage: 2,
            ..input()
        };
        assert_eq!(
            calculate(
                missile,
                XpConfig {
                    missilemod: 1,
                    ..base.clone()
                }
            )
            .amount,
            2
        );
        assert_eq!(calculate(missile, base.clone()).amount, 10);
        assert_eq!(
            calculate(
                input(),
                XpConfig {
                    defaultweapdam: 1,
                    ..base.clone()
                }
            )
            .amount,
            8
        );
        assert_eq!(
            calculate(
                input(),
                XpConfig {
                    modifier: 0,
                    ..base
                }
            )
            .amount,
            1
        );
    }

    /// Trivial hits are allowed only when the difficulty option is disabled; invalid settings reject safely.
    #[test]
    fn battle_value_formula_gates_and_invalid_inputs() {
        for bth in [-2, 2] {
            let input = BattleValueExperienceInput {
                base_to_hit: bth,
                ..input()
            };
            assert!(input.calculate(&XpConfig::default()).unwrap().is_some());
            assert!(
                input
                    .calculate(&XpConfig {
                        bthmod: 1,
                        ..Default::default()
                    })
                    .unwrap()
                    .is_none()
            );
        }
        for sample in [
            BattleValueExperienceInput {
                damage: 0,
                ..input()
            },
            BattleValueExperienceInput {
                base_to_hit: 13,
                ..input()
            },
        ] {
            assert!(sample.calculate(&XpConfig::default()).unwrap().is_none());
        }
        for config in [
            XpConfig {
                defaultweapdam: 0,
                ..Default::default()
            },
            XpConfig {
                xpgain_cap: 0,
                ..Default::default()
            },
            XpConfig {
                missilemod: -1,
                ..Default::default()
            },
        ] {
            assert!(input().calculate(&config).is_err());
        }
        assert!(
            BattleValueExperienceInput {
                target_value: f64::NAN,
                ..input()
            }
            .calculate(&XpConfig::default())
            .is_err()
        );
    }

    /// Poor-skill extrapolation can leave the square-root domain; record that case and award the minimum.
    #[test]
    fn pilot_adjustment_and_defined_minimum() {
        for (gun, pilot, expected) in [
            (-1, 0, 2.25),
            (0, 0, 2.05),
            (4, 5, 1.0),
            (6, 6, 0.75),
            (7, 0, 0.95),
            (8, 6, -0.15),
        ] {
            assert!((pilot_battle_value_modifier(gun, pilot) - expected).abs() < 1e-10);
        }
        let bad = BattleValueExperienceInput {
            attacker_pilot_modifier: -1.0,
            ..input()
        };
        let result = bad.calculate(&XpConfig::default()).unwrap().unwrap();
        assert_eq!(
            result,
            BattleValueExperienceCalculation {
                difficulty: None,
                amount: 1
            }
        );
        assert!(
            bad.calculate(&XpConfig {
                use_pilot_bv_mod: 0,
                ..Default::default()
            })
            .unwrap()
            .unwrap()
            .difficulty
            .is_some()
        );
    }
}
