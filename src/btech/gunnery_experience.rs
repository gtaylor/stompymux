//! Gunnery experience difficulty and award gates, independent of shot mutation and publication.
use anyhow::{Result, ensure};

/// Unit facts needed by the configured classic gunnery experience formula.
/// Speeds are cargo-adjusted maximum speeds in the same units as unit motion.
#[derive(Debug, Clone, Copy)]
pub struct GunneryExperienceInput {
    pub attacker_tons: u16,
    pub target_tons: u16,
    pub attacker_speed: f64,
    pub target_speed: f64,
    pub base_to_hit: i32,
    pub damage: u16,
    /// Use one when per-unit experience scaling is disabled.
    pub unit_modifier: f64,
}

/// Pre-damage award difficulty; the caller owns eligibility and the random stream.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize)]
pub struct GunneryExperienceChance {
    /// Percentage-like difficulty before multiplication by packet damage.
    pub difficulty: f64,
    pub damage: u16,
}

impl GunneryExperienceInput {
    /// Compute the classic formula selected by `oldxpsystem`, without consuming RNG.
    /// Impossible or nearly certain hits and zero damage do not request an award roll.
    pub fn classic_chance(self) -> Result<Option<GunneryExperienceChance>> {
        ensure!(
            self.attacker_tons > 0,
            "Gunnery XP requires positive attacker tonnage"
        );
        ensure!(
            self.attacker_speed.is_finite()
                && self.attacker_speed >= 0.0
                && self.target_speed.is_finite()
                && self.target_speed >= 0.0,
            "Gunnery XP speeds must be finite and nonnegative"
        );
        ensure!(
            self.unit_modifier.is_finite() && self.unit_modifier >= 0.0,
            "Gunnery XP modifier must be finite and nonnegative"
        );
        if self.damage == 0 || !(3..=12).contains(&self.base_to_hit) {
            return Ok(None);
        }
        // Tonnage scaling truncates before bounding; speed scaling retains fractional difficulty.
        let mass = (100 * u32::from(self.target_tons.max(1)) / u32::from(self.attacker_tons))
            .clamp(50, 150);
        let speed_ratio = f64::from(speed_weight(self.target_speed))
            / f64::from(speed_weight(self.attacker_speed));
        let failures = (1..=6)
            .flat_map(|first| (1..=6).map(move |second| first + second))
            .filter(|total| *total < self.base_to_hit)
            .count();
        let difficulty = f64::from(mass) * speed_ratio * speed_ratio * failures as f64 / 36.0
            * 2.0
            * self.unit_modifier;
        ensure!(
            difficulty.is_finite(),
            "Gunnery XP difficulty exceeds supported range"
        );
        Ok(Some(GunneryExperienceChance {
            difficulty,
            damage: self.damage,
        }))
    }
}

impl GunneryExperienceChance {
    /// Apply the inclusive 1..=50 gate and the classic hard cap, without mutating a character.
    pub fn award(self, roll: u8) -> Result<Option<u32>> {
        ensure!(
            (1..=50).contains(&roll),
            "Gunnery XP roll must be between 1 and 50"
        );
        ensure!(
            self.difficulty.is_finite() && self.difficulty >= 0.0,
            "Gunnery XP difficulty must be finite and nonnegative"
        );
        let weighted_damage = self.difficulty * f64::from(self.damage);
        ensure!(
            weighted_damage.is_finite(),
            "Gunnery XP weighted damage exceeds supported range"
        );
        if f64::from(roll) > weighted_damage {
            return Ok(None);
        }
        Ok(Some(
            (weighted_damage / 100.0).trunc().clamp(1.0, 50.0) as u32
        ))
    }
}

/// Classic speed bands count two through six, with inclusive upper boundaries.
fn speed_weight(speed: f64) -> u8 {
    [21.5, 43.0, 64.5, 96.75]
        .into_iter()
        .position(|limit| speed <= limit)
        .map_or(6, |index| index as u8 + 2)
}

/// Persisted unit scaling and target suppression for shooting experience.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct UnitExperience {
    pub multiplier: f64,
    /// Used by the battle-value formula; the classic formula does not consult this flag.
    pub suppress_gunnery: bool,
}

impl Default for UnitExperience {
    fn default() -> Self {
        Self {
            multiplier: 1.0,
            suppress_gunnery: false,
        }
    }
}

