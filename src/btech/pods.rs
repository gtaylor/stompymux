//! Pod inspection and biped swatting, sharing damage, electronics and host transaction boundaries.
use super::*;
use crate::{Flag, ObjectId, World};
use anyhow::{Context, Result, ensure};
use serde::Serialize;
use std::collections::BTreeSet;

/// One installed section in the cockpit's pod inspection table.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BattlePodRow {
    pub section: BattleUnitSection,
    pub destroyed: bool,
    pub kinds: BTreeSet<BattleBeaconKind>,
}

/// A completed swat, including any self-inflicted damage and its nested consequences.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct BattlePodRemoval {
    pub section: BattleSection,
    pub kind: BattleBeaconKind,
    pub arm: BattleArm,
    pub target_number: i32,
    pub roll: u8,
    pub removed: bool,
    pub self_damage: u16,
    pub impact: Option<BattleTacticalImpact>,
    pub notices: Vec<BattleNotice>,
    /// Nested self-damage checks indexed into the attempt notice stream.
    pub pilot_notices: Vec<BattlePilotNotice>,
}

/// Inspect a running unit after the same cockpit prerequisites as the native PODS command.
pub fn inspect_pods(world: &World, id: ObjectId, pilot: ObjectId) -> Result<Vec<BattlePodRow>> {
    controlled(world, id, pilot)?;
    if let Some(unit) = world.btech.vehicles().get(&id) {
        return Ok(rows(
            unit.definition()
                .sections
                .iter()
                .map(|(section, original)| {
                    (
                        *section,
                        original.internal > 0,
                        unit.sections()[section].internal == 0,
                    )
                }),
            unit.beacons(),
            BattleUnitSection::Vehicle,
        ));
    }
    let unit = &world.btech.constructed_units()[&id];
    Ok(rows(
        BattleSection::ALL.into_iter().map(|section| {
            (
                section,
                unit.definition().sections[&section].internal > 0,
                unit.sections()[&section].internal == 0,
            )
        }),
        unit.beacons(),
        BattleUnitSection::Mech,
    ))
}

/// Both construction adapters supply installed sections to one pod-row policy.
fn rows<S: Copy + Ord>(
    sections: impl Iterator<Item = (S, bool, bool)>,
    beacons: &std::collections::BTreeMap<S, BTreeSet<BattleBeaconKind>>,
    identity: impl Fn(S) -> BattleUnitSection,
) -> Vec<BattlePodRow> {
    if beacons.is_empty() {
        return Vec::new();
    }
    sections
        .filter(|(_, installed, _)| *installed)
        .map(|(section, _, destroyed)| BattlePodRow {
            section: identity(section),
            destroyed,
            kinds: beacons.get(&section).cloned().unwrap_or_default(),
        })
        .collect()
}

/// Shared cockpit and map prerequisites, without unrelated physical-attack restrictions.
fn controlled(world: &World, id: ObjectId, pilot: ObjectId) -> Result<()> {
    super::targeting::controlled(world, id, pilot)?;
    ensure!(
        super::scanner::scanner_unit(world, id).is_some_and(|unit| unit.position.is_some()),
        "Unit is not placed"
    );
    Ok(())
}

/// Slot identity and damage both matter for the swatting arm's actuator penalties.
fn actuator(unit: &BattleUnit, section: BattleSection, slot: u8, system: BattleSystem) -> bool {
    let location = CriticalLocation { section, slot };
    !unit.critical_unavailable(location)
        && unit.definition().sections[&section]
            .criticals
            .get(&slot)
            .is_some_and(|part| BattleSystem::parse(&part.equipment).ok() == Some(system))
}

/// Return the arm's aiming penalty and halved damage, or reject unavailable/recycling arms.
fn arm_profile(unit: &BattleUnit, arm: BattleArm) -> Result<Option<(i32, u16)>> {
    let section = arm.section();
    if unit.sections()[&section].internal == 0 || unit.limb_recycle().contains_key(&section) {
        return Ok(None);
    }
    if unit
        .loadout()?
        .weapons
        .iter()
        .enumerate()
        .any(|(index, mount)| {
            unit.weapon_recycle().contains_key(&index)
                && mount
                    .criticals
                    .iter()
                    .any(|location| location.section == section)
        })
    {
        return Ok(None);
    }
    let upper = actuator(unit, section, 1, BattleSystem::UpperActuator);
    let lower = actuator(unit, section, 2, BattleSystem::LowerActuator);
    let hand = actuator(unit, section, 3, BattleSystem::HandOrFootActuator);
    let penalty = 2 * i32::from(!upper) + 2 * i32::from(!lower) + i32::from(!hand);
    let mut damage = (unit.definition().tons + 5) / 10;
    if !lower {
        damage /= 2;
    }
    if !upper {
        damage /= 2;
    }
    Ok(Some((penalty, damage)))
}

