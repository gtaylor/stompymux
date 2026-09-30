//! Wizard packet damage shares location, material, crew and publication rules with combat.
use super::{BattleBlastImpact, BattleFallRules, BattleHitArc, BattleHitTable};
use crate::{Config, Flag, ObjectId, Scripts};
use anyhow::{Context, Result, ensure};
use serde::Serialize;

/// Native cluster size denotes packet count; integer division discards the remainder.
#[derive(Debug, Clone, Copy)]
pub struct BattleScenarioSalvo {
    pub damage: i32,
    pub clusters: i32,
    pub rear: bool,
    /// Accepted by the command grammar; random location routing chooses critical eligibility.
    pub critical: bool,
}

/// Every requested packet is retained, including traversal after destruction or combat-safe absorption.
#[derive(Debug, Serialize)]
pub struct BattleScenarioSalvoReport {
    pub packet_damage: u16,
    pub discarded_damage: u16,
    pub impacts: Vec<BattleBlastImpact>,
}

/// Apply a wizard-authored packet sequence without requiring power, pilot authority or placement.
pub fn damage_action(
    scripts: &Scripts,
    config: &Config,
    actor: ObjectId,
    unit: ObjectId,
    request: BattleScenarioSalvo,
) -> Result<BattleScenarioSalvoReport> {
    scripts.atomic(|before| {
        super::scenario_damage::admit(before, actor, unit, request.damage)?;
        ensure!(request.clusters > 0, "Invalid cluster size!");
        ensure!(
            request.clusters <= request.damage,
            "Invalid cluster size! (must be smaller than damage amount, but > 0)"
        );
        let mut report = BattleScenarioSalvoReport {
            packet_damage: (request.damage / request.clusters) as u16,
            discarded_damage: (request.damage % request.clusters) as u16,
            impacts: Vec::with_capacity(request.clusters as usize),
        };
        let mut rear = request.rear;
        let mut notices = Vec::new();
        let mut private = Vec::new();
        for _ in 0..request.clusters {
            let mut world = scripts.world_mut();
            let arc = super::hit_direction::HitDirection::SelfHit {
                mode: config.battletech.hit_arcs,
            }
            .current(&world, unit)?;
            rear |= arc == BattleHitArc::Rear;
            let character = world.objects[&unit].flags.contains(Flag::InCharacter);
            let pilot = world
                .btech
                .vehicles()
                .get(&unit)
                .and_then(|unit| unit.pilot())
                .or_else(|| {
                    world
                        .btech
                        .constructed_units()
                        .get(&unit)
                        .and_then(|unit| unit.pilot())
                });
            let mut rules = BattleFallRules::configured(config);
            rules.toughness = pilot
                .and_then(|pilot| world.btech.character_values().get(&pilot))
                .is_some_and(|values| super::advantages::enabled(values, "Toughness"));
            rules.vehicle_impact.criticals.toughness = rules.toughness;
            rules.vehicle_impact.criticals.combat_safe = super::battle_combat_safe(&world, unit)?;
            // Self-hits sample their own current sightline instead of another observer's cached cover.
            let placed = super::scanner::scanner_unit(&world, unit)
                .unwrap()
                .position
                .is_some();
            let table = if placed && super::unit_terrain_los(&world, unit, unit)?.partial_cover {
                BattleHitTable::Punch
            } else {
                BattleHitTable::Weapon
            };
            let (impact, messages) = super::blast_damage::resolve_packet(
                &mut world,
                unit,
                super::blast_damage::MaterialPacket {
                    amount: report.packet_damage,
                    table,
                    arc,
                    character,
                    attacker: Some(unit),
                    class: super::BattleDamageClass::Ordinary,
                },
                rear,
                rules,
            )?;
            impact.append_feedback(&mut private, notices.len());
            report.impacts.push(impact);
            notices.extend(messages);
        }
        super::piloting::publish_ordered_notices(scripts, &notices, &private)?;
        super::evacuation::publish_blast_consequences(scripts, config, &report.impacts, None)?;
        super::evacuation::publish_new_casualties(scripts, config, before)?;
        scripts.world().validate_action(config)?;
        Ok(report)
    })
}

/// Four signed integer arguments preserve native flag and cluster-count semantics.
pub(crate) fn command(
    ctx: &crate::CommandContext<'_>,
    input: &crate::CommandInput,
) -> Result<crate::CommandAction> {
    let result = (|| {
        let args: Vec<_> = input.args.split_whitespace().collect();
        ensure!(args.len() == 4, "Invalid arguments!");
        let damage = args[0].parse::<i32>().context("Invalid damage!")?;
        let clusters = args[1].parse::<i32>().context("Invalid cluster size!")?;
        let rear = args[2].parse::<i32>().context("Invalid isrear flag!")? != 0;
        let critical = args[3].parse::<i32>().context("Invalid iscritical flag!")? != 0;
        let unit = ctx
            .scripts
            .world()
            .objects
            .get(&ctx.player)
            .and_then(|actor| actor.location)
            .context("Player has no location")?;
        damage_action(
            ctx.scripts,
            ctx.config,
            ctx.player,
            unit,
            BattleScenarioSalvo {
                damage,
                clusters,
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
