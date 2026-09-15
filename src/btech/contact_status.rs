//! Five-column visible contact condition summary with per-column precedence.
use super::{BattleBeaconKind, BattleElectronicMode, BattlePosture, BattlePower};
use crate::{ObjectId, World};
use anyhow::{Context, Result};

/// Call only after acquired visibility is established; blocked terrain hides condition details.
pub(super) fn contact_status(
    world: &World,
    observer: ObjectId,
    target: ObjectId,
) -> Result<String> {
    if !super::visibility::unit_unblocked(world, observer, target)? {
        return Ok("     ".into());
    }
    known_status(world, observer, target)
}

/// Condition columns after the caller has established a clear direct or network sighting.
pub(super) fn known_status(world: &World, observer: ObjectId, target: ObjectId) -> Result<String> {
    if let Some(vehicle) = world.btech.vehicles().get(&target) {
        let condition = if vehicle.is_destroyed() {
            'D'
        } else if vehicle.searchlight().on && !vehicle.searchlight().destroyed {
            'L'
        } else if super::unit_illuminated(world, target) {
            'l'
        } else {
            ' '
        };
        let power = match vehicle.power() {
            BattlePower::Off => 'S',
            BattlePower::Starting { .. } => 's',
            BattlePower::Running if vehicle.inferno_remaining() > 0 => 'I',
            BattlePower::Running if !vehicle.burning_sections().is_empty() => 'B',
            BattlePower::Running => ' ',
        };
        return Ok([' ', condition, ' ', power, ' '].into_iter().collect());
    }
    let unit = &world.btech.constructed_units()[&target];
    let observer =
        super::scanner::scanner_unit(world, observer).context("Observer is unavailable")?;
    let first = if unit.carried_club().is_some() {
        'C'
    } else {
        ' '
    };
    let second = if unit.is_destroyed() {
        'D'
    } else if unit.searchlight().on && !unit.searchlight().destroyed {
        'L'
    } else if super::unit_illuminated(world, target) {
        'l'
    } else {
        ' '
    };
    let third = if unit.flight().is_some() {
        'J'
    } else if unit.posture() == BattlePosture::Prone {
        'F'
    } else if unit.stand_timer().is_some() {
        'f'
    } else if unit.hull_down().pending.is_some() {
        'h'
    } else if unit.hull_down().active {
        'H'
    } else {
        ' '
    };
    let fourth = match unit.power() {
        BattlePower::Off => 'S',
        BattlePower::Starting { .. } => 's',
        BattlePower::Running if unit.heat().excess != 0.0 => '+',
        BattlePower::Running if unit.inferno_remaining() > 0 => 'I',
        BattlePower::Running => ' ',
    };
    let electronics = unit.electronics();
    let modes = [electronics.guardian, electronics.angel];
    let fifth =
        if unit.has_beacon(BattleBeaconKind::Narc) || unit.has_beacon(BattleBeaconKind::Homing) {
            if unit.sensor_signature().team == observer.signature.team {
                'n'
            } else {
                'N'
            }
        } else if modes.contains(&BattleElectronicMode::Eccm) {
            'P'
        } else if modes.contains(&BattleElectronicMode::Ecm) {
            'E'
        } else if electronics.field.protected || electronics.field.angel_protected {
            'p'
        } else if electronics.field.disturbed || electronics.field.angel_disturbed {
            'e'
        } else {
            ' '
        };
    Ok([first, second, third, fourth, fifth].into_iter().collect())
}
