//! Persistence for the Rust autopilot controller state.
//!
//! Each controller is one typed row in `btech_autopilot_controllers`, with child rows
//! for its orders, patrol waypoints and feedback history. Saves compare against the
//! stored rows, so recording feedback inserts one row and advancing an order rewrites
//! only its changed progress columns.

use super::write::{Cell, Fields, sync_rows};
use crate::btech::BattlePosition;
use crate::btech::autopilot::{
    AutopilotConfig, AutopilotController, AutopilotFeedback, AutopilotFeedbackEvent,
    AutopilotFireMode, AutopilotOrder, AutopilotOrderProgress, AutopilotOrderRecord,
    AutopilotOrderState, AutopilotRangeBand, AutopilotReason, AutopilotState,
};
use crate::{BtechState, ObjectId};
use anyhow::{Context, Result, bail, ensure};
use sqlx::{Row, SqliteConnection, sqlite::SqliteRow};
use std::collections::{BTreeMap, BTreeSet};

/// Controller table; its children are listed in [`TABLES`].
const CONTROLLERS: &str = "btech_autopilot_controllers";

/// Order table, one row per active or queued order.
const ORDERS: &str = "btech_autopilot_controller_orders";

/// Patrol waypoint table, one row per waypoint of a patrol order.
const WAYPOINTS: &str = "btech_autopilot_controller_waypoints";

/// Feedback history table, one row per retained record.
const FEEDBACK: &str = "btech_autopilot_controller_feedback";

/// Every table holding controller state, children before their parents.
const TABLES: [&str; 4] = [WAYPOINTS, FEEDBACK, ORDERS, CONTROLLERS];

/// Stored controller columns besides the unit.
const CONTROLLER_COLUMNS: &[&str] = &[
    "speed_percent",
    "fire_mode",
    "heat_ceiling",
    "range_minimum",
    "range_maximum",
    "state",
    "blocking_reason",
    "revision",
    "next_order_id",
    "next_feedback_sequence",
];

/// Stored order columns besides the unit and order ID.
const ORDER_COLUMNS: &[&str] = &[
    "queue_position",
    "kind",
    "state",
    "destination_map",
    "destination_x",
    "destination_y",
    "arrival_radius",
    "target_dbref",
    "separation",
    "range_minimum",
    "range_maximum",
    "waypoint_index",
    "recovery_attempts",
    "stagnant_ticks",
    "origin_map",
    "origin_x",
    "origin_y",
    "suppressed_target",
];

/// Stored waypoint columns besides the unit, order ID and position.
const WAYPOINT_COLUMNS: &[&str] = &["map_dbref", "x", "y"];

/// Stored feedback columns besides the unit and sequence number.
const FEEDBACK_COLUMNS: &[&str] = &["simulation_time", "order_id", "event", "reason"];

/// Define a stored integer code for each variant of a field-less enum, in both
/// directions. The encoder's match is exhaustive, so a new variant fails to compile
/// until it is given a code.
macro_rules! codes {
    ($ty:ident, $encode:ident, $decode:ident { $($variant:ident = $code:literal),+ $(,)? }) => {
        /// Stored code for a value.
        fn $encode(value: $ty) -> i64 {
            match value {
                $($ty::$variant => $code),+
            }
        }

        /// Value for a stored code.
        fn $decode(code: i64) -> Result<$ty> {
            Ok(match code {
                $($code => $ty::$variant,)+
                other => bail!("Unknown {} code {other}", stringify!($ty)),
            })
        }
    };
}

codes!(AutopilotFireMode, fire_mode_code, fire_mode_from_code {
    Hold = 0,
    AssignedTarget = 1,
    Opportunistic = 2,
});

codes!(AutopilotState, state_code, state_from_code {
    Paused = 0,
    Idle = 1,
    Executing = 2,
    Blocked = 3,
});

codes!(AutopilotReason, reason_code, reason_from_code {
    ManualTakeover = 0,
    ContactLost = 1,
    Stuck = 2,
    Unreachable = 3,
    Invalidated = 4,
    ResourceLimit = 5,
    Congested = 6,
    InvalidTarget = 7,
    UnitUnavailable = 8,
    MapChanged = 9,
    Unsupported = 10,
    StaleRevision = 11,
});

