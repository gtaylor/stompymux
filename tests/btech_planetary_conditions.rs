//! Tactical Operations terrain and planetary-condition modifiers in live play: magma and snow
//! heat, the piloting modifiers terrain and wind add, wind and gravity aim, magma burns and
//! crust breaking, and the roll ultra rubble takes to cross.
use crate::btech_motion_common::{RULES, fixture, fixture_assets, fixture_source};
use crate::support;
use crate::support::btech_firing as firing;
use std::{cell::RefCell, rc::Rc};
use stompymux_rs::*;

/// The map a fixture unit stands on.
fn unit_map(world: &World, id: ObjectId) -> ObjectId {
    world.btech.units()[&id].map.unwrap()
}

/// Replace the hex under `id` with `terrain` at ground level, as an operator would.
fn set_hex_under(world: World, config: &Config, id: ObjectId, terrain: Terrain) -> World {
    let position = world
        .btech
        .constructed_units()
        .get(&id)
        .and_then(|unit| unit.position())
        .or_else(|| world.btech.vehicles()[&id].position())
        .unwrap();
    let (map, x, y) = (position.map, i32::from(position.x), i32::from(position.y));
    let scripts = Scripts::new(config, Rc::new(RefCell::new(world))).unwrap();
    set_battle_map_hex_action(
        &scripts,
        config,
        ObjectId(1),
        map,
        HexCoordinate { x, y },
        Hex::new(terrain, 0),
    )
    .unwrap();
    scripts.world().clone()
}

/// Crust and liquid magma heat a 'Mech by five and ten points a turn, which no cap limits,
/// and deep snow cools one with heat sinks in its legs.
#[tokio::test]
async fn magma_heats_and_deep_snow_cools_mechs() {
    let rates = |terrain| async move {
        let (_dir, _config, world, id) = fixture(terrain).await;
        world.btech.constructed_units()[&id].heat_rates(&world)
    };
    let clear = rates('.').await;
    assert_eq!(rates('m').await.production, clear.production + 5.0);
    assert_eq!(rates('M').await.production, clear.production + 10.0);
    assert_eq!(rates('M').await.dissipation, clear.dissipation);
    // The fixture Jenner carries no heat sinks in its legs.
    assert_eq!(rates('+').await.dissipation, clear.dissipation);
    let leg_sink = include_str!("fixtures/btech/units/JR7-D.toml")
        .replace(
            "[sections.head]\narmor = 7\nslots = [\n    { at = 4, item = \"HeatSink\" },\n]",
            "[sections.head]\narmor = 7",
        )
        .replace(
            "[sections.left_leg]\narmor = 6\n",
            "[sections.left_leg]\narmor = 6\nslots = [\n    { at = 5, item = \"HeatSink\" },\n]\n",
        );
    let template = MechTemplate::parse("JR7-D", &leg_sink).unwrap();
    let cooling = |terrain: char| {
        let template = template.clone();
        async move {
            let row = format!("{terrain}0").repeat(12);
            let source = format!("12 12\n{}", format!("{row}\n").repeat(12));
            let (_dir, _config, world, id) = fixture_assets(&source, template).await;
            world.btech.constructed_units()[&id]
                .heat_rates(&world)
                .dissipation
        }
    };
    assert_eq!(cooling('+').await, cooling('.').await + 1.0);
}

