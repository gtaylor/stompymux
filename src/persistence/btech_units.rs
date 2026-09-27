//! Versioned persistence for complete Rust-owned unit records.
use crate::{BattleUnit, BtechState, ObjectId, World};
use anyhow::{Result, ensure};
use sqlx::SqliteConnection;
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

/// The table holding these records.
const TABLE: &str = "btech_units";

/// Read complete records, merged from each row's core and live parts, without
/// interpreting deferred C runtime fields.
pub(super) async fn load(
    c: &mut SqliteConnection,
    state: &mut BtechState,
    clock: super::btech_deadlines::Clock,
) -> Result<()> {
    let mut units = BTreeMap::new();
    for (id, unit) in super::btech_unit_rows::load::<BattleUnit>(c, TABLE, clock).await? {
        unit.validate()?;
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
    state.constructed = units.into();
    Ok(())
}

/// Incorporate supported unit writes into the enclosing identity change validator.
pub(super) fn validate_changes(expected: &mut BtechState, after: &BtechState) -> Result<()> {
    for (&id, unit) in after.constructed_units() {
        if expected.constructed.shares_entry(&after.constructed, &id)
            || expected.constructed_units().get(&id) == Some(unit)
        {
            continue;
        }
        unit.validate()?;
        if !expected.constructed.contains_key(&id) {
            // A registered raw unit gains its construction here; only its own
            // earlier MECH registration may precede the unit row.
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
        expected
            .constructed
            .share_entry_from(&after.constructed, &id);
    }
    Ok(())
}

/// Commit unit state and registration inside the world transaction after object creation.
/// Only the parts of each record that changed are rewritten.
pub(super) async fn save(c: &mut SqliteConnection, before: &World, after: &World) -> Result<bool> {
    let (changed, inserted) = super::btech_unit_rows::save(
        c,
        TABLE,
        &before.btech.constructed,
        &after.btech.constructed,
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
    super::write::purge_rows(c, TABLE, "dbref", ids).await
}
