//! Magma hazards from Tactical Operations (p. 34): crust that can break into liquid magma under
//! a unit crossing it, and liquid magma that burns a 'Mech's exposed locations and destroys any
//! other unit that touches it. Magma's heat is part of the heat model; see `heat.rs`.
use super::{FallRules, Hex, MechSection, Notice, PilotNotice, Posture, TacticalImpact};
use crate::{Config, Flag, ObjectId, Scripts, World};
use anyhow::{Context, Result, ensure};
use std::collections::BTreeMap;

/// Notices and damage from magma, for the enclosing action to publish.
#[derive(Default)]
#[must_use = "Publish magma notices and impact consequences in the enclosing action"]
pub(super) struct MagmaReport {
    pub notices: Vec<Notice>,
    /// Private feedback indexed into `notices`.
    pub pilot_notices: Vec<PilotNotice>,
    pub impacts: Vec<TacticalImpact>,
}

/// How a unit came onto magma, which decides how likely crust is to break beneath it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum MagmaEntry {
    /// Walking or driving in: the crust breaks on a 6.
    Ground,
    /// Landing from a jump: the crust breaks on a 4, 5 or 6.
    JumpLanding,
}

/// The map, coordinates, hex and elevation level of a placed, intact unit.
fn footing(world: &World, id: ObjectId) -> Option<(ObjectId, (i64, i64), Hex, i32)> {
    let unit = super::scanner::scanner_unit(world, id)?;
    if unit.destroyed {
        return None;
    }
    let position = unit.position?;
    let (x, y) = (i64::from(position.x), i64::from(position.y));
    let tile = world.btech.maps().get(&position.map)?.base_hex(x, y).ok()?;
    let level = super::unit_elevation(world, id).ok()??;
    Some((position.map, (x, y), tile, level))
}

/// Roll for magma crust under `id` to break into liquid magma, which it then stays.
pub(super) fn crack_crust(
    world: &mut World,
    id: ObjectId,
    entry: MagmaEntry,
) -> Result<Vec<Notice>> {
    let Some((map, (x, y), tile, level)) = footing(world, id) else {
        return Ok(Vec::new());
    };
    if !tile.on_magma_crust(level) || hovering(world, id) {
        return Ok(Vec::new());
    }
    let roll = super::dice::unit_dice_mut(world, id)?.d6();
    let breaks = match entry {
        MagmaEntry::Ground => roll == 6,
        MagmaEntry::JumpLanding => roll >= 4,
    };
    if !breaks {
        return Ok(Vec::new());
    }
    world
        .btech
        .maps
        .get_mut(&map)
        .context("Map not found")?
        .write_hex(x, y, tile.with_crust_broken())?;
    let mut notices =
        super::broadcast::observer_notices(world, id, "breaks through the magma crust!");
    notices.push(Notice {
        unit: id,
        text: "[fg=red bold]The magma crust cracks open beneath you![reset]".into(),
    });
    Ok(notices)
}

/// Whether `id` is a hovercraft, which rides above the ground rather than on it.
fn hovering(world: &World, id: ObjectId) -> bool {
    world
        .btech
        .vehicles()
        .get(&id)
        .is_some_and(|unit| unit.definition().movement == super::VehicleMovement::Hover)
}

/// Liquid magma's effect on a unit in it: a 'Mech takes 2D6 to each exposed location, its
/// legs or, while it is down, every location; any other unit on the ground is destroyed
/// outright. `character` lets the damage reach in-character crews, which only a host action
/// that publishes casualties may allow.
pub(super) fn scorch(
    world: &mut World,
    id: ObjectId,
    rules: FallRules,
    character: bool,
) -> Result<MagmaReport> {
    let Some((_, _, tile, level)) = footing(world, id) else {
        return Ok(MagmaReport::default());
    };
    if !tile.in_liquid_magma(level) {
        return Ok(MagmaReport::default());
    }
    if world.btech.vehicles().contains_key(&id) {
        return Ok(MagmaReport {
            notices: engulf_vehicle(world, id, character)?,
            ..Default::default()
        });
    }
    burn_mech(world, id, rules, character)
}

/// Destroy the vehicle `id` if it stands in liquid magma, as soon as it drives in.
pub(super) fn engulf_if_molten(
    world: &mut World,
    id: ObjectId,
    character: bool,
) -> Result<Vec<Notice>> {
    let molten = footing(world, id).is_some_and(|(_, _, tile, level)| tile.in_liquid_magma(level));
    if !molten || !world.btech.vehicles().contains_key(&id) {
        return Ok(Vec::new());
    }
    engulf_vehicle(world, id, character)
}

/// Destroy a vehicle that has driven or fallen into liquid magma, crew and all.
fn engulf_vehicle(world: &mut World, id: ObjectId, character: bool) -> Result<Vec<Notice>> {
    ensure!(
        character || !world.objects[&id].flags.contains(Flag::InCharacter),
        "Character vehicles in magma require a host movement transaction"
    );
    let mut notices = super::broadcast::observer_notices(world, id, "is swallowed by the magma!");
    world.btech.vehicles.get_mut(&id).unwrap().kill_crew();
    notices.push(Notice {
        unit: id,
        text: "[fg=red bold]Your vehicle sinks into the molten rock and is consumed![reset]".into(),
    });
    Ok(notices)
}

