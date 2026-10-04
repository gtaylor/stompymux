//! Map export writes terrain, permanent fire and smoke and settings without mutating the map.
use crate::support;
use stompymux_rs::*;

/// Fire and smoke that will burn out save the terrain beneath them; permanent fire drawn in the
/// map file and permanent smoke over woods are kept in the overlay grid; settings are always
/// written.
#[tokio::test]
async fn export_terrain_effects_and_metadata_match_asset_contract() {
    let (_dir, config, mut world) = support::isolated_world().await;
    let map = world.create(&config, "Export field".into(), Kind::Room);
    create_battle_map(
        &mut world,
        map,
        "export",
        BattleMapAsset::from_cells("5 1\n.0#1`2&3.4\n").unwrap(),
    )
    .unwrap();
    support::seed_object_dice(&mut world, map, support::FIXTURE_DICE_SEED);
    for flags in [0, 1, 2, 4, 5, 6] {
        let mut candidate = world.clone();
        let mut state = serde_json::to_value(&candidate.btech).unwrap();
        state["maps"][map.0.to_string()]["flags"] = flags.into();
        state["maps"][map.0.to_string()]["gravity"] = 50.into();
        state["maps"][map.0.to_string()]["temperature"] = (-40).into();
        candidate.btech = serde_json::from_value(state).unwrap();
        for (x, kind, remaining) in [
            (0, BattleDecorationKind::Fire, 30),
            (1, BattleDecorationKind::Smoke, 30),
            (2, BattleDecorationKind::Smoke, 0),
        ] {
            set_map_decoration(
                &mut candidate,
                map,
                BattleHexCoordinate { x, y: 0 },
                Some(BattleDecoration::new(kind, remaining, None)),
            )
            .unwrap();
        }
        let before = candidate.btech.clone();
        let export = candidate.btech.maps()[&map].export_asset().unwrap();
        assert_eq!(candidate.btech, before);
        let decoded = BattleMapAsset::parse(&export).unwrap();
        assert_eq!(
            decoded.hexes.as_slice(),
            [
                BattleHex::new(Terrain::Grassland, 0),
                BattleHex::new(Terrain::Road, 1),
                BattleHex::new(Terrain::LightForest, 2)
                    .with_overlay(Some(BattleDecorationKind::Smoke)),
                BattleHex::new(Terrain::Fire, 3),
                BattleHex::new(Terrain::Grassland, 4),
            ]
        );
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

/// Every terrain survives export. Permanent smoke added over clear ground is saved; fire that
/// smoke has replaced is not.
#[tokio::test]
async fn export_base_smoke_and_all_canonical_tiles() {
    let (_dir, config, mut world) = support::isolated_world().await;
    let map = world.create(&config, "All terrain".into(), Kind::Room);
    let cells = "15 1\n.0#1`2\"3~4-5/6}7%8^9&0.1+2@3=4\n";
    create_battle_map(
        &mut world,
        map,
        "export",
        BattleMapAsset::from_cells(cells).unwrap(),
    )
    .unwrap();
    assert_eq!(
        world.btech.maps()[&map].base_hex(10, 0).unwrap(),
        BattleHex::new(Terrain::Grassland, 0)
    );
    for (x, remaining) in [(10, 30), (11, 0)] {
        set_map_decoration(
            &mut world,
            map,
            BattleHexCoordinate { x, y: 0 },
            Some(BattleDecoration::new(
                BattleDecorationKind::Smoke,
                remaining,
                None,
            )),
        )
        .unwrap();
    }
    let export = world.btech.maps()[&map].export_asset().unwrap();
    let mut expected = BattleMapAsset::from_cells(cells).unwrap();
    std::sync::Arc::make_mut(&mut expected.hexes)[10] = BattleHex::new(Terrain::Grassland, 0);
    std::sync::Arc::make_mut(&mut expected.hexes)[11] = BattleHex::new(Terrain::Smoke, 1);
    assert_eq!(BattleMapAsset::parse(&export).unwrap(), expected);
}