/// Resolve tactical removal atomically; character self-damage requires the casualty-publishing host action.
pub fn remove_pod(
    world: &mut World,
    id: ObjectId,
    pilot: ObjectId,
    section: BattleSection,
    kind: BattleBeaconKind,
    rules: BattleFallRules,
) -> Result<BattlePodRemoval> {
    ensure!(
        world
            .objects
            .get(&id)
            .is_some_and(|object| !object.flags.contains(Flag::InCharacter)),
        "Character pod removal requires a host action"
    );
    resolve(world, id, pilot, section, kind, rules)
}

/// Raw attack rolls award no control XP; a failed swat feeds the ordinary self-damage cascade.
fn resolve(
    world: &mut World,
    id: ObjectId,
    pilot: ObjectId,
    section: BattleSection,
    kind: BattleBeaconKind,
    rules: BattleFallRules,
) -> Result<BattlePodRemoval> {
    controlled(world, id, pilot)?;
    ensure!(
        world.btech.constructed_units().contains_key(&id),
        "Only biped BattleMechs can swat individual pods"
    );
    ensure!(
        world.btech.constructed_units()[&id].chassis() != BattleMechChassis::Quad,
        "Quads can not knock of iNARC pods!"
    );
    ensure!(
        kind != BattleBeaconKind::Narc,
        "Conventional NARC pods cannot be swatted off"
    );
    let unit = &world.btech.constructed_units()[&id];
    ensure!(
        unit.sections()[&section].internal > 0,
        "That section is destroyed!"
    );
    ensure!(
        unit.beacons()
            .get(&section)
            .is_some_and(|kinds| kinds.contains(&kind)),
        "There are no iNarc {kind:?} pods attached to your {}!",
        section.name().replace('_', " ")
    );
    let choices: &[BattleArm] = match section {
        BattleSection::LeftArm => &[BattleArm::Right],
        BattleSection::RightArm => &[BattleArm::Left],
        _ => &[BattleArm::Left, BattleArm::Right],
    };
    let mut selected = None;
    for &arm in choices {
        if let Some((penalty, damage)) = arm_profile(unit, arm)?
            && selected.is_none_or(|(_, old, _)| penalty < old)
        {
            selected = Some((arm, penalty, damage));
        }
    }
    let (arm, penalty, damage) = selected.context(
        "You need an intact arm without recycling weapons or physical recovery to remove that pod!",
    )?;
    let target_number =
        i32::from(unit_piloting_target(world, id, rules.extended_piloting)?) + 4 + penalty;
    world.attempt(|world| {
        let roll = world
            .btech
            .constructed
            .get_mut(&id)
            .unwrap()
            .dice
            .generic_roll();
        let removed = i32::from(roll) >= target_number;
        let mut pilot_notices = Vec::new();
        let mut notices = vec![BattleNotice {
            unit: id,
            text: format!(
                "You try to swat at the iNarc pods attached to your {} with your {}.  BTH:  {target_number},\tRoll:  {roll}",
                section.name().replace('_', " "),
                arm.section().name().replace('_', " ")
            ),
        }];
        let impact = if removed {
            let unit = world.btech.constructed.get_mut(&id).unwrap();
            let kinds = unit.beacons.get_mut(&section).unwrap();
            kinds.remove(&kind);
            if kinds.is_empty() {
                unit.beacons.remove(&section);
            }
            notices.push(BattleNotice {
                unit: id,
                text: format!(
                    "You knock a {kind:?} pod off your {}!",
                    section.name().replace('_', " ")
                ),
            });
            notices.extend(super::broadcast::observer_notices(
                world,
                id,
                "knocks an iNarc pod off itself.",
            ));
            None
        } else {
            notices.push(BattleNotice {
                unit: id,
                text: "Uh oh. You miss the pod and hit yourself!".into(),
            });
            notices.extend(super::broadcast::observer_notices(
                world,
                id,
                "tries to swat off an iNarc pod, but misses and hits itself!",
            ));
            let hit = BattleHit {
                section,
                rear_armor: false,
                through_armor_critical: false,
                crew_stun: false,
            };
            let impact = if world.objects[&id].flags.contains(Flag::InCharacter) {
                super::impact::resolve_character_impact_with_rules(
                    world,
                    id,
                    hit,
                    damage,
                    Some(rules),
                )?
            } else {
                resolve_tactical_impact(world, id, hit, damage, rules)?
            };
            super::piloting::append_feedback(
                &mut pilot_notices,
                impact.pilot_notices.iter().cloned(),
                notices.len(),
            );
            notices.extend(impact.notices.iter().cloned());
            Some(impact)
        };
        world
            .btech
            .constructed
            .get_mut(&id)
            .unwrap()
            .limb_recycle
            .insert(arm.section(), 60);
        notices.extend(refresh_electronic_fields(world)?);
        world.btech.validate_action(world)?;
        Ok(BattlePodRemoval {
            section,
            kind,
            arm,
            target_number,
            roll,
            removed,
            self_damage: if removed { 0 } else { damage },
            impact,
            notices,
            pilot_notices,
    })
    })
}