/// Burn each exposed location of a 'Mech standing in liquid magma for 2D6.
fn burn_mech(
    world: &mut World,
    id: ObjectId,
    rules: FallRules,
    character: bool,
) -> Result<MagmaReport> {
    let unit = &world.btech.constructed_units()[&id];
    let exposed: Vec<MechSection> = if unit.posture() == Posture::Prone {
        MechSection::ALL.to_vec()
    } else {
        unit.chassis().legs().to_vec()
    };
    let mut report = MagmaReport {
        notices: vec![Notice {
            unit: id,
            text: if unit.posture() == Posture::Prone {
                "[fg=red bold]Molten rock washes over your 'Mech![reset]"
            } else {
                "[fg=red bold]Molten rock sears your legs![reset]"
            }
            .into(),
        }],
        ..Default::default()
    };
    for section in exposed {
        let unit = &world.btech.constructed_units()[&id];
        if unit.is_destroyed() || unit.sections()[&section].internal == 0 {
            continue;
        }
        let damage = super::dice::unit_dice_mut(world, id)?.generic_roll();
        let impact = super::impact::resolve_environmental_damage(
            world,
            id,
            section,
            u16::from(damage),
            rules,
            character,
        )?;
        super::piloting::append_feedback(
            &mut report.pilot_notices,
            impact.pilot_notices.iter().cloned(),
            report.notices.len(),
        );
        report.notices.extend(impact.notices.clone());
        report.impacts.push(impact);
    }
    Ok(report)
}

/// Where each unit stood before a tick's movement, so the magma step can tell which units have
/// just arrived in liquid magma.
pub struct MagmaSnapshot(BTreeMap<ObjectId, (ObjectId, (i64, i64), bool)>);

/// Record where every unit stands, and whether it is in liquid magma, before movement.
pub fn magma_snapshot(world: &World) -> MagmaSnapshot {
    MagmaSnapshot(
        world
            .btech
            .constructed_units()
            .keys()
            .chain(world.btech.vehicles().keys())
            .filter_map(|&id| {
                let (map, hex, tile, level) = footing(world, id)?;
                Some((id, (map, hex, tile.in_liquid_magma(level))))
            })
            .collect(),
    )
}

/// Whether any unit stands in liquid magma, so the turn boundary has burns to apply even when
/// nothing else on the battlefield is moving.
pub(crate) fn pending(world: &World) -> bool {
    world
        .btech
        .constructed_units()
        .keys()
        .chain(world.btech.vehicles().keys())
        .any(|&id| {
            footing(world, id).is_some_and(|(_, _, tile, level)| tile.in_liquid_magma(level))
        })
}

/// Burn every unit in liquid magma that arrived there since `before` was taken, by moving,
/// jumping, falling or the crust breaking under it, and once a turn burn everyone still in it.
fn advance_magma(
    world: &mut World,
    config: &Config,
    before: &MagmaSnapshot,
) -> Result<Vec<MagmaReport>> {
    let turn = world.btech.turn_clock.starts_turn();
    let ids: Vec<_> = world
        .btech
        .constructed_units()
        .keys()
        .chain(world.btech.vehicles().keys())
        .copied()
        .filter(|id| {
            world
                .objects
                .get(id)
                .is_some_and(|object| !object.flags.contains(Flag::Going))
        })
        .collect();
    let mut reports = Vec::new();
    for id in ids {
        let Some((map, hex, tile, level)) = footing(world, id) else {
            continue;
        };
        if !tile.in_liquid_magma(level) {
            continue;
        }
        let arrived = before.0.get(&id) != Some(&(map, hex, true));
        if !arrived && !turn {
            continue;
        }
        let mut rules = FallRules::configured(config);
        rules.toughness = world
            .btech
            .unit(id)
            .and_then(|unit| super::with_unit!(unit, |unit| unit.pilot()))
            .is_some_and(|pilot| super::skills::boolean_advantage(world, pilot, "Toughness"));
        reports.push(scorch(world, id, rules, true)?);
    }
    Ok(reports)
}

/// Apply liquid magma's burns to units that have entered it since `before`, and to everyone in
/// it at the start of a turn, publishing notices and casualties in one action.
pub fn advance_magma_action(
    scripts: &Scripts,
    config: &Config,
    before: &MagmaSnapshot,
) -> Result<()> {
    scripts.atomic(|world_before| {
        let reports = advance_magma(&mut scripts.world_mut(), config, before)?;
        for report in &reports {
            super::piloting::publish_ordered_notices(
                scripts,
                &report.notices,
                &report.pilot_notices,
            )?;
            for impact in &report.impacts {
                super::evacuation::publish_impact_consequences(scripts, config, impact)?;
            }
        }
        super::evacuation::publish_new_casualties(scripts, config, world_before)?;
        scripts.world().validate_action(config)?;
        Ok(())
    })
}
