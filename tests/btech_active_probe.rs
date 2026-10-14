//! Active probes: the hidden units each family reveals within its reach and clear line, and the
//! woods a probe lets its carrier aim through.
use crate::support;
use stompymux_rs::*;

/// Every probe family in order of increasing reach, with its template equipment name.
const FAMILIES: [(ActiveProbe, &str); 3] = [
    (ActiveProbe::Light, "Light_BAP"),
    (ActiveProbe::Beagle, "BeagleProbe"),
    (ActiveProbe::Bloodhound, "BloodhoundProbe"),
];

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

/// A lane whose second hex is a hill that blocks every sightline from the first hex.
async fn hill_lane(length: usize) -> (tempfile::TempDir, Config, World, ObjectId) {
    let mut rows = vec![".0"; length];
    rows[1] = ".9";
    lane(&rows).await
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
    let mut definition = VehicleTemplate::parse(
        "test",
        if stationary {
            include_str!("../game/units/RadioTower.toml")
        } else {
            include_str!("../game/units/Demolisher.toml")
        },
    )
    .unwrap();
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
    create_battle_vehicle(world, id, definition).unwrap();
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

/// Every probe family reveals a hidden enemy exactly as far as Total Warfare lets it (Light 3,
/// Beagle 4, Bloodhound 8 hexes), never one hex farther or through a hill.
#[tokio::test]
async fn probes_reveal_hidden_units_within_their_reach_and_line_of_sight() {
    for (family, equipment) in FAMILIES {
        let reach = i64::from(family.range(false));
        let (_dir, config, mut world, map) = lane(&[".0"; 12]).await;
        let carrier = probe_vehicle(&mut world, &config, map, Some(equipment), false, 0);
        seat(&mut world, carrier, ObjectId(1));
        let hidden = target(&mut world, &config, map, reach + 1);
        let conceal_unit = |world: &mut World| {
            set_battle_unit_signature(
                world,
                hidden,
                UnitSignature {
                    team: 2,
                    hidden: true,
                    illuminated: false,
                },
            )
            .unwrap();
        };
        conceal_unit(&mut world);
        let events = refresh_battle_contacts(&mut world, &[carrier]).unwrap();
        assert!(
            events.iter().all(|event| event.revealed.is_none()),
            "{family:?} one hex past its reach"
        );
        assert!(battle_perceive(&world, carrier, hidden).unwrap().is_none());
        relocate(&mut world, hidden, map, reach);
        conceal_unit(&mut world);
        let events = refresh_battle_contacts(&mut world, &[carrier]).unwrap();
        assert!(
            events.iter().any(|event| event.revealed.is_some()),
            "{family:?} at its reach"
        );
        assert!(
            !world.btech.constructed_units()[&hidden].signature().hidden,
            "{family:?} revealed the unit"
        );
        assert_eq!(
            battle_perceive(&world, carrier, hidden)
                .unwrap()
                .map(|perception| perception.channel),
            Some(DetectionChannel::Sight)
        );
        world.validate(&config).unwrap();
    }
    // A hill between the carrier and its target breaks the probe's line.
    let (_dir, config, mut world, map) = hill_lane(12).await;
    let carrier = probe_vehicle(&mut world, &config, map, Some("BloodhoundProbe"), false, 0);
    seat(&mut world, carrier, ObjectId(1));
    let hidden = target(&mut world, &config, map, 3);
    set_battle_unit_signature(
        &mut world,
        hidden,
        UnitSignature {
            team: 2,
            hidden: true,
            illuminated: false,
        },
    )
    .unwrap();
    let events = refresh_battle_contacts(&mut world, &[carrier]).unwrap();
    assert!(events.iter().all(|event| event.revealed.is_none()));
    assert!(world.btech.constructed_units()[&hidden].signature().hidden);
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
    relocate(&mut world, enemy, map, 5);
    refresh_battle_contacts(&mut world, &[bare, probed]).unwrap();
    assert_eq!(
        aim(&world, probed),
        2,
        "five hexes is past the Beagle's reach"
    );
}
