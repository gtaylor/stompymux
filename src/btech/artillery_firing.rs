//! Configured artillery launches share weapon expenditure and enqueue delayed impacts atomically.
use super::*;
use crate::{Config, Flag, ObjectId, World};
use anyhow::{Context, Result, ensure};
use serde::Serialize;

/// A completed launch attempt; impact effects belong to the persistent queued round.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct BattleArtilleryLaunchReport {
    pub shooter: ObjectId,
    pub map: ObjectId,
    pub coordinate: BattleHexCoordinate,
    pub weapon_index: usize,
    pub aim: BattleArtilleryAim,
    pub roll: u8,
    pub hit: bool,
    pub launched: bool,
    pub jammed: bool,
    pub loader_destroyed: bool,
    pub propellant_roll: Option<u8>,
    pub expenditure: BattleWeaponUse,
    pub misload: Option<BattleLaunchMisload>,
    pub ammunition_warning: Option<String>,
    /// Cocoon opening feedback from the shared launch stage.
    pub launch_notices: Vec<BattleNotice>,
    pub queued_shot: Option<u32>,
}

impl BattleArtilleryLaunchReport {
    /// Immediate shooter notices, excluding delayed impact feedback.
    pub fn notices(&self) -> Vec<BattleNotice> {
        let mut notices = self
            .misload
            .as_ref()
            .map(BattleLaunchMisload::notices)
            .unwrap_or_default();
        notices.extend(self.launch_notices.clone());
        if let Some(text) = &self.ammunition_warning {
            notices.push(BattleNotice {
                unit: self.shooter,
                text: text.clone(),
            });
        }
        notices
    }
}

/// Admit a coordinate launch within the enclosing configured firing transaction.
pub(super) fn resolve_in_action(
    world: &mut World,
    config: &Config,
    shooter: ObjectId,
    pilot: ObjectId,
    index: usize,
    requested: BattleFireTarget,
) -> Result<super::firing::BattleFiringAction> {
    let operator = super::combat_operator::controlled(world, shooter, pilot)?;
    let PreparedArtillery {
        map,
        coordinate,
        origin,
        weapon,
        ammunition,
        distance,
        aim,
    } = prepare(world, config, shooter, pilot, index, requested)?;
    let rules = BattleShotRules::configured(&config.battletech, false);
    let mode = ammunition.artillery_payload()?;
    let character_shooter = world.objects[&shooter].flags.contains(Flag::InCharacter);
    let fall = BattleFallRules {
        vehicle_impact: rules.vehicle_impact,
        stacking: rules.stacking,
        stagger: rules.stagger,
        hit: rules.hit,
        extended_piloting: rules.extended_piloting,
        toughness: character_shooter
            && world
                .btech
                .vehicles()
                .get(&shooter)
                .and_then(|unit| unit.pilot())
                .or_else(|| {
                    world
                        .btech
                        .constructed_units()
                        .get(&shooter)
                        .and_then(|unit| unit.pilot())
                })
                .and_then(|pilot| world.btech.character_values().get(&pilot))
                .is_some_and(|values| super::advantages::enabled(values, "Toughness")),
    };
    let mut candidate = world.clone();
    let super::coordinate_launch::CoordinateLaunch {
        roll,
        hit,
        launched,
        jammed,
        loader_destroyed,
        propellant_roll,
        expenditure,
        misload,
        ammunition_warning,
        launch_notices,
    } = super::coordinate_launch::resolve(
        &mut candidate,
        super::weapon_launch::WeaponLaunchRequest {
            shooter,
            pilot,
            weapon_index: index,
            distance,
            target_number: Some(aim.target_number),
            streak_confused: false,
            glancing: BattleGlancingMode::Disabled,
            fall,
            character_shooter,
        },
        true,
    )?;
    let queued_shot = if launched {
        Some(super::artillery_queue::enqueue_for_source(
            &mut candidate,
            map,
            operator.source,
            BattleArtilleryFlight::new(origin, coordinate, weapon, mode, hit)?,
        )?)
    } else {
        None
    };
    let report = BattleArtilleryLaunchReport {
        shooter,
        map,
        coordinate,
        weapon_index: index,
        aim,
        roll,
        hit,
        launched,
        jammed,
        loader_destroyed,
        propellant_roll,
        expenditure,
        misload,
        ammunition_warning,
        launch_notices,
        queued_shot,
    };
    let explosions = observer_messages(world, shooter, "shudders from an internal explosion!");
    let mut private = Vec::new();
    let messages = if let Some(messages) =
        super::launch_feedback::failure_messages((&report).into(), explosions, &mut private)
    {
        messages
    } else {
        let name = weapon.name().split_once('.').unwrap().1;
        let mut messages = vec![(
            shooter,
            format!(
                "You fire {name} at ({},{}) - BTH: {} Roll: {}.",
                coordinate.x, coordinate.y, aim.target_number, report.roll
            ),
        )];
        messages.extend(observer_messages(
            world,
            shooter,
            &super::artillery_feedback::launch_text(weapon, origin, coordinate)?,
        ));
        messages.extend(
            report
                .notices()
                .into_iter()
                .map(|notice| (notice.unit, notice.text)),
        );
        messages
    };
    candidate.btech.validate_action(&candidate)?;
    *world = candidate;
    Ok(super::firing::BattleFiringAction {
        pilot_notices: private,
        report: report.into(),
        messages,
    })
}

