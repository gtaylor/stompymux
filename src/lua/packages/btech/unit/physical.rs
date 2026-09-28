//! Mech physical attack commands: club swings, grab-club, charges, kicks, trips, arm weapons, and arm flips.

use super::super::*;

/// Register this package slice on the private native table.
pub(super) fn register(lua: &Lua, native: &Table, _world: &SharedWorld) -> mlua::Result<()> {
    let club = lua.create_function(|lua, (id, pilot, target): (i64, i64, Option<i64>)| {
        crate::lua::transactions::require(lua)?;
        let scripts = crate::Scripts::services(lua)?;
        crate::lua::transactions::run(lua, &scripts.world, || {
            let report = crate::btech::physical::configured_club(
                &scripts,
                &crate::lua::configuration(lua),
                ObjectId(id),
                ObjectId(pilot),
                target.map(ObjectId),
            )
            .map_err(mlua::Error::external)?;
            detached(lua, &report)
        })
    })?;
    native.set(
        "unit_club",
        error::wrap(lua, club, "btech.operation.failed")?,
    )?;
    let grab = lua.create_function(|lua, (id, pilot, arm): (i64, i64, Option<String>)| {
        crate::lua::transactions::require(lua)?;
        let scripts = crate::Scripts::services(lua)?;
        crate::lua::transactions::run(lua, &scripts.world, || {
            let notices = crate::grab_battle_club(
                &mut scripts.world.borrow_mut(),
                ObjectId(id),
                ObjectId(pilot),
                arm.as_deref(),
            )
            .map_err(mlua::Error::external)?;
            for notice in &notices {
                crate::btech::notify_unit(&scripts, notice.clone())
                    .map_err(mlua::Error::external)?;
            }
            detached(lua, &notices)
        })
    })?;
    native.set(
        "unit_grabclub",
        error::wrap(lua, grab, "btech.operation.failed")?,
    )?;
    let charge = lua.create_function(|lua, (id, pilot, target): (i64, i64, mlua::Value)| {
        crate::lua::transactions::require(lua)?;
        let selection = match target {
            mlua::Value::Nil => crate::BattleChargeSelection::Default,
            mlua::Value::Integer(id) => crate::BattleChargeSelection::Target(ObjectId(id)),
            mlua::Value::String(value) if value.to_str()? == "-" => {
                crate::BattleChargeSelection::Cancel
            }
            _ => return Err(mlua::Error::external("Expected a target dbref, '-' or nil")),
        };
        let scripts = crate::Scripts::services(lua)?;
        crate::lua::transactions::run(lua, &scripts.world, || {
            let notices = crate::select_battle_charge(
                &mut scripts.world.borrow_mut(),
                ObjectId(id),
                ObjectId(pilot),
                selection,
            )
            .map_err(mlua::Error::external)?;
            for notice in &notices {
                crate::btech::notify_unit(&scripts, notice.clone())
                    .map_err(mlua::Error::external)?;
            }
            detached(lua, &notices)
        })
    })?;
    native.set(
        "unit_charge",
        error::wrap(lua, charge, "btech.operation.failed")?,
    )?;
    for (name, trip) in [("unit_kick", false), ("unit_trip", true)] {
        let callback = lua.create_function(
            move |lua, (id, pilot, leg, target): (i64, i64, Option<String>, Option<i64>)| {
                crate::lua::transactions::require(lua)?;
                let scripts = crate::Scripts::services(lua)?;
                crate::lua::transactions::run(lua, &scripts.world, || {
                    let config = crate::lua::configuration(lua);
                    let resolve = if trip {
                        crate::btech::physical::configured_trip
                    } else {
                        crate::btech::physical::configured_kick
                    };
                    let report = resolve(
                        &scripts,
                        &config,
                        ObjectId(id),
                        ObjectId(pilot),
                        leg.as_deref(),
                        target.map(ObjectId),
                    )
                    .map_err(mlua::Error::external)?;
                    detached(lua, &report)
                })
            },
        )?;
        native.set(name, error::wrap(lua, callback, "btech.operation.failed")?)?;
    }
    for (name, kind) in [
        ("unit_punch", crate::BattleArmAttack::Punch.into()),
        ("unit_axe", crate::BattleArmAttack::Axe.into()),
        ("unit_sword", crate::BattleArmAttack::Sword.into()),
        ("unit_mace", crate::BattleArmAttack::Mace.into()),
        ("unit_saw", crate::BattleArmAttack::Saw.into()),
        ("unit_claw", crate::BattleArmAttack::Claw.into()),
        ("unit_melee", crate::BattleArmWeapon::Installed),
    ] {
        let callback = lua.create_function(
            move |lua, (id, pilot, leg, target): (i64, i64, Option<String>, Option<i64>)| {
                crate::lua::transactions::require(lua)?;
                let scripts = crate::Scripts::services(lua)?;
                crate::lua::transactions::run(lua, &scripts.world, || {
                    let config = crate::lua::configuration(lua);
                    let report = crate::btech::physical::configured_arm_attack(
                        &scripts,
                        &config,
                        ObjectId(id),
                        ObjectId(pilot),
                        leg.as_deref(),
                        target.map(ObjectId),
                        kind,
                    )
                    .map_err(mlua::Error::external)?;
                    detached(lua, &report)
                })
            },
        )?;
        native.set(name, error::wrap(lua, callback, "btech.operation.failed")?)?;
    }
    let arms = lua.create_function(move |lua, (unit, pilot): (i64, i64)| {
        crate::lua::transactions::require(lua)?;
        let scripts = crate::Scripts::services(lua)?;
        crate::lua::transactions::run(lua, &scripts.world, || {
            let notice = crate::flip_battle_arms(
                &mut scripts.world.borrow_mut(),
                ObjectId(unit),
                ObjectId(pilot),
            )
            .map_err(|e| error::failure("btech.operation.failed", format!("{e:#}")))?;
            crate::btech::notify_unit(&scripts, notice)
                .map_err(|e| error::failure("btech.operation.failed", e))?;
            Ok(true)
        })
    })?;
    native.set(
        "unit_fliparms",
        error::wrap(lua, arms, "btech.operation.failed")?,
    )?;
    Ok(())
}
