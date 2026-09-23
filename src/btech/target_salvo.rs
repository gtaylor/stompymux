//! Shared target damage reports, geometry and feedback for every firing unit class.
use super::*;
use crate::{ObjectId, World};
use anyhow::Result;
use serde::Serialize;

/// Target-side consequences reuse each unit class's existing damage resolver.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "kind", content = "report", rename_all = "snake_case")]
pub enum BattleTargetSalvo {
    Mech(BattleSalvoReport),
    Vehicle(BattleVehicleSalvoReport),
    Swarm(super::BattleSwarmReport),
}

impl BattleTargetSalvo {
    /// Actual missile cluster size before defensive interception, used to cap the defensive report.
    pub(super) fn missiles_before_defense(&self) -> Option<u8> {
        match self {
            Self::Mech(report) => report.missiles_before_defense,
            Self::Vehicle(report) => report.missiles_before_defense,
            Self::Swarm(report) => Some(report.launched),
        }
    }
}

impl BattleTargetSalvo {
    /// Inspect Mech anatomy without assuming the target's unit class.
    pub fn as_mech(&self) -> Option<&BattleSalvoReport> {
        match self {
            Self::Mech(report) => Some(report),
            Self::Vehicle(_) | Self::Swarm(_) => None,
        }
    }

    /// Take ownership of Mech-specific consequences.
    pub fn into_mech(self) -> Option<BattleSalvoReport> {
        match self {
            Self::Mech(report) => Some(report),
            Self::Vehicle(_) | Self::Swarm(_) => None,
        }
    }

    /// Assemble private checks at their original positions within damage groups.
    pub(super) fn notices_with_feedback(
        &self,
        shooter: ObjectId,
        target: ObjectId,
        private: &mut Vec<super::BattlePilotNotice>,
    ) -> Vec<BattleNotice> {
        if let Self::Swarm(report) = self {
            super::piloting::append_feedback(private, report.pilot_notices.clone(), 0);
            return report.notices.clone();
        }
        let (mut notices, destroyed) = match self {
            Self::Swarm(_) => unreachable!("handled swarm"),
            Self::Mech(report) => (
                super::fire_feedback::mech_salvo_notices(report, private),
                report.groups.iter().any(|group| group.impact.destroyed),
            ),
            Self::Vehicle(report) => (
                super::fire_feedback::vehicle_salvo_notices(report, private),
                report.inferno.as_ref().is_some_and(|inferno| {
                    inferno.explosion.is_some()
                        || inferno.damage.iter().any(|damage| damage.unit_destroyed)
                }) || report.groups.iter().any(|group| {
                    group
                        .impact
                        .damage
                        .as_ref()
                        .is_some_and(|damage| damage.unit_destroyed)
                }),
            ),
        };
        notices.extend(super::fire_feedback::destruction_notices(
            shooter, target, destroyed,
        ));
        notices
    }

    /// Raw vehicle location and critical broadcasts, before the host applies visibility.
    pub(super) fn broadcasts(&self) -> Vec<BattleNotice> {
        match self {
            Self::Mech(_) => Vec::new(),
            Self::Swarm(report) => report.broadcasts.clone(),
            Self::Vehicle(report) => report
                .groups
                .iter()
                .flat_map(|group| group.impact.broadcasts.iter().cloned())
                .chain(
                    report
                        .inferno
                        .iter()
                        .flat_map(|inferno| inferno.broadcasts.iter().cloned()),
                )
                .collect(),
        }
    }
}

/// Derive vehicle hit direction and guidance from the same world used by an admitted shot.
/// Reuse vehicle geometry and damage for ordinary shots and surviving Swarm flights.
pub(super) fn resolve_vehicle_target(
    world: &mut World,
    shooter: ObjectId,
    target: ObjectId,
    request: BattleVehicleSalvoRequest,
    hit_arc_mode: i64,
    rules: BattleVehicleImpactRules,
    context: super::vehicle_salvo::SalvoContext<'_>,
) -> Result<BattleTargetSalvo> {
    resolve_vehicle_target_mode(
        world,
        shooter,
        target,
        request,
        hit_arc_mode,
        rules,
        context,
        super::shot_transaction::EffectMode::Atomic,
    )
}

/// Direct damage shares the enclosing shot's rollback boundary.
pub(super) fn resolve_vehicle_target_in_candidate(
    world: &mut super::shot_transaction::ShotCandidate,
    shooter: ObjectId,
    target: ObjectId,
    request: BattleVehicleSalvoRequest,
    hit_arc_mode: i64,
    rules: BattleVehicleImpactRules,
    context: super::vehicle_salvo::SalvoContext<'_>,
) -> Result<BattleTargetSalvo> {
    let mode = world.mode();
    resolve_vehicle_target_mode(
        world,
        shooter,
        target,
        request,
        hit_arc_mode,
        rules,
        context,
        mode,
    )
}

fn resolve_vehicle_target_mode(
    world: &mut World,
    shooter: ObjectId,
    target: ObjectId,
    mut request: BattleVehicleSalvoRequest,
    hit_arc_mode: i64,
    rules: BattleVehicleImpactRules,
    context: super::vehicle_salvo::SalvoContext<'_>,
    mode: super::shot_transaction::EffectMode,
) -> Result<BattleTargetSalvo> {
    let direction = super::hit_direction::HitDirection::Direct {
        shooter,
        mode: hit_arc_mode,
    };
    direction.current(world, target)?;
    let source = electronic_field(world, shooter)?;
    let recipient = electronic_field(world, target)?;
    request.guidance_blocked =
        source.blocks_outgoing_guidance() || recipient.blocks_incoming_guidance();
    request.angel_blocked = source.angel_disturbed || recipient.angel_protected;
    super::vehicle_salvo::resolve_with_context_mode(
        world, target, direction, request, rules, context, mode,
    )
    .map(BattleTargetSalvo::Vehicle)
}