codes!(AutopilotFeedbackEvent, event_code, event_from_code {
    Configured = 0,
    Paused = 1,
    Resumed = 2,
    ManualTakeover = 3,
    OrderQueued = 4,
    OrderStarted = 5,
    OrderSucceeded = 6,
    OrderFailed = 7,
    OrderCanceled = 8,
    Blocked = 9,
});

codes!(AutopilotOrderState, order_state_code, order_state_from_code {
    Queued = 0,
    Running = 1,
    Succeeded = 2,
    Failed = 3,
    Canceled = 4,
});

/// Store an unsigned counter, refusing values beyond SQLite's integer range.
fn unsigned(value: u64) -> Result<Cell> {
    Ok(Cell::Integer(
        i64::try_from(value).context("Autopilot counter exceeds storage range")?,
    ))
}

/// Store an optional integer as NULL when absent.
fn optional(value: Option<i64>) -> Cell {
    value.map_or(Cell::Null, Cell::Integer)
}

/// Read a column that must fit the target integer type.
fn get<T: TryFrom<i64>>(row: &SqliteRow, column: &str) -> Result<T> {
    let value: i64 = row.try_get(column)?;
    T::try_from(value)
        .ok()
        .with_context(|| format!("Autopilot column {column} is out of range: {value}"))
}

/// Read a nullable column that must fit the target integer type.
fn get_optional<T: TryFrom<i64>>(row: &SqliteRow, column: &str) -> Result<Option<T>> {
    row.try_get::<Option<i64>, _>(column)?
        .map(|value| {
            T::try_from(value)
                .ok()
                .with_context(|| format!("Autopilot column {column} is out of range: {value}"))
        })
        .transpose()
}

/// Read a range band stored as a nullable minimum and maximum pair.
fn get_range(row: &SqliteRow) -> Result<Option<AutopilotRangeBand>> {
    Ok(
        match (
            get_optional(row, "range_minimum")?,
            get_optional(row, "range_maximum")?,
        ) {
            (Some(minimum), Some(maximum)) => Some(AutopilotRangeBand { minimum, maximum }),
            (None, None) => None,
            _ => bail!("Autopilot range band is incomplete"),
        },
    )
}

/// Read a position stored as nullable map, x and y columns with a shared prefix.
fn get_position(row: &SqliteRow, prefix: &str) -> Result<Option<BattlePosition>> {
    let map: Option<i64> = row.try_get(format!("{prefix}_map").as_str())?;
    let Some(map) = map else {
        return Ok(None);
    };
    Ok(Some(BattlePosition {
        map: ObjectId(map),
        x: get(row, &format!("{prefix}_x"))?,
        y: get(row, &format!("{prefix}_y"))?,
    }))
}

/// Owned columns for the controller row itself.
fn encode_controller(controller: &AutopilotController) -> Result<Fields> {
    let config = &controller.config;
    let range = config.preferred_range;
    Ok(Fields::from([
        (
            "speed_percent",
            Cell::Integer(i64::from(config.speed_percent)),
        ),
        ("fire_mode", Cell::Integer(fire_mode_code(config.fire_mode))),
        (
            "heat_ceiling",
            Cell::Integer(i64::from(config.heat_ceiling)),
        ),
        (
            "range_minimum",
            optional(range.map(|range| i64::from(range.minimum))),
        ),
        (
            "range_maximum",
            optional(range.map(|range| i64::from(range.maximum))),
        ),
        ("state", Cell::Integer(state_code(controller.state))),
        (
            "blocking_reason",
            optional(controller.blocking_reason.map(reason_code)),
        ),
        ("revision", unsigned(controller.revision)?),
        ("next_order_id", unsigned(controller.next_order_id)?),
        (
            "next_feedback_sequence",
            unsigned(controller.next_feedback_sequence)?,
        ),
    ]))
}

