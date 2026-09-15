//! Reactor destruction and radial blast effects use the shared damage and casualty transaction.
use super::*;
use crate::{Config, Flag, ObjectId, Scripts, World};
use anyhow::{Context, Result, ensure};
use serde::Serialize;
use std::sync::Arc;

/// Ordered damage to one occupant of a reactor blast cell.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BattleReactorBlastHit {
    pub unit: ObjectId,
    pub coordinate: BattleHexCoordinate,
    pub damage: u16,
    pub heat: i32,
    pub arc: BattleHitArc,
    pub impacts: Vec<BattleBlastImpact>,
    pub vehicle_heat: Option<BattleVehicleHeatExposure>,
}

/// Destruction, sensory effects and neighboring material damage from one reactor.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BattleReactorExplosion {
    /// An instability blast caused by dismantling this reactor, completed before its own blast.
    pub section_explosion: Option<Box<BattleReactorExplosion>>,
    pub unit: ObjectId,
    pub map: ObjectId,
    pub hits: Vec<BattleReactorBlastHit>,
    pub blinded: Vec<ObjectId>,
    pub ignited: Vec<BattleHexCoordinate>,
    /// Final unit-owned injury, applied after all radial blast effects.
    pub crew_injury: Option<BattlePilotInjury>,
    pub notices: Vec<BattleNotice>,
    /// Private packet feedback indexed into the complete reactor notice stream.
    pub pilot_notices: Vec<BattlePilotNotice>,
}

/// Detonate a constructed Mech through an authorized host action, including casualty callbacks.
/// Cockpit countdown and engine-critical admission belong to their respective callers.
pub fn reactor_explosion_action(
    scripts: &Scripts,
    config: &Config,
    id: ObjectId,
) -> Result<BattleReactorExplosion> {
    let before = scripts.world.borrow().clone();
    let checkpoint = scripts.effects.checkpoint();
    let result = (|| {
        let report = detonate(
            &mut scripts.world.borrow_mut(),
            id,
            BattleFallRules::configured(config),
            config.battletech.explode_reactor > 1,
        )?;
        super::piloting::publish_ordered_notices(scripts, &report.notices, &report.pilot_notices)?;
        super::evacuation::publish_reactor_consequences(scripts, config, &report)?;
        super::evacuation::publish_new_casualties(scripts, config, &before)?;
        scripts.world.borrow().validate(config)?;
        Ok(report)
    })();
    if result.is_err() {
        *scripts.world.borrow_mut() = before;
        scripts.effects.restore(checkpoint);
    }
    result
}

/// Resolve a single explosion inside the enclosing publication checkpoint.
pub(super) fn detonate(
    world: &mut World,
    id: ObjectId,
    rules: BattleFallRules,
    punch: bool,
) -> Result<BattleReactorExplosion> {
    ensure!(
        world
            .objects
            .get(&id)
            .is_some_and(|o| !o.flags.contains(Flag::Going)),
        "Unit is unavailable"
    );
    let unit = world
        .btech
        .constructed_units()
        .get(&id)
        .context("A Mech reactor is required")?;
    let position = unit.position().context("Reactor must be on a map")?;
    // Head loss is terminal for this action; engine criticals may already have stopped the reactor.
    ensure!(
        unit.sections()[&BattleSection::Head].internal > 0,
        "Reactor has already exploded"
    );
    let point = unit
        .motion()
        .context("Reactor has no battlefield position")?
        .point;
    let origin = BattleHexCoordinate {
        x: i32::from(position.x),
        y: i32::from(position.y),
    };
    let map = world
        .btech
        .maps()
        .get(&position.map)
        .context("Map is unavailable")?;
    ensure!(
        world
            .objects
            .get(&position.map)
            .is_some_and(|o| !o.flags.contains(Flag::Going)),
        "Map is unavailable"
    );
    map.validate()?;
    let source_tile = map.base_hex(i64::from(position.x), i64::from(position.y))?;
    let flash_elevation = f64::from(unit.elevation_level(source_tile)) + 6.0;
    let definition = unit.definition();
    let rating = super::engine::rated_output(definition.tons, definition.max_speed)?;
    let damage = u16::try_from((u32::from(definition.tons) / 5).max(rating / 10))?;
    let heat = i32::try_from((u32::from(definition.tons) / 10).max(rating / 25))?;
    let mut cells = vec![(origin, 1)];
    for x in origin.x - 2..=origin.x + 2 {
        for y in origin.y - 2..=origin.y + 2 {
            let coordinate = BattleHexCoordinate { x, y };
            let distance = origin.distance(coordinate);
            if distance == 0 || distance > 2 || map.base_hex(i64::from(x), i64::from(y)).is_err() {
                continue;
            }
            cells.push((coordinate, distance as u16 + 1));
        }
    }
    let mut report = BattleReactorExplosion {
        section_explosion: None,
        unit: id,
        map: position.map,
        hits: Vec::new(),
        blinded: Vec::new(),
        ignited: Vec::new(),
        crew_injury: None,
        notices: super::broadcast::observer_notices(world, id, "suddenly explodes!"),
        pilot_notices: Vec::new(),
    };
    report.notices.push(BattleNotice {
        unit: id,
        text: "Suddenly you feel great heat overcoming your senses.. you faint.. (and die)".into(),
    });
    for section in [
        BattleSection::CenterTorso,
        BattleSection::LeftTorso,
        BattleSection::RightTorso,
        BattleSection::LeftLeg,
        BattleSection::RightLeg,
        BattleSection::Head,
    ] {
        let unit = Arc::make_mut(&mut world.btech.constructed)
            .get_mut(&id)
            .unwrap();
        let power = unit.power();
        let engine_hits = unit.system_hits(BattleSystem::Engine);
        let amount = unit.sections()[&section].internal;
        let phase = unit.damage_phase(section, amount, BattleDamagePhase::Internal);
        if !phase.destroyed_sections.is_empty()
            && let Some(blast) =
                super::reactor_instability::section_loss(world, id, power, engine_hits, rules)?
        {
            super::piloting::append_feedback(
                &mut report.pilot_notices,
                blast.pilot_notices.iter().cloned(),
                report.notices.len(),
            );
            report.notices.extend(blast.notices.iter().cloned());
            report.section_explosion = Some(Box::new(blast));
        }
    }
    // Raising the event source affects visibility only; retain the wreck's actual position.
    let mut visibility = world.clone();
    let source = Arc::make_mut(&mut visibility.btech.constructed)
        .get_mut(&id)
        .unwrap();
    source.ground_elevation = Some(flash_elevation);
    source.free_fall = None;
    let flashes = super::sensor_flash::scramble(world, id, &visibility);
    report
        .blinded
        .extend(flashes.iter().map(|notice| notice.unit));
    report.notices.extend(flashes);
    for (coordinate, divisor) in cells {
        if damage / divisor == 0 {
            continue;
        }
        hit_cell(
            world,
            &mut report,
            coordinate,
            point,
            (damage / divisor, heat / i32::from(divisor)),
            rules,
            punch,
        )?;
        if coordinate == origin {
            continue;
        }
        if super::blast_damage::ignite_forest(world, position.map, coordinate)? {
            report.ignited.push(coordinate);
        }
    }
    let injury = super::pilot_injury::injure_terminal_crew(world, id, 4)?;
    if let Some(notice) = injury.notice(id) {
        report.notices.push(notice);
    }
    report.crew_injury = Some(injury);
    Ok(report)
}