/// Every piloting roll a unit makes picks up its hex's Tactical Operations modifier and gale
/// penalties, which differ between 'Mechs, tracked vehicles and hovercraft.
#[tokio::test]
async fn terrain_and_wind_modify_piloting_rolls() {
    let templates = firing::templates();
    // Jenner, tracked Demolisher and hover Demolisher, with what a strong gale adds to each.
    for (source, gale) in [(&templates[0], 1), (&templates[2], 0), (&templates[4], 2)] {
        let (_dir, config, world, unit, _, _) =
            firing::fixture_with_target(source, None, include_str!("../game/units/AS7-D.toml"))
                .await;
        let environment = |world: &mut World| {
            roll_battle_piloting(world, unit, 0, false)
                .unwrap()
                .environment
        };
        let mut clear = world.clone();
        assert_eq!(environment(&mut clear), 0);
        let mut sand = set_hex_under(world, &config, unit, Terrain::Sand);
        assert_eq!(environment(&mut sand), 1);
        let map = unit_map(&sand, unit);
        set_map_wind(&mut sand, map, 90, 80).unwrap();
        assert_eq!(environment(&mut sand), 1 + gale);
        let mut magma = set_hex_under(sand, &config, unit, Terrain::Magma);
        set_map_wind(&mut magma, map, 90, 0).unwrap();
        assert_eq!(environment(&mut magma), 4);
    }
}

/// Wind spoils missiles first and ballistic fire next, gravity spoils both, and a tornado
/// keeps missiles from firing at all.
#[tokio::test]
async fn wind_and_gravity_modify_aim_by_weapon_class() {
    let jenner = &firing::templates()[0];
    for (weapon, gale, storm, low_gravity) in [
        (Weapon::Lrm5, Some(1), Some(3), Some(2)),
        (Weapon::Ac5, Some(0), Some(2), Some(2)),
        (Weapon::MediumLaser, Some(0), Some(0), Some(0)),
    ] {
        let (_dir, _config, mut world, shooter, target, index) = firing::fixture_with_target(
            jenner,
            Some(weapon),
            include_str!("../game/units/AS7-D.toml"),
        )
        .await;
        let rules = crate::btech_motion_common::optical_aim_rules();
        let environment = |world: &World| {
            battle_pilot_aim_modifiers(world, shooter, target, index, false, rules)
                .unwrap()
                .environment
        };
        let map = unit_map(&world, shooter);
        assert_eq!(environment(&world), Some(0), "{weapon:?}");
        set_map_wind(&mut world, map, 0, 62).unwrap();
        assert_eq!(environment(&world), gale, "{weapon:?}");
        set_map_wind(&mut world, map, 0, 100).unwrap();
        assert_eq!(environment(&world), storm, "{weapon:?}");
        set_map_wind(&mut world, map, 0, 150).unwrap();
        let tornado = environment(&world);
        assert_eq!(tornado.is_none(), weapon == Weapon::Lrm5, "{weapon:?}");
        set_map_wind(&mut world, map, 0, 0).unwrap();
        set_battle_map_environment(
            &mut world,
            ObjectId(1),
            map,
            MapEnvironment {
                gravity: 60,
                temperature: 20,
                vacuum: false,
                underground: false,
            },
        )
        .unwrap();
        assert_eq!(environment(&world), low_gravity, "{weapon:?}");
    }
}

/// A 'Mech arriving in liquid magma has its legs burned, and a vehicle that enters it is
/// destroyed outright, crew and all.
#[tokio::test]
async fn liquid_magma_burns_mechs_and_destroys_vehicles() {
    let (_dir, config, world, id) = fixture('.').await;
    let before = battle_magma_snapshot(&world);
    let legs = |world: &World| {
        let unit = &world.btech.constructed_units()[&id];
        [MechSection::LeftLeg, MechSection::RightLeg]
            .map(|section| unit.sections()[&section].armor + unit.sections()[&section].internal)
    };
    let intact = legs(&world);
    let arms = world.btech.constructed_units()[&id].sections()[&MechSection::LeftArm].armor;
    let world = set_hex_under(world, &config, id, Terrain::Magma);
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
    scripts.drain_outbox();
    advance_battle_magma_action(&scripts, &config, &before).unwrap();
    let burned = legs(&scripts.world());
    for (after, before) in burned.iter().zip(intact) {
        assert!(
            *after < before && before - after >= 2,
            "{intact:?} -> {burned:?}"
        );
    }
    assert_eq!(
        scripts.world().btech.constructed_units()[&id].sections()[&MechSection::LeftArm].armor,
        arms
    );
    assert!(
        scripts
            .drain_outbox()
            .iter()
            .any(|(_, text)| text.source().contains("Molten rock sears your legs!"))
    );

    let templates = firing::templates();
    for source in [&templates[2], &templates[3], &templates[4]] {
        let (_dir, config, world, unit, _, _) =
            firing::fixture_with_target(source, None, include_str!("../game/units/AS7-D.toml"))
                .await;
        let before = battle_magma_snapshot(&world);
        let world = set_hex_under(world, &config, unit, Terrain::Magma);
        let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
        advance_battle_magma_action(&scripts, &config, &before).unwrap();
        let world = scripts.world();
        let vehicle = &world.btech.vehicles()[&unit];
        assert!(vehicle.is_destroyed() && vehicle.crew_killed());
        world.validate(&config).unwrap();
    }
}