/// Owned columns for one order; `queue_position` is `None` for the active order.
fn encode_order(record: &AutopilotOrderRecord, queue_position: Option<usize>) -> Fields {
    let mut destination = None;
    let mut arrival_radius = None;
    let mut target = None;
    let mut separation = None;
    let mut range = None;
    match &record.order {
        AutopilotOrder::Move {
            destination: to,
            arrival_radius: radius,
        }
        | AutopilotOrder::AttackMove {
            destination: to,
            arrival_radius: radius,
        } => {
            destination = Some(*to);
            arrival_radius = Some(i64::from(*radius));
        }
        AutopilotOrder::Hold | AutopilotOrder::Patrol { .. } => {}
        AutopilotOrder::Follow {
            target: unit,
            separation: distance,
        } => {
            target = Some(unit.0);
            separation = Some(i64::from(*distance));
        }
        AutopilotOrder::Attack {
            target: unit,
            range: band,
        } => {
            target = Some(unit.0);
            range = *band;
        }
    }
    let progress = &record.progress;
    let origin = progress.attack_move_origin;
    Fields::from([
        (
            "queue_position",
            optional(queue_position.map(|position| position as i64)),
        ),
        ("kind", Cell::Integer(i64::from(record.order.kind_code()))),
        ("state", Cell::Integer(order_state_code(record.state))),
        ("destination_map", optional(destination.map(|to| to.map.0))),
        (
            "destination_x",
            optional(destination.map(|to| i64::from(to.x))),
        ),
        (
            "destination_y",
            optional(destination.map(|to| i64::from(to.y))),
        ),
        ("arrival_radius", optional(arrival_radius)),
        ("target_dbref", optional(target)),
        ("separation", optional(separation)),
        (
            "range_minimum",
            optional(range.map(|band| i64::from(band.minimum))),
        ),
        (
            "range_maximum",
            optional(range.map(|band| i64::from(band.maximum))),
        ),
        (
            "waypoint_index",
            Cell::Integer(i64::from(progress.waypoint_index)),
        ),
        (
            "recovery_attempts",
            Cell::Integer(i64::from(progress.recovery_attempts)),
        ),
        (
            "stagnant_ticks",
            Cell::Integer(i64::from(progress.stagnant_ticks)),
        ),
        ("origin_map", optional(origin.map(|at| at.map.0))),
        ("origin_x", optional(origin.map(|at| i64::from(at.x)))),
        ("origin_y", optional(origin.map(|at| i64::from(at.y)))),
        (
            "suppressed_target",
            optional(progress.attack_move_suppressed_target.map(|unit| unit.0)),
        ),
    ])
}

/// Owned columns for one feedback record.
fn encode_feedback(record: &AutopilotFeedback) -> Result<Fields> {
    Ok(Fields::from([
        ("simulation_time", Cell::Integer(record.simulation_time)),
        (
            "order_id",
            record.order_id.map_or(Ok(Cell::Null), unsigned)?,
        ),
        ("event", Cell::Integer(event_code(record.event))),
        ("reason", optional(record.reason.map(reason_code))),
    ]))
}

/// Rebuild one order from its row and its patrol waypoints.
fn decode_order(row: &SqliteRow, waypoints: Vec<BattlePosition>) -> Result<AutopilotOrderRecord> {
    let destination =
        || get_position(row, "destination")?.context("Autopilot order lacks a destination");
    let target = || -> Result<ObjectId> {
        Ok(ObjectId(
            get_optional::<i64>(row, "target_dbref")?.context("Autopilot order lacks a target")?,
        ))
    };
    let arrival_radius = || -> Result<u16> {
        get_optional(row, "arrival_radius")?.context("Autopilot order lacks an arrival radius")
    };
    let kind: i64 = row.try_get("kind")?;
    ensure!(
        kind == 3 || waypoints.is_empty(),
        "Only patrol orders have waypoints"
    );
    let order = match kind {
        0 => AutopilotOrder::Move {
            destination: destination()?,
            arrival_radius: arrival_radius()?,
        },
        1 => AutopilotOrder::Hold,
        2 => AutopilotOrder::Follow {
            target: target()?,
            separation: get_optional(row, "separation")?
                .context("Autopilot follow order lacks a separation")?,
        },
        3 => AutopilotOrder::Patrol { waypoints },
        4 => AutopilotOrder::Attack {
            target: target()?,
            range: get_range(row)?,
        },
        5 => AutopilotOrder::AttackMove {
            destination: destination()?,
            arrival_radius: arrival_radius()?,
        },
        other => bail!("Unknown autopilot order kind {other}"),
    };
    Ok(AutopilotOrderRecord {
        id: get(row, "order_id")?,
        order,
        state: order_state_from_code(row.try_get("state")?)?,
        progress: AutopilotOrderProgress {
            waypoint_index: get(row, "waypoint_index")?,
            recovery_attempts: get(row, "recovery_attempts")?,
            stagnant_ticks: get(row, "stagnant_ticks")?,
            attack_move_origin: get_position(row, "origin")?,
            attack_move_suppressed_target: row
                .try_get::<Option<i64>, _>("suppressed_target")?
                .map(ObjectId),
        },
    })
}