impl UnitExperience {
    /// Reject invalid saved or administratively supplied scaling before it reaches arithmetic.
    pub(super) fn validate(self) -> Result<()> {
        ensure!(
            self.multiplier.is_finite() && self.multiplier >= 0.0,
            "Unit XP multiplier must be finite and nonnegative"
        );
        Ok(())
    }
}

impl super::Mech {
    /// Current persisted shooting-XP policy for this unit.
    pub fn experience_settings(&self) -> UnitExperience {
        self.experience
    }
}

impl super::Vehicle {
    /// Current persisted shooting-XP policy, shared with Mechs.
    pub fn experience_settings(&self) -> UnitExperience {
        self.experience
    }
}

/// Chassis-independent pre-impact eligibility and scaling facts.
struct ExperienceUnit {
    pilot: Option<crate::ObjectId>,
    team: i32,
    destroyed: bool,
    tons: u16,
    settings: UnitExperience,
}

/// Read shared award inputs without coupling either construction layout to the formula.
fn experience_unit(world: &crate::World, id: crate::ObjectId) -> Option<ExperienceUnit> {
    let unit = world.btech.unit(id)?;
    Some(super::with_unit!(unit, |unit| ExperienceUnit {
        pilot: unit.pilot(),
        team: unit.signature().team,
        destroyed: unit.is_destroyed(),
        tons: unit.definition().tons,
        settings: unit.experience_settings(),
    }))
}

/// Set trusted administrative XP policy without changing character balances or RNG.
pub fn set_unit_experience(
    world: &mut crate::World,
    id: crate::ObjectId,
    settings: UnitExperience,
) -> Result<()> {
    use anyhow::Context;
    settings.validate()?;
    super::with_unit_mut!(
        world
            .btech
            .unit_mut(id)
            .context("Unit construction state is unavailable")?,
        |unit| {
            unit.experience = settings;
            Ok(())
        }
    )
}

/// Formula selection affects sure-hit and target-suppression eligibility.
#[derive(Debug, Clone, Copy)]
pub enum GunneryExperienceMode {
    Classic,
    BattleValue { difficulty_modifier: bool },
}

/// Test pre-impact gunnery eligibility without consuming dice or modifying character data.
/// Firing admission remains responsible for weapon, range and cockpit authorization.
pub fn gunnery_experience_eligible(
    world: &crate::World,
    attacker: crate::ObjectId,
    pilot: crate::ObjectId,
    target: crate::ObjectId,
    base_to_hit: i32,
    mode: GunneryExperienceMode,
) -> bool {
    use crate::Flag;
    if attacker == target || base_to_hit > 12 {
        return false;
    }
    let needs_difficulty = matches!(
        mode,
        GunneryExperienceMode::Classic
            | GunneryExperienceMode::BattleValue {
                difficulty_modifier: true
            }
    );
    if needs_difficulty && base_to_hit < 3 {
        return false;
    }
    if [attacker, target].into_iter().any(|id| {
        world.objects.get(&id).is_none_or(|object| {
            object.flags.contains(Flag::Going) || !object.flags.contains(Flag::InCharacter)
        })
    }) {
        return false;
    }
    let Some(source) = experience_unit(world, attacker) else {
        return false;
    };
    let Some(victim) = experience_unit(world, target) else {
        return false;
    };
    if victim.destroyed || source.team == victim.team {
        return false;
    }
    if matches!(mode, GunneryExperienceMode::BattleValue { .. }) && victim.settings.suppress_gunnery
    {
        return false;
    }
    world.objects.get(&pilot).is_some_and(|object| {
        object.location == Some(attacker)
            && source.pilot == Some(pilot)
            && object.flags.contains(Flag::Connected)
            && !object.flags.contains(Flag::Going)
    })
}

/// Pre-impact shooting facts; unit speed, mass and environment are derived from the world.
#[derive(Debug, Clone, Copy)]
pub struct GunneryAwardRequest {
    /// Apply configured hot-myomer assistance to both participants' external towing load.
    pub tsm_tow_bonus: bool,
    pub attacker: crate::ObjectId,
    pub pilot: crate::ObjectId,
    pub target: crate::ObjectId,
    pub weapon: super::Weapon,
    pub damage: u16,
    pub base_to_hit: i32,
    pub extended_gunnery: bool,
    pub extended_piloting: bool,
    pub use_unit_modifier: bool,
    pub now: i64,
}

impl GunneryAwardRequest {
    /// Share the towing policy between classic difficulty and battle-value awards.
    pub(super) fn speed_policy(self) -> super::SpeedPolicy {
        super::SpeedPolicy {
            tsm_tow_bonus: self.tsm_tow_bonus,
        }
    }
}

