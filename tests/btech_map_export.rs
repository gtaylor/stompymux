//! Map export encodes transient effects and metadata without mutating the running map.
use crate::support;
use stompymux_rs::*;

/// Export every tile spelling, including the distinct permanent and temporary effect cases.
#[tokio::test]
async fn export_terrain_effects_and_metadata_match_asset_contract() {
    let (_dir, config, mut world) = support::isolated_world().await;
    let map = world.create(&config, "Export field".into(), Kind::Room);
    create_battle_map(
        &mut world,
        map,
        "export",
        BattleMapAsset::parse("5 1\n.0#1`2&3.4\n").unwrap(),
    )
    .unwrap();
    for flags in [0, 1, 2, 8, 9, 10] {
        let mut candidate = world.clone();
        let mut state = serde_json::to_value(&candidate.btech).unwrap();
        state["maps"][map.0.to_string()]["flags"] = flags.into();
        state["maps"][map.0.to_string()]["gravity"] = 50.into();
        state["maps"][map.0.to_string()]["temperature"] = (-40).into();
        candidate.btech = serde_json::from_value(state).unwrap();
        set_map_decoration(
            &mut candidate,
            map,
            BattleHexCoordinate { x: 0, y: 0 },
            Some(BattleDecoration::new(BattleDecorationKind::Fire, 30, None)),
        )
        .unwrap();
        set_map_decoration(
            &mut candidate,
            map,
            BattleHexCoordinate { x: 1, y: 0 },
            Some(BattleDecoration::new(BattleDecorationKind::Smoke, 30, None)),
        )
        .unwrap();
        let before = candidate.btech.clone();
        let export = candidate.btech.maps()[&map].export_asset().unwrap();
        assert_eq!(candidate.btech, before);
        let permanent = flags & 8 != 0;
        let mut expected = format!("5 1\n>0#1`2{}3.4\n", if permanent { '&' } else { '.' });
        // Non-default conditions are saved even when no flag is set.
        expected.push_str(&format!("{}: 50 -40\n", flags & !1));
        assert_eq!(export.source, expected);
        assert_eq!(
            export.stale_effects,
            if permanent {
                vec![]
            } else {
                vec![BattleHexCoordinate { x: 3, y: 0 }]
            }
        );
        let decoded = BattleMapAsset::parse(&export.source).unwrap();
        assert_eq!(decoded.hex(0, 0).unwrap().terrain(), Terrain::Grassland);
        assert_eq!(decoded.hex(1, 0).unwrap().terrain(), Terrain::Road);
        assert_eq!(decoded.flags, flags & !1);
        assert_eq!((decoded.gravity, decoded.temperature), (50, -40));
        persistence::save(&config.database(), &candidate)
            .await
            .unwrap();
        assert_eq!(
            persistence::load(&config.database())
                .await
                .unwrap()
                .btech
                .maps()[&map]
                .export_asset()
                .unwrap(),
            export
        );
    }
}

/// Explicit base smoke is stale; smoke over fire exports the underlying fire without another fire pass.
#[tokio::test]
async fn export_base_smoke_and_all_canonical_tiles() {
    let (_dir, config, mut world) = support::isolated_world().await;
    let map = world.create(&config, "All terrain".into(), Kind::Room);
    create_battle_map(
        &mut world,
        map,
        "export",
        BattleMapAsset::parse("15 1\n.0#1`2\"3~4-5/6}7%8^9&0.1+2@3=4\n8: 100 20\n").unwrap(),
    )
    .unwrap();
    let mut state = serde_json::to_value(&world.btech).unwrap();
    crate::support::set_hex_terrain(
        &mut state["maps"][map.0.to_string()]["terrain"][11],
        stompymux_rs::Terrain::Smoke,
    );
    world.btech = serde_json::from_value(state).unwrap();
    set_map_decoration(
        &mut world,
        map,
        BattleHexCoordinate { x: 10, y: 0 },
        Some(BattleDecoration::new(BattleDecorationKind::Smoke, 30, None)),
    )
    .unwrap();
    let export = world.btech.maps()[&map].export_asset().unwrap();
    assert_eq!(
        export.source,
        "15 1\n.0#1`2\"3~4-5/6}7%8^9&0.1+2@3=4\n8: 100 20\n"
    );
    assert_eq!(
        export.stale_effects,
        vec![BattleHexCoordinate { x: 11, y: 0 }]
    );
    BattleMapAsset::parse(&export.source).unwrap();
    let mut state = serde_json::to_value(&world.btech).unwrap();
    state["maps"][map.0.to_string()]["flags"] = 0.into();
    world.btech = serde_json::from_value(state).unwrap();
    let unflagged = world.btech.maps()[&map].export_asset().unwrap();
    assert_eq!(
        unflagged.source,
        export.source.strip_suffix("8: 100 20\n").unwrap()
    );
    assert_eq!(unflagged.stale_effects, export.stale_effects);
    // A single non-default condition is enough to write the line.
    let mut state = serde_json::to_value(&world.btech).unwrap();
    state["maps"][map.0.to_string()]["temperature"] = (-5).into();
    world.btech = serde_json::from_value(state).unwrap();
    let cold = world.btech.maps()[&map].export_asset().unwrap();
    assert_eq!(cold.source, format!("{}0: 100 -5\n", unflagged.source));
    let decoded = BattleMapAsset::parse(&cold.source).unwrap();
    assert_eq!(
        (decoded.flags, decoded.gravity, decoded.temperature),
        (0, 100, -5)
    );
}