/// Walking across magma crust breaks some of it into liquid magma for good.
#[tokio::test]
async fn walking_on_crust_can_break_it() {
    let row = "m0".repeat(12);
    let source = format!("12 40\n{}", format!("{row}\n").repeat(40));
    let (_dir, config, mut world, id) = fixture_source(&source).await;
    world
        .btech
        .rewrite_unit_record(id, |unit| {
            unit["motion"]["speed"] = 53.75.into();
            unit["motion"]["desired_speed"] = 53.75.into();
            unit["motion"]["heading"] = 180.into();
            unit["motion"]["desired_heading"] = 180.into();
        })
        .unwrap();
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
    for _ in 0..500 {
        advance_battle_motion_action(&scripts, &config, RULES).unwrap();
        if scripts.world().btech.constructed_units()[&id]
            .position()
            .unwrap()
            .y
            >= 35
        {
            break;
        }
    }
    let world = scripts.world();
    assert!(world.btech.constructed_units()[&id].position().unwrap().y >= 35);
    let map = &world.btech.maps()[&unit_map(&world, id)];
    let broken = (0..40)
        .flat_map(|y| (0..12).map(move |x| (x, y)))
        .filter(|&(x, y)| map.hex(x, y).unwrap().terrain() == Terrain::Magma)
        .count();
    assert!(broken > 0, "no crust broke");
    let output = scripts.drain_outbox();
    assert!(
        output
            .iter()
            .any(|(_, text)| text.source().contains("magma crust cracks open"))
    );
}

/// Entering ultra rubble takes a piloting roll.
#[tokio::test]
async fn ultra_rubble_takes_a_roll_to_enter() {
    let mut source = String::from("12 12\n");
    for y in 0..12 {
        for x in 0..12 {
            source.push_str(if (x, y) == (5, 4) { "!0" } else { ".0" });
        }
        source.push('\n');
    }
    let (_dir, config, mut world, id) = fixture_source(&source).await;
    world
        .objects
        .get_mut(&ObjectId(1))
        .unwrap()
        .flags
        .insert(Flag::Connected);
    walk_north(&mut world, id);
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
    let mut output = Vec::new();
    for _ in 0..40 {
        advance_battle_motion_action(&scripts, &config, RULES).unwrap();
        output.extend(scripts.drain_outbox());
        if scripts.world().btech.constructed_units()[&id]
            .position()
            .unwrap()
            .y
            == 4
        {
            break;
        }
    }
    let texts: Vec<_> = output.iter().map(|(_, text)| text.source()).collect();
    let warning = texts
        .iter()
        .position(|text| *text == "You pick your way across the shattered rubble.")
        .expect("ultra rubble warning");
    assert_eq!(texts[warning + 1], "You make a piloting skill roll!");
    support::drain_traces(&scripts, logging::TraceTopic::PilotingRolls);
}

/// Set the fixture 'Mech walking north at walking speed.
fn walk_north(world: &mut World, id: ObjectId) {
    world
        .btech
        .rewrite_unit_record(id, |unit| {
            unit["motion"]["speed"] = 21.5.into();
            unit["motion"]["desired_speed"] = 21.5.into();
            unit["motion"]["heading"] = 0.into();
            unit["motion"]["desired_heading"] = 0.into();
        })
        .unwrap();
}
