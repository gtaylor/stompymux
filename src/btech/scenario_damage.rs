//! Wizard located damage delegates material, critical and casualty effects to ordinary impact actions.
use crate::{Config, Flag, ObjectId, Scripts};
use anyhow::{Context, Result, ensure};
use serde::Serialize;

/// Located damage retains the existing anatomy-specific reports without duplicating resolution.
#[derive(Debug, Serialize)]
#[serde(tag = "kind", content = "impact", rename_all = "snake_case")]
pub enum BattleScenarioDamage {
    Mech(super::BattleImpactReport),
    Vehicle(super::BattleVehicleArmorDamage),
}

/// Named hit input shared by native and Lua scenario controls.
#[derive(Debug, Clone, Copy)]
pub struct BattleScenarioHit<'a> {
    pub section: &'a str,
    pub damage: i32,
    pub rear: bool,
    pub critical: bool,
}

/// Apply one wizard-authored hit to a named section; source power, pilot and map are not required.
pub fn damage_section_action(
    scripts: &Scripts,
    config: &Config,
    actor: ObjectId,
    unit: ObjectId,
    hit: BattleScenarioHit<'_>,
) -> Result<BattleScenarioDamage> {
    let BattleScenarioHit {
        section,
        damage,
        rear,
        critical,
    } = hit;
    scripts.atomic(|before| {
        admit(before, actor, unit, damage)?;
        if let Some(vehicle) = before.btech.vehicles().get(&unit) {
            let section =
                super::BattleVehicleSection::parse_location(section).context("Invalid section!")?;
            ensure!(
                vehicle.definition().sections.contains_key(&section),
                "Invalid section!"
            );
            let mut rules =
                super::BattleVehicleImpactRules::configured(&config.battletech, false).criticals;
            rules.combat_safe = super::battle_combat_safe(before, unit)?;
            return super::evacuation::directed_vehicle_damage_action(
                scripts,
                config,
                unit,
                super::BattleVehicleArmorHit {
                    section,
                    amount: damage as u32,
                    through_armor_critical: critical,
                    armor_piercing: None,
                },
                rear,
                rules,
            )
            .map(BattleScenarioDamage::Vehicle);
        }
        let mech = before
            .btech
            .constructed_units()
            .get(&unit)
            .context("Unit construction state is unavailable")?;
        let section = mech
            .chassis()
            .parse_location(section)
            .context("Invalid section!")?;
        super::evacuation::scenario_impact_action(
            scripts,
            config,
            unit,
            super::BattleHit {
                section,
                rear_armor: rear,
                through_armor_critical: critical,
                crew_stun: false,
            },
            damage as u16,
        )
        .map(BattleScenarioDamage::Mech)
    })
}

/// Native section damage accepts four arguments with signed numeric flags interpreted as booleans.
pub(crate) fn command(
    ctx: &crate::CommandContext<'_>,
    input: &crate::CommandInput,
) -> Result<crate::CommandAction> {
    let result = (|| {
        let args: Vec<_> = input.args.split_whitespace().collect();
        ensure!(
            args.len() == 4,
            "Usage: @DAMAGESECTION section damage isrear iscritical"
        );
        let damage = args[1].parse::<i32>().context("Invalid damage!")?;
        let rear = args[2].parse::<i32>().context("Invalid isrear flag!")? != 0;
        let critical = args[3].parse::<i32>().context("Invalid iscritical flag!")? != 0;
        let unit = ctx
            .scripts
            .world()
            .objects
            .get(&ctx.player)
            .and_then(|actor| actor.location)
            .context("Player has no location")?;
        damage_section_action(
            ctx.scripts,
            ctx.config,
            ctx.player,
            unit,
            BattleScenarioHit {
                section: args[0],
                damage,
                rear,
                critical,
            },
        )
    })();
    Ok(match result {
        Ok(_) => crate::CommandAction::Continue,
        Err(error) => {
            crate::CommandAction::Report(crate::CommandReport::Reply(format!("{error:#}")))
        }
    })
}

/// Shared wizard authority and object checks for located and random scenario hits.
pub(super) fn admit(
    world: &crate::World,
    actor: ObjectId,
    unit: ObjectId,
    damage: i32,
) -> Result<()> {
    ensure!(
        crate::authority::is_wizard(world, actor),
        "Permission denied."
    );
    ensure!(
        world.objects.get(&unit).is_some_and(
            |object| object.kind == crate::Kind::Thing && !object.flags.contains(Flag::Going)
        ),
        "Unit is unavailable"
    );
    ensure!(
        (1..=1000).contains(&damage),
        "Damage must be between 1 and 1000"
    );
    ensure!(
        world.btech.vehicles().contains_key(&unit)
            || world.btech.constructed_units().contains_key(&unit),
        "Unit construction state is unavailable"
    );
    Ok(())
}