/// Publish the attempt, nested injuries, falls and casualties as one rollback-capable host action.
pub fn remove_pod_action(
    scripts: &crate::Scripts,
    config: &crate::Config,
    id: ObjectId,
    pilot: ObjectId,
    section: BattleSection,
    kind: BattleBeaconKind,
    rules: BattleFallRules,
) -> Result<BattlePodRemoval> {
    scripts.atomic(|before| {
        let report = resolve(
            &mut scripts.world.borrow_mut(),
            id,
            pilot,
            section,
            kind,
            rules,
        )?;
        super::piloting::publish_ordered_notices(scripts, &report.notices, &report.pilot_notices)?;
        if let Some(impact) = &report.impact {
            super::evacuation::publish_impact_consequences(scripts, config, impact)?;
        }
        super::evacuation::publish_new_casualties(scripts, config, &before)?;
        scripts.world.borrow().validate_action(config)?;
        Ok(report)
    })
}

/// Resolve command locations against the unit's own anatomy.
pub(crate) fn section(world: &World, id: ObjectId, value: &str) -> Result<BattleSection> {
    world
        .btech
        .constructed_units()
        .get(&id)
        .context("Unit construction state is unavailable")?
        .chassis()
        .parse_location(value)
}

/// Only haywire and ECM need explicit selectors; the default is a homing pod.
pub(crate) fn kind(value: &str) -> BattleBeaconKind {
    match value.chars().next().map(|c| c.to_ascii_uppercase()) {
        Some('Y') => BattleBeaconKind::Haywire,
        Some('E') => BattleBeaconKind::Ecm,
        _ => BattleBeaconKind::Homing,
    }
}

/// Read-only cockpit table; destroyed sections remain visible when other pods are attached.
pub fn pod_status(world: &World, id: ObjectId, pilot: ObjectId) -> Result<String> {
    let rows = inspect_pods(world, id, pilot)?;
    if rows.is_empty() {
        return Ok("There are no NARC or iNARC pods attached to this unit.".into());
    }
    let mut lines = vec![
        "Attached NARC and iNARC Pods".into(),
        "Location       | NARC | iHoming | iHaywire | iECM | iNemesis".into(),
    ];
    for row in rows {
        let marks = [
            BattleBeaconKind::Narc,
            BattleBeaconKind::Homing,
            BattleBeaconKind::Haywire,
            BattleBeaconKind::Ecm,
        ]
        .map(|kind| {
            if row.destroyed {
                '*'
            } else if row.kinds.contains(&kind) {
                'X'
            } else {
                '.'
            }
        });
        lines.push(format!(
            "{:<14} |  {}   |    {}    |    {}     |  {}   |    {}",
            match row.section {
                BattleUnitSection::Mech(section) => world.btech.constructed_units()[&id]
                    .chassis()
                    .section_name(section),
                BattleUnitSection::Vehicle(section) => section.name(),
            }
            .replace('_', " "),
            marks[0],
            marks[1],
            marks[2],
            marks[3],
            if row.destroyed { '*' } else { '.' }
        ));
    }
    Ok(lines.join("\n"))
}

/// Native inspection and removal adapters share the typed selectors and core authorization.
pub(crate) fn command(
    ctx: &crate::CommandContext<'_>,
    input: &crate::CommandInput,
) -> Result<crate::CommandAction> {
    let result = (|| {
        let id = ctx
            .scripts
            .world
            .borrow()
            .objects
            .get(&ctx.player)
            .and_then(|player| player.location)
            .context("Enter a unit first")?;
        if input.name == "pods" {
            return pod_status(&ctx.scripts.world.borrow(), id, ctx.player).map(Some);
        }
        let args: Vec<_> = input.args.split_ascii_whitespace().collect();
        ensure!(args.len() == 2, "Invalid number of arguments!");
        let location = section(&ctx.scripts.world.borrow(), id, args[0])?;
        remove_pod_action(
            ctx.scripts,
            ctx.config,
            id,
            ctx.player,
            location,
            kind(args[1]),
            super::physical::configured_rules(ctx.config).fall,
        )?;
        Ok(None)
    })();
    Ok(match result {
        Ok(Some(text)) => crate::CommandAction::Report(crate::CommandReport::Reply(text)),
        Ok(None) => crate::CommandAction::Continue,
        Err(error) => {
            crate::CommandAction::Report(crate::CommandReport::Reply(format!("{error:#}")))
        }
    })
}
