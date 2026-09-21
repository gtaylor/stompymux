//! Bridge activation preserves parser bytes and checks opposite water corridors independently.
use crate::support;
use stompymux_rs::*;

/// Vertical corridors characterize every accepted distance and both forms of water.
#[test]
fn road_spans_require_two_water_ends_within_three_steps() {
    for top in 1..=4 {
        for bottom in 1..=4 {
            for water in ['~', '-'] {
                let height = top + bottom + 1;
                let rows = (0..height)
                    .map(|y| {
                        format!(
                            "{}2\n",
                            if y == 0 || y == height - 1 {
                                water
                            } else {
                                '#'
                            }
                        )
                    })
                    .collect::<String>();
                let mut asset = BattleMapAsset::parse(&format!("1 {height}\n{rows}")).unwrap();
                let original = asset.clone();
                asset.generate_bridges().unwrap();
                assert_eq!(
                    asset.hex(0, top).unwrap().terrain,
                    if top <= 3 && bottom <= 3 {
                        Terrain::Bridge
                    } else {
                        Terrain::Road
                    }
                );
                assert_eq!(asset.hex(0, top).unwrap().elevation, 2);
                assert_eq!(original.hex(0, top).unwrap().terrain, Terrain::Road);
                let mut disabled = original;
                disabled.flags = 128;
                assert_eq!(disabled.generate_bridges().unwrap(), 0);
                assert_eq!(disabled.hex(0, top).unwrap().terrain, Terrain::Road);
                assert_eq!(asset.generate_bridges().unwrap(), 0);
            }
        }
    }
    for obstruction in ['.', '%', '?', '^', '@'] {
        let mut asset =
            BattleMapAsset::parse(&format!("1 5\n~0\n{obstruction}0\n#0\n#0\n~0\n")).unwrap();
        asset.generate_bridges().unwrap();
        assert_eq!(asset.hex(0, 2).unwrap().terrain, Terrain::Road);
    }
}

/// The special western-edge step is distinct from ordinary odd-column hex neighbors.
#[test]
fn diagonal_corridors_and_western_edge_use_asset_search_coordinates() {
    for source in [
        "3 3\n.0.0.0\n~0#0~0\n.0.0.0\n",
        "5 3\n.0.0.0.0.0\n.0.0#0~0.0\n.0~0.0.0.0\n",
        "3 3\n~0.0.0\n.0#0.0\n.0.0~0\n",
    ] {
        let mut asset = BattleMapAsset::parse(source).unwrap();
        let x = if asset.width == 3 { 1 } else { 2 };
        assert_eq!(asset.generate_bridges().unwrap(), 1);
        assert_eq!(asset.hex(x, 1).unwrap().terrain, Terrain::Bridge);
    }
}

/// Creation and terrain reload activate the same rule, and persistence retains generated bridges.
#[tokio::test]
async fn map_activation_generates_bridges_and_persists_them() {
    let (_dir, config, mut world) = support::isolated_world().await;
    let map = world.create(&config, "Bridge map".into(), Kind::Room);
    let asset = BattleMapAsset::parse("1 3\n~0\n#2\n-0\n").unwrap();
    create_battle_map(&mut world, map, "span", asset.clone()).unwrap();
    assert_eq!(
        world.btech.maps()[&map].base_hex(0, 1).unwrap().terrain,
        Terrain::Bridge
    );
    persistence::save(&config.database(), &world).await.unwrap();
    set_battle_map_cloud_base(&mut world, ObjectId(1), map, 37).unwrap();
    let mut disabled = asset.clone();
    disabled.flags = 128;
    reload_battle_map(&mut world, map, "disabled", disabled).unwrap();
    assert_eq!(
        world.btech.maps()[&map].base_hex(0, 1).unwrap().terrain,
        Terrain::Road
    );
    reload_battle_map(&mut world, map, "span", asset).unwrap();
    assert_eq!(world.btech.maps()[&map].cloud_base, 37);
    persistence::save(&config.database(), &world).await.unwrap();
    assert_eq!(
        persistence::load(&config.database()).await.unwrap().btech,
        world.btech
    );
}
