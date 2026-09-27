//! Vehicle beacon attachment uses ordinary hit routing and persistent surviving-section effects.
use super::*;
use crate::{ObjectId, World};
use anyhow::Result;
use std::collections::{BTreeMap, BTreeSet};

impl BattleVehicle {
    /// Persistent pod effects on surviving hull faces or the turret.
    pub fn beacons(&self) -> &BTreeMap<BattleVehicleSection, BTreeSet<BattleBeaconKind>> {
        &self.beacons
    }

    /// Whether any surviving section carries a particular attached effect.
    pub fn has_beacon(&self, kind: BattleBeaconKind) -> bool {
        self.beacons.values().any(|kinds| kinds.contains(&kind))
    }
}

/// Resolve a non-damaging pod inside the firing candidate, retaining hit-table mechanical effects.
pub(super) fn attach(
    world: &mut World,
    shooter: ObjectId,
    target: ObjectId,
    shot: super::narc::PodShot,
    rules: BattleVehicleImpactRules,
    hit_arc_mode: i64,
) -> Result<BattleNarcReport<BattleUnitSection>> {
    let mut report = BattleNarcReport {
        kind: shot.kind,
        hit: shot.hit,
        intercepted: shot.intercepted,
        section: None,
        rear: false,
        notices: Vec::new(),
        broadcasts: Vec::new(),
    };
    if !shot.hit || shot.intercepted {
        return Ok(report);
    }
    let arc = super::hit_direction::HitDirection::Direct {
        shooter,
        mode: hit_arc_mode,
    }
    .current(world, target)?;
    let location = super::vehicle_impact::resolve_location(world, target, arc, rules)?;
    report.notices = location.notices;
    report.broadcasts = location.broadcasts;
    report.rear = arc == BattleHitArc::Rear;
    let Some(hit) = location.hit else {
        return Ok(report);
    };
    let vehicle = world.btech.vehicles.get_mut(&target).unwrap();
    if vehicle
        .sections()
        .get(&hit.section)
        .is_none_or(|state| state.internal == 0)
    {
        return Ok(report);
    }
    vehicle
        .beacons
        .entry(hit.section)
        .or_default()
        .insert(shot.kind);
    report.section = Some(BattleUnitSection::Vehicle(hit.section));
    if shot.kind == BattleBeaconKind::Haywire {
        report.notices.push(BattleNotice {
            unit: target,
            text: "Your targetting system goes a bit haywire!".into(),
        });
    }
    if shot.kind == BattleBeaconKind::Ecm {
        report
            .notices
            .extend(super::electronics::refresh_receiver(world, target)?);
    }
    Ok(report)
}
