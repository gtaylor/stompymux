//! Torpedo launchers fire only from a submerged mount at a target in the water.
use crate::support;
use stompymux_rs::*;

/// Set isolated runtime facts through the persisted representation.
fn edit(world: &mut World, id: ObjectId, change: impl FnOnce(&mut serde_json::Value)) {
    let mut saved = serde_json::to_value(&world.btech).unwrap();
    change(&mut saved["constructed"][id.0.to_string()]);
    world.btech = serde_json::from_value(saved).unwrap();
}

/// A Jenner whose only weapon is an IS SRT-6 in its left leg, fed by two torpedo bins. A leg
/// launcher is submerged even in shallow water.
fn launcher() -> BattleUnitTemplate {
    let weapon = BattleWeapon::Srt6;
    let mut definition = BattleUnitTemplate::parse(include_str!("../game/mechs/JR7-D")).unwrap();
    let BattleUnitTemplate::Mech(unit) = &mut definition else {
        panic!("The Jenner is a Mech");
    };
    for section in unit.sections.values_mut() {
        section.criticals.retain(|_, part| {
            !part.equipment.starts_with("Ammo_")
                && !BattleWeapon::ALL.iter().any(|w| w.name() == part.equipment)
        });
    }
    let mount = unit.sections.get_mut(&BattleSection::LeftLeg).unwrap();
    mount.criticals.retain(|slot, _| *slot < 4);
    for slot in 0..weapon.profile().critical_slots {
        mount.criticals.insert(
            slot + 4,
            CriticalDefinition {
                equipment: weapon.name().into(),
                data: "-".into(),
                modes: Vec::new(),
                brand: None,
            },
        );
    }
    let bins = unit.sections.get_mut(&BattleSection::RightTorso).unwrap();
    bins.criticals.clear();
    for slot in 0..2 {
        bins.criticals.insert(
            slot as u8,
            CriticalDefinition {
                equipment: format!("Ammo_{}", weapon.name()),
                data: weapon.profile().ammunition_per_ton.to_string(),
                modes: Vec::new(),
                brand: None,
            },
        );
    }
    definition
}

/// A launcher and a target on a strip whose north end is a deep lake with a shallow southern
/// ford. `rows` places each unit.
async fn fixture(rows: [i64; 2]) -> (tempfile::TempDir, Config, World, ObjectId, ObjectId) {
    let (dir, config, mut world) = support::isolated_world().await;
    let map = world.create(&config, "Lake".into(), Kind::Room);
    create_battle_map(
        &mut world,
        map,
        "lake",
        BattleMapAsset::parse("1 8\n~2\n~2\n~2\n~2\n~2\n~1\n.0\n.0\n").unwrap(),
    )
    .unwrap();
    let mut ids = Vec::new();
    for (index, row) in rows.into_iter().enumerate() {
        let id = world.create(&config, format!("Unit {index}"), Kind::Thing);
        world.objects.get_mut(&id).unwrap().home = Some(ObjectId(config.home()));
        if index == 0 {
            launcher()
        } else {
            BattleUnitTemplate::parse(include_str!("../game/mechs/JR7-D")).unwrap()
        }
        .create(&mut world, id)
        .unwrap();
        place_battle_unit(&mut world, id, map, 0, row).unwrap();
        edit(&mut world, id, |state| {
            state["power"] = serde_json::to_value(BattlePower::Running).unwrap();
            state["dice"] = serde_json::to_value(BattleDice::seeded([19; 32])).unwrap();
        });
        ids.push(id);
    }
    world.objects.get_mut(&ObjectId(1)).unwrap().location = Some(ids[0]);
    assign_battle_pilot(&mut world, ids[0], ObjectId(1)).unwrap();
    world.validate(&config).unwrap();
    (dir, config, world, ids[0], ids[1])
}

