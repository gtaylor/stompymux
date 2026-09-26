//! Ordered mine activation, shared by physical events and host publication.
use super::*;
use crate::{ObjectId, World};
use anyhow::{Context, Result};
use serde::Serialize;

/// Applied mine effects and queued callbacks for one physical event.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[must_use = "Publish mine notices, character consequences and callbacks in the enclosing action"]
pub struct BattleMineEventReport {
    pub unit: ObjectId,
    pub reason: BattleMineTriggerReason,
    pub blasts: Vec<BattleMineBlastReport>,
    /// Each selected scripted field queues one callback on the triggering unit.
    pub triggers: usize,
    pub notices: Vec<BattleNotice>,
    /// Pilot-only messages indexed into the enclosing notice stream.
    pub pilot_notices: Vec<BattlePilotNotice>,
}

/// Resolve a physical mine event; callers own movement admission and publication.
pub fn activate_mines(
    world: &mut World,
    unit: ObjectId,
    reason: BattleMineTriggerReason,
    rules: BattleFallRules,
) -> Result<BattleMineEventReport> {
    resolve(world, unit, reason, rules, false)
}

/// Character-capable activation inside a host transaction.
pub(super) fn resolve(
    world: &mut World,
    unit: ObjectId,
    reason: BattleMineTriggerReason,
    rules: BattleFallRules,
    character: bool,
) -> Result<BattleMineEventReport> {
    let position = super::scanner::scanner_unit(world, unit)
        .context("Unit is not constructed")?
        .position
        .context("Unit is not placed")?;
    let selected = mine_activations(world, unit, reason)?;
    let mut report = BattleMineEventReport {
        unit,
        reason,
        blasts: Vec::new(),
        triggers: 0,
        notices: Vec::new(),
        pilot_notices: Vec::new(),
    };
    if selected.is_empty() {
        return Ok(report);
    }
    world.attempt(|world| {
        for activation in selected {
            // Earlier explosions can remove colocated fields from the activation snapshot.
            if world.btech.maps()[&position.map]
                .minefields()
                .get(&activation.ordinal)
                != Some(&activation.mine)
            {
                continue;
            }
            match activation.response {
                BattleMineResponse::Spotted => report.notices.push(BattleNotice {
                    unit,
                    text: "You spot small bomblets lying on the ground here..".into(),
                }),
                BattleMineResponse::Trigger => report.triggers += 1,
                BattleMineResponse::Explode => {
                    let (cockpit, observed) = if reason == BattleMineTriggerReason::Step {
                        (
                            format!(
                                "As you move to {},{}, you trigger a mine!",
                                position.x, position.y
                            ),
                            format!(
                                "moves to {},{}, and triggers a mine!",
                                position.x, position.y
                            ),
                        )
                    } else {
                        ("You trigger a mine!".into(), "triggers a mine!".into())
                    };
                    report
                        .notices
                        .extend(super::broadcast::observer_notices(world, unit, &observed));
                    report.notices.push(BattleNotice {
                        unit,
                        text: cockpit,
                    });
                    if activation.mine.kind == BattleMineKind::Vibra
                        && activation.mine.coordinate
                            != (BattleHexCoordinate {
                                x: i32::from(position.x),
                                y: i32::from(position.y),
                            })
                    {
                        report.notices.extend(explosion_notices(
                            world,
                            position.map,
                            activation.mine.coordinate,
                        )?);
                    }
                    let blast = if character {
                        super::mine_blast::resolve_in_action
                    } else {
                        super::resolve_mine_blast
                    };
                    let blast = blast(world, position.map, activation.ordinal, rules)?;
                    super::piloting::append_feedback(
                        &mut report.pilot_notices,
                        blast.pilot_notices.iter().cloned(),
                        report.notices.len(),
                    );
                    report.notices.extend(blast.notices.iter().cloned());
                    report.blasts.push(blast);
                }
            }
        }
        world.btech.validate_action(world)?;
        Ok(report)
    })
}

/// Visible remote detonations name the affected hex without disclosing unseen units.
pub(super) fn explosion_notices(
    world: &World,
    map: ObjectId,
    coordinate: BattleHexCoordinate,
) -> Result<Vec<BattleNotice>> {
    super::broadcast::hex_notices(world, map, coordinate, true, |location| {
        format!("A mine explodes in {location}!")
    })
}
