//! Configured Mech hit routing and critical-proof construction share host actions and durable state.
use crate::support;
use std::{cell::RefCell, rc::Rc};
use stompymux_rs::*;

/// Update either anatomy's saved runtime facts without duplicating combat mechanics.
fn edit(world: &mut World, id: ObjectId, update: impl FnOnce(&mut serde_json::Value)) {
    world.btech.rewrite_unit_record(id, update).unwrap();
}

/// Use host configuration, a piloted laser or cannon, and a constructed Mech target.
async fn fixture(
    vehicle: bool,
    quad: bool,
    proof: bool,
) -> (tempfile::TempDir, Config, World, ObjectId, ObjectId) {
    let (dir, _, mut world) = support::isolated_world().await;
    let path = dir.path().join("stompymux.toml");
    let mut settings: toml::Value =
        toml::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    for (key, value) in [("exile_stun_code", 1), ("glancing_blows", 0)] {
        settings["battletech"]
            .as_table_mut()
            .unwrap()
            .insert(key.into(), value.into());
    }
    std::fs::write(&path, toml::to_string(&settings).unwrap()).unwrap();
    let config = Config::load(dir.path()).unwrap();
    let map = world.create(&config, "Hit field".into(), Kind::Room);
    create_battle_map(
        &mut world,
        map,
        "field",
        MapAsset::from_cells("1 4\n.0\n.0\n.0\n.0\n").unwrap(),
    )
    .unwrap();
    support::seed_object_dice(&mut world, map, support::FIXTURE_DICE_SEED);
    let shooter = world.create(&config, "Shooter".into(), Kind::Thing);
    let target = world.create(&config, "Target".into(), Kind::Thing);
    for id in [shooter, target] {
        world.objects.get_mut(&id).unwrap().home = Some(ObjectId(config.home()));
    }
    BattleUnitTemplate::parse(
        "test",
        if vehicle {
            include_str!("../game/mechs/Demolisher.toml")
        } else {
            include_str!("fixtures/btech/mechs/JR7-D.toml")
        },
    )
    .unwrap()
    .create(&mut world, shooter)
    .unwrap();
    support::seed_object_dice(&mut world, shooter, support::FIXTURE_DICE_SEED);
    let mut template = BattleTemplate::parse(
        "test",
        if quad {
            include_str!("../game/mechs/GOL-1H.toml")
        } else {
            include_str!("fixtures/btech/mechs/JR7-D.toml")
        },
    )
    .unwrap();
    if proof {
        template
            .attributes
            .entry("specials".into())
            .and_modify(|flags| flags.push_str(" CritProof_Tech"))
            .or_insert("CritProof_Tech".into());
    }
    assert!(check_battle_template(&template).constructible);
    create_battle_unit(&mut world, target, template).unwrap();
    support::seed_object_dice(&mut world, target, support::FIXTURE_DICE_SEED);
    place_battle_unit(&mut world, shooter, map, 0, 3).unwrap();
    place_battle_unit(&mut world, target, map, 0, 0).unwrap();
    for id in [shooter, target] {
        edit(&mut world, id, |state| {
            state["power"] = serde_json::to_value(BattlePower::Running).unwrap();
        });
    }
    world.objects.get_mut(&ObjectId(1)).unwrap().location = Some(shooter);
    assign_battle_pilot(&mut world, shooter, ObjectId(1)).unwrap();
    support::seed_object_dice(&mut world, ObjectId(1), support::FIXTURE_DICE_SEED);
    for seed in 0..=255 {
        edit(&mut world, shooter, |state| {
            state["dice"] = serde_json::to_value(BattleDice::seeded([seed; 32])).unwrap()
        });
        refresh_battle_contacts(&mut world, &[shooter]).unwrap();
        if visible_battle_contact(&world, shooter, target)
            .unwrap()
            .is_some()
        {
            break;
        }
    }
    assert!(
        visible_battle_contact(&world, shooter, target)
            .unwrap()
            .is_some()
    );
    let high = (0..=255)
        .find(|seed| BattleDice::seeded([*seed; 32]).two_d6() == 12)
        .unwrap();
    edit(&mut world, shooter, |state| {
        state["dice"] = serde_json::to_value(BattleDice::seeded([high; 32])).unwrap()
    });
    world.validate(&config).unwrap();
    (dir, config, world, shooter, target)
}

