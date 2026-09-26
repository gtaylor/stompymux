//! Atomic BattleMech pivot attempts using shared piloting, fall and casualty rules.
use super::BattleUnit;
use crate::{Config, ObjectId, Scripts, World};
use anyhow::{Context, Result, ensure};
use serde::Serialize;
use std::sync::Arc;

/// Detached maneuver outcome, including any committed fall and skill award.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct BattleBootleggerReport {
    pub modifier: i16,
    pub check: super::BattlePilotingCheck,
    pub fall: Option<super::BattleFallReport>,
    pub notices: Vec<super::BattleNotice>,
    /// Pilot-only roll feedback ordered before the maneuver consequences.
    pub pilot_notices: Vec<super::BattlePilotNotice>,
}

impl BattleUnit {
    /// Require available, idle legs and sum their derived weapon-mount actuator penalties.
    pub fn bootlegger_leg_modifier(&self) -> Result<i16> {
        let loadout = self.loadout()?;
        let mut modifier = 0;
        for &section in self.chassis().legs() {
            ensure!(
                !self.leg_unavailable(section),
                "You can't perform a bootlegger with destroyed legs!"
            );
            ensure!(
                !self.limb_recycle().contains_key(&section),
                "Your {} is still recovering from its last action.",
                self.chassis().section_name(section)
            );
            ensure!(
                !loadout
                    .weapons
                    .iter()
                    .enumerate()
                    .any(|(index, weapon)| weapon
                        .criticals
                        .iter()
                        .any(|slot| slot.section == section)
                        && self.weapon_recycle().contains_key(&index)),
                "You have weapons recycling in your {}.",
                self.chassis().section_name(section)
            );
            modifier += i16::from(self.mounting_modifier(section));
        }
        Ok(modifier)
    }
}

/// Validate readiness and derive situational difficulty without consuming dice.
pub fn bootlegger_modifier(world: &World, id: ObjectId, pilot: ObjectId) -> Result<i16> {
    super::power::controlled_unit(world, id, pilot)?;
    let unit = &world.btech.constructed_units()[&id];
    ensure!(
        unit.power() == super::BattlePower::Running,
        "Start the unit first"
    );
    let position = unit.position().context("Unit is not on a battlefield")?;
    let speed = unit.motion().context("Unit has no motion")?.speed;
    ensure!(
        speed >= 43.0,
        "You are going too slow to perform a bootlegger! The required minimum speed is 43.0 KPH."
    );
    let mut modifier = unit.bootlegger_leg_modifier()?;
    modifier += if speed <= 43.0 {
        0
    } else if speed <= 86.0 {
        1
    } else if speed <= 129.0 {
        2
    } else {
        3
    };
    modifier += match unit.definition().tons {
        0..=35 => 0,
        36..=55 => 1,
        56..=75 => 2,
        _ => 3,
    };
    let tile =
        world.btech.maps()[&position.map].hex(i64::from(position.x), i64::from(position.y))?;
    if matches!(
        tile.terrain,
        super::Terrain::Water | super::Terrain::Ice | super::Terrain::Bridge
    ) && super::unit_elevation(world, id)?.is_some_and(|height| height < 0)
    {
        modifier += 2;
    }
    Ok(modifier.max(1))
}