/// Deterministic conventional firing policy.
fn shot_rules() -> BattleShotRules {
    BattleShotRules {
        range_damage: false,
        tsm_tow_bonus: true,
        vehicle_impact: BattleVehicleImpactRules::STANDARD,
        stacking: BattleStackingRules::STANDARD,
        stagger: BattleStaggerMode::Retain,
        glancing: BattleGlancingMode::Disabled,
        aim: BattleAimRules {
            woods_damage: false,
            dig_bonus: 3,
            dig_only_front: false,
            hit_arc_mode: 0,
            fasa_turning: false,
            extended_movement: false,
            extended_ranges: false,
            hotload_half_minimum: false,
            override_weapon_arcs: true,
        },
        hit: BattleHitRules {
            inferno_penalty: false,
            exile_stun_mode: 0,
        },
        hit_arc_mode: 0,
        extended_gunnery: false,
        extended_piloting: false,
        target_toughness: false,
    }
}

/// Seed the shooter until its contacts include the target.
fn acquire(world: &mut World, shooter: ObjectId, target: ObjectId) {
    for seed in 0..=255 {
        edit(world, shooter, |unit| {
            unit["dice"] = serde_json::to_value(BattleDice::seeded([seed; 32])).unwrap()
        });
        refresh_battle_contacts(world, &[shooter]).unwrap();
        if visible_battle_contact(world, shooter, target)
            .unwrap()
            .is_some()
        {
            return;
        }
    }
    panic!("Fixture contact was not acquired");
}

/// Fire the SRT at the target and describe any refusal.
fn fire(
    world: &mut World,
    shooter: ObjectId,
    target: ObjectId,
) -> Result<BattleShotReport, String> {
    resolve_battle_shot(world, shooter, ObjectId(1), target, 0, shot_rules())
        .map_err(|error| error.to_string())
}

/// A submerged launcher's torpedoes reach a submerged target, and AMS cannot stop them.
#[tokio::test]
async fn submerged_launchers_fire_torpedoes_at_targets_in_water() {
    let (_dir, config, mut world, shooter, target) = fixture([4, 1]).await;
    acquire(&mut world, shooter, target);
    edit(&mut world, target, |unit| unit["ams_enabled"] = true.into());
    let report = fire(&mut world, shooter, target).unwrap();
    assert!(report.expenditure.heat > 0);
    assert!(report.ams.is_none(), "AMS cannot engage torpedoes");
    world.validate(&config).unwrap();
}

/// Torpedoes stay in their tubes on dry land and cannot reach a target ashore.
#[tokio::test]
async fn torpedoes_need_water_at_both_ends() {
    for (rows, message) in [
        ([6, 5], "Torpedoes can only be fired underwater."),
        ([5, 7], "Torpedoes can only strike targets in the water!"),
    ] {
        let (_dir, _config, mut world, shooter, target) = fixture(rows).await;
        acquire(&mut world, shooter, target);
        let before = world.clone();
        assert_eq!(fire(&mut world, shooter, target).unwrap_err(), message);
        assert_eq!(world.btech, before.btech);
    }
}

/// Torpedo launchers are their own catalogue weapons with part identities above the old limit.
#[test]
fn torpedo_launchers_are_catalogue_weapons() {
    for (name, torpedo) in [
        ("IS.LRT-20", BattleWeapon::Lrt20),
        ("CL.SRT-2", BattleWeapon::ClanSrt2),
    ] {
        assert_eq!(BattleWeapon::parse(name).unwrap(), torpedo);
        assert!(torpedo.is_torpedo());
        let ammunition = BattlePart::from_id(torpedo.ammunition_part_id()).unwrap();
        assert_eq!(ammunition.name, format!("Ammo_{name}"));
        assert_eq!(ammunition.kind, BattlePartKind::Ammunition);
    }
    assert!(BattleWeapon::ClanSrt6.part_id() > 192);
    assert!(!BattleWeapon::Srm6.is_torpedo());
}
