//! Fire and smoke over hexes that are not clear ground: burnout, spreading, heat, steam and
//! the decorations sharing a hex all respect the layers beneath the overlay.
use crate::support;
use std::sync::Arc;
use stompymux_rs::*;

/// A map built from `cells`, with each hex replaced by `layers` of it.
async fn field(
    cells: &str,
    layers: impl Fn(BattleHex) -> BattleHex,
) -> (tempfile::TempDir, Config, World, ObjectId) {
    let (dir, config, mut world) = support::isolated_world().await;
    let map = world.create(&config, "Fire field".into(), Kind::Room);
    let asset = BattleMapAsset::from_cells(cells).unwrap();
    let hexes = asset.hexes.iter().map(|hex| layers(*hex)).collect();
    create_battle_map(
        &mut world,
        map,
        "fire.map",
        BattleMapAsset {
            hexes: Arc::new(hexes),
            ..asset
        },
    )
    .unwrap();
    (dir, config, world, map)
}

/// Set the wind blowing over a map through its saved state.
fn wind(world: &mut World, map: ObjectId, speed: i64) {
    let mut state = serde_json::to_value(&world.btech).unwrap();
    state["maps"][map.0.to_string()]["wind_speed"] = speed.into();
    world.btech = serde_json::from_value(state).unwrap();
}

/// Light a fire with `seconds` of fuel at a hex.
fn ignite(world: &mut World, map: ObjectId, x: i32, y: i32, seconds: i64) {
    set_map_decoration(
        world,
        map,
        BattleHexCoordinate { x, y },
        Some(BattleDecoration::new(
            BattleDecorationKind::Fire,
            seconds,
            None,
        )),
    )
    .unwrap();
}

/// The overlay at a hex.
fn overlay(world: &World, map: ObjectId, x: i32, y: i32) -> Option<BattleDecoration> {
    world.btech.maps()[&map]
        .decoration(BattleHexCoordinate { x, y })
        .unwrap()
}

/// Light woods over a road burn away to the road, not to clear or rough ground.
#[tokio::test]
async fn burnt_out_woods_leave_the_ground_they_grew_on() {
    let (_dir, _config, mut world, map) =
        field("1 1\n#2\n", |hex| hex.with_woods(Some(BattleWoods::Light))).await;
    let road = BattleHex::new(Terrain::Road, 2);
    assert_eq!(
        world.btech.maps()[&map].base_hex(0, 0).unwrap(),
        road.with_woods(Some(BattleWoods::Light))
    );
    ignite(&mut world, map, 0, 0, 30);
    for _ in 0..600 {
        advance_map_fire(&mut world).unwrap();
        if overlay(&world, map, 0, 0).is_none() {
            break;
        }
    }
    assert_eq!(overlay(&world, map, 0, 0), None);
    assert_eq!(world.btech.maps()[&map].base_hex(0, 0).unwrap(), road);
}

/// A fire spreading into woods that are already burning leaves that fire's own record and
/// timer alone instead of lighting it afresh.
#[tokio::test]
async fn spreading_fire_never_relights_a_burning_hex() {
    let (_dir, _config, mut world, map) = field(
        "4 4\n\"0\"0\"0\"0\n\"0\"0\"0\"0\n\"0\"0\"0\"0\n\"0\"0\"0\"0\n",
        |hex| hex,
    )
    .await;
    wind(&mut world, map, 60);
    for y in 0..4 {
        for x in 0..4 {
            ignite(&mut world, map, x, y, 600);
        }
    }
    let orders = |world: &World| -> Vec<_> {
        (0..16)
            .map(|index| overlay(world, map, index % 4, index / 4).map(|fire| fire.order))
            .collect()
    };
    let lit = orders(&world);
    for _ in 0..300 {
        advance_map_fire(&mut world).unwrap();
        assert_eq!(orders(&world), lit);
    }
}

