//! Frequency-matched command mine detonation after an admitted radio transmission.
use super::*;
use crate::{Flag, ObjectId, World};
use anyhow::{Context, Result, ensure};
use serde::Serialize;

/// Applied command detonations in stable field order; the radio caller owns transmission admission.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[must_use = "Publish command mine notices and character consequences in the transmission checkpoint"]
pub struct BattleCommandMineReport {
    pub sender: ObjectId,
    pub map: ObjectId,
    pub frequency: i32,
    pub blasts: Vec<BattleMineBlastReport>,
    pub notices: Vec<BattleNotice>,
    /// Pilot-only messages indexed into the enclosing notice stream.
    pub pilot_notices: Vec<BattlePilotNotice>,
}

/// Detonate matching command fields on the sender's current map without spending radio resources.
/// Matching is map-wide: mine ownership, sender altitude and distance do not restrict reception.
pub fn detonate_command_mines(
    world: &mut World,
    sender: ObjectId,
    frequency: i32,
    rules: BattleFallRules,
) -> Result<BattleCommandMineReport> {
    resolve(world, sender, frequency, rules, false)
}

/// Character-capable detonation within a host transmission checkpoint.
pub(super) fn resolve(
    world: &mut World,
    sender: ObjectId,
    frequency: i32,
    rules: BattleFallRules,
    character: bool,
) -> Result<BattleCommandMineReport> {
    ensure!(
        world
            .objects
            .get(&sender)
            .is_some_and(|object| !object.flags.contains(Flag::Going)),
        "Sender is unavailable"
    );
    let map = super::scanner::scanner_unit(world, sender)
        .context("Sender is not constructed")?
        .position
        .context("Sender is not placed")?
        .map;
    ensure!(
        world
            .objects
            .get(&map)
            .is_some_and(|object| !object.flags.contains(Flag::Going)),
        "Map is unavailable"
    );
    let record = world.btech.maps().get(&map).context("Map is unavailable")?;
    record.validate()?;
    let selected: Vec<_> = record
        .ordered_minefields()
        .filter(|(_, mine)| mine.kind == BattleMineKind::Command && mine.extra == frequency)
        .map(|(&ordinal, &mine)| (ordinal, mine))
        .collect();
    let mut report = BattleCommandMineReport {
        sender,
        map,
        frequency,
        blasts: Vec::new(),
        notices: Vec::new(),
        pilot_notices: Vec::new(),
    };
    if selected.is_empty() {
        return Ok(report);
    }
    let mut candidate = world.clone();
    for (ordinal, mine) in selected {
        // Damage cascades can already have removed another selected field.
        if candidate.btech.maps()[&map].minefields().get(&ordinal) != Some(&mine) {
            continue;
        }
        report.notices.extend(super::mine_event::explosion_notices(
            &candidate,
            map,
            mine.coordinate,
        )?);
        let detonate = if character {
            super::mine_blast::resolve_in_action
        } else {
            super::resolve_mine_blast
        };
        let blast = detonate(&mut candidate, map, ordinal, rules)?;
        super::piloting::append_feedback(
            &mut report.pilot_notices,
            blast.pilot_notices.iter().cloned(),
            report.notices.len(),
        );
        report.notices.extend(blast.notices.iter().cloned());
        report.blasts.push(blast);
    }
    candidate.btech.validate(&candidate)?;
    *world = candidate;
    Ok(report)
}
