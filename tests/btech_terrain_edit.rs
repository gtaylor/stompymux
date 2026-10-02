//! Live terrain edits and seasonal changes commit against occupied, already-persisted battlefields.
use crate::support;
use crate::support::btech_firing as firing;
use std::{cell::RefCell, rc::Rc};
use stompymux_rs::*;

/// Write a serialized hex as a Lua table constructor.
fn lua_table(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::Object(fields) => {
            let fields: Vec<_> = fields
                .iter()
                .map(|(key, value)| format!("{key}={}", lua_table(value)))
                .collect();
            format!("{{{}}}", fields.join(","))
        }
        serde_json::Value::String(text) => format!("'{text}'"),
        other => other.to_string(),
    }
}

/// Keep the assigned pilot in its cockpit while a separate wizard edits its map.
fn operator(world: &mut World, config: &Config, map: ObjectId) -> ObjectId {
    let id = world.create(config, "Terrain operator".into(), Kind::Player);
    world
        .objects
        .get_mut(&id)
        .unwrap()
        .flags
        .insert(Flag::Wizard);
    world.objects.get_mut(&id).unwrap().location = Some(map);
    id
}

/// All canonical terrain types change live, preserving unit altitude, dice and controls on all chassis.
#[tokio::test]
async fn occupied_edits_share_native_lua_and_incremental_persistence() {
    for source in firing::templates() {
        let (_dir, config, mut world, unit, _, _) =
            firing::fixture_with_target(&source, None, include_str!("../game/mechs/AS7-D.toml"))
                .await;
        let map = world.btech.units()[&unit].map.unwrap();
        let actor = operator(&mut world, &config, map);
        persistence::save(&config.database(), &world).await.unwrap();
        let altitude = battle_unit_altitude(&world, unit).unwrap();
        let native = Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
        let lua = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
        for (symbol, terrain) in [
            ('^', Terrain::Mountains),
            ('~', Terrain::Water),
            ('-', Terrain::Ice),
            ('/', Terrain::Bridge),
            ('#', Terrain::Road),
            ('`', Terrain::LightForest),
            ('"', Terrain::HeavyForest),
            ('%', Terrain::Rough),
            ('+', Terrain::Snow),
            ('@', Terrain::Building),
            ('=', Terrain::Wall),
            ('.', Terrain::Grassland),
        ] {
            // ADDHEX caps a depth at 9 and any other height at 35.
            let expected = BattleHex::new(
                terrain,
                if matches!(terrain, Terrain::Water | Terrain::Ice) {
                    9
                } else {
                    30
                },
            );
            let before = serde_json::to_value(&lua.world().btech).unwrap();
            let call = format!(
                "btech.map.set_hex({},{},0,11,{})",
                actor.0,
                map.0,
                lua_table(&serde_json::to_value(expected).unwrap())
            );
            assert!(
                lua.eval_callback::<()>(&format!("{call}; error('abort edit')"))
                    .is_err()
            );
            assert_eq!(serde_json::to_value(&lua.world().btech).unwrap(), before);
            lua.drain_outbox();
            let report: mlua::Table = lua.eval_callback(&format!("return {call}")).unwrap();
            // The report's tiles are the hex layers; the native comparison below checks values.
            assert!(
                report
                    .get::<mlua::Table>("after")
                    .unwrap()
                    .contains_key("ground")
                    .unwrap()
            );
            let output = support::run_text(
                &native,
                &config,
                actor,
                1,
                &format!("addhex 0 11 {symbol} -30"),
            );
            assert!(output.contains("Hex set!"), "{output}");
            assert_eq!(native.world().btech, lua.world().btech);
            assert_eq!(
                battle_unit_altitude(&native.world(), unit).unwrap(),
                altitude
            );
            assert_eq!(
                native.world().btech.maps()[&map].base_hex(0, 11).unwrap(),
                expected
            );
            let after = serde_json::to_value(&native.world().btech).unwrap();
            let kind = if native.world().btech.vehicles().contains_key(&unit) {
                "vehicles"
            } else {
                "constructed"
            };
            let mut expected = before[kind][unit.0.to_string()].clone();
            expected["ground_elevation"] =
                after[kind][unit.0.to_string()]["ground_elevation"].clone();
            assert_eq!(after[kind][unit.0.to_string()], expected, "{terrain:?}");
            let saved = native.world().clone();
            saved.validate(&config).unwrap();
            persistence::save(&config.database(), &saved).await.unwrap();
            assert_eq!(
                persistence::load(&config.database()).await.unwrap().btech,
                saved.btech
            );
        }
        // Fire and smoke are not terrain; ADDFIRE and ADDSMOKE place them instead.
        for symbol in ['&', ':'] {
            let before = native.world().btech.clone();
            let output = support::run_text(
                &native,
                &config,
                actor,
                1,
                &format!("addhex 0 11 {symbol} 1"),
            );
            assert!(
                output.contains("Fire and smoke are not terrain"),
                "{output}"
            );
            assert_eq!(native.world().btech, before);
        }
        // Lua takes a hex's layers; operator symbols belong to ADDHEX.
        let before = lua.world().btech.clone();
        for hex in [
            "'^'",
            "btech.map.terrain_types.MOUNTAINS",
            "{level=1}",
            "{level=1,ground='bogus'}",
            "{level=36,ground=btech.map.ground_types.CLEAR}",
            "{level=1,ground=btech.map.ground_types.CLEAR,overlay='fire'}",
        ] {
            assert!(
                lua.eval_callback::<()>(&format!(
                    "btech.map.set_hex({},{},0,11,{hex})",
                    actor.0, map.0
                ))
                .is_err()
            );
            assert_eq!(lua.world().btech, before);
        }
        lua.drain_outbox();
        let mut candidate = native.world().clone();
        let before = candidate.btech.clone();
        assert!(
            reload_battle_map(
                &mut candidate,
                map,
                "replacement",
                BattleMapAsset::from_cells(&format!("1 12\n{}", ".0\n".repeat(12))).unwrap()
            )
            .is_err()
        );
        assert_eq!(candidate.btech, before);
    }
}