/// A new fire or smoke cloud replaces stored fire and smoke records at its hex but leaves a
/// generic decoration there, with the terrain it restores.
#[tokio::test]
async fn new_fire_keeps_the_decorations_beneath_it() {
    let (_dir, _config, mut world, map) = field("2 1\n.0.0\n", |hex| hex).await;
    let record = BattleStaticDecoration {
        coordinate: BattleHexCoordinate { x: 1, y: 0 },
        restored_terrain: None,
        object: ObjectId(1),
        duration: 0,
        scalar: 0,
    };
    let decoration = BattleStaticDecoration {
        restored_terrain: Some(Terrain::Water),
        ..record
    };
    set_battle_static_decoration(
        &mut world,
        map,
        BattleStaticDecorationKind::Smoke,
        1,
        Some(record),
    )
    .unwrap();
    set_battle_static_decoration(
        &mut world,
        map,
        BattleStaticDecorationKind::Decoration,
        2,
        Some(decoration),
    )
    .unwrap();
    ignite(&mut world, map, 1, 0, 30);
    let stored = &world.btech.maps()[&map];
    assert!(
        stored
            .static_decorations(BattleStaticDecorationKind::Smoke)
            .is_empty()
    );
    assert_eq!(
        stored
            .static_decorations(BattleStaticDecorationKind::Decoration)
            .get(&2),
        Some(&decoration)
    );
}

/// A running Jenner at a hex on `map`, with steady dice.
fn mech(world: &mut World, config: &Config, map: ObjectId, x: i64, y: i64) -> ObjectId {
    let id = world.create(config, format!("Mech {x},{y}"), Kind::Thing);
    world.objects.get_mut(&id).unwrap().home = Some(ObjectId(config.home()));
    create_battle_unit(
        world,
        id,
        BattleTemplate::parse("JR7-D", include_str!("fixtures/btech/mechs/JR7-D.toml")).unwrap(),
    )
    .unwrap();
    place_battle_unit(world, id, map, x, y).unwrap();
    let mut state = serde_json::to_value(&world.btech).unwrap();
    let unit = &mut state["constructed"][id.0.to_string()];
    unit["power"] = serde_json::to_value(BattlePower::Running).unwrap();
    unit["dice"] = serde_json::to_value(BattleDice::seeded([19; 32])).unwrap();
    world.btech = serde_json::from_value(state).unwrap();
    id
}

/// Fire adds heat to a Mech standing or wading in the flames, but not to one under the water
/// or beneath the bridge deck the fire burns on.
#[tokio::test]
async fn fire_heats_only_mechs_in_the_flames() {
    // Ground, shallow water, deep water, then two bridge decks four levels over shallow
    // water: a Mech stands on the first deck and on the river bed under the second.
    let (_dir, config, mut world, map) = field("5 1\n.0~1~2/4/4\n", |hex| hex).await;
    let mut cases = Vec::new();
    for (x, burns) in [(0, true), (1, true), (2, false), (3, true), (4, false)] {
        let id = mech(&mut world, &config, map, x, 0);
        if x == 4 {
            let mut state = serde_json::to_value(&world.btech).unwrap();
            state["constructed"][id.0.to_string()]["ground_elevation"] = (-1).into();
            world.btech = serde_json::from_value(state).unwrap();
        }
        let cool = world.btech.constructed_units()[&id].heat_rates(&world);
        ignite(&mut world, map, x as i32, 0, 30);
        let hot = world.btech.constructed_units()[&id].heat_rates(&world);
        cases.push((x, hot.production - cool.production, burns));
    }
    for (x, extra, burns) in cases {
        assert_eq!(extra, if burns { 5.0 } else { 0.0 }, "hex {x}");
    }
}

/// The steam of an inferno quenched in water does not put out a fire burning on that water.
#[tokio::test]
async fn steam_leaves_a_fire_burning() {
    let (_dir, config, mut world, map) = field("1 1\n~2\n", |hex| hex).await;
    let id = mech(&mut world, &config, map, 0, 0);
    ignite(&mut world, map, 0, 0, 30);
    apply_inferno_burn(&mut world, id, 30).unwrap();
    assert!(
        !extinguish_inferno_in_water(&mut world, id)
            .unwrap()
            .is_empty()
    );
    assert_eq!(world.btech.constructed_units()[&id].inferno_remaining(), 0);
    assert_eq!(
        overlay(&world, map, 0, 0).map(|effect| effect.kind),
        Some(BattleDecorationKind::Fire)
    );
}