/// One eligible classic XP attempt, including unsuccessful award gates and skill rate limits.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct GunneryExperienceAward {
    pub chance: GunneryExperienceChance,
    pub roll: u8,
    pub amount: Option<u32>,
    pub skill: &'static str,
    pub award: Option<super::ExperienceAward>,
}

/// Award classic gunnery XP atomically before a damage packet is applied.
/// Uses the attacker's persisted dice; ineligible hits consume neither dice nor character state.
/// The enclosing shot must roll this operation back if later damage or casualty publication fails.
pub fn award_classic_gunnery_experience(
    world: &mut crate::World,
    request: GunneryAwardRequest,
) -> Result<Option<GunneryExperienceAward>> {
    if request.damage == 0
        || !gunnery_experience_eligible(
            world,
            request.attacker,
            request.pilot,
            request.target,
            request.base_to_hit,
            GunneryExperienceMode::Classic,
        )
    {
        return Ok(None);
    }
    let attacker = experience_unit(world, request.attacker).expect("eligible attacker");
    let target = experience_unit(world, request.target).expect("eligible target");
    let chance = GunneryExperienceInput {
        attacker_tons: attacker.tons,
        target_tons: target.tons,
        attacker_speed: super::effective_speed::configured(
            world,
            request.attacker,
            request.speed_policy(),
        )?,
        target_speed: super::effective_speed::configured(
            world,
            request.target,
            request.speed_policy(),
        )?,
        base_to_hit: request.base_to_hit,
        damage: request.damage,
        unit_modifier: if request.use_unit_modifier {
            attacker.settings.multiplier
        } else {
            1.0
        },
    }
    .classic_chance()?
    .expect("Eligible nonzero damage has classic difficulty");
    let mut candidate = world.clone();
    let roll = super::dice::unit_dice_mut(&mut candidate, request.attacker)?.die(50)? as u8;
    let amount = chance.award(roll)?;
    let skill = award_skill(world, request);
    let award = amount
        .map(|amount| {
            super::award_skill_experience(
                &mut candidate,
                request.pilot,
                skill,
                amount,
                request.now,
                false,
            )
        })
        .transpose()?;
    *world = candidate;
    Ok(Some(GunneryExperienceAward {
        chance,
        roll,
        amount,
        skill,
        award,
    }))
}

/// Hit calculations and XP awards use the same chassis and weapon-family skill policy.
pub(super) fn award_skill(world: &crate::World, request: GunneryAwardRequest) -> &'static str {
    super::skills::unit_gunnery_skill(
        world,
        request.attacker,
        request.weapon,
        request.extended_gunnery,
    )
}

/// Formula-specific award evidence retained by a completed shot.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(tag = "formula", rename_all = "snake_case")]
pub enum ShotExperienceAward {
    Classic(GunneryExperienceAward),
    BattleValue(super::BattleValueExperienceAward),
}

impl ShotExperienceAward {
    /// Proposed amount before ordinary skill rate limiting; classic gates may reject the award.
    pub fn amount(&self) -> Option<u32> {
        match self {
            Self::Classic(report) => report.amount,
            Self::BattleValue(report) => Some(report.amount),
        }
    }

    /// Only the classic formula consumes an additional award die.
    pub fn roll(&self) -> Option<u8> {
        match self {
            Self::Classic(report) => Some(report.roll),
            Self::BattleValue(_) => None,
        }
    }

    /// Character mutation report, including rejected skill rate limits.
    pub fn award(&self) -> Option<&super::ExperienceAward> {
        match self {
            Self::Classic(report) => report.award.as_ref(),
            Self::BattleValue(report) => Some(&report.award),
        }
    }
}

/// Select the configured formula while keeping its distinct eligibility and RNG behavior.
pub fn award_gunnery_experience(
    world: &mut crate::World,
    request: GunneryAwardRequest,
    config: &crate::config::XpConfig,
) -> Result<Option<ShotExperienceAward>> {
    if config.oldxpsystem != 0 {
        return award_classic_gunnery_experience(
            world,
            GunneryAwardRequest {
                use_unit_modifier: config.perunit_xpmod != 0,
                ..request
            },
        )
        .map(|report| report.map(ShotExperienceAward::Classic));
    }
    super::award_battle_value_gunnery_experience(world, request, config)
        .map(|report| report.map(ShotExperienceAward::BattleValue))
}

