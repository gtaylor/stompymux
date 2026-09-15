//! Selective BattleTech map persistence and read-only identities for deferred unit simulation.
use crate::{BtechState, ObjectId, StoredBattleMap, StoredBattleUnit, World};
use anyhow::{Result, ensure};
use sqlx::{Row, SqliteConnection};
use std::collections::{BTreeMap, BTreeSet};

/// Load available identity metadata in the enclosing world snapshot transaction.
pub(super) async fn load(c: &mut SqliteConnection) -> Result<BtechState> {
    let mut registrations = BTreeMap::new();
    for row in
        sqlx::query("SELECT dbref,special_type FROM btech_special_registrations ORDER BY dbref")
            .fetch_all(&mut *c)
            .await?
    {
        registrations.insert(
            ObjectId(row.try_get("dbref")?),
            row.try_get("special_type")?,
        );
    }
    let mut maps = BTreeMap::new();
    for row in sqlx::query(
        "SELECT dbref,first_free,on_map,map_name,width,height,gravity,temperature,flags,move_mod,light,visibility,max_visibility,cloudbase,sensor_flags,wind_direction,wind_speed,cf,cf_max,build_flag,regen_factor FROM btech_maps ORDER BY dbref",
    )
    .fetch_all(&mut *c)
    .await?
    {
        maps.insert(
            ObjectId(row.try_get("dbref")?),
            StoredBattleMap {
                membership_extent: row.try_get("first_free")?,
                building_parent: row.try_get("on_map")?,
                artillery_shots: Default::default(),                name: row.try_get("map_name")?,
                width: row.try_get("width")?,
                height: row.try_get("height")?,
                gravity: row.try_get("gravity")?,
                temperature: row.try_get("temperature")?,
                flags: row.try_get("flags")?,
                building_entrances: Default::default(),
                building_entry_points: Default::default(),
                building_exits: Default::default(),
            authored_link: None,
                building_repair: None,
                cargo_transfer_point: None,
                landing_exclusions: Default::default(),
                lookup_bits: None,
                landing_exclusion_order: Default::default(),
                minefields: Default::default(),
            minefield_order: Default::default(),
                building: crate::BattleBuildingState {
                    integrity: row.try_get("cf")?,
                    maximum_integrity: row.try_get("cf_max")?,
                    flags: row.try_get("build_flag")?,
                    regeneration: row.try_get("regen_factor")?,
                },
                movement_modifier: row.try_get("move_mod")?,
                linked_markers: Default::default(),
                light: row.try_get("light")?,
                visibility: row.try_get("visibility")?,
                maximum_visibility: row.try_get("max_visibility")?,
                cloud_base: i16::try_from(row.try_get::<i64, _>("cloudbase")?)?,
                sensor_flags: row.try_get("sensor_flags")?,
                wind_direction: row.try_get("wind_direction")?,
                wind_speed: row.try_get("wind_speed")?,
            fire_dice: None,
            decorations: Default::default(),
            static_decorations: Default::default(),
                terrain: None,
            },
        );
    }
    let mut units = BTreeMap::new();
    for row in sqlx::query(
        "SELECT dbref,mech_name,mech_type,unit_class,movement_type,tons,map_dbref \
         FROM btech_mechs ORDER BY dbref",
    )
    .fetch_all(&mut *c)
    .await?
    {
        let map: i64 = row.try_get("map_dbref")?;
        units.insert(
            ObjectId(row.try_get("dbref")?),
            StoredBattleUnit {
                name: row.try_get("mech_name")?,
                template: row.try_get("mech_type")?,
                class_code: row.try_get("unit_class")?,
                movement_code: row.try_get("movement_type")?,
                tons: row.try_get("tons")?,
                map: (map >= 0).then_some(ObjectId(map)),
            },
        );
    }
    super::btech_cargo_bay::load(c, &mut maps).await?;
    super::btech_terrain::load(c, &mut maps).await?;
    super::btech_entrances::load(c, &mut maps).await?;
    super::btech_building_routes::load(c, &mut maps).await?;
    super::btech_map_links::load(c, &mut maps).await?;
    super::btech_landing_exclusions::load(c, &mut maps).await?;
    super::btech_wrapping::load(c, &mut maps).await?;
    super::btech_minefields::load(c, &mut maps).await?;
    super::btech_map_bits::load(c, &mut maps).await?;
    super::btech_building_repair::load(c, &mut maps).await?;
    super::btech_decorations::load(c, &mut maps).await?;
    super::btech_static_decorations::load(c, &mut maps).await?;
    super::btech_map_random::load(c, &mut maps).await?;
    super::btech_artillery::load(c, &mut maps).await?;
    let mut state = BtechState {
        sensor_recoveries: super::btech_sensor_recovery::load(c).await?.into(),
        turn_clock: super::btech_turn_clock::load(c).await?,
        gunner_stations: super::btech_gunner_stations::load(c).await?.into(),
        inventories: super::btech_inventory::load(c).await?.into(),
        weapon_settings: Default::default(),
        reactor: super::btech_reactor::load(c).await?,
        wrecks: super::btech_wrecks::load(c).await?.into(),
        tows: super::btech_tows::load(c).await?.into(),
        player_preferences: super::btech_view_preferences::load(c).await?.into(),
        seismic_detect_stopped: false,
        skill_thresholds: Default::default(),
        character_values: super::btech_values::load(c).await?.into(),
        characters: super::btech_character::load(c).await?.into(),
        recoveries: super::btech_recovery::load(c).await?.into(),
        constructed: Default::default(),
        vehicles: Default::default(),
        registrations: registrations.into(),
        maps: maps.into(),
        units: units.into(),
    };
    super::btech_units::load(c, &mut state).await?;
    super::btech_vehicles::load(c, &mut state).await?;
    Ok(state)
}