/// Load all Rust controllers.
pub(super) async fn load(
    c: &mut SqliteConnection,
) -> Result<BTreeMap<ObjectId, AutopilotController>> {
    let mut waypoints: BTreeMap<(i64, i64), Vec<BattlePosition>> = BTreeMap::new();
    for row in sqlx::query(
        "SELECT unit_dbref,order_id,map_dbref,x,y FROM btech_autopilot_controller_waypoints
         ORDER BY unit_dbref,order_id,position",
    )
    .fetch_all(&mut *c)
    .await?
    {
        waypoints
            .entry((row.try_get("unit_dbref")?, row.try_get("order_id")?))
            .or_default()
            .push(BattlePosition {
                map: ObjectId(row.try_get("map_dbref")?),
                x: get(&row, "x")?,
                y: get(&row, "y")?,
            });
    }

    let mut active: BTreeMap<i64, AutopilotOrderRecord> = BTreeMap::new();
    let mut queues: BTreeMap<i64, Vec<AutopilotOrderRecord>> = BTreeMap::new();
    let query = format!(
        "SELECT unit_dbref,order_id,{} FROM {ORDERS} ORDER BY unit_dbref,queue_position",
        ORDER_COLUMNS.join(",")
    );
    for row in sqlx::query(sqlx::AssertSqlSafe(query))
        .fetch_all(&mut *c)
        .await?
    {
        let unit: i64 = row.try_get("unit_dbref")?;
        let id: i64 = row.try_get("order_id")?;
        let record = decode_order(&row, waypoints.remove(&(unit, id)).unwrap_or_default())
            .with_context(|| format!("decoding autopilot order {id} for #{unit}"))?;
        if row.try_get::<Option<i64>, _>("queue_position")?.is_some() {
            queues.entry(unit).or_default().push(record);
            continue;
        }
        ensure!(
            active.insert(unit, record).is_none(),
            "Autopilot controller #{unit} has more than one active order"
        );
    }
    ensure!(
        waypoints.is_empty(),
        "Autopilot waypoints reference a missing order"
    );

    let mut feedback: BTreeMap<i64, Vec<AutopilotFeedback>> = BTreeMap::new();
    for row in sqlx::query(
        "SELECT unit_dbref,sequence,simulation_time,order_id,event,reason
         FROM btech_autopilot_controller_feedback ORDER BY unit_dbref,sequence",
    )
    .fetch_all(&mut *c)
    .await?
    {
        feedback
            .entry(row.try_get("unit_dbref")?)
            .or_default()
            .push(AutopilotFeedback {
                sequence: get(&row, "sequence")?,
                simulation_time: row.try_get("simulation_time")?,
                order_id: get_optional(&row, "order_id")?,
                event: event_from_code(row.try_get("event")?)?,
                reason: row
                    .try_get::<Option<i64>, _>("reason")?
                    .map(reason_from_code)
                    .transpose()?,
            });
    }

    let mut controllers = BTreeMap::new();
    let query = format!(
        "SELECT unit_dbref,{} FROM {CONTROLLERS} ORDER BY unit_dbref",
        CONTROLLER_COLUMNS.join(",")
    );
    for row in sqlx::query(sqlx::AssertSqlSafe(query))
        .fetch_all(&mut *c)
        .await?
    {
        let unit: i64 = row.try_get("unit_dbref")?;
        let controller = AutopilotController {
            config: AutopilotConfig {
                speed_percent: get(&row, "speed_percent")?,
                fire_mode: fire_mode_from_code(row.try_get("fire_mode")?)?,
                heat_ceiling: get(&row, "heat_ceiling")?,
                preferred_range: get_range(&row)?,
            },
            state: state_from_code(row.try_get("state")?)?,
            blocking_reason: row
                .try_get::<Option<i64>, _>("blocking_reason")?
                .map(reason_from_code)
                .transpose()?,
            revision: get(&row, "revision")?,
            next_order_id: get(&row, "next_order_id")?,
            active: active.remove(&unit),
            queue: queues.remove(&unit).unwrap_or_default(),
            feedback: feedback.remove(&unit).unwrap_or_default(),
            next_feedback_sequence: get(&row, "next_feedback_sequence")?,
            sightings: BTreeMap::new(),
        };
        controller
            .validate()
            .with_context(|| format!("validating autopilot controller for #{unit}"))?;
        controllers.insert(ObjectId(unit), controller);
    }
    ensure!(
        active.is_empty() && queues.is_empty() && feedback.is_empty(),
        "Autopilot orders or feedback reference a missing controller"
    );
    Ok(controllers)
}

