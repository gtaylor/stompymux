//! Fire and smoke over hexes that are not clear ground: burnout, spreading, heat, steam, the
//! decorations sharing a hex and the map displays all respect the layers beneath the overlay.
use crate::support;
use std::sync::Arc;
use stompymux_rs::*;

/// A map built from `cells`, with each hex replaced by `layers` of it.
async fn field(
    cells: &str,
    layers: impl Fn(Hex) -> Hex,
) -> (tempfile::TempDir, Config, World, ObjectId) {
    let (dir, config, mut world) = support::isolated_world().await;
    let map = world.create(&config, "Fire field".into(), Kind::Room);
    let asset = MapAsset::from_cells(cells).unwrap();
    let hexes = asset.hexes.iter().map(|hex| layers(*hex)).collect();
    create_battle_map(
        &mut world,
        map,
        "fire.map",
        MapAsset {
            hexes: Arc::new(hexes),
            ..asset
        },
    )
    .unwrap();
    support::seed_object_dice(&mut world, map, support::FIXTURE_DICE_SEED);
    (dir, config, world, map)
}

/// Set the wind blowing over a map through its saved state.
fn wind(world: &mut World, map: ObjectId, speed: i64) {
    world
        .btech
        .rewrite_map_record(map, |record| {
            record["wind_speed"] = speed.into();
        })
        .unwrap();
}

/// Light a fire with `seconds` of fuel at a hex.
fn ignite(world: &mut World, map: ObjectId, x: i32, y: i32, seconds: i64) {
    set_map_decoration(
        world,
        map,
        HexCoordinate { x, y },
        Some(Decoration::new(DecorationKind::Fire, seconds, None)),
    )
    .unwrap();
}

/// The overlay at a hex.
fn overlay(world: &World, map: ObjectId, x: i32, y: i32) -> Option<Decoration> {
    world.btech.maps()[&map]
        .decoration(HexCoordinate { x, y })
        .unwrap()
}

/// Light woods over a road burn away to the road, not to clear or rough ground.
#[tokio::test]
async fn burnt_out_woods_leave_the_ground_they_grew_on() {
    let (_dir, _config, mut world, map) =
        field("1 1\n#2\n", |hex| hex.with_woods(Some(Woods::Light))).await;
    let road = Hex::new(Terrain::Road, 2);
    assert_eq!(
        world.btech.maps()[&map].base_hex(0, 0).unwrap(),
        road.with_woods(Some(Woods::Light))
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
    let record = StaticDecoration {
        coordinate: HexCoordinate { x: 1, y: 0 },
        restored_terrain: None,
        object: ObjectId(1),
        duration: 0,
        scalar: 0,
    };
    let decoration = StaticDecoration {
        restored_terrain: Some(Terrain::Water),
        ..record
    };
    set_battle_static_decoration(
        &mut world,
        map,
        StaticDecorationKind::Smoke,
        1,
        Some(record),
    )
    .unwrap();
    set_battle_static_decoration(
        &mut world,
        map,
        StaticDecorationKind::Decoration,
        2,
        Some(decoration),
    )
    .unwrap();
    ignite(&mut world, map, 1, 0, 30);
    let stored = &world.btech.maps()[&map];
    assert!(
        stored
            .static_decorations(StaticDecorationKind::Smoke)
            .is_empty()
    );
    assert_eq!(
        stored
            .static_decorations(StaticDecorationKind::Decoration)
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
        MechTemplate::parse("JR7-D", include_str!("fixtures/btech/units/JR7-D.toml")).unwrap(),
    )
    .unwrap();
    support::seed_object_dice(world, id, support::FIXTURE_DICE_SEED);
    place_battle_unit(world, id, map, x, y).unwrap();
    world
        .btech
        .rewrite_unit_record(id, |record| {
            let unit = record;
            unit["power"] = serde_json::to_value(Power::Running).unwrap();
            unit["dice"] = serde_json::to_value(Dice::seeded([19; 32])).unwrap();
        })
        .unwrap();
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
            world
                .btech
                .rewrite_unit_record(id, |record| {
                    record["ground_elevation"] = (-1).into();
                })
                .unwrap();
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
        Some(DecorationKind::Fire)
    );
}

