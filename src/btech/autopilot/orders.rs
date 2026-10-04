//! Durable ground-autopilot orders.
//!
//! Orders deliberately contain intent rather than a path or a movement command.  A
//! later tactical director can therefore replace the planner without changing the
//! Lua or persistence contract.

use crate::{ObjectId, btech::Position};
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};

/// A preferred engagement interval, measured in hexes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct AutopilotRangeBand {
    pub minimum: u16,
    pub maximum: u16,
}

impl AutopilotRangeBand {
    pub fn validate(self) -> Result<()> {
        ensure!(
            self.minimum <= self.maximum,
            "Autopilot range minimum exceeds maximum"
        );
        Ok(())
    }
}

/// Unit-level intent accepted by the ground autopilot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum AutopilotOrder {
    Move {
        destination: Position,
        #[serde(default)]
        arrival_radius: u16,
    },
    Hold,
    Follow {
        target: ObjectId,
        #[serde(default = "default_follow_separation")]
        separation: u16,
    },
    Patrol {
        waypoints: Vec<Position>,
    },
    Attack {
        target: ObjectId,
        #[serde(default)]
        range: Option<AutopilotRangeBand>,
    },
    AttackMove {
        destination: Position,
        #[serde(default)]
        arrival_radius: u16,
    },
}

const fn default_follow_separation() -> u16 {
    2
}

impl AutopilotOrder {
    /// Validate structural limits that do not require looking up a unit or map.
    pub fn validate(&self) -> Result<()> {
        let valid_position = |position: &Position| {
            ensure!(
                position.map.0 >= 0,
                "Autopilot order references an invalid map"
            );
            Ok(())
        };
        match self {
            Self::Move {
                destination,
                arrival_radius: _,
            }
            | Self::AttackMove {
                destination,
                arrival_radius: _,
            } => valid_position(destination),
            Self::Hold => Ok(()),
            Self::Follow { target, separation } => {
                ensure!(target.0 >= 0, "Autopilot follow references an invalid unit");
                ensure!(
                    *separation > 0,
                    "Autopilot follow separation must be positive"
                );
                Ok(())
            }
            Self::Patrol { waypoints } => {
                ensure!(
                    (2..=MAX_PATROL_WAYPOINTS).contains(&waypoints.len()),
                    "Autopilot patrols require two to {MAX_PATROL_WAYPOINTS} waypoints"
                );
                for waypoint in waypoints {
                    valid_position(waypoint)?;
                }
                ensure!(
                    waypoints
                        .iter()
                        .map(|waypoint| waypoint.map)
                        .collect::<std::collections::BTreeSet<_>>()
                        .len()
                        <= 1,
                    "Autopilot patrol waypoints must share a map"
                );
                Ok(())
            }
            Self::Attack { target, range } => {
                ensure!(target.0 >= 0, "Autopilot attack references an invalid unit");
                if let Some(range) = range {
                    range.validate()?;
                }
                Ok(())
            }
        }
    }

    /// Stable numeric identifier used by the Lua constants package.
    pub const fn kind_code(&self) -> u8 {
        match self {
            Self::Move { .. } => 0,
            Self::Hold => 1,
            Self::Follow { .. } => 2,
            Self::Patrol { .. } => 3,
            Self::Attack { .. } => 4,
            Self::AttackMove { .. } => 5,
        }
    }
}

/// Lifecycle state for a submitted order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum AutopilotOrderState {
    #[default]
    Queued,
    Running,
    Succeeded,
    Failed,
    Canceled,
}

/// Small, durable execution cursor.  Paths and search frontiers remain transient.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AutopilotOrderProgress {
    #[serde(default)]
    pub waypoint_index: u16,
    #[serde(default)]
    pub recovery_attempts: u8,
    #[serde(default)]
    pub stagnant_ticks: u16,
    #[serde(default)]
    pub attack_move_origin: Option<Position>,
    /// A contact already reached during this attack-move, so resuming the
    /// destination does not immediately restart the same diversion.
    #[serde(default)]
    pub attack_move_suppressed_target: Option<ObjectId>,
}

/// Order identity, intent, lifecycle, and durable execution cursor.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AutopilotOrderRecord {
    pub id: u64,
    pub order: AutopilotOrder,
    #[serde(default)]
    pub state: AutopilotOrderState,
    #[serde(default)]
    pub progress: AutopilotOrderProgress,
}

impl AutopilotOrderRecord {
    pub(crate) fn queued(id: u64, order: AutopilotOrder) -> Self {
        Self {
            id,
            order,
            state: AutopilotOrderState::Queued,
            progress: AutopilotOrderProgress::default(),
        }
    }

    pub(crate) fn validate(&self, active: bool) -> Result<()> {
        ensure!(self.id > 0, "Autopilot order ID must be positive");
        self.order.validate()?;
        if active {
            ensure!(
                matches!(
                    self.state,
                    AutopilotOrderState::Running | AutopilotOrderState::Failed
                ),
                "Active autopilot order has an invalid state"
            );
        } else {
            ensure!(
                self.state == AutopilotOrderState::Queued,
                "Queued autopilot order has an invalid state"
            );
        }
        Ok(())
    }
}

pub const MAX_QUEUED_ORDERS: usize = 64;
pub const MAX_PATROL_WAYPOINTS: usize = 64;

/// Validate live references through the same admission path for Lua and tactical callers.
/// Enemy validation uses acquired observations and never reveals hidden target state.
pub fn validate_for_unit(
    world: &crate::World,
    unit: ObjectId,
    order: &AutopilotOrder,
) -> Result<()> {
    order.validate()?;
    let own = super::super::scanner::scanner_unit(world, unit)
        .ok_or_else(|| anyhow::anyhow!("Unit is unavailable"))?;
    let position = own
        .position
        .ok_or_else(|| anyhow::anyhow!("Unit is not on a battlefield"))?;
    let destination = |destination: &Position| -> Result<()> {
        ensure!(
            destination.map == position.map,
            "Order destination must be on the unit's map"
        );
        world
            .btech
            .maps()
            .get(&destination.map)
            .ok_or_else(|| anyhow::anyhow!("Map is unavailable"))?
            .hex(i64::from(destination.x), i64::from(destination.y))?;
        Ok(())
    };
    match order {
        AutopilotOrder::Move {
            destination: target,
            ..
        }
        | AutopilotOrder::AttackMove {
            destination: target,
            ..
        } => destination(target)?,
        AutopilotOrder::Patrol { waypoints } => {
            for waypoint in waypoints {
                destination(waypoint)?;
            }
        }
        AutopilotOrder::Follow { target, .. } => {
            let ally = super::super::scanner::scanner_unit(world, *target)
                .ok_or_else(|| anyhow::anyhow!("Follow target is unavailable"))?;
            ensure!(
                *target != unit
                    && own.signature.team == ally.signature.team
                    && ally.position.is_some_and(|p| p.map == position.map),
                "Follow requires another friendly unit on the same map"
            );
        }
        AutopilotOrder::Attack { target, .. } => {
            let observed =
                super::observations::observe(world, unit, world.btech.simulation_time())?;
            ensure!(
                observed
                    .contacts
                    .iter()
                    .any(|contact| contact.unit == *target
                        && contact.identified
                        && !contact.friendly
                        && !contact.known_destroyed),
                "Attack requires a hostile contact held by the unit or its C3/C3i network"
            );
        }
        AutopilotOrder::Hold => {}
    }
    Ok(())
}
