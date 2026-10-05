//! Vehicle ammunition cascades distinguish Gauss, plasma, missile and special-mode bins.
use crate::support;
use stompymux_rs::*;

/// A running vehicle with a present pilot, initially disconnected from a session.
async fn fixture(template: &str) -> (tempfile::TempDir, Config, World, ObjectId) {
    let (dir, config, mut world) = support::isolated_world().await;
    let map = world.create(&config, "Test field".into(), Kind::Room);
    create_battle_map(
        &mut world,
        map,
        "test",
        MapAsset::from_cells("1 1\n.0\n").unwrap(),
    )
    .unwrap();
    let id = world.create(&config, "Vehicle".into(), Kind::Thing);
    world.objects.get_mut(&id).unwrap().home = Some(ObjectId(config.home()));
    create_battle_vehicle(
        &mut world,
        id,
        VehicleTemplate::parse("test", template).unwrap(),
    )
    .unwrap();
    place_battle_unit(&mut world, id, map, 0, 0).unwrap();
    world.objects.get_mut(&ObjectId(1)).unwrap().location = Some(id);
    assign_battle_pilot(&mut world, id, ObjectId(1)).unwrap();
    support::seed_world_dice(&mut world, support::FIXTURE_DICE_SEED);
    start_battle_unit(&mut world, id, ObjectId(1), true).unwrap();
    for _ in 0..5 {
        advance_battle_units(&mut world, 0);
    }
    (dir, config, world, id)
}

/// Mixed bins in separate hull and turret sections, including an inert Gauss supply.
fn template() -> String {
    include_str!("../game/units/Demolisher.toml").replace(
        "[sections.left_side]\n",
        "[sections.left_side]\nslots = [\n    { at = 1, item = \"Ammo_IS.GaussRifle\", rounds = 3 },\n    { at = 2, item = \"Ammo_IS.PlasmaRifle\", rounds = 3 },\n    { at = 3, item = \"Ammo_IS.LRM-5\", rounds = 2 },\n    { at = 4, item = \"Ammo_IS.AC/20\", rounds = 2, modes = [\"Precision\"] },\n    { at = 5, item = \"Ammo_IS.NarcBeacon\", rounds = 2 },\n]\n",
    )
}

#[tokio::test]
async fn cascade_spends_all_explosive_bins_and_preserves_inert_ammunition_on_replay() {
    let (_dir, config, mut world, id) = fixture(&template()).await;
    let vehicle = &world.btech.vehicles()[&id];
    let expected = vehicle.ammunition_cascade(VehicleSection::Front).unwrap();
    assert_eq!(expected.damage, 488);
    assert_eq!(expected.ammunition.len(), 8);
    assert_eq!(Weapon::PlasmaRifle.ammunition_explosion_damage(3), 0);
    let sections = vehicle.sections().clone();
    let definition = vehicle.definition().clone();
    let before = world.btech.clone();
    assert_eq!(
        vehicle.ammunition_cascade(VehicleSection::Front).unwrap(),
        expected
    );
    assert_eq!(world.btech, before);
    persistence::save(&config.database(), &world).await.unwrap();
    let mut loaded = persistence::load(&config.database()).await.unwrap();
    let report =
        discharge_battle_vehicle_ammunition_cascade(&mut world, id, VehicleSection::Front).unwrap();
    assert_eq!(report, expected);
    assert_eq!(
        report,
        discharge_battle_vehicle_ammunition_cascade(&mut loaded, id, VehicleSection::Front)
            .unwrap()
    );
    assert_eq!(world.btech, loaded.btech);
    let vehicle = &world.btech.vehicles()[&id];
    assert_eq!(vehicle.ammunition()[0], 3);
    assert!(vehicle.ammunition()[1..].iter().all(|rounds| *rounds == 0));
    assert_eq!(vehicle.sections(), &sections);
    assert_eq!(vehicle.definition(), &definition);
    let empty = vehicle.ammunition_cascade(VehicleSection::Front).unwrap();
    assert_eq!(empty.damage, 0);
    assert!(empty.ammunition.is_empty());
    persistence::save(&config.database(), &world).await.unwrap();
    assert_eq!(
        persistence::load(&config.database()).await.unwrap().btech,
        world.btech
    );
}

#[tokio::test]
async fn cascade_ignores_lost_bins_and_rejects_invalid_victims_without_mutation() {
    let (_dir, _config, mut world, id) = fixture(&template()).await;
    let plasma = world.btech.vehicles()[&id].loadout().unwrap().ammunition[1].location;
    destroy_battle_vehicle_critical(&mut world, id, plasma).unwrap();
    damage_battle_vehicle_phase(
        &mut world,
        id,
        VehicleSection::Turret,
        100,
        DamagePhase::Internal,
    )
    .unwrap();
    assert_eq!(
        world.btech.vehicles()[&id]
            .ammunition_cascade(VehicleSection::Front)
            .unwrap()
            .damage,
        58
    );
    let before = world.btech.clone();
    assert!(
        discharge_battle_vehicle_ammunition_cascade(&mut world, id, VehicleSection::Turret)
            .is_err()
    );
    assert_eq!(world.btech, before);
    world
        .objects
        .get_mut(&id)
        .unwrap()
        .flags
        .insert(Flag::Going);
    assert!(
        discharge_battle_vehicle_ammunition_cascade(&mut world, id, VehicleSection::Front).is_err()
    );
    assert_eq!(world.btech, before);
    assert!(
        discharge_battle_vehicle_ammunition_cascade(
            &mut world,
            ObjectId(-1),
            VehicleSection::Front
        )
        .is_err()
    );
    assert_eq!(world.btech, before);
}
