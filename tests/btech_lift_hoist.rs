//! Lift hoists load and unload cargo for a unit without cargo handling, within the half of its
//! tonnage each hoist lifts, while the unit runs (Total Warfare pp. 136, 261).
use crate::support;
use stompymux_rs::*;

/// Gold in the hangar: each unit weighs 204/1024 of a ton.
const GOLD: i32 = 528;

/// One movement point, in km/h.
const MP: f64 = 10.75;

/// Place `template` on a hangar map at its loading point, running with its pilot aboard, with
/// `gold` units of hangar stock.
async fn fixture(
    template: UnitTemplate,
    gold: i32,
) -> (tempfile::TempDir, Config, World, ObjectId, ObjectId) {
    let (dir, config, mut world) = support::isolated_world().await;
    let map = world.create(&config, "Hangar".into(), Kind::Room);
    create_battle_map(
        &mut world,
        map,
        "bay",
        MapAsset::from_cells("3 2\n.0.0.0\n.0.0.0\n").unwrap(),
    )
    .unwrap();
    set_battle_cargo_transfer_point(
        &mut world,
        ObjectId(1),
        map,
        Some(CargoTransferPoint {
            x: 0,
            y: 0,
            reveal_hint: false,
        }),
    )
    .unwrap();
    let unit = world.create(&config, "Hauler".into(), Kind::Thing);
    template.create(&mut world, unit).unwrap();
    place_battle_unit(&mut world, unit, map, 0, 0).unwrap();
    world.objects.get_mut(&ObjectId(1)).unwrap().location = Some(unit);
    assign_battle_pilot(&mut world, unit, ObjectId(1)).unwrap();
    support::seed_world_dice(&mut world, support::FIXTURE_DICE_SEED);
    start_battle_unit(&mut world, unit, ObjectId(1), true).unwrap();
    for _ in 0..5 {
        advance_battle_units(&mut world, 0, true);
    }
    set_battle_inventory_named(&mut world, ObjectId(1), map, "Gold", gold).unwrap();
    (dir, config, world, map, unit)
}

/// The Demolisher with `hoists` lift hoists in its rear and no cargo handling, fitted to its
/// tonnage.
fn hauler(hoists: u8) -> UnitTemplate {
    let mut template =
        VehicleTemplate::parse("Demolisher", include_str!("../game/units/Demolisher.toml"))
            .unwrap();
    let rear = template.sections.get_mut(&VehicleSection::Rear).unwrap();
    for slot in 0..hoists {
        rear.criticals.insert(
            slot,
            CriticalDefinition {
                equipment: "Lift_Hoist".into(),
                data: "-".into(),
                modes: vec![],
            },
        );
    }
    UnitTemplate::Vehicle(support::templates::fitted(template))
}

/// An 80-ton vehicle's hoist lifts 40 tons, two lift 80: 200 Gold weigh 39.8 tons and 201 more
/// than 40. A unit with neither hoists nor cargo handling hauls nothing.
#[tokio::test]
async fn lift_hoists_lift_half_the_unit_each() {
    let (_dir, config, mut world, map, unit) = fixture(hauler(1), 400).await;
    let refused = transfer_battle_cargo(&mut world, &config, ObjectId(1), true, "Gold", 201)
        .unwrap_err()
        .to_string();
    assert!(
        refused.contains("Your lift hoists lift at most 40 tons!"),
        "{refused}"
    );
    let stock = |world: &World, holder| {
        battle_inventory(world, holder)
            .unwrap()
            .iter()
            .filter(|entry| entry.part_id == GOLD)
            .map(|entry| entry.quantity)
            .sum::<i32>()
    };
    assert_eq!((stock(&world, map), stock(&world, unit)), (400, 0));
    transfer_battle_cargo(&mut world, &config, ObjectId(1), true, "Gold", 200).unwrap();
    assert_eq!((stock(&world, map), stock(&world, unit)), (200, 200));

    let (_dir, config, mut world, _, _) = fixture(hauler(2), 400).await;
    transfer_battle_cargo(&mut world, &config, ObjectId(1), true, "Gold", 400).unwrap();

    let (_dir, config, mut world, _, _) = fixture(hauler(0), 10).await;
    let refused = transfer_battle_cargo(&mut world, &config, ObjectId(1), true, "Gold", 1)
        .unwrap_err()
        .to_string();
    assert!(
        refused.contains("This unit cannot haul cargo!"),
        "{refused}"
    );
}