/// A piloted Jenner standing in smoky heavy woods beside a burning bridge, with ANSI colours.
async fn smoky_crossing() -> (tempfile::TempDir, World, ObjectId) {
    let (dir, config, mut world, map) = field("3 3\n\"0/2.0\n.0.0.0\n.0.0.0\n", |hex| hex).await;
    let id = mech(&mut world, &config, map, 0, 0);
    world.objects.get_mut(&ObjectId(1)).unwrap().location = Some(id);
    world
        .objects
        .get_mut(&ObjectId(1))
        .unwrap()
        .flags
        .insert(Flag::Ansi);
    assign_battle_pilot(&mut world, id, ObjectId(1)).unwrap();
    support::seed_object_dice(&mut world, ObjectId(1), support::FIXTURE_DICE_SEED);
    set_map_decoration(
        &mut world,
        map,
        HexCoordinate { x: 0, y: 0 },
        Some(Decoration::new(DecorationKind::Smoke, 60, None)),
    )
    .unwrap();
    ignite(&mut world, map, 1, 0, 60);
    (dir, world, id)
}

/// The tactical map draws fire and smoke in the top of a hex and keeps the terrain beneath in
/// the bottom, with its own glyph, colour and height.
#[tokio::test]
async fn tactical_map_shows_the_terrain_beneath_fire_and_smoke() {
    let (_dir, world, id) = smoky_crossing().await;
    let report =
        battle_tactical_map(&world, id, ObjectId(1), "", ViewDimensions::default()).unwrap();
    let plain = text::plain(&report.text);
    let lines: Vec<_> = plain.lines().collect();
    let hex = |x: usize, y: usize| {
        let row = 3 + y * 2 + usize::from(x.is_multiple_of(2));
        let column = 5 + x * 3;
        (
            &lines[row][column..column + 2],
            &lines[row + 1][column..column + 2],
        )
    };
    // Our own marker covers the woods' top row; the woods stay underneath.
    assert_eq!(hex(0, 0).1, "\"\"", "{plain}");
    // The bridge deck burns: fire above, the deck and its height below.
    assert_eq!(hex(1, 0), ("&&", "+2"), "{plain}");
    // Each row takes its own colour: red fire, and green woods under the grey smoke.
    for styled in ["[fg=red bold]&&[reset]", "[fg=green]\"\"[reset]"] {
        assert!(report.text.contains(styled), "{}", report.text);
    }
}

/// Long-range terrain shows fire and smoke, and the U mode shows the terrain beneath them.
#[tokio::test]
async fn long_range_u_mode_shows_the_terrain_beneath_fire_and_smoke() {
    let (_dir, world, id) = smoky_crossing().await;
    let map = |mode| {
        text::plain(
            &battle_long_range_map(&world, id, ObjectId(1), mode, "", ViewDimensions::default())
                .unwrap()
                .text,
        )
    };
    let terrain = map(LongRangeMode::Terrain);
    assert!(terrain.contains('&') && terrain.contains(':'), "{terrain}");
    let beneath = map(LongRangeMode::UnderlyingTerrain);
    assert!(
        !beneath.contains('&') && !beneath.contains(':'),
        "{beneath}"
    );
    assert!(beneath.contains('"') && beneath.contains('/'), "{beneath}");
}

/// The navigation readout names the terrain under a unit and the smoke over it separately.
#[tokio::test]
async fn navigation_names_terrain_and_smoke_separately() {
    let (_dir, world, id) = smoky_crossing().await;
    let text = text::plain(&battle_navigate(&world, id, ObjectId(1), "").unwrap().text);
    assert!(text.contains("Terrain:   Heavy Forest"), "{text}");
    assert!(text.contains("Effect:           Smoke"), "{text}");
}