/// Regression: freezing an existing map is a live edit, not an occupied-map asset reload.
#[tokio::test]
async fn ice_growth_commits_against_an_occupied_durable_map() {
    for source in firing::templates() {
        let (_dir, config, mut world, unit, _, _) =
            firing::fixture_with_target(&source, None, include_str!("../game/mechs/AS7-D.toml"))
                .await;
        let map = world.btech.units()[&unit].map.unwrap();
        let actor = operator(&mut world, &config, map);
        let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
        set_battle_map_hex_action(
            &scripts,
            &config,
            actor,
            map,
            BattleHexCoordinate { x: 0, y: 5 },
            BattleHex::new(Terrain::Water, 2),
        )
        .unwrap();
        let baseline = scripts.world().clone();
        persistence::save(&config.database(), &baseline)
            .await
            .unwrap();
        let growth =
            change_battle_map_ice_action(&scripts, &config, actor, map, 100, BattleIceChange::Grow)
                .unwrap();
        assert_eq!(growth.changed, vec![BattleHexCoordinate { x: 0, y: 5 }]);
        let saved = scripts.world().clone();
        persistence::save(&config.database(), &saved).await.unwrap();
        let loaded = persistence::load(&config.database()).await.unwrap();
        assert_eq!(loaded.btech, saved.btech);
        assert_eq!(
            loaded.btech.maps()[&map].base_hex(0, 5).unwrap().terrain(),
            Terrain::Ice
        );
        change_battle_map_ice_action(&scripts, &config, actor, map, 100, BattleIceChange::Melt)
            .unwrap();
        let melted = scripts.world().clone();
        persistence::save(&config.database(), &melted)
            .await
            .unwrap();
        assert_eq!(
            persistence::load(&config.database()).await.unwrap().btech,
            melted.btech
        );
    }
}