/// Hoists do not work while the unit is shut down, for unloading too, and a Mech's hoist
/// stops with any of its three slots (Total Warfare p. 136).
#[tokio::test]
async fn lift_hoists_need_power_and_their_slots() {
    let (_dir, config, mut world, _, unit) = fixture(hauler(1), 10).await;
    transfer_battle_cargo(&mut world, &config, ObjectId(1), true, "Gold", 10).unwrap();
    stop_battle_unit(&mut world, unit, ObjectId(1)).unwrap();
    let refused = transfer_battle_cargo(&mut world, &config, ObjectId(1), false, "Gold", 10)
        .unwrap_err()
        .to_string();
    assert!(
        refused.contains("Your lift hoists need the reactor online!"),
        "{refused}"
    );

    let mut jenner =
        MechTemplate::parse("JR7-D", include_str!("../game/units/JR7-D.toml")).unwrap();
    let torso = jenner.sections.get_mut(&MechSection::LeftTorso).unwrap();
    for slot in 4..7 {
        torso.criticals.insert(
            slot,
            CriticalDefinition {
                equipment: "Lift_Hoist".into(),
                data: "-".into(),
                modes: vec![],
            },
        );
    }
    let jenner = support::templates::legal(jenner);
    let (_dir, config, mut world, _, unit) = fixture(UnitTemplate::Mech(jenner), 100).await;
    let mech = &world.btech.constructed_units()[&unit];
    assert_eq!(mech.working_lift_hoists().unwrap(), 1);
    // A 35-ton Mech's hoist lifts 17.5 tons: 87 Gold.
    assert_eq!(mech.lift_hoist_capacity().unwrap(), 35 * 512);
    transfer_battle_cargo(&mut world, &config, ObjectId(1), true, "Gold", 87).unwrap();
    destroy_battle_critical(
        &mut world,
        unit,
        CriticalLocation {
            section: MechSection::LeftTorso,
            slot: 5,
        },
    )
    .unwrap();
    assert_eq!(
        world.btech.constructed_units()[&unit]
            .working_lift_hoists()
            .unwrap(),
        0
    );
    let refused = transfer_battle_cargo(&mut world, &config, ObjectId(1), true, "Gold", 1)
        .unwrap_err()
        .to_string();
    assert!(
        refused.contains("This unit cannot haul cargo!"),
        "{refused}"
    );
}

/// Cargo in lift hoists costs nothing up to half the unit's tonnage and a cruising MP above
/// it, with flanking refigured from what is left; the carried-load formula does not charge it
/// as well (Total Warfare p. 136). The 80-ton Demolisher cruises at 3 MP and flanks at 5: with
/// 200 Gold (39.8 tons) it still does, and with 201 (40 tons and a bit) it cruises at 2 and
/// flanks at 3.
#[tokio::test]
async fn hoisted_cargo_costs_a_cruising_mp_above_half_the_tonnage() {
    let (_dir, config, mut world, _, unit) = fixture(hauler(2), 400).await;
    let flank = |world: &World| battle_throttle_maximum(world, unit, false).unwrap();
    assert_eq!(flank(&world), 5.0 * MP);
    transfer_battle_cargo(&mut world, &config, ObjectId(1), true, "Gold", 200).unwrap();
    assert_eq!(
        battle_unit_load(&world, unit, false).unwrap().carried_mass,
        0
    );
    assert_eq!(flank(&world), 5.0 * MP);
    transfer_battle_cargo(&mut world, &config, ObjectId(1), true, "Gold", 1).unwrap();
    assert_eq!(
        battle_unit_load(&world, unit, false).unwrap().hoist_penalty,
        1
    );
    assert_eq!(flank(&world), 3.0 * MP);
    transfer_battle_cargo(&mut world, &config, ObjectId(1), false, "Gold", 201).unwrap();
    assert_eq!(flank(&world), 5.0 * MP);
    world.validate(&config).unwrap();
}

/// A 'Mech carrying cargo in its lift hoists may not jump, however light the cargo (Total
/// Warfare pp. 137, 261); once it sets the cargo down it may.
#[tokio::test]
async fn hoisted_cargo_bars_jumping() {
    let mut jenner =
        MechTemplate::parse("JR7-D", include_str!("../game/units/JR7-D.toml")).unwrap();
    let torso = jenner.sections.get_mut(&MechSection::LeftTorso).unwrap();
    for slot in 4..7 {
        torso.criticals.insert(
            slot,
            CriticalDefinition {
                equipment: "Lift_Hoist".into(),
                data: "-".into(),
                modes: vec![],
            },
        );
    }
    let jenner = support::templates::legal(jenner);
    let (_dir, config, mut world, _, unit) = fixture(UnitTemplate::Mech(jenner), 10).await;
    let walking = world.btech.constructed_units()[&unit].walking_speed();
    transfer_battle_cargo(&mut world, &config, ObjectId(1), true, "Gold", 1).unwrap();
    assert_eq!(
        battle_unit_load(&world, unit, false).unwrap().hoist_penalty,
        0
    );
    assert_eq!(
        world.btech.constructed_units()[&unit].walking_speed(),
        walking
    );
    let refused = launch_battle_jump(&mut world.clone(), unit, ObjectId(1), 180, 1.0)
        .unwrap_err()
        .to_string();
    assert!(
        refused.contains("You cannot jump with cargo in your lift hoists."),
        "{refused}"
    );
    transfer_battle_cargo(&mut world, &config, ObjectId(1), false, "Gold", 1).unwrap();
    launch_battle_jump(&mut world, unit, ObjectId(1), 180, 1.0).unwrap();
}