/// Permit checked map operations and maintenance, rejecting changes to deferred unit state.
pub(super) fn validate_changes(
    before: &World,
    after: &World,
    purges: Option<&BTreeSet<ObjectId>>,
) -> Result<()> {
    let mut expected = before.btech.clone();
    expected.sensor_recoveries = after.btech.sensor_recoveries.clone();
    expected.turn_clock = after.btech.turn_clock;
    expected.reactor = after.btech.reactor.clone();
    for id in crate::btech::wreck_cleanup::retired(before, after)? {
        crate::btech::wreck_cleanup::forget(&mut expected, id);
    }
    crate::btech::map_lifecycle::forget(
        &mut expected,
        &crate::btech::map_lifecycle::removed(before, after)?,
    );
    super::btech_gunner_stations::forget_removed(&mut expected, &after.btech)?;
    // DEBUG has no domain record; retire it before admitting a replacement role.
    for (id, kind) in before.btech.registrations() {
        if kind == "DEBUG"
            && after.btech.registrations().get(id).map(String::as_str) != Some("DEBUG")
        {
            std::sync::Arc::make_mut(&mut expected.registrations).remove(id);
        }
    }
    for (id, kind) in after.btech.registrations() {
        if kind != "DEBUG"
            || before.btech.registrations().get(id).map(String::as_str) == Some("DEBUG")
        {
            continue;
        }
        ensure!(
            !expected.registrations().contains_key(id),
            "Object already has BattleTech state"
        );
        ensure!(
            after
                .objects
                .get(id)
                .is_some_and(|object| object.kind == crate::Kind::Thing
                    && !object.flags.contains(crate::Flag::Going)),
            "DEBUG requires a live thing object"
        );
        std::sync::Arc::make_mut(&mut expected.registrations).insert(*id, "DEBUG".into());
    }
    expected.wrecks = after.btech.wrecks.clone();
    // Runtime policy participates in transactions but is not saved in the database.
    expected.tows = after.btech.tows.clone();
    expected.inventories = after.btech.inventories.clone();
    expected.skill_thresholds = after.btech.skill_thresholds.clone();
    expected.weapon_settings = after.btech.weapon_settings.clone();
    expected.seismic_detect_stopped = after.btech.seismic_detect_stopped;
    if let Some(purges) = purges {
        expected.purge(purges);
    }
    for (id, map) in after.btech.maps() {
        if expected.maps().get(id) == Some(map) {
            continue;
        }
        ensure!(
            map.terrain_ready(),
            "Map changes require a complete terrain dictionary"
        );
        if !expected.maps().contains_key(id) {
            ensure!(
                !expected.registrations().contains_key(id) && !expected.units().contains_key(id),
                "Object already has BattleTech state"
            );
            std::sync::Arc::make_mut(&mut expected.registrations).insert(*id, "MAP".into());
        }
        std::sync::Arc::make_mut(&mut expected.maps).insert(*id, map.clone());
    }
    super::btech_gunner_stations::validate_changes(&mut expected, &after.btech)?;
    super::btech_units::validate_changes(&mut expected, &after.btech)?;
    super::btech_vehicles::validate_changes(&mut expected, &after.btech)?;
    for (&id, recovery) in after.btech.recoveries() {
        std::sync::Arc::make_mut(&mut expected.recoveries).insert(id, recovery.clone());
    }
    for (&id, entries) in after.btech.character_values() {
        std::sync::Arc::make_mut(&mut expected.character_values)
            .entry(id)
            .or_default()
            .extend(entries.iter().map(|(name, value)| (name.clone(), *value)));
    }
    for (&id, &dimensions) in after.btech.player_preferences.iter() {
        std::sync::Arc::make_mut(&mut expected.player_preferences).insert(id, dimensions);
    }
    for (&id, &profile) in after.btech.characters() {
        std::sync::Arc::make_mut(&mut expected.characters).insert(id, profile);
    }
    ensure!(
        expected == after.btech,
        "Unsupported BattleTech identity change"
    );
    after.btech.validate(after)?;
    Ok(())
}