/// Freeze facing per occupant; rear armor selection persists through one cell's ordered occupants.
fn hit_cell(
    world: &mut World,
    report: &mut BattleReactorExplosion,
    coordinate: BattleHexCoordinate,
    point: BattlePoint,
    (damage, heat): (u16, i32),
    rules: BattleFallRules,
    punch: bool,
) -> Result<()> {
    let tile = world.btech.maps()[&report.map]
        .base_hex(i64::from(coordinate.x), i64::from(coordinate.y))?;
    let ground = if matches!(tile.terrain, Terrain::Water | Terrain::Ice) {
        -i32::from(tile.elevation)
    } else {
        i32::from(tile.elevation)
    };
    let blast = super::blast_damage::BlastCell {
        coordinate,
        origin: point,
        tile,
        lower: ground - 5,
        upper: ground + 3,
    };
    let source = world.btech.constructed_units()[&report.unit]
        .position()
        .unwrap();
    let direct = (coordinate.x, coordinate.y) == (i32::from(source.x), i32::from(source.y));
    let mut rear = false;
    for id in super::map_slots::all_unit_order(world, report.map)? {
        // The disintegrating source is raised above its own blast during resolution.
        if id == report.unit {
            continue;
        }
        let Some(target) = blast.target(world, id, rules)? else {
            continue;
        };
        rear |= target.arc == BattleHitArc::Rear;
        let (observer, cockpit) = if direct {
            (
                "is hit badly by the blast!",
                "[fg=red bold]You bear full brunt of the blast![reset]",
            )
        } else {
            (
                "is hit by the blast!",
                "[fg=yellow bold]You receive some damage from the blast![reset]",
            )
        };
        report
            .notices
            .extend(super::broadcast::observer_notices(world, id, observer));
        report.notices.push(BattleNotice {
            unit: id,
            text: cockpit.into(),
        });
        let effects = super::blast_damage::resolve(
            world,
            id,
            super::blast_damage::BlastDamage {
                damage,
                packet_size: 3,
                table: if punch {
                    BattleHitTable::Punch
                } else {
                    BattleHitTable::Weapon
                },
                arc: target.arc,
                heat,
                character: target.character,
            },
            &mut rear,
            target.rules,
        )?;
        report.hits.push(BattleReactorBlastHit {
            unit: id,
            coordinate,
            damage,
            heat,
            arc: target.arc,
            impacts: effects.impacts,
            vehicle_heat: effects.vehicle_heat,
        });
        super::piloting::append_feedback(
            &mut report.pilot_notices,
            effects.pilot_notices.iter().cloned(),
            report.notices.len(),
        );
        report.notices.extend(effects.notices);
    }
    Ok(())
}
