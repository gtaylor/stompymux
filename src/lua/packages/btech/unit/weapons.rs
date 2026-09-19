//! Weapon operation commands: firing-mode toggles, rotary bursts, unjamming, turret automation, and ammunition bins.

use super::super::*;

/// Register this package slice on the private native table.
pub(super) fn register(lua: &Lua, native: &Table, _world: &SharedWorld) -> mlua::Result<()> {
    let flamerheat = lua.create_function(move |lua, (unit, pilot, index): (i64, i64, usize)| {
        crate::lua::transactions::require(lua)?;
        let scripts = crate::Scripts::services(lua)?;
        crate::lua::transactions::run(lua, &scripts.world, || {
            let mode = crate::toggle_battle_flamer_heat(
                &mut scripts.world.borrow_mut(),
                ObjectId(unit),
                ObjectId(pilot),
                index,
            )
            .map_err(|e| error::failure("btech.operation.failed", format!("{e:#}")))?;
            crate::btech::notify_unit_text(&scripts, ObjectId(unit), &mode.message(index))
                .map_err(|e| error::failure("btech.operation.failed", e))?;
            detached(lua, &mode)
        })
    })?;
    native.set(
        "unit_flamerheat",
        error::wrap(lua, flamerheat, "btech.operation.failed")?,
    )?;
    let lbx = lua.create_function(move |lua, (unit, pilot, index): (i64, i64, usize)| {
        crate::lua::transactions::require(lua)?;
        let scripts = crate::Scripts::services(lua)?;
        crate::lua::transactions::run(lua, &scripts.world, || {
            let mode = crate::toggle_battle_lbx(
                &mut scripts.world.borrow_mut(),
                ObjectId(unit),
                ObjectId(pilot),
                index,
            )
            .map_err(|e| error::failure("btech.operation.failed", format!("{e:#}")))?;
            crate::btech::notify_unit_text(&scripts, ObjectId(unit), &mode.message(index))
                .map_err(|e| error::failure("btech.operation.failed", e))?;
            detached(lua, &mode)
        })
    })?;
    native.set("unit_lbx", error::wrap(lua, lbx, "btech.operation.failed")?)?;
    for (name, mode) in [
        ("unit_firesmoke", crate::BattleAmmunitionMode::Smoke),
        ("unit_firemine", crate::BattleAmmunitionMode::Mine),
    ] {
        let action = lua.create_function(move |lua, (unit, pilot, index): (i64, i64, usize)| {
            crate::lua::transactions::require(lua)?;
            let scripts = crate::Scripts::services(lua)?;
            crate::lua::transactions::run(lua, &scripts.world, || {
                let selected = crate::toggle_battle_missile_rounds(
                    &mut scripts.world.borrow_mut(),
                    ObjectId(unit),
                    ObjectId(pilot),
                    index,
                    mode,
                )
                .map_err(|e| error::failure("btech.operation.failed", format!("{e:#}")))?;
                crate::btech::notify_unit_text(
                    &scripts,
                    ObjectId(unit),
                    &selected.special_round_message(index),
                )
                .map_err(|e| error::failure("btech.operation.failed", e))?;
                detached(lua, &selected)
            })
        })?;
        native.set(name, error::wrap(lua, action, "btech.operation.failed")?)?;
    }
    let cluster = lua.create_function(move |lua, (unit, pilot, index): (i64, i64, usize)| {
        crate::lua::transactions::require(lua)?;
        let scripts = crate::Scripts::services(lua)?;
        crate::lua::transactions::run(lua, &scripts.world, || {
            let mode = crate::toggle_battle_cluster(
                &mut scripts.world.borrow_mut(),
                ObjectId(unit),
                ObjectId(pilot),
                index,
            )
            .map_err(|e| error::failure("btech.operation.failed", format!("{e:#}")))?;
            crate::btech::notify_unit_text(&scripts, ObjectId(unit), &mode.cluster_message(index))
                .map_err(|e| error::failure("btech.operation.failed", e))?;
            detached(lua, &mode)
        })
    })?;
    native.set(
        "unit_cluster",
        error::wrap(lua, cluster, "btech.operation.failed")?,
    )?;
    let artemis = lua.create_function(move |lua, (unit, pilot, index): (i64, i64, usize)| {
        crate::lua::transactions::require(lua)?;
        let scripts = crate::Scripts::services(lua)?;
        crate::lua::transactions::run(lua, &scripts.world, || {
            let mode = crate::toggle_battle_artemis(
                &mut scripts.world.borrow_mut(),
                ObjectId(unit),
                ObjectId(pilot),
                index,
            )
            .map_err(|e| error::failure("btech.operation.failed", format!("{e:#}")))?;
            crate::btech::notify_unit_text(&scripts, ObjectId(unit), &mode.artemis_message(index))
                .map_err(|e| error::failure("btech.operation.failed", e))?;
            detached(lua, &mode)
        })
    })?;
    native.set(
        "unit_artemis",
        error::wrap(lua, artemis, "btech.operation.failed")?,
    )?;
    let hotload = lua.create_function(move |lua, (unit, pilot, index): (i64, i64, usize)| {
        crate::lua::transactions::require(lua)?;
        let scripts = crate::Scripts::services(lua)?;
        crate::lua::transactions::run(lua, &scripts.world, || {
            let mode = crate::toggle_battle_hotload(
                &mut scripts.world.borrow_mut(),
                ObjectId(unit),
                ObjectId(pilot),
                index,
            )
            .map_err(|e| error::failure("btech.operation.failed", format!("{e:#}")))?;
            crate::btech::notify_unit_text(&scripts, ObjectId(unit), &mode.hotload_message(index))
                .map_err(|e| error::failure("btech.operation.failed", e))?;
            detached(lua, &mode)
        })
    })?;
    native.set(
        "unit_hotload",
        error::wrap(lua, hotload, "btech.operation.failed")?,
    )?;
    let ultra = lua.create_function(move |lua, (unit, pilot, index): (i64, i64, usize)| {
        crate::lua::transactions::require(lua)?;
        let scripts = crate::Scripts::services(lua)?;
        crate::lua::transactions::run(lua, &scripts.world, || {
            let mode = crate::toggle_battle_ultra(
                &mut scripts.world.borrow_mut(),
                ObjectId(unit),
                ObjectId(pilot),
                index,
            )
            .map_err(|e| error::failure("btech.operation.failed", format!("{e:#}")))?;
            crate::btech::notify_unit_text(&scripts, ObjectId(unit), &mode.ultra_message(index))
                .map_err(|e| error::failure("btech.operation.failed", e))?;
            detached(lua, &mode)
        })
    })?;
    native.set(
        "unit_ultra",
        error::wrap(lua, ultra, "btech.operation.failed")?,
    )?;
    let rapid = lua.create_function(move |lua, (unit, pilot, index): (i64, i64, usize)| {
        crate::lua::transactions::require(lua)?;
        let scripts = crate::Scripts::services(lua)?;
        crate::lua::transactions::run(lua, &scripts.world, || {
            let mode = crate::toggle_battle_rapid(
                &mut scripts.world.borrow_mut(),
                ObjectId(unit),
                ObjectId(pilot),
                index,
            )
            .map_err(|e| error::failure("btech.operation.failed", format!("{e:#}")))?;
            crate::btech::notify_unit_text(&scripts, ObjectId(unit), &mode.rapid_message(index))
                .map_err(|e| error::failure("btech.operation.failed", e))?;
            detached(lua, &mode)
        })
    })?;
    native.set(
        "unit_rapid",
        error::wrap(lua, rapid, "btech.operation.failed")?,
    )?;
    let gatling = lua.create_function(move |lua, (unit, pilot, index): (i64, i64, usize)| {
        crate::lua::transactions::require(lua)?;
        let scripts = crate::Scripts::services(lua)?;
        crate::lua::transactions::run(lua, &scripts.world, || {
            let mode = crate::toggle_battle_gatling(
                &mut scripts.world.borrow_mut(),
                ObjectId(unit),
                ObjectId(pilot),
                index,
            )
            .map_err(|e| error::failure("btech.operation.failed", format!("{e:#}")))?;
            crate::btech::notify_unit_text(&scripts, ObjectId(unit), &mode.gatling_message(index))
                .map_err(|e| error::failure("btech.operation.failed", e))?;
            detached(lua, &mode)
        })
    })?;
    native.set(
        "unit_gatling",
        error::wrap(lua, gatling, "btech.operation.failed")?,
    )?;
    let armor_piercing =
        lua.create_function(move |lua, (unit, pilot, index): (i64, i64, usize)| {
            crate::lua::transactions::require(lua)?;
            let scripts = crate::Scripts::services(lua)?;
            crate::lua::transactions::run(lua, &scripts.world, || {
                let mode = crate::toggle_battle_armor_piercing(
                    &mut scripts.world.borrow_mut(),
                    ObjectId(unit),
                    ObjectId(pilot),
                    index,
                )
                .map_err(|e| error::failure("btech.operation.failed", format!("{e:#}")))?;
                crate::btech::notify_unit_text(
                    &scripts,
                    ObjectId(unit),
                    &mode.armor_piercing_message(index),
                )
                .map_err(|e| error::failure("btech.operation.failed", e))?;
                detached(lua, &mode)
            })
        })?;
    native.set(
        "unit_armor_piercing",
        error::wrap(lua, armor_piercing, "btech.operation.failed")?,
    )?;
    let caseless = lua.create_function(move |lua, (unit, pilot, index): (i64, i64, usize)| {
        crate::lua::transactions::require(lua)?;
        let scripts = crate::Scripts::services(lua)?;
        crate::lua::transactions::run(lua, &scripts.world, || {
            let mode = crate::toggle_battle_caseless(
                &mut scripts.world.borrow_mut(),
                ObjectId(unit),
                ObjectId(pilot),
                index,
            )
            .map_err(|e| error::failure("btech.operation.failed", format!("{e:#}")))?;
            crate::btech::notify_unit_text(&scripts, ObjectId(unit), &mode.caseless_message(index))
                .map_err(|e| error::failure("btech.operation.failed", e))?;
            detached(lua, &mode)
        })
    })?;
    native.set(
        "unit_caseless",
        error::wrap(lua, caseless, "btech.operation.failed")?,
    )?;
    let rac = lua.create_function(
        move |lua, (unit, pilot, index, rounds): (i64, i64, usize, Option<u8>)| {
            crate::lua::transactions::require(lua)?;
            let scripts = crate::Scripts::services(lua)?;
            let rounds = rounds.unwrap_or(1);
            crate::lua::transactions::run(lua, &scripts.world, || {
                let changed = crate::set_battle_rotary(
                    &mut scripts.world.borrow_mut(),
                    ObjectId(unit),
                    ObjectId(pilot),
                    index,
                    rounds,
                )
                .map_err(|e| error::failure("btech.operation.failed", format!("{e:#}")))?;
                crate::btech::notify_unit_text(
                    &scripts,
                    ObjectId(unit),
                    &crate::btech::rotary::message(index, rounds, changed),
                )
                .map_err(|e| error::failure("btech.operation.failed", e))?;
                Ok(changed)
            })
        },
    )?;
    native.set("unit_rac", error::wrap(lua, rac, "btech.operation.failed")?)?;
    let unjam = lua.create_function(move |lua, (unit, pilot, index): (i64, i64, usize)| {
        crate::lua::transactions::require(lua)?;
        let scripts = crate::Scripts::services(lua)?;
        crate::lua::transactions::run(lua, &scripts.world, || {
            let message = crate::begin_battle_unjam(
                &mut scripts.world.borrow_mut(),
                ObjectId(unit),
                ObjectId(pilot),
                index,
            )
            .map_err(|e| error::failure("btech.operation.failed", format!("{e:#}")))?;
            crate::btech::notify_unit_text(&scripts, ObjectId(unit), &message)
                .map_err(|e| error::failure("btech.operation.failed", e))?;
            Ok(true)
        })
    })?;
    native.set(
        "unit_unjam",
        error::wrap(lua, unjam, "btech.operation.failed")?,
    )?;
    let autoturret = lua.create_function(move |lua, (unit, pilot): (i64, i64)| {
        crate::lua::transactions::require(lua)?;
        let scripts = crate::Scripts::services(lua)?;
        crate::lua::transactions::run(lua, &scripts.world, || {
            let notice = crate::toggle_battle_automatic_turret(
                &mut scripts.world.borrow_mut(),
                ObjectId(unit),
                ObjectId(pilot),
            )
            .map_err(|e| error::failure("btech.operation.failed", format!("{e:#}")))?;
            crate::btech::notify_unit(&scripts, notice)
                .map_err(|e| error::failure("btech.operation.failed", format!("{e:#}")))?;
            Ok(true)
        })
    })?;
    native.set(
        "unit_autoturret",
        error::wrap(lua, autoturret, "btech.operation.failed")?,
    )?;
    let disable = lua.create_function(move |lua, (unit, pilot, weapon): (i64, i64, usize)| {
        crate::lua::transactions::require(lua)?;
        let scripts = crate::Scripts::services(lua)?;
        crate::lua::transactions::run(lua, &scripts.world, || {
            let notice = crate::disable_gauss_weapon(
                &mut scripts.world.borrow_mut(),
                ObjectId(unit),
                ObjectId(pilot),
                weapon,
            )
            .map_err(|e| error::failure("btech.operation.failed", format!("{e:#}")))?;
            crate::btech::notify_unit(&scripts, notice)
                .map_err(|e| error::failure("btech.operation.failed", format!("{e:#}")))?;
            Ok(true)
        })
    })?;
    native.set(
        "unit_disable",
        error::wrap(lua, disable, "btech.operation.failed")?,
    )?;
    let usebin = lua.create_function(
        move |lua, (unit, pilot, weapon, section): (i64, i64, usize, Option<String>)| {
            crate::lua::transactions::require(lua)?;
            let scripts = crate::Scripts::services(lua)?;
            crate::lua::transactions::run(lua, &scripts.world, || {
                let notice = crate::set_battle_ammunition_section(
                    &mut scripts.world.borrow_mut(),
                    ObjectId(unit),
                    ObjectId(pilot),
                    weapon,
                    section.as_deref().filter(|s| !s.starts_with('-')),
                )
                .map_err(|e| error::failure("btech.operation.failed", format!("{e:#}")))?;
                crate::btech::notify_unit(&scripts, notice)
                    .map_err(|e| error::failure("btech.operation.failed", format!("{e:#}")))?;
                Ok(true)
            })
        },
    )?;
    native.set(
        "unit_usebin",
        error::wrap(lua, usebin, "btech.operation.failed")?,
    )?;
    Ok(())
}
