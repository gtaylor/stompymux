//! Minefield recognition and combined hex reports with shared transactional perception and output.
use super::{BattleBuildingScan, BattleChannelMessage, BattleHexCoordinate};
use crate::{Config, ObjectId, Scripts, World};
use anyhow::Result;
use serde::Serialize;

/// Mine recognition discloses presence only, never field type, strength, owner or trigger settings.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BattleMineScan {
    /// Whether the range and perception checks recognized mines.
    pub found: bool,
    /// Recognition or nondisclosure text, without mine configuration.
    pub text: String,
    /// Accepted perception XP diagnostics.
    pub experience_messages: Vec<BattleChannelMessage>,
}

/// Ordered structure and minefield results from a full hex scan.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BattleHexScan {
    /// Building phase, resolved first.
    pub building: BattleBuildingScan,
    /// Mine recognition phase, resolved second.
    pub mines: BattleMineScan,
}

/// Recognize mines at their authored coordinate, using a range gate followed by perception.
/// Empty coordinates spend no dice; occupied coordinates draw an inclusive 2..9
/// gate even when no in-character perception attempt is eligible. All state is atomic.
pub fn scan_mines(
    world: &mut World,
    observer: ObjectId,
    pilot: ObjectId,
    coordinate: BattleHexCoordinate,
    now: i64,
) -> Result<BattleMineScan> {
    scan_with_range(world, observer, pilot, coordinate, now, false)
}

/// Mine recognition for selected coordinates may bypass hardware distance for observers.
fn scan_with_range(
    world: &mut World,
    observer: ObjectId,
    pilot: ObjectId,
    coordinate: BattleHexCoordinate,
    now: i64,
    observer_range: bool,
) -> Result<BattleMineScan> {
    let observer = super::combat_operator::for_owner(world, observer, pilot)?
        .source
        .unit;
    let mut candidate = world.clone();
    let map =
        super::scan::check_coordinate(&candidate, observer, pilot, coordinate, observer_range)?;
    let mut report = BattleMineScan {
        found: false,
        text: "You see nothing else of interest in the hex, either.".into(),
        experience_messages: Vec::new(),
    };
    if !candidate.btech.maps()[&map].mine_coverage(coordinate)?
        || !candidate.btech.maps()[&map]
            .minefields()
            .values()
            .any(|mine| mine.coordinate == coordinate)
    {
        return Ok(report);
    }
    let (_, range) = super::los::unit_hex_los(&candidate, observer, coordinate)?;
    let dice = if candidate.btech.vehicles().contains_key(&observer) {
        &mut candidate
            .btech
            .vehicles
            .get_mut(&observer)
            .expect("checked vehicle scanner")
            .dice
    } else {
        &mut candidate
            .btech
            .constructed
            .get_mut(&observer)
            .expect("checked Mech scanner")
            .dice
    };
    let gate = dice.die(8)? + 1;
    if f64::from(gate) >= range.trunc() {
        let (success, message) =
            super::perception_check::attempt(&mut candidate, observer, pilot, 0, now)?;
        if success {
            report.found = true;
            report.text = "Small bomblets litter the hex, interesting... You vaguely recall them from some class or other.".into();
            report.experience_messages.extend(message);
        }
    }
    *world = candidate;
    Ok(report)
}

/// Scan structures then mines and publish both phases in one rollback boundary.
/// Failed mine recognition is private to the pilot; successful recognition reaches the cockpit.
pub fn scan_hex_action(
    scripts: &Scripts,
    config: &Config,
    observer: ObjectId,
    pilot: ObjectId,
    coordinate: BattleHexCoordinate,
) -> Result<BattleHexScan> {
    action_with_range(scripts, config, observer, pilot, coordinate, false)
}

/// Publish both phases with one selected-target observer distance policy.
pub(super) fn action_with_range(
    scripts: &Scripts,
    config: &Config,
    observer: ObjectId,
    pilot: ObjectId,
    coordinate: BattleHexCoordinate,
    observer_range: bool,
) -> Result<BattleHexScan> {
    let source = super::combat_operator::for_owner(&scripts.world(), observer, pilot)?.source;
    scripts.atomic(|_| {
        let now = crate::clock::wall_time();
        let building = super::scan_building::scan_with_range(
            &mut scripts.world.borrow_mut(),
            observer,
            pilot,
            coordinate,
            now,
            observer_range,
        )?;
        super::channels::publish(scripts, config, &building.experience_messages)?;
        super::notify_unit_text(scripts, source.unit, &building.text)?;
        let mines = scan_with_range(
            &mut scripts.world.borrow_mut(),
            observer,
            pilot,
            coordinate,
            now,
            observer_range,
        )?;
        super::channels::publish(scripts, config, &mines.experience_messages)?;
        let recipient = if mines.found {
            super::BattleMessageTarget::Unit(source.unit)
        } else {
            super::BattleMessageTarget::Player(pilot)
        };
        super::notify_message(scripts, recipient, &mines.text)?;
        Ok(BattleHexScan { building, mines })
    })
}