/// Write changed maps after their world objects exist, preserving unowned columns.
pub(super) async fn save(c: &mut SqliteConnection, before: &World, after: &World) -> Result<bool> {
    use super::write::{Cell, fields, row};
    let mut changed = super::btech_map_lifecycle::save(c, before, after).await?;
    for (id, kind) in before.btech.registrations() {
        if kind == "DEBUG"
            && after.btech.registrations().get(id).map(String::as_str) != Some("DEBUG")
        {
            sqlx::query(
                "DELETE FROM btech_special_registrations WHERE dbref=? AND special_type='DEBUG'",
            )
            .bind(id.0)
            .execute(&mut *c)
            .await?;
            changed = true;
        }
    }
    changed |= super::btech_gunner_stations::save(c, before, after).await?;
    for (id, kind) in after.btech.registrations() {
        if kind == "DEBUG"
            && before.btech.registrations().get(id).map(String::as_str) != Some("DEBUG")
        {
            sqlx::query(
                "INSERT INTO btech_special_registrations(dbref,special_type) VALUES(?,'DEBUG')",
            )
            .bind(id.0)
            .execute(&mut *c)
            .await?;
            changed = true;
        }
    }
    for (id, map) in after.btech.maps() {
        let previous = before.btech.maps().get(id);
        if previous == Some(map) {
            continue;
        }
        let mut values = map_fields(map);
        if previous.is_none() {
            // Fixed map initialization defaults; unrelated fields are never rewritten on reload.
            for (name, value) in [("reserved", 0), ("moves", 0)] {
                values.insert(name.into(), Cell::Integer(value));
            }
        }
        // Conditions can change on occupied maps without rewriting terrain or deferred objects.
        if previous.is_some_and(|old| {
            old.terrain == map.terrain && old.width == map.width && old.height == map.height
        }) {
            row(
                c,
                "btech_maps",
                fields([("dbref", Cell::Integer(id.0))]),
                previous.map(map_fields).as_ref(),
                &values,
            )
            .await?;
            sqlx::query("DELETE FROM btech_map_los WHERE map_dbref=?")
                .bind(id.0)
                .execute(&mut *c)
                .await?;
            changed = true;
            continue;
        }
        // Mines, building routes, wrapping and landing exclusions have selective writers.
        // Other map-object records remain outside terrain replacement ownership.
        let objects: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM btech_map_objects WHERE map_dbref=? AND object_type NOT IN (0,1,2,3,4,5,6,7,9)",
        )
        .bind(id.0)
        .fetch_one(&mut *c)
        .await?;
        ensure!(
            objects == 0,
            "Map #{} has map objects; terrain reload with map objects is not implemented",
            id.0
        );
        row(
            c,
            "btech_maps",
            fields([("dbref", Cell::Integer(id.0))]),
            previous.map(map_fields).as_ref(),
            &values,
        )
        .await?;
        if previous.is_none() {
            row(
                c,
                "btech_special_registrations",
                fields([("dbref", Cell::Integer(id.0))]),
                None,
                &fields([("special_type", Cell::Text("MAP".into()))]),
            )
            .await?;
        }
        super::btech_terrain::save(c, *id, map).await?;
        sqlx::query("DELETE FROM btech_map_los WHERE map_dbref=?")
            .bind(id.0)
            .execute(&mut *c)
            .await?;
        changed = true;
    }
    changed |= super::btech_units::save(c, before, after).await?;
    changed |= super::btech_vehicles::save(c, before, after).await?;
    changed |= super::btech_view_preferences::save(c, before, after).await?;
    changed |= super::btech_character::save(c, before, after).await?;
    changed |= super::btech_values::save(c, before, after).await?;
    changed |= super::btech_tows::save(c, before, after).await?;
    changed |= super::btech_inventory::save(c, before, after).await?;
    changed |= super::btech_cargo_bay::save(c, before, after).await?;
    changed |= super::btech_recovery::save(c, before, after).await?;
    changed |= super::btech_reactor::save(c, before, after).await?;
    changed |= super::btech_sensor_recovery::save(c, before, after).await?;
    changed |= super::btech_turn_clock::save(c, before, after).await?;
    changed |= super::btech_wrecks::save(c, before, after).await?;
    changed |= super::btech_decorations::save(c, before, after).await?;
    changed |= super::btech_static_decorations::save(c, before, after).await?;
    changed |= super::btech_map_random::save(c, before, after).await?;
    changed |= super::btech_entrances::save(c, before, after).await?;
    changed |= super::btech_building_routes::save(c, before, after).await?;
    changed |= super::btech_map_links::save(c, before, after).await?;
    changed |= super::btech_landing_exclusions::save(c, before, after).await?;
    changed |= super::btech_wrapping::save(c, before, after).await?;
    changed |= super::btech_minefields::save(c, before, after).await?;
    changed |= super::btech_building_repair::save(c, after).await?;
    changed |= super::btech_artillery::save(c, before, after).await?;
    changed |= super::btech_map_bits::save(c, before, after).await?;
    Ok(changed)
}

