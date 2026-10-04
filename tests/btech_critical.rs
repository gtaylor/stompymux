//! Slot loss, broken multi-slot weapons, selection depletion, secondary effects and restart.
use crate::support;
use stompymux_rs::{
    CriticalLocation, CriticalLoss as Loss, Kind, Mech, MechSection as Section, MechTemplate,
    ObjectId, System, create_battle_unit, destroy_battle_critical, persistence,
};

/// Independent scenario unit with a conventional supported loadout.
fn unit(source: &str) -> Mech {
    Mech::from_template(MechTemplate::parse("test", source).unwrap()).unwrap()
}
const JENNER: &str = include_str!("fixtures/btech/mechs/JR7-D.toml");

#[test]
fn multi_slot_weapon_fails_once_but_remaining_slots_can_absorb_more_criticals() {
    let mut atlas = unit(include_str!("fixtures/btech/mechs/AS7-D.toml"));
    let loadout = atlas.loadout().unwrap();
    let (index, weapon) = loadout
        .weapons
        .iter()
        .enumerate()
        .find(|(_, mount)| mount.criticals.len() == 10)
        .unwrap();
    let first = weapon.criticals[0];
    let second = weapon.criticals[1];
    assert!(atlas.weapon_intact(index).unwrap());
    assert_eq!(
        atlas.destroy_critical(first).unwrap(),
        Some(Loss::Weapon {
            index,
            explosion_damage: 0
        })
    );
    assert!(!atlas.weapon_intact(index).unwrap());
    assert!(atlas.critical_candidates(second.section).contains(&second));
    assert!(!atlas.critical_candidates(first.section).contains(&first));
    let before = atlas.clone();
    assert_eq!(atlas.destroy_critical(first).unwrap(), None);
    assert!(
        atlas
            .destroy_critical(CriticalLocation {
                section: Section::Head,
                slot: 99
            })
            .is_err()
    );
    assert_eq!(atlas, before);
    assert_eq!(
        atlas.destroy_critical(second).unwrap(),
        Some(Loss::Weapon {
            index,
            explosion_damage: 0
        })
    );
    assert!(atlas.weapon_intact(999).is_err());
}

#[test]
fn selection_exhausts_only_installed_slots_and_ammunition_preserves_explosion_accounting() {
    let mut mech = unit(JENNER);
    let mut selected = std::collections::BTreeSet::new();
    while let Some(location) = mech.choose_critical(Section::LeftArm) {
        assert!(selected.insert(location));
        mech.destroy_critical(location).unwrap();
    }
    assert_eq!(selected.len(), 4);
    let before = mech.clone();
    assert_eq!(mech.choose_critical(Section::LeftArm), None);
    assert_eq!(mech, before);
    let bin = mech.loadout().unwrap().ammunition[0].location;
    assert_eq!(
        mech.destroy_critical(bin).unwrap(),
        Some(Loss::Ammunition {
            index: 0,
            rounds: 25,
            explosion_damage: 200
        })
    );
    assert_eq!(mech.ammunition(), &[0]);
    assert_eq!(mech.destroy_critical(bin).unwrap(), None);
}

#[tokio::test]
async fn engine_and_cockpit_losses_destroy_units_and_slot_state_survives_restart() {
    let (_dir, config, mut world) = support::isolated_world().await;
    let id = world.create(&config, "Critical Jenner".into(), Kind::Thing);
    let object = world.objects.get_mut(&id).unwrap();
    object.location = Some(ObjectId(config.start()));
    object.home = Some(ObjectId(config.home()));
    create_battle_unit(
        &mut world,
        id,
        MechTemplate::parse("JR7-D", JENNER).unwrap(),
    )
    .unwrap();
    support::seed_object_dice(&mut world, id, support::FIXTURE_DICE_SEED);
    persistence::save(&config.database(), &world).await.unwrap();
    for slot in 0..3 {
        destroy_battle_critical(
            &mut world,
            id,
            CriticalLocation {
                section: Section::CenterTorso,
                slot,
            },
        )
        .unwrap();
        assert_eq!(
            world.btech.constructed_units()[&id].system_hits(System::Engine),
            slot + 1
        );
        assert_eq!(
            world.btech.constructed_units()[&id].is_destroyed(),
            slot == 2
        );
    }
    persistence::save(&config.database(), &world).await.unwrap();
    let loaded = persistence::load(&config.database()).await.unwrap();
    assert_eq!(loaded.btech, world.btech);
    let scripts =
        stompymux_rs::Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(loaded)))
            .unwrap();
    assert_eq!(
        scripts
            .eval_callback::<usize>(&format!(
                "return #btech.unit.state({}).lost_criticals",
                id.0
            ))
            .unwrap(),
        3
    );
    let mut mech = unit(JENNER);
    let cockpit = mech
        .loadout()
        .unwrap()
        .systems
        .iter()
        .find(|part| part.system == System::Cockpit)
        .unwrap()
        .location;
    mech.destroy_critical(cockpit).unwrap();
    assert!(mech.is_destroyed());
    assert!(mech.sections()[&Section::Head].internal > 0);
}
