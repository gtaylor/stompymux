//! Transactional storage for owned ground-vehicle state and its common unit identity.
use crate::{BattleVehicle, BtechState, ObjectId, World};
use anyhow::{Result, ensure};
use sqlx::SqliteConnection;
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

/// The table holding these records.
const TABLE: &str = "btech_vehicles";

/// Read complete records, merged from each row's core and live parts, without
/// interpreting deferred C runtime fields.
pub(super) async fn load(
    c: &mut SqliteConnection,
    state: &mut BtechState,
    clock: super::btech_deadlines::Clock,
) -> Result<()> {
    if !super::btech_unit_rows::installed(c, TABLE).await? {
        return Ok(());
    }
    let mut units = BTreeMap::new();
    for (id, unit) in super::btech_unit_rows::load::<BattleVehicle>(c, TABLE, clock).await? {
        ensure!(
            !state.units.contains_key(&id) && !state.maps.contains_key(&id),
            "Conflicting unit records for #{}",
            id.0
        );
        ensure!(
            state
                .registrations
                .get(&id)
                .is_some_and(|kind| kind == "MECH"),
            "Unit #{} lacks MECH registration",
            id.0
        );
        state.units.insert(id, unit.identity());
        units.insert(id, unit);
    }
    state.vehicles = units.into();
    Ok(())
}

/// Incorporate supported unit writes into the enclosing identity change validator.
pub(super) fn validate_changes(expected: &mut BtechState, after: &BtechState) -> Result<()> {
    for (&id, unit) in after.vehicles() {
        if expected.vehicles.shares_entry(&after.vehicles, &id)
            || expected.vehicles().get(&id) == Some(unit)
        {
            continue;
        }

        if !expected.vehicles.contains_key(&id) {
            // A registered raw unit gains its construction here; only its own
            // earlier MECH registration may precede the vehicle row.
            ensure!(
                (!expected.registrations.contains_key(&id)
                    || expected.registrations().get(&id).map(String::as_str) == Some("MECH"))
                    && !expected.units.contains_key(&id)
                    && !expected.maps.contains_key(&id),
                "Object already has BattleTech state"
            );
            Arc::make_mut(&mut expected.registrations).insert(id, "MECH".into());
        }
        expected.units.insert(id, unit.identity());
        expected.vehicles.share_entry_from(&after.vehicles, &id);
    }
    Ok(())
}

/// Commit unit state and registration inside the world transaction after object creation.
/// Only the parts of each record that changed are rewritten.
pub(super) async fn save(c: &mut SqliteConnection, before: &World, after: &World) -> Result<bool> {
    let (changed, inserted) = super::btech_unit_rows::save(
        c,
        TABLE,
        include_str!("btech_vehicles.sql"),
        &before.btech.vehicles,
        &after.btech.vehicles,
        super::btech_deadlines::Clock::of(before),
        super::btech_deadlines::Clock::of(after),
    )
    .await?;
    for id in inserted {
        super::btech::ensure_mech_registration(c, id).await?;
    }
    Ok(changed)
}

/// Remove owned state during the same explicit object-purge transaction.
pub(super) async fn purge(c: &mut SqliteConnection, ids: &BTreeSet<ObjectId>) -> Result<()> {
    if !super::btech_unit_rows::installed(c, TABLE).await? {
        return Ok(());
    }
    for id in ids {
        sqlx::query("DELETE FROM btech_vehicles WHERE dbref=?")
            .bind(id.0)
            .execute(&mut *c)
            .await?;
    }
    Ok(())
}
