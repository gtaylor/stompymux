//! Torpedo launchers fire from units in the water at targets in the water (Total Warfare
//! p. 138), from a submerged mount or from one above the surface.
use crate::support;
use std::sync::Arc;
use stompymux_rs::*;

/// How far the raised copy of the lake is lifted.
const LIFT: u8 = 3;

/// Set isolated runtime facts through the persisted representation.
fn edit(world: &mut World, id: ObjectId, change: impl FnOnce(&mut serde_json::Value)) {
    world.btech.rewrite_unit_record(id, change).unwrap();
}

/// A Jenner whose only weapon is an IS SRT-6 in its left leg, fed by two torpedo bins. A leg
/// launcher is submerged even in shallow water.
fn launcher() -> UnitTemplate {
    launcher_in(MechSection::LeftLeg)
}

/// A Jenner whose only weapon is an IS SRT-6 in `section`, fed by two torpedo bins.
fn launcher_in(section: MechSection) -> UnitTemplate {
    let weapon = Weapon::Srt6;
    let mut definition =
        UnitTemplate::parse("JR7-D", include_str!("../game/units/JR7-D.toml")).unwrap();
    let UnitTemplate::Mech(unit) = &mut definition else {
        panic!("The Jenner is a Mech");
    };
    for section in unit.sections.values_mut() {
        section.criticals.retain(|_, part| {
            !part.equipment.starts_with("Ammo_")
                && !Weapon::ALL.iter().any(|w| w.name() == part.equipment)
        });
    }
    let mount = unit.sections.get_mut(&section).unwrap();
    mount.criticals.retain(|slot, _| *slot < 4);
    for slot in 0..weapon.profile().critical_slots {
        mount.criticals.insert(
            slot + 4,
            CriticalDefinition {
                equipment: weapon.name().into(),
                data: "-".into(),
                modes: Vec::new(),
            },
        );
    }
    let bins = unit.sections.get_mut(&MechSection::RightTorso).unwrap();
    bins.criticals.clear();
    for slot in 0..2 {
        bins.criticals.insert(
            slot as u8,
            CriticalDefinition {
                equipment: format!("Ammo_{}", weapon.name()),
                data: weapon.profile().ammunition_per_ton.to_string(),
                modes: Vec::new(),
            },
        );
    }
    crate::support::templates::legal_unit(definition)
}

/// A launcher and a target on a strip whose north end is a deep lake with a shallow southern
/// ford, with every hex raised `lift` levels. `rows` places each unit.
async fn fixture(
    rows: [i64; 2],
    lift: u8,
) -> (tempfile::TempDir, Config, World, ObjectId, ObjectId) {
    fixture_with(rows, lift, launcher()).await
}

/// [`fixture`] with the given launcher design.
async fn fixture_with(
    rows: [i64; 2],
    lift: u8,
    launcher: UnitTemplate,
) -> (tempfile::TempDir, Config, World, ObjectId, ObjectId) {
    let (dir, config, mut world) = support::isolated_world().await;
    let map = world.create(&config, "Lake".into(), Kind::Room);
    let lake = MapAsset::from_cells("1 8\n~2\n~2\n~2\n~2\n~2\n~1\n.0\n.0\n").unwrap();
    let hexes = lake
        .hexes
        .iter()
        .map(|hex| hex.with_level(hex.level() + lift))
        .collect();
    create_battle_map(
        &mut world,
        map,
        "lake",
        MapAsset {
            hexes: Arc::new(hexes),
            ..lake
        },
    )
    .unwrap();
    support::seed_object_dice(&mut world, map, support::FIXTURE_DICE_SEED);
    let mut ids = Vec::new();
    for (index, row) in rows.into_iter().enumerate() {
        let id = world.create(&config, format!("Unit {index}"), Kind::Thing);
        world.objects.get_mut(&id).unwrap().home = Some(ObjectId(config.home()));
        if index == 0 {
            launcher.clone()
        } else {
            UnitTemplate::parse("JR7-D", include_str!("../game/units/JR7-D.toml")).unwrap()
        }
        .create(&mut world, id)
        .unwrap();
        support::seed_object_dice(&mut world, id, support::FIXTURE_DICE_SEED);
        place_battle_unit(&mut world, id, map, 0, row).unwrap();
        edit(&mut world, id, |state| {
            state["power"] = serde_json::to_value(Power::Running).unwrap();
            state["dice"] = serde_json::to_value(Dice::seeded([19; 32])).unwrap();
        });
        ids.push(id);
    }
    world.objects.get_mut(&ObjectId(1)).unwrap().location = Some(ids[0]);
    assign_battle_pilot(&mut world, ids[0], ObjectId(1)).unwrap();
    support::seed_object_dice(&mut world, ObjectId(1), support::FIXTURE_DICE_SEED);
    world.validate(&config).unwrap();
    (dir, config, world, ids[0], ids[1])
}

/// Deterministic conventional firing policy.
fn shot_rules() -> ShotRules {
    ShotRules {
        tsm_tow_bonus: true,
        vehicle_impact: VehicleImpactRules::STANDARD,
        aim: AimRules {
            override_weapon_arcs: true,
        },
        extended_gunnery: false,
        extended_piloting: false,
        target_toughness: false,
    }
}

