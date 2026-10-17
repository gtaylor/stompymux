//! Active probes: the woods a probe lets its carrier aim through within its reach.
use crate::support;
use stompymux_rs::*;

/// A one-hex-wide lane built from per-hex rows, listed north to south.
async fn lane(rows: &[&str]) -> (tempfile::TempDir, Config, World, ObjectId) {
    let (dir, config, mut world) = support::isolated_world().await;
    let map = world.create(&config, "Probe lane".into(), Kind::Room);
    create_battle_map(
        &mut world,
        map,
        "probe",
        MapAsset::from_cells(&format!("1 {}\n{}\n", rows.len(), rows.join("\n"))).unwrap(),
    )
    .unwrap();
    support::seed_world_dice(&mut world, support::FIXTURE_DICE_SEED);
    (dir, config, world, map)
}

/// Start a unit without the startup countdown by editing its saved power state.
fn running(world: &mut World, id: ObjectId) {
    world.btech.set_unit_power(id, Power::Running).unwrap();
}

/// Place a running tank, or a stationary tower, whose only front item is the given equipment.
fn probe_vehicle(
    world: &mut World,
    config: &Config,
    map: ObjectId,
    equipment: Option<&str>,
    stationary: bool,
    y: i64,
) -> ObjectId {
    let id = world.create(config, "Probe carrier".into(), Kind::Thing);
    world.objects.get_mut(&id).unwrap().home = Some(ObjectId(config.home()));
    let mut definition = crate::support::templates::fitted(
        VehicleTemplate::parse(
            "test",
            if stationary {
                include_str!("../game/units/RadioTower.toml")
            } else {
                include_str!("../game/units/Demolisher.toml")
            },
        )
        .unwrap(),
    );
    let front = definition.sections.get_mut(&VehicleSection::Front).unwrap();
    front.criticals.clear();
    if let Some(equipment) = equipment {
        front.criticals.insert(
            0,
            CriticalDefinition {
                equipment: equipment.into(),
                data: "-".into(),
                modes: vec![],
            },
        );
    }
    create_battle_vehicle(world, id, crate::support::templates::fitted(definition)).unwrap();
    support::seed_object_dice(world, id, support::FIXTURE_DICE_SEED);
    place_battle_unit(world, id, map, 0, y).unwrap();
    running(world, id);
    id
}

/// Place a running Mech from template text on the given team.
fn mech(
    world: &mut World,
    config: &Config,
    map: ObjectId,
    source: &str,
    team: i32,
    y: i64,
) -> ObjectId {
    let id = world.create(config, "Probe subject".into(), Kind::Thing);
    world.objects.get_mut(&id).unwrap().home = Some(ObjectId(config.home()));
    create_battle_unit(world, id, MechTemplate::parse("test", source).unwrap()).unwrap();
    support::seed_object_dice(world, id, support::FIXTURE_DICE_SEED);
    place_battle_unit(world, id, map, 0, y).unwrap();
    running(world, id);
    set_battle_unit_signature(
        world,
        id,
        UnitSignature {
            team,
            ..Default::default()
        },
    )
    .unwrap();
    id
}

/// A hostile Jenner target.
fn target(world: &mut World, config: &Config, map: ObjectId, y: i64) -> ObjectId {
    mech(
        world,
        config,
        map,
        include_str!("../game/units/JR7-D.toml"),
        2,
        y,
    )
}

/// Administratively move a running unit, which requires a brief shutdown.
fn relocate(world: &mut World, id: ObjectId, map: ObjectId, y: i64) {
    world.btech.set_unit_power(id, Power::Off).unwrap();
    place_battle_unit(world, id, map, 0, y).unwrap();
    running(world, id);
}

/// Seat a pilot in a unit so it can operate equipment.
fn seat(world: &mut World, id: ObjectId, pilot: ObjectId) {
    world.objects.get_mut(&pilot).unwrap().location = Some(id);
    assign_battle_pilot(world, id, pilot).unwrap();
    support::seed_object_dice(world, pilot, support::FIXTURE_DICE_SEED);
}

/// Tactical conventional shot configuration without optional damage or arc rules.
fn shot_rules() -> ShotRules {
    ShotRules {
        tsm_tow_bonus: true,
        vehicle_impact: VehicleImpactRules::STANDARD,
        aim: AimRules {
            override_weapon_arcs: false,
        },
        extended_gunnery: false,
        extended_piloting: false,
        target_toughness: false,
    }
}

/// A working probe within its reach lets the carrier shoot through one point of woods.
#[tokio::test]
async fn probes_let_the_carrier_aim_through_a_point_of_woods() {
    let (_dir, config, mut world, map) = lane(&[".0", "`0", "`0", ".0", ".0", ".0"]).await;
    let bare = probe_vehicle(&mut world, &config, map, None, false, 0);
    let probed = probe_vehicle(&mut world, &config, map, Some("BeagleProbe"), false, 0);
    let enemy = target(&mut world, &config, map, 3);
    seat(&mut world, bare, ObjectId(1));
    seat(&mut world, probed, ObjectId(2));
    refresh_battle_contacts(&mut world, &[bare, probed]).unwrap();
    let aim = |world: &World, shooter| {
        battle_aim_modifiers(world, shooter, enemy, 0, 4, shot_rules().aim)
            .unwrap()
            .perception
            .unwrap()
            .modifier
    };
    assert_eq!(aim(&world, bare), 2, "two light woods on the way");
    assert_eq!(aim(&world, probed), 1, "the probe sees through one of them");
    // Electromagnetic interference renders the probe useless (Tactical Operations: Advanced
    // Rules p. 53): it no longer sees through the woods.
    let mut interfered = world.clone();
    interfered
        .btech
        .rewrite_map_record(map, |record| {
            let flags = record["flags"].as_i64().unwrap();
            record["flags"] = MapFlag::ElectromagneticInterference
                .apply(flags, true)
                .into();
        })
        .unwrap();
    assert_eq!(
        aim(&interfered, probed),
        2,
        "the useless probe sees no woods"
    );
    relocate(&mut world, enemy, map, 5);
    refresh_battle_contacts(&mut world, &[bare, probed]).unwrap();
    assert_eq!(
        aim(&world, probed),
        2,
        "five hexes is past the Beagle's reach"
    );
}
