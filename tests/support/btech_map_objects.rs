//! Shared map-object fixture for selection, listing and atomic operator actions.
use crate::support;
use stompymux_rs::*;

pub async fn fixture() -> (tempfile::TempDir, Config, World, ObjectId, ObjectId) {
    let (dir, config, mut world) = support::isolated_world().await;
    let map = world.create(&config, "Exterior".into(), Kind::Room);
    let interior = world.create(&config, "Interior".into(), Kind::Room);
    for id in [map, interior] {
        create_battle_map(
            &mut world,
            id,
            "grid",
            BattleMapAsset::parse("3 3\n.0.0.0\n.0.0.0\n.0.0.0\n").unwrap(),
        )
        .unwrap();
    }
    world.objects.get_mut(&ObjectId(1)).unwrap().location = Some(map);
    world
        .objects
        .get_mut(&ObjectId(2))
        .unwrap()
        .flags
        .remove(Flag::Wizard);
    let p = BattleHexCoordinate { x: 1, y: 1 };
    let other = BattleHexCoordinate { x: 2, y: 2 };
    set_map_decoration(
        &mut world,
        map,
        p,
        Some(BattleDecoration::new(BattleDecorationKind::Fire, 0, None)),
    )
    .unwrap();
    set_map_decoration(
        &mut world,
        map,
        other,
        Some(BattleDecoration::new(BattleDecorationKind::Smoke, 20, None)),
    )
    .unwrap();
    set_battle_static_decoration(
        &mut world,
        map,
        BattleStaticDecorationKind::Decoration,
        0,
        Some(BattleStaticDecoration {
            coordinate: p,
            restored_terrain: Terrain::Water,
            object: ObjectId(0),
            duration: 0,
            scalar: 0,
        }),
    )
    .unwrap();
    for ordinal in [0, 4] {
        set_minefield(
            &mut world,
            map,
            ordinal,
            Some(BattleMinefield {
                coordinate: p,
                kind: BattleMineKind::Standard,
                strength: 5,
                extra: 0,
                owner: ObjectId(1),
            }),
        )
        .unwrap();
        set_battle_landing_exclusion(
            &mut world,
            map,
            ordinal,
            Some(BattleLandingExclusion {
                coordinate: p,
                radius: 1,
                exempt_team: 0,
                owner: ObjectId(1),
                data_short: 0,
            }),
        )
        .unwrap();
    }
    set_building_entrance(
        &mut world,
        map,
        0,
        Some(BattleBuildingEntrance {
            coordinate: p,
            interior,
            data_char: 0,
            data_short: 0,
            data_int: 0,
        }),
    )
    .unwrap();
    set_battle_building_exit(&mut world, interior, 0, Some(map)).unwrap();
    set_battle_building_return_link(
        &mut world,
        map,
        0,
        Some(BattleBuildingExit {
            coordinate: p,
            destination: interior,
            data_char: 0,
            data_short: 0,
            data_int: 0,
        }),
    )
    .unwrap();
    set_battle_building_entry_point(
        &mut world,
        map,
        0,
        Some(BattleBuildingEntryPoint {
            coordinate: p,
            direction: b'n',
            object: ObjectId(-1),
            data_short: 0,
            data_int: 0,
        }),
    )
    .unwrap();
    set_battle_linked_marker(&mut world, map, 0, Some(p)).unwrap();
    set_battle_linked_marker(&mut world, map, 1, Some(other)).unwrap();
    persistence::save(&config.database(), &world).await.unwrap();
    (dir, config, world, map, interior)
}