/// Seed the shooter until its contacts include the target.
fn acquire(world: &mut World, shooter: ObjectId, target: ObjectId) {
    assert!(
        try_acquire(world, shooter, target).is_some(),
        "Fixture contact was not acquired"
    );
}

/// Seed the shooter until its contacts include the target, returning the seed that worked.
fn try_acquire(world: &mut World, shooter: ObjectId, target: ObjectId) -> Option<u8> {
    for seed in 0..=255 {
        edit(world, shooter, |unit| {
            unit["dice"] = serde_json::to_value(Dice::seeded([seed; 32])).unwrap()
        });
        refresh_battle_contacts(world, &[shooter]).unwrap();
        if visible_battle_contact(world, shooter, target)
            .unwrap()
            .is_some()
        {
            return Some(seed);
        }
    }
    None
}

/// Fire the SRT at the target and describe any refusal.
fn fire(world: &mut World, shooter: ObjectId, target: ObjectId) -> Result<MechShotReport, String> {
    resolve_battle_shot(world, shooter, ObjectId(1), target, 0, shot_rules())
        .map_err(|error| error.to_string())
}

/// A submerged launcher's torpedoes reach a submerged target, and AMS cannot stop them, whether
/// the lake lies at level 0 or higher.
#[tokio::test]
async fn submerged_launchers_fire_torpedoes_at_targets_in_water() {
    for lift in [0, LIFT] {
        let (_dir, config, mut world, shooter, target) = fixture([4, 1], lift).await;
        acquire(&mut world, shooter, target);
        edit(&mut world, target, |unit| unit["ams_enabled"] = true.into());
        let report = fire(&mut world, shooter, target).unwrap();
        assert!(report.expenditure.heat > 0, "lift {lift}");
        assert!(report.ams.is_none(), "AMS cannot engage torpedoes");
        world.validate(&config).unwrap();
    }
}

/// Torpedoes stay in their tubes on dry land and cannot reach a target ashore, whether the lake
/// lies at level 0 or higher.
#[tokio::test]
async fn torpedoes_need_water_at_both_ends() {
    for lift in [0, LIFT] {
        for (rows, message) in [
            ([6, 5], "Torpedoes can only be fired from the water."),
            ([5, 7], "Torpedoes can only strike targets in the water!"),
        ] {
            let (_dir, _config, mut world, shooter, target) = fixture(rows, lift).await;
            acquire(&mut world, shooter, target);
            let before = world.clone();
            assert_eq!(
                fire(&mut world, shooter, target).unwrap_err(),
                message,
                "lift {lift}"
            );
            assert_eq!(world.btech, before.btech);
        }
    }
}

/// A Mech wading in the shallow ford fires the torpedo launcher in its torso, above the
/// surface, at a Mech submerged in the deep lake: torpedoes may be fired from the surface at a
/// submerged target (Total Warfare p. 138). Its other weapons cannot reach that target.
#[tokio::test]
async fn a_wading_mech_fires_torpedoes_from_the_surface_at_a_submerged_target() {
    for lift in [0, LIFT] {
        let (_dir, config, mut world, shooter, target) =
            fixture_with([5, 3], lift, launcher_in(MechSection::LeftTorso)).await;
        let terrain = battle_unit_terrain_los(&world, shooter, target).unwrap();
        assert!(!terrain.blocked, "lift {lift}");
        acquire(&mut world, shooter, target);
        let report = fire(&mut world, shooter, target).unwrap();
        assert!(report.expenditure.heat > 0, "lift {lift}");
        world.validate(&config).unwrap();
    }
}

/// Every torpedo duel on the lake plays out identically when the lake is lifted: the same
/// contacts, the same refusals, and the same hits on the same sections.
#[tokio::test]
async fn lifting_the_lake_leaves_torpedo_duels_unchanged() {
    let mut fired = 0;
    for rows in [[4, 1], [2, 0], [5, 3], [3, 5], [6, 5], [5, 7], [6, 4]] {
        let mut results = Vec::new();
        for lift in [0, LIFT] {
            let (_dir, _config, mut world, shooter, target) = fixture(rows, lift).await;
            let seed = try_acquire(&mut world, shooter, target);
            let shot = fire(&mut world, shooter, target);
            let target = serde_json::to_value(&world.btech.constructed_units()[&target]).unwrap();
            results.push(format!("{seed:?} {shot:?} {}", target["sections"]));
        }
        assert_eq!(results[0], results[1], "rows {rows:?}");
        fired += usize::from(results[0].contains("Ok("));
    }
    assert!(fired >= 2, "too few torpedo duels fired");
}

/// Torpedo launchers are their own catalogue weapons with part identities above the old limit.
#[test]
fn torpedo_launchers_are_catalogue_weapons() {
    for (name, torpedo) in [("IS.LRT-20", Weapon::Lrt20), ("CL.SRT-2", Weapon::ClanSrt2)] {
        assert_eq!(Weapon::parse(name).unwrap(), torpedo);
        assert!(torpedo.is_torpedo());
        let ammunition = Part::from_id(torpedo.ammunition_part_id()).unwrap();
        assert_eq!(ammunition.name, format!("Ammo_{name}"));
        assert_eq!(ammunition.kind, PartKind::Ammunition);
    }
    assert!(Weapon::ClanSrt6.part_id() > 192);
    assert!(!Weapon::Srm6.is_torpedo());
}