/// Invalid requests are state-free; overlays remain independent of an edited base tile.
#[tokio::test]
async fn edit_admission_overlays_and_extreme_elevations() {
    let (_dir, config, mut world, unit, _, _) = firing::fixture_with_target(
        &firing::templates()[0],
        None,
        include_str!("../game/mechs/AS7-D.toml"),
    )
    .await;
    let map = world.btech.units()[&unit].map.unwrap();
    let actor = operator(&mut world, &config, map);
    world
        .objects
        .get_mut(&ObjectId(2))
        .unwrap()
        .flags
        .remove(Flag::Wizard);
    let coordinate = BattleHexCoordinate { x: 0, y: 5 };
    set_map_decoration(
        &mut world,
        map,
        coordinate,
        Some(BattleDecoration::new(BattleDecorationKind::Smoke, 30, None)),
    )
    .unwrap();
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
    let before = scripts.world().btech.clone();
    for command in [
        "addhex",
        "addhex 0 0 . 1 extra",
        "addhex -1 0 . 0",
        "addhex 0 12 . 0",
        "addhex 0 0 X 0",
        "addhex 0 0 . 2147483648",
        "addhex/bad 0 0 . 0",
    ] {
        support::run_text(&scripts, &config, actor, 1, command);
        assert_eq!(scripts.world().btech, before, "{command}");
    }
    assert!(
        set_battle_map_hex_action(
            &scripts,
            &config,
            ObjectId(2),
            map,
            coordinate,
            BattleHex::new(Terrain::Water, 2)
        )
        .is_err()
    );
    assert!(
        set_battle_map_hex_action(
            &scripts,
            &config,
            actor,
            unit,
            coordinate,
            BattleHex::new(Terrain::Water, 2)
        )
        .is_err()
    );
    assert_eq!(scripts.world().btech, before);
    let output = support::run_text(&scripts, &config, actor, 1, "addhex 0 5 ~ -2147483648");
    assert!(output.contains("Hex set!"), "{output}");
    let world = scripts.world();
    assert_eq!(
        world.btech.maps()[&map]
            .base_hex(0, 5)
            .unwrap()
            .water_depth(),
        9
    );
    let map = &world.btech.maps()[&map];
    assert_eq!(map.hex(0, 5).unwrap().terrain(), Terrain::Smoke);
    assert_eq!(map.base_hex(0, 5).unwrap().terrain(), Terrain::Water);
    assert_eq!(map.decoration(coordinate).unwrap().unwrap().remaining, 30);
    world.validate(&config).unwrap();
}

/// Editing a bridge clears the routing hint without lifting a hovercraft; flights keep their own altitude.
#[tokio::test]
async fn bridge_edits_and_airborne_edits_preserve_physical_position() {
    for source in [
        firing::templates()[4].clone(),
        firing::templates()[0].clone(),
    ] {
        let (_dir, config, mut world, unit, _, _) =
            firing::fixture_with_target(&source, None, include_str!("../game/mechs/AS7-D.toml"))
                .await;
        let map = world.btech.units()[&unit].map.unwrap();
        let actor = operator(&mut world, &config, map);
        let hover = world.btech.vehicles().contains_key(&unit);
        if hover {
            let mut state = serde_json::to_value(&world.btech).unwrap();
            state["maps"][map.0.to_string()]["terrain"][11] = serde_json::to_value(
                stompymux_rs::BattleHex::new(stompymux_rs::Terrain::Bridge, 4),
            )
            .unwrap();
            world.btech = serde_json::from_value(state).unwrap();
            firing::edit(&mut world, unit, |state| {
                state["under_bridge"] = true.into();
                state["ground_elevation"] = serde_json::Value::Null;
            });
        } else {
            launch_battle_jump(&mut world, unit, ObjectId(1), 0, 2.0).unwrap();
        }
        world.validate(&config).unwrap();
        persistence::save(&config.database(), &world).await.unwrap();
        let altitude = battle_unit_altitude(&world, unit).unwrap();
        let flight = world
            .btech
            .constructed_units()
            .get(&unit)
            .and_then(|unit| unit.flight());
        let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
        set_battle_map_hex_action(
            &scripts,
            &config,
            actor,
            map,
            BattleHexCoordinate { x: 0, y: 11 },
            BattleHex::new(Terrain::Grassland, 9),
        )
        .unwrap();
        assert_eq!(
            battle_unit_altitude(&scripts.world(), unit).unwrap(),
            altitude
        );
        if hover {
            assert!(!scripts.world().btech.vehicles()[&unit].under_bridge());
        } else {
            assert_eq!(
                scripts.world().btech.constructed_units()[&unit].flight(),
                flight
            );
        }
        let saved = scripts.world().clone();
        saved.validate(&config).unwrap();
        persistence::save(&config.database(), &saved).await.unwrap();
        assert_eq!(
            persistence::load(&config.database()).await.unwrap().btech,
            saved.btech
        );
    }
}