/// Both launchers reach the configured target route, with exact callback rollback and restart replay.
#[tokio::test]
async fn configured_hit_routes_share_native_lua_and_restart() {
    let selected = (0..=255)
        .find(|seed| {
            let mut dice = BattleDice::seeded([*seed; 32]);
            dice.two_d6() == 3 && dice.two_d6() == 12
        })
        .expect("distinct entry and delegated head roll");
    for vehicle in [false, true] {
        for quad in [false, true] {
            for proof in [false, true] {
                let (_dir, config, base, shooter, target) = fixture(vehicle, quad, proof).await;
                for safe in [false, true] {
                    let mut world = base.clone();
                    set_battle_combat_safe(&mut world, target, safe).unwrap();
                    edit(&mut world, target, |state| {
                        state["dice"] =
                            serde_json::to_value(BattleDice::seeded([selected; 32])).unwrap()
                    });
                    let mut expected_dice = BattleDice::seeded([selected; 32]);
                    let entry = expected_dice.two_d6();
                    let roll = if proof { expected_dice.two_d6() } else { entry };
                    // The shooter approaches the target from behind.
                    let section = if safe {
                        BattleSection::LeftArm
                    } else if roll == 12 {
                        BattleHitTable::Punch
                            .location(
                                base.btech.constructed_units()[&target].chassis(),
                                BattleHitArc::Rear,
                                expected_dice.d6(),
                            )
                            .unwrap()
                    } else {
                        BattleHitTable::Weapon
                            .location(
                                base.btech.constructed_units()[&target].chassis(),
                                BattleHitArc::Rear,
                                roll,
                            )
                            .unwrap()
                    };
                    let native =
                        Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
                    let lua = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
                    let before = lua.world().clone();
                    let call = format!("btech.unit.fire({},1,0,{})", shooter.0, target.0);
                    assert!(
                        lua.eval_callback::<()>(&format!("{call}; error('abort')"))
                            .is_err()
                    );
                    assert_eq!(lua.world().btech, before.btech);
                    assert!(lua.drain_outbox().is_empty());
                    persistence::save(&config.database(), &before)
                        .await
                        .unwrap();
                    let loaded = persistence::load(&config.database()).await.unwrap();
                    let replay = Scripts::new(&config, Rc::new(RefCell::new(loaded))).unwrap();
                    let output = support::run_text(
                        &native,
                        &config,
                        ObjectId(1),
                        1,
                        &format!("fire 0 #{}", target.0),
                    );
                    assert!(output.contains("Hit."), "{output}");
                    let inspect = format!(
                        "local r={call}; local h=r.salvo.report.groups[1].hit; return h.section,h.crew_stun,h.through_armor_critical"
                    );
                    let actual: (String, bool, bool) = lua.eval_callback(&inspect).unwrap();
                    assert_eq!(
                        actual,
                        (
                            format!("{section:?}"),
                            !safe && roll == 12 && section != BattleSection::Head,
                            false
                        )
                    );
                    assert_eq!(native.world().btech, lua.world().btech);
                    assert_eq!(
                        replay
                            .eval_callback::<(String, bool, bool)>(&inspect)
                            .unwrap(),
                        actual
                    );
                    assert_eq!(lua.world().btech, replay.world().btech);
                    if safe {
                        expected_dice.two_d6(); // Immune damage still consumes its entry diagnostic.
                        let state =
                            serde_json::to_value(&lua.world().btech.constructed_units()[&target])
                                .unwrap();
                        assert_eq!(state["dice"], serde_json::to_value(expected_dice).unwrap());
                        assert_eq!(
                            lua.world().btech.constructed_units()[&target].sections(),
                            before.btech.constructed_units()[&target].sections()
                        );
                    }
                    lua.world().validate(&config).unwrap();
                }
            }
        }
    }
}

/// Critical-proof construction suppresses selected components but retains material and catastrophic hits.
#[tokio::test]
async fn critical_proof_preserves_damage_head_injury_and_limb_loss() {
    for quad in [false, true] {
        let (_dir, config, base, _, target) = fixture(false, quad, true).await;
        for section in [
            BattleSection::CenterTorso,
            BattleSection::LeftArm,
            BattleSection::Head,
        ] {
            for (roll, tac) in [(8, false), (12, false), (8, true), (12, true)] {
                let mut world = base.clone();
                let seed = (0..=255)
                    .find(|seed| {
                        let mut dice = BattleDice::seeded([*seed; 32]);
                        dice.two_d6(); // Material entry precedes either critical check.
                        dice.two_d6() == roll
                    })
                    .unwrap();
                edit(&mut world, target, |state| {
                    state["sections"][format!("{section:?}")]["armor"] = 0.into();
                    state["dice"] = serde_json::to_value(BattleDice::seeded([seed; 32])).unwrap();
                });
                let mut expected_dice = BattleDice::seeded([seed; 32]);
                expected_dice.two_d6();
                assert_eq!(expected_dice.two_d6(), roll);
                if tac {
                    expected_dice.two_d6();
                }
                let original =
                    world.btech.constructed_units()[&target].sections()[&section].internal;
                let report = resolve_battle_impact(
                    &mut world,
                    target,
                    BattleHit {
                        section,
                        rear_armor: false,
                        through_armor_critical: tac,
                        crew_stun: false,
                    },
                    1,
                )
                .unwrap();
                assert!(report.criticals.is_empty());
                assert_eq!(
                    world.btech.constructed_units()[&target].sections()[&section].internal,
                    if !tac && roll == 12 && section != BattleSection::CenterTorso {
                        0
                    } else {
                        original - 1
                    }
                );
                assert_eq!(
                    report
                        .pending_effects
                        .contains(&BattleImpactEffect::HeadInjury),
                    section == BattleSection::Head
                );
                let state =
                    serde_json::to_value(&world.btech.constructed_units()[&target]).unwrap();
                assert_eq!(state["dice"], serde_json::to_value(expected_dice).unwrap());
                world.validate(&config).unwrap();
            }
        }
    }
}
