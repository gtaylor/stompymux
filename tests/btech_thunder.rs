//! Thunder rounds fired at a hex lay minefields, and active mines reach hovering units.
use crate::support;
use stompymux_rs::*;

/// Set isolated runtime facts through the persisted representation for either anatomy.
fn edit(world: &mut World, id: ObjectId, change: impl FnOnce(&mut serde_json::Value)) {
    let collection = if world.btech.vehicles().contains_key(&id) {
        "vehicles"
    } else {
        "constructed"
    };
    let mut saved = serde_json::to_value(&world.btech).unwrap();
    change(&mut saved[collection][id.0.to_string()]);
    world.btech = serde_json::from_value(saved).unwrap();
}

/// A Jenner whose only weapon is an IS LRM-20 fed by one bin of each Thunder round.
fn launcher() -> BattleUnitTemplate {
    let weapon = BattleWeapon::Lrm20;
    let mut definition =
        BattleUnitTemplate::parse("JR7-D", include_str!("../game/mechs/JR7-D.toml")).unwrap();
    let BattleUnitTemplate::Mech(unit) = &mut definition else {
        panic!("The Jenner is a Mech");
    };
    for section in unit.sections.values_mut() {
        section.criticals.retain(|_, part| {
            !part.equipment.starts_with("Ammo_")
                && !BattleWeapon::ALL.iter().any(|w| w.name() == part.equipment)
        });
    }
    let mount = unit.sections.get_mut(&BattleSection::LeftTorso).unwrap();
    mount.criticals.clear();
    for slot in 0..weapon.profile().critical_slots {
        mount.criticals.insert(
            slot,
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
    for (slot, flag) in ["Mine", "ThunderAug", "ThunderVibra", "ThunderActive"]
        .into_iter()
        .enumerate()
    {
        bins.criticals.insert(
            slot as u8,
            CriticalDefinition {
                equipment: format!("Ammo_{}", weapon.name()),
                data: "3".into(),
                modes: vec![flag.into()],
                brand: None,
            },
        );
    }
    definition
}

/// A piloted launcher at the south end of an open field, facing the target hex.
async fn fixture() -> (tempfile::TempDir, Config, World, ObjectId, ObjectId) {
    let (dir, config, mut world) = support::isolated_world().await;
    let map = world.create(&config, "Thunder range".into(), Kind::Room);
    let rows = [".0.0.0"; 12].join("\n");
    create_battle_map(
        &mut world,
        map,
        "thunder",
        BattleMapAsset::from_cells(&format!("3 12\n{rows}\n")).unwrap(),
    )
    .unwrap();
    let id = world.create(&config, "Launcher".into(), Kind::Thing);
    world.objects.get_mut(&id).unwrap().home = Some(ObjectId(config.home()));
    launcher().create(&mut world, id).unwrap();
    place_battle_unit(&mut world, id, map, 1, 11).unwrap();
    edit(&mut world, id, |state| {
        state["power"] = serde_json::to_value(BattlePower::Running).unwrap();
    });
    world.objects.get_mut(&ObjectId(1)).unwrap().location = Some(id);
    world
        .objects
        .get_mut(&ObjectId(1))
        .unwrap()
        .flags
        .insert(Flag::Connected);
    assign_battle_pilot(&mut world, id, ObjectId(1)).unwrap();
    set_battle_character(
        &mut world,
        ObjectId(1),
        BattleCharacter {
            build: 5,
            reflexes: 4,
            intuition: 3,
            learn: 2,
            charisma: 1,
            bruise: 0,
            lethal: 0,
        },
    )
    .unwrap();
    set_battle_character_value(
        &mut world,
        ObjectId(1),
        "Gunnery-Battlemech",
        BattleCharacterValue {
            value: 10,
            experience: 0,
            last_used: 0,
        },
    )
    .unwrap();
    world.validate(&config).unwrap();
    (dir, config, world, id, map)
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

/// Fire the launcher at a hex with the first dice seed that hits.
fn fire_until_hit(world: &mut World, shooter: ObjectId, coordinate: BattleHexCoordinate) {
    select_battle_hex_target(
        world,
        shooter,
        ObjectId(1),
        coordinate,
        BattleHexTargetMode::Hex,
    )
    .unwrap();
    for seed in 0..=255 {
        let mut trial = world.clone();
        edit(&mut trial, shooter, |unit| {
            unit["dice"] = serde_json::to_value(BattleDice::seeded([seed; 32])).unwrap()
        });
        let report = resolve_battle_hex_shot(
            &mut trial,
            shooter,
            ObjectId(1),
            coordinate,
            0,
            shot_rules(),
        )
        .unwrap();
        if report.hit {
            assert!(
                report.terrain.is_empty(),
                "Thunder rounds do not blast terrain"
            );
            assert!(!report.thunder.fields.is_empty());
            assert!(
                report
                    .notices()
                    .iter()
                    .any(|notice| notice.text.contains("Thunder salvo seeds"))
            );
            *world = trial;
            return;
        }
    }
    panic!("No seed hit the target hex");
}

/// Let the launcher recycle before its next salvo.
fn recycle(world: &mut World, shooter: ObjectId) {
    edit(world, shooter, |unit| {
        unit["weapon_recycle"] = serde_json::json!({});
    });
}

/// Each Thunder round lays its own kind of field, owned by the shooter and sized by the salvo.
#[tokio::test]
async fn thunder_rounds_lay_their_minefields() {
    let (_dir, config, initial, shooter, map) = fixture().await;
    let target = BattleHexCoordinate { x: 1, y: 3 };
    for (mode, kind, extra, cells) in [
        (BattleAmmunitionMode::Mine, BattleMineKind::Standard, 0, 1),
        (
            BattleAmmunitionMode::ThunderAugmented,
            BattleMineKind::Standard,
            0,
            7,
        ),
        // A negative threshold stands for one ton above the shooter's weight after firing.
        (
            BattleAmmunitionMode::ThunderVibrabomb,
            BattleMineKind::Vibra,
            -1,
            1,
        ),
        (
            BattleAmmunitionMode::ThunderActive,
            BattleMineKind::Active,
            0,
            1,
        ),
    ] {
        let mut world = initial.clone();
        let selected = if mode == BattleAmmunitionMode::Mine {
            toggle_battle_missile_rounds(&mut world, shooter, ObjectId(1), 0, mode).unwrap()
        } else {
            toggle_battle_thunder(&mut world, shooter, ObjectId(1), 0, mode).unwrap()
        };
        assert_eq!(selected, mode);
        fire_until_hit(&mut world, shooter, target);
        let extra = if extra < 0 {
            let mass = world.btech.constructed_units()[&shooter]
                .effective_mass()
                .unwrap();
            i32::try_from(mass / 1024).unwrap() + 1
        } else {
            extra
        };
        let fields: Vec<_> = world.btech.maps()[&map].minefields().values().collect();
        assert_eq!(fields.len(), cells, "{mode:?}");
        assert!(
            fields.iter().all(|field| field.kind == kind
                && field.extra == extra
                && field.owner == shooter
                && field.strength > 0),
            "{mode:?} {fields:?}"
        );
        assert!(fields.iter().any(|field| field.coordinate == target));
        if cells > 1 {
            let strength = fields[0].strength;
            assert!(fields.iter().all(|field| field.strength == strength));
        }
        assert!(world.btech.maps()[&map].mine_coverage(target).unwrap());
        world.validate(&config).unwrap();
        // A second salvo thickens the same field, never past the cap.
        if mode == BattleAmmunitionMode::Mine {
            let first = fields[0].strength;
            recycle(&mut world, shooter);
            fire_until_hit(&mut world, shooter, target);
            let fields: Vec<_> = world.btech.maps()[&map].minefields().values().collect();
            assert_eq!(fields.len(), 1);
            assert!(fields[0].strength > first);
            assert!(fields[0].strength <= THUNDER_MAXIMUM_STRENGTH);
        }
        persistence::save(&config.database(), &world).await.unwrap();
        let replay = persistence::load(&config.database()).await.unwrap();
        assert_eq!(replay.btech.maps(), world.btech.maps());
    }
}

/// Only LRM launchers accept specialized Thunder rounds, and toggling twice restores normal fire.
#[tokio::test]
async fn thunder_controls_toggle_and_reject_other_weapons() {
    let (_dir, _config, mut world, shooter, _map) = fixture().await;
    let mode = BattleAmmunitionMode::ThunderVibrabomb;
    assert_eq!(
        toggle_battle_thunder(&mut world, shooter, ObjectId(1), 0, mode).unwrap(),
        mode
    );
    assert_eq!(
        toggle_battle_thunder(&mut world, shooter, ObjectId(1), 0, mode).unwrap(),
        BattleAmmunitionMode::Normal
    );
    assert!(
        toggle_battle_thunder(
            &mut world,
            shooter,
            ObjectId(1),
            0,
            BattleAmmunitionMode::Smoke
        )
        .is_err()
    );
    assert!(!BattleWeapon::Srm6.supports_thunder());
}

/// Hovercraft skim above the mines on a flooded hex; only an active field reaches them.
#[tokio::test]
async fn active_mines_catch_hovercraft_over_water() {
    let (_dir, config, mut world) = support::isolated_world().await;
    let map = world.create(&config, "Flooded ford".into(), Kind::Room);
    create_battle_map(
        &mut world,
        map,
        "ford",
        BattleMapAsset::from_cells("3 3\n.0.0.0\n.0~1.0\n.0.0.0\n").unwrap(),
    )
    .unwrap();
    let id = world.create(&config, "Hover".into(), Kind::Thing);
    world.objects.get_mut(&id).unwrap().home = Some(ObjectId(config.home()));
    create_battle_vehicle(
        &mut world,
        id,
        BattleVehicleTemplate::parse(
            "test",
            &include_str!("../game/mechs/Demolisher.toml")
                .replace("movement = \"track\"", "movement = \"hover\""),
        )
        .unwrap(),
    )
    .unwrap();
    place_battle_unit(&mut world, id, map, 1, 1).unwrap();
    let coordinate = BattleHexCoordinate { x: 1, y: 1 };
    for (ordinal, kind) in [(0, BattleMineKind::Standard), (1, BattleMineKind::Active)] {
        set_minefield(
            &mut world,
            map,
            ordinal,
            Some(BattleMinefield {
                coordinate,
                kind,
                strength: 10,
                extra: 0,
                owner: ObjectId(1),
            }),
        )
        .unwrap();
    }
    let selected = mine_activations(&world, id, BattleMineTriggerReason::Step).unwrap();
    assert_eq!(
        selected
            .iter()
            .map(|field| field.mine.kind)
            .collect::<Vec<_>>(),
        [BattleMineKind::Active]
    );
    assert_eq!(
        BattleMineKind::parse("ACTIVE").unwrap(),
        BattleMineKind::Active
    );
}