/// A notification failure restores terrain and altitude overrides, leaving the persisted baseline intact.
#[tokio::test]
async fn failed_edit_publication_restores_state() {
    let (dir, config, mut world, unit, _, _) = firing::fixture_with_target(
        &firing::templates()[0],
        None,
        include_str!("../game/mechs/AS7-D.toml"),
    )
    .await;
    let map = world.btech.units()[&unit].map.unwrap();
    let actor = operator(&mut world, &config, map);
    persistence::save(&config.database(), &world).await.unwrap();
    let path = dir.path().join("stompymux.toml");
    let mut table: toml::Table = std::fs::read_to_string(&path).unwrap().parse().unwrap();
    table
        .entry("runtime")
        .or_insert(toml::Value::Table(toml::Table::new()))
        .as_table_mut()
        .unwrap()
        .insert("output_message_limit".into(), 1.into());
    std::fs::write(path, toml::to_string(&table).unwrap()).unwrap();
    let config = Config::load(dir.path()).unwrap();
    let before = world.btech.clone();
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
    let error = set_battle_map_hex_action(
        &scripts,
        &config,
        actor,
        map,
        BattleHexCoordinate { x: 0, y: 11 },
        BattleHex::new(Terrain::Ice, 4),
    )
    .unwrap_err();
    assert!(error.to_string().contains("output limit"), "{error:#}");
    assert_eq!(scripts.world().btech, before);
    assert!(scripts.drain_outbox().is_empty());
    assert_eq!(
        persistence::load(&config.database()).await.unwrap().btech,
        before
    );
}

/// Existing landing restrictions and independent row fields survive live edits and terrain reload.
#[tokio::test]
async fn terrain_saves_preserve_persisted_landing_exclusions() {
    use sqlx::Connection;
    for source in firing::templates() {
        let (_dir, config, mut world, unit, target, _) =
            firing::fixture_with_target(&source, None, include_str!("../game/mechs/AS7-D.toml"))
                .await;
        let map = world.btech.units()[&unit].map.unwrap();
        let actor = operator(&mut world, &config, map);
        let zone = BattleLandingExclusion {
            coordinate: BattleHexCoordinate { x: 0, y: 11 },
            radius: 3,
            exempt_team: 7,
            owner: actor,
            data_short: 0,
        };
        set_battle_landing_exclusion(&mut world, map, 7, Some(zone)).unwrap();
        persistence::save(&config.database(), &world).await.unwrap();
        let mut sql =
            sqlx::SqliteConnection::connect(&format!("sqlite:{}", config.database().display()))
                .await
                .unwrap();
        sqlx::query("UPDATE btech_map_objects SET data_short=77 WHERE map_dbref=? AND object_type=9 AND ordinal=7")
            .bind(map.0).execute(&mut sql).await.unwrap();
        world = persistence::load(&config.database()).await.unwrap();
        let zone = BattleLandingExclusion {
            data_short: 77,
            ..zone
        };
        let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
        let output = support::run_text(&scripts, &config, actor, 1, "addhex 0 11 ^ 2");
        assert!(output.contains("Hex set"), "{output}");
        let edited = scripts.world().clone();
        persistence::save(&config.database(), &edited)
            .await
            .unwrap();
        assert_eq!(
            persistence::load(&config.database()).await.unwrap().btech,
            edited.btech
        );
        assert_eq!(edited.btech.maps()[&map].landing_exclusions()[&7], zone);
        assert_eq!(sqlx::query_scalar::<_, i64>("SELECT data_short FROM btech_map_objects WHERE map_dbref=? AND object_type=9 AND ordinal=7")
            .bind(map.0).fetch_one(&mut sql).await.unwrap(), 77);
        clear_battle_map_units_action(&scripts, &config, actor, map).unwrap();
        let mut reloaded = scripts.world().clone();
        assert_eq!(reloaded.btech.units()[&target].map, None);
        reload_battle_map(
            &mut reloaded,
            map,
            "replacement",
            BattleMapAsset::from_cells(&format!("1 12\n{}", "#1\n".repeat(12))).unwrap(),
        )
        .unwrap();
        persistence::save(&config.database(), &reloaded)
            .await
            .unwrap();
        assert_eq!(
            persistence::load(&config.database()).await.unwrap().btech,
            reloaded.btech
        );
        assert_eq!(reloaded.btech.maps()[&map].landing_exclusions()[&7], zone);
        assert_eq!(sqlx::query_scalar::<_, i64>("SELECT data_short FROM btech_map_objects WHERE map_dbref=? AND object_type=9 AND ordinal=7")
            .bind(map.0).fetch_one(&mut sql).await.unwrap(), 77);
    }
}