/// Validate controller identity changes as part of the enclosing world
/// transaction.  Durable controller state is copied as a whole because its
/// individual fields are owned by the autopilot domain.
pub(super) fn validate_changes(expected: &mut BtechState, after: &BtechState) -> Result<()> {
    for (&id, controller) in after.controllers() {
        controller
            .validate()
            .with_context(|| format!("validating autopilot controller for #{}", id.0))?;
        ensure!(
            after.registrations().get(&id).map(String::as_str) == Some("MECH"),
            "Autopilot controller #{} requires a MECH registration",
            id.0
        );
    }
    expected.controllers = after.controllers.clone();
    Ok(())
}

/// Bring every stored row of one controller in line with `controller`, or remove them
/// all when it is `None`. Parts equal to `previous` are skipped.
async fn sync_controller(
    c: &mut SqliteConnection,
    id: ObjectId,
    previous: Option<&AutopilotController>,
    controller: Option<&AutopilotController>,
) -> Result<bool> {
    let scope = [("unit_dbref", id.0)];
    let mut changed = false;

    let same_row = previous.zip(controller).is_some_and(|(old, new)| {
        old.config == new.config
            && old.state == new.state
            && old.blocking_reason == new.blocking_reason
            && old.revision == new.revision
            && old.next_order_id == new.next_order_id
            && old.next_feedback_sequence == new.next_feedback_sequence
    });
    // The controller row goes first so its children never lack a parent.
    if let Some(controller) = controller.filter(|_| !same_row) {
        let desired = BTreeMap::from([(Vec::new(), encode_controller(controller)?)]);
        changed |= sync_rows(c, CONTROLLERS, &scope, &[], CONTROLLER_COLUMNS, &desired).await?;
    }

    let same_orders = previous
        .zip(controller)
        .is_some_and(|(old, new)| old.active == new.active && old.queue == new.queue);
    if !same_orders {
        let mut orders = BTreeMap::new();
        let mut waypoints = BTreeMap::new();
        let records = controller.into_iter().flat_map(|controller| {
            controller.active.iter().map(|record| (record, None)).chain(
                controller
                    .queue
                    .iter()
                    .enumerate()
                    .map(|(position, record)| (record, Some(position))),
            )
        });
        for (record, position) in records {
            let order = i64::try_from(record.id)?;
            orders.insert(vec![order], encode_order(record, position));
            if let AutopilotOrder::Patrol { waypoints: points } = &record.order {
                for (index, point) in points.iter().enumerate() {
                    waypoints.insert(
                        vec![order, index as i64],
                        Fields::from([
                            ("map_dbref", Cell::Integer(point.map.0)),
                            ("x", Cell::Integer(i64::from(point.x))),
                            ("y", Cell::Integer(i64::from(point.y))),
                        ]),
                    );
                }
            }
        }
        changed |= sync_rows(c, ORDERS, &scope, &["order_id"], ORDER_COLUMNS, &orders).await?;
        changed |= sync_rows(
            c,
            WAYPOINTS,
            &scope,
            &["order_id", "position"],
            WAYPOINT_COLUMNS,
            &waypoints,
        )
        .await?;
    }

    let same_feedback = previous
        .zip(controller)
        .is_some_and(|(old, new)| old.feedback == new.feedback);
    if !same_feedback {
        let mut desired = BTreeMap::new();
        for record in controller.into_iter().flat_map(|c| &c.feedback) {
            desired.insert(
                vec![i64::try_from(record.sequence)?],
                encode_feedback(record)?,
            );
        }
        changed |= sync_rows(
            c,
            FEEDBACK,
            &scope,
            &["sequence"],
            FEEDBACK_COLUMNS,
            &desired,
        )
        .await?;
    }

    // A removed controller's row goes last, after its children.
    if controller.is_none() {
        changed |= sync_rows(
            c,
            CONTROLLERS,
            &scope,
            &[],
            CONTROLLER_COLUMNS,
            &BTreeMap::new(),
        )
        .await?;
    }
    Ok(changed)
}