/// Perform a left/right pivot and publish all nested consequences under one checkpoint.
pub fn bootlegger(
    scripts: &Scripts,
    config: &Config,
    id: ObjectId,
    pilot: ObjectId,
    direction: &str,
) -> Result<BattleBootleggerReport> {
    let before = scripts.world.borrow().clone();
    let effects = scripts.effects.checkpoint();
    let result = (|| {
        let words: Vec<_> = direction.split_whitespace().collect();
        ensure!(words.len() == 1, "Invalid number of arguments!");
        let delta = match words[0].chars().next().unwrap().to_ascii_uppercase() {
            'L' => -90.0,
            'R' => 90.0,
            _ => anyhow::bail!("Invalid turn direction!"),
        };
        let modifier = bootlegger_modifier(&scripts.world(), id, pilot)?;
        let settings = &config.battletech;
        let rules = super::BattleFallRules {
            vehicle_impact: crate::BattleVehicleImpactRules::configured(settings, false),
            stacking: super::BattleStackingRules {
                mode: settings.stacking,
                damage_percent: settings.stackdamage,
                hit_arcs: settings.hit_arcs,
            },
            stagger: super::BattleStaggerMode::from_setting(settings.newstagger),
            hit: super::BattleHitRules {
                inferno_penalty: settings.inferno_penalty != 0,
                exile_stun_mode: settings.exile_stun_code.clamp(0, 2) as u8,
            },
            extended_piloting: settings.extended_piloting != 0,
            toughness: scripts
                .world()
                .btech
                .character_values()
                .get(&pilot)
                .is_some_and(|values| super::advantages::enabled(values, "Toughness")),
        };
        let mut world = scripts.world.borrow_mut();
        let mut check = super::roll_piloting(&mut world, id, modifier, rules.extended_piloting)?;
        let experience: Vec<_> = super::piloting::award_control_check(
            &mut world,
            id,
            &mut check,
            rules.extended_piloting,
        )?
        .into_iter()
        .collect();
        let mut notices = Vec::new();
        let mut pilot_notices = Vec::new();
        super::piloting::capture_feedback(
            id,
            Some(pilot),
            &check,
            &mut notices,
            &mut pilot_notices,
        );
        let fall = if check.success {
            let unit = Arc::make_mut(&mut world.btech.constructed)
                .get_mut(&id)
                .unwrap();
            let motion = unit.motion.as_mut().unwrap();
            motion.heading = (motion.heading.trunc() + delta).rem_euclid(360.0);
            motion.desired_heading = motion.heading;
            motion.speed *= 0.5;
            notices.push(super::BattleNotice {
                unit: id,
                text: format!(
                    "You plant a foot and swivel, changing your heading to {:.0}.",
                    motion.heading
                ),
            });
            for &section in unit.chassis().legs() {
                unit.limb_recycle.insert(section, 30);
            }
            None
        } else {
            notices.push(super::BattleNotice { unit: id, text: "You plant a foot and try to swivel...\r\n... but realize a little late that this is harder than it looks!".into() });
            notices.extend(super::broadcast::observer_notices(
                &world,
                id,
                "attempts to fight the forces of inertia but looses the battle miserably!",
            ));
            if modifier > 2 {
                notices.extend(super::broadcast::observer_notices(
                    &world,
                    id,
                    "tumbles over and over and over!",
                ));
            }
            let levels = u8::try_from(modifier).context("Bootlegger severity is out of range")?;
            let fall = if world.objects[&id].flags.contains(crate::Flag::InCharacter) {
                super::fall::resolve_character_fall(&mut world, id, levels, rules)?
            } else {
                super::resolve_fall(&mut world, id, levels, rules)?
            };
            fall.append_notices(id, &mut notices, &mut pilot_notices);
            Some(fall)
        };
        drop(world);
        super::channels::publish(scripts, config, &experience)?;
        super::piloting::publish_maneuver_feedback(
            scripts,
            config,
            &notices,
            &pilot_notices,
            Some(&check),
            true,
        )?;
        if let Some(fall) = &fall {
            super::evacuation::publish_fall_consequences(scripts, config, fall)?;
        }
        super::evacuation::publish_new_casualties(scripts, config, &before)?;
        scripts.world().validate(config)?;
        Ok(BattleBootleggerReport {
            modifier,
            check,
            fall,
            notices,
            pilot_notices,
        })
    })();
    if result.is_err() {
        *scripts.world.borrow_mut() = before;
        scripts.effects.restore(effects);
    }
    result
}

/// Native maneuver uses the invoking player's current cockpit.
pub(crate) fn command(
    ctx: &crate::CommandContext<'_>,
    input: &crate::CommandInput,
) -> Result<crate::CommandAction> {
    let result = (|| {
        let id = ctx
            .scripts
            .world()
            .objects
            .get(&ctx.player)
            .and_then(|player| player.location)
            .context("Enter a unit first")?;
        bootlegger(ctx.scripts, ctx.config, id, ctx.player, &input.args)
    })();
    Ok(match result {
        Ok(_) => crate::CommandAction::Continue,
        Err(error) => {
            crate::CommandAction::Report(crate::CommandReport::Reply(format!("{error:#}")))
        }
    })
}