/// Artillery geometry and aim shared by sighting and live launch, without inventory changes.
pub(super) struct PreparedArtillery {
    pub map: ObjectId,
    pub coordinate: BattleHexCoordinate,
    pub origin: BattleHexCoordinate,
    pub weapon: BattleWeapon,
    pub ammunition: BattleAmmunitionMode,
    pub distance: f64,
    pub aim: BattleArtilleryAim,
}

/// Resolve the current observer/coordinate and calculate artillery aim before launch effects.
pub(super) fn prepare(
    world: &World,
    config: &Config,
    shooter: ObjectId,
    pilot: ObjectId,
    index: usize,
    requested: BattleFireTarget,
) -> Result<PreparedArtillery> {
    let operator = super::combat_operator::controlled(world, shooter, pilot)?;
    let vehicle = world.btech.vehicles().contains_key(&shooter);
    ensure!(
        !matches!(requested, BattleFireTarget::Unit { .. }),
        "You can only target hexes with this kind of artillery."
    );
    let explicit_hex = match requested {
        BattleFireTarget::Hex { coordinate } => Some(coordinate),
        _ => None,
    };
    let (position, motion, weapon, ammunition, spotter, mut adjustment) = if vehicle {
        let unit = &world.btech.vehicles()[&shooter];
        ensure!(!unit.is_destroyed(), "Unit is destroyed");
        unit.check_spotter_fire(shooter, index)?;
        (
            unit.position(),
            unit.motion(),
            unit.weapon_readiness(index)?.weapon,
            unit.ammunition_mode(index)?,
            unit.spotter(),
            unit.artillery_adjustment(),
        )
    } else {
        let unit = &world.btech.constructed_units()[&shooter];
        unit.validate()?;
        ensure!(!unit.is_destroyed(), "Unit is destroyed");
        let weapon = unit.weapon_readiness(index)?.weapon;
        unit.check_spotter_fire(shooter, index)?;
        (
            unit.position(),
            unit.motion(),
            weapon,
            unit.ammunition_mode(index)?,
            unit.spotter(),
            unit.artillery_adjustment(),
        )
    };
    let (target_lock, hex_lock) = match operator.source.selection(world) {
        Some(BattleTargetSelection::Unit(lock)) => (Some(lock), None),
        Some(BattleTargetSelection::Hex(lock)) => (None, Some(lock)),
        None => (None, None),
    };
    if let Some(station) = operator.station {
        adjustment = world.btech.gunner_stations()[&station.station].artillery_adjustment;
    }
    let position = position.context("Shooter is not placed")?;
    let motion = motion.context("Shooter is not placed")?;
    let map = position.map;
    ensure!(
        world
            .objects
            .get(&map)
            .is_some_and(|object| !object.flags.contains(Flag::Going)),
        "Map is unavailable"
    );
    ensure!(weapon.is_artillery(), "Weapon is not artillery");
    ensure!(
        explicit_hex.is_some() || target_lock.is_none(),
        "You can only target hexes with this kind of artillery."
    );
    let observer = spotter
        .filter(|_| explicit_hex.is_none())
        .map(|_| super::spotter::active_observer(world, shooter))
        .transpose()?;
    let coordinate = if let Some(hex) = explicit_hex {
        hex
    } else if let Some(id) = observer {
        let selection = super::targeting::selection(world, id);
        ensure!(
            !matches!(selection, Some(BattleTargetSelection::Unit(_))),
            "You can only target hexes with this kind of artillery."
        );
        let coordinate = selection
            .and_then(|selection| match selection {
                BattleTargetSelection::Hex(lock) => Some(lock.hex),
                _ => None,
            })
            .context("Your spotter has no target set!")?;
        ensure!(
            hex_visible(world, id, coordinate)?,
            "That target is not in your spotters line of sight!"
        );
        coordinate
    } else {
        hex_lock.context("Select a hex target for artillery")?.hex
    };
    let record = &world.btech.maps()[&map];
    record.base_hex(i64::from(coordinate.x), i64::from(coordinate.y))?;
    let source_tile = record.base_hex(i64::from(position.x), i64::from(position.y))?;
    let (_, distance) = super::los::unit_hex_los(world, shooter, coordinate)?;
    // Observed artillery uses indirect aim even when the shooter can see the hex.
    let visible =
        observer.is_none() && super::visibility::hex_unblocked(world, shooter, coordinate)?;
    ensure!(
        explicit_hex.is_some() || observer.is_some() || visible || record.flags & 16 == 0,
        "You cannot fire indirect weapons underground!"
    );
    ensure!(
        explicit_hex.is_some()
            || visible
            || config.battletech.idf_requires_spotter == 0
            || spotter.is_some(),
        "That hex target is not in your direct line of sight and you do not have a spotter set!!"
    );
    operator.check_point_arc(world, coordinate.center())?;
    let bearing = motion.point.bearing(coordinate.center())?.unwrap_or(180.0);
    let rules = BattleShotRules::configured(&config.battletech, false);
    ensure!(
        observer.is_some()
            || operator.aim_rules(rules.aim).override_weapon_arcs
            || if vehicle {
                world.btech.vehicles()[&shooter].weapon_bears_on(index, bearing)?
            } else {
                let unit = &world.btech.constructed_units()[&shooter];
                unit.loadout()?.weapons[index].bears_on(
                    unit.chassis(),
                    motion.heading,
                    bearing,
                    unit.facing(),
                )?
            },
        "Target is outside weapon arc"
    );
    let observer = if let Some(id) = observer {
        BattleArtilleryObserver::Spotting(super::skills::unit_spotting_target(world, id)?)
    } else {
        BattleArtilleryObserver::Unassisted
    };
    let submerged = if vehicle {
        world.btech.vehicles()[&shooter].elevation_level(source_tile) < -1
    } else {
        let unit = &world.btech.constructed_units()[&shooter];
        let elevation = unit.elevation_level(source_tile);
        elevation < -1
            || (elevation < 0
                && (unit.posture() == BattlePosture::Prone
                    || unit
                        .chassis()
                        .is_leg(unit.loadout()?.weapons[index].criticals[0].section)))
    };
    let aim = weapon.artillery_aim(BattleArtilleryAimInput {
        distance,
        extended_range: rules.aim.extended_ranges,
        submerged,
        visible,
        gunnery: if let Some(station) = operator.station {
            station.artillery_gunnery_target(world)?
        } else {
            unit_artillery_gunnery_target(world, shooter)?
        },
        observer,
        adjustment,
    })?;
    Ok(PreparedArtillery {
        map,
        coordinate,
        weapon,
        ammunition,
        distance,
        aim,
        origin: BattleHexCoordinate {
            x: i32::from(position.x),
            y: i32::from(position.y),
        },
    })
}