/// The explicitly owned columns of a map identity.
fn map_fields(map: &StoredBattleMap) -> super::write::Fields {
    use super::write::{Cell, fields};
    fields([
        (
            "first_free",
            Cell::Integer(i64::from(map.membership_extent)),
        ),
        ("on_map", Cell::Integer(map.building_parent)),
        ("map_name", Cell::Text(map.name.clone())),
        ("width", Cell::Integer(map.width)),
        ("height", Cell::Integer(map.height)),
        ("gravity", Cell::Integer(map.gravity)),
        ("temperature", Cell::Integer(map.temperature)),
        ("flags", Cell::Integer(map.flags)),
        ("cf", Cell::Integer(map.building.integrity)),
        ("cf_max", Cell::Integer(map.building.maximum_integrity)),
        ("build_flag", Cell::Integer(map.building.flags)),
        ("regen_factor", Cell::Integer(map.building.regeneration)),
        ("move_mod", Cell::Integer(map.movement_modifier)),
        ("light", Cell::Integer(map.light)),
        ("visibility", Cell::Integer(map.visibility)),
        ("max_visibility", Cell::Integer(map.maximum_visibility)),
        ("cloudbase", Cell::Integer(i64::from(map.cloud_base))),
        ("sensor_flags", Cell::Integer(map.sensor_flags)),
        ("wind_direction", Cell::Integer(map.wind_direction)),
        ("wind_speed", Cell::Integer(map.wind_speed)),
    ])
}