/// Borrowed configuration and immutable attack facts reused between damage groups.
#[derive(Clone, Copy)]
pub(super) struct GunneryAwardContext<'a> {
    pub request: GunneryAwardRequest,
    pub config: &'a crate::config::XpConfig,
}

impl GunneryAwardContext<'_> {
    /// Capture diagnostic text while names and pre-impact eligibility still describe this packet.
    pub fn messages(
        self,
        world: &crate::World,
        damage: u16,
        attempt: Option<&ShotExperienceAward>,
    ) -> Vec<super::DiagnosticMessage> {
        super::channels::gunnery_messages(
            world,
            GunneryAwardRequest {
                damage,
                ..self.request
            },
            self.config,
            attempt,
        )
    }

    /// Derive current participants' BV or effective speed immediately before this damage packet.
    pub fn award(
        self,
        world: &mut crate::World,
        damage: u16,
    ) -> Result<Option<ShotExperienceAward>> {
        award_gunnery_experience(
            world,
            GunneryAwardRequest {
                damage,
                ..self.request
            },
            self.config,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Equal units at target seven produce a five-point award for six damage.
    fn sample() -> GunneryExperienceInput {
        GunneryExperienceInput {
            attacker_tons: 35,
            target_tons: 35,
            attacker_speed: 53.75,
            target_speed: 53.75,
            base_to_hit: 7,
            damage: 6,
            unit_modifier: 1.0,
        }
    }

    /// Difficulty follows the number of losing 2d6 outcomes and gates low-value packets inclusively.
    #[test]
    fn difficulty_and_random_gate_boundaries() {
        let chance = sample().classic_chance().unwrap().unwrap();
        assert!((chance.difficulty - 100.0 * 30.0 / 36.0).abs() < 1e-10);
        assert_eq!(chance.award(50).unwrap(), Some(5));
        let chance = GunneryExperienceInput {
            base_to_hit: 3,
            damage: 1,
            ..sample()
        }
        .classic_chance()
        .unwrap()
        .unwrap();
        assert_eq!(chance.award(5).unwrap(), Some(1));
        assert_eq!(chance.award(6).unwrap(), None);
        for bth in [-1, 0, 1, 2, 13] {
            assert!(
                GunneryExperienceInput {
                    base_to_hit: bth,
                    ..sample()
                }
                .classic_chance()
                .unwrap()
                .is_none()
            );
        }
        assert!(chance.award(0).is_err());
        assert!(chance.award(51).is_err());
    }

    /// Mass truncates before clamps; relative speed is squared and the configured unit multiplier is applied last.
    #[test]
    fn mass_speed_scaling_and_cap() {
        let input = GunneryExperienceInput {
            attacker_tons: 60,
            target_tons: 35,
            attacker_speed: 21.5,
            target_speed: 96.751,
            unit_modifier: 2.0,
            ..sample()
        };
        let chance = input.classic_chance().unwrap().unwrap();
        assert!((chance.difficulty - 58.0 * 9.0 * 30.0 / 36.0 * 2.0).abs() < 1e-10);
        assert_eq!(chance.award(50).unwrap(), Some(50));
        for (speed, weight) in [
            (0.0, 2),
            (21.5, 2),
            (21.501, 3),
            (43.0, 3),
            (43.001, 4),
            (64.5, 4),
            (64.501, 5),
            (96.75, 5),
            (96.751, 6),
        ] {
            assert_eq!(speed_weight(speed), weight);
        }
        let zero = GunneryExperienceInput {
            unit_modifier: 0.0,
            ..sample()
        }
        .classic_chance()
        .unwrap()
        .unwrap();
        assert_eq!(zero.award(1).unwrap(), None);
    }

    /// Invalid physical inputs fail before an award roll can be requested.
    #[test]
    fn invalid_and_zero_damage_inputs() {
        for input in [
            GunneryExperienceInput {
                attacker_tons: 0,
                ..sample()
            },
            GunneryExperienceInput {
                attacker_speed: f64::NAN,
                ..sample()
            },
            GunneryExperienceInput {
                target_speed: -1.0,
                ..sample()
            },
            GunneryExperienceInput {
                unit_modifier: f64::INFINITY,
                ..sample()
            },
        ] {
            assert!(input.classic_chance().is_err());
        }
        assert!(
            GunneryExperienceInput {
                damage: 0,
                ..sample()
            }
            .classic_chance()
            .unwrap()
            .is_none()
        );
    }
}