/// Write only changed controller records and remove explicitly detached ones.
pub(super) async fn save(
    c: &mut SqliteConnection,
    before: &BtechState,
    after: &BtechState,
) -> Result<bool> {
    if before.controllers().is_empty() && after.controllers().is_empty() {
        return Ok(false);
    }
    let mut changed = false;
    for (&id, controller) in after.controllers().iter() {
        let previous = before.controllers().get(&id);
        if before.controllers().shares_entry(after.controllers(), &id)
            || previous.is_some_and(|old| old.same_saved_state(controller))
        {
            continue;
        }
        changed |= sync_controller(c, id, previous, Some(controller)).await?;
    }

    for (&id, previous) in before.controllers().iter() {
        if after.controllers().contains_key(&id) {
            continue;
        }
        changed |= sync_controller(c, id, Some(previous), None).await?;
    }
    Ok(changed)
}

/// Remove controller rows for objects explicitly purged by maintenance.
pub(super) async fn purge(c: &mut SqliteConnection, ids: &BTreeSet<ObjectId>) -> Result<()> {
    if ids.is_empty() {
        return Ok(());
    }
    for id in ids {
        for table in TABLES {
            // Table names are code-owned constants; the identifier is bound.
            sqlx::query(sqlx::AssertSqlSafe(format!(
                "DELETE FROM {table} WHERE unit_dbref=?"
            )))
            .bind(id.0)
            .execute(&mut *c)
            .await?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stored_codes_round_trip() {
        for reason in [
            AutopilotReason::ManualTakeover,
            AutopilotReason::StaleRevision,
        ] {
            assert_eq!(reason_from_code(reason_code(reason)).unwrap(), reason);
        }
        for event in [
            AutopilotFeedbackEvent::Configured,
            AutopilotFeedbackEvent::Blocked,
        ] {
            assert_eq!(event_from_code(event_code(event)).unwrap(), event);
        }
        assert!(state_from_code(4).is_err());
        assert!(order_state_from_code(-1).is_err());
    }

    #[test]
    fn orders_store_only_their_own_fields() {
        let record = AutopilotOrderRecord::queued(
            3,
            AutopilotOrder::Follow {
                target: ObjectId(9),
                separation: 2,
            },
        );
        let encoded = encode_order(&record, Some(0));
        assert_eq!(encoded["target_dbref"], Cell::Integer(9));
        assert_eq!(encoded["separation"], Cell::Integer(2));
        assert_eq!(encoded["destination_map"], Cell::Null);
        assert_eq!(encoded["range_minimum"], Cell::Null);
    }
}
