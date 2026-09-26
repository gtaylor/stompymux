//! Station contacts share visibility, preferences and callback revalidation across chassis.
use crate::support;
use crate::support::btech_firing as firing;
use std::{cell::RefCell, rc::Rc};
use stompymux_rs::*;

/// Create a physical scanner and registered gunner with a visible hostile contact.
async fn fixture(
    template: &str,
) -> (
    tempfile::TempDir,
    Config,
    Rc<Scripts>,
    ObjectId,
    ObjectId,
    ObjectId,
    ObjectId,
) {
    let (dir, config, mut world, parent, target, _) =
        firing::fixture_with_target(template, Some(BattleWeapon::MediumLaser), template).await;
    let station = world.create(&config, "Station".into(), Kind::Thing);
    let gunner = world.create(&config, "Gunner".into(), Kind::Player);
    world.objects.get_mut(&gunner).unwrap().location = Some(station);
    register_gunner_station(&mut world, ObjectId(1), station, parent, 0).unwrap();
    let scripts = Rc::new(Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap());
    gunner_station_action(&scripts, station, gunner, true).unwrap();
    scripts.drain_outbox();
    (dir, config, scripts, parent, target, station, gunner)
}

/// Independent selection controls inclusion and highlighting, never acquired visibility.
#[tokio::test]
async fn station_contacts_share_chassis_reports_and_persist_preferences() {
    for template in firing::templates() {
        let (_dir, config, scripts, parent, target, station, gunner) = fixture(&template).await;
        for owner in [ObjectId(1), gunner] {
            set_battle_contact_preferences(
                &mut scripts.world_mut(),
                owner,
                BattleContactPreferences {
                    include_allies: false,
                    include_enemies: false,
                    include_target: true,
                    ..Default::default()
                },
            )
            .unwrap();
        }
        for mode in 0..=3 {
            battle_brief(&scripts, parent, ObjectId(1), &format!("C {mode}")).unwrap();
            scripts.drain_outbox();
            select_battle_target(&mut scripts.world_mut(), station, gunner, None).unwrap();
            let empty = support::run_text(&scripts, &config, gunner, 1, "contacts +");
            let cockpit = support::run_text(&scripts, &config, ObjectId(1), 1, "contacts +");
            assert_ne!(
                empty, cockpit,
                "parent target must not leak into station filtering"
            );
            select_battle_target(&mut scripts.world_mut(), station, gunner, Some(target)).unwrap();
            let before = scripts.world().btech.clone();
            for argument in [
                "".to_string(),
                "+".into(),
                "t".into(),
                "dsae".into(),
                "tQ".into(),
                format!("#{}", target.0),
            ] {
                let expected = support::run_text(
                    &scripts,
                    &config,
                    ObjectId(1),
                    1,
                    &format!("contacts {argument}"),
                );
                let actual = support::run_text(
                    &scripts,
                    &config,
                    gunner,
                    1,
                    &format!("contacts {argument}"),
                );
                assert_eq!(actual, expected, "{template}: mode {mode}, {argument}");
                let lua: String = scripts
                    .eval_callback(&format!(
                        "return btech.gunner.contacts({}, {}, '{argument}')",
                        station.0, gunner.0
                    ))
                    .unwrap();
                assert_eq!(actual.trim(), lua.trim());
            }
            assert_eq!(scripts.world().btech, before);
        }
        let saved = scripts.world().clone();
        persistence::save(&config.database(), &saved).await.unwrap();
        let restored = persistence::load(&config.database()).await.unwrap();
        let restarted = Scripts::new(&config, Rc::new(RefCell::new(restored))).unwrap();
        assert_eq!(
            support::run_text(&scripts, &config, gunner, 1, "contacts +"),
            support::run_text(&restarted, &config, gunner, 1, "contacts +")
        );
        // A stale selection never reveals a target that is no longer available.
        scripts
            .world_mut()
            .objects
            .get_mut(&target)
            .unwrap()
            .flags
            .insert(Flag::Going);
        assert!(
            visible_battle_contacts(&scripts.world(), parent)
                .unwrap()
                .is_empty()
        );
        let hidden_contacts: String = scripts
            .eval_callback(&format!(
                "return btech.gunner.contacts({}, {}, '+')",
                station.0, gunner.0
            ))
            .unwrap();
        assert!(!hidden_contacts.contains(" x:"), "{hidden_contacts}");
        assert!(
            scripts
                .eval_callback::<String>(&format!(
                    "return btech.gunner.contacts({}, {})",
                    parent.0, gunner.0
                ))
                .is_err()
        );
        gunner_station_action(&scripts, station, gunner, false).unwrap();
        scripts.drain_outbox();
        assert!(
            support::run_text(&scripts, &config, gunner, 1, "contacts")
                .contains("hasn't been initialized")
        );
        scripts
            .world_mut()
            .objects
            .get_mut(&gunner)
            .unwrap()
            .location = Some(parent);
        assert_eq!(
            support::run_text(&scripts, &config, gunner, 1, "contacts"),
            support::run_text(&scripts, &config, ObjectId(1), 1, "contacts")
        );
    }
}

/// Structure callbacks receive the parent as cause and cannot silently replace station authority.
#[tokio::test]
async fn station_building_contacts_revalidate_owner_and_rollback_callbacks() {
    for template in firing::templates() {
        let (_dir, config, scripts, parent, target, station, gunner) = fixture(&template).await;
        let map = {
            let world = scripts.world();
            world
                .btech
                .constructed_units()
                .get(&parent)
                .and_then(|unit| unit.position())
                .or_else(|| {
                    world
                        .btech
                        .vehicles()
                        .get(&parent)
                        .and_then(|unit| unit.position())
                })
                .unwrap()
                .map
        };
        let interior = {
            let mut world = scripts.world_mut();
            let interior = world.create(&config, "Hangar".into(), Kind::Room);
            create_battle_map(
                &mut world,
                interior,
                "hangar.map",
                BattleMapAsset::parse("1 1\n.0\n").unwrap(),
            )
            .unwrap();
            set_building_state(
                &mut world,
                interior,
                BattleBuildingState {
                    integrity: 31,
                    maximum_integrity: 50,
                    flags: 4,
                    regeneration: 1,
                },
            )
            .unwrap();
            set_building_entrance(
                &mut world,
                map,
                0,
                Some(BattleBuildingEntrance {
                    coordinate: BattleHexCoordinate { x: 0, y: 9 },
                    interior,
                    data_char: 0,
                    data_short: 0,
                    data_int: 0,
                }),
            )
            .unwrap();
            interior
        };
        let parents: mlua::Table = scripts
            .inspect_lua()
            .named_registry_value("mux.parents")
            .unwrap();
        scripts
            .inspect_lua()
            .globals()
            .set("_parents", parents)
            .unwrap();
        scripts.eval_callback::<()>(&format!("_parents['default_room.lua'].locks={{identify_building=function(ctx) assert(ctx.object=={} and ctx.enactor=={} and ctx.subject=={} and ctx.cause=={} and ctx.silent); return true end}}", interior.0, gunner.0, gunner.0, parent.0)).unwrap();
        let query = format!(
            "return btech.gunner.contacts({}, {}, 'b')",
            station.0, gunner.0
        );
        let initial: String = scripts.eval_callback(&query).unwrap();
        assert!(initial.contains("Hangar"), "{initial}");
        assert_eq!(
            initial.trim(),
            support::run_text(&scripts, &config, gunner, 1, "contacts b").trim()
        );
        scripts.eval_callback::<()>("_parents['default_room.lua'].locks.identify_building=function(ctx) return false end").unwrap();
        let denied: String = scripts.eval_callback(&query).unwrap();
        assert!(!denied.contains("Hangar"));
        // Invisible structures are discarded before their identification callback can execute.
        let mut building = scripts.world().btech.maps()[&interior].building;
        building.flags = 16;
        set_building_state(&mut scripts.world_mut(), interior, building).unwrap();
        scripts.eval_callback::<()>("_parents['default_room.lua'].locks.identify_building=function(ctx) error('invisible lock invoked') end").unwrap();
        assert!(
            !scripts
                .eval_callback::<String>(&query)
                .unwrap()
                .contains("Hangar")
        );
        building.flags = 4;
        set_building_state(&mut scripts.world_mut(), interior, building).unwrap();
        for change in 0..3 {
            let weak = Rc::downgrade(&scripts);
            let mutate = scripts
                .inspect_lua()
                .create_function(move |_, ()| {
                    let scripts = weak.upgrade().unwrap();
                    let mut world = scripts.world_mut();
                    match change {
                        0 => world.objects.get_mut(&gunner).unwrap().location = Some(parent),
                        1 => {
                            let mut state = serde_json::to_value(&world.btech).unwrap();
                            state["gunner_stations"][station.0.to_string()]["parent"] =
                                target.0.into();
                            world.btech = serde_json::from_value(state).unwrap();
                        }
                        _ => {
                            world
                                .objects
                                .get_mut(&station)
                                .unwrap()
                                .flags
                                .insert(Flag::Going);
                        }
                    }
                    Ok(true)
                })
                .unwrap();
            scripts
                .inspect_lua()
                .globals()
                .set("mutate", mutate)
                .unwrap();
            scripts.eval_callback::<()>("_parents['default_room.lua'].locks.identify_building=function(ctx) return mutate() end").unwrap();
            let before = scripts.world().clone();
            assert!(scripts.eval_callback::<String>(&query).is_err());
            assert_eq!(scripts.world().btech, before.btech);
            assert_eq!(
                scripts.world().objects[&gunner].location,
                before.objects[&gunner].location
            );
            assert_eq!(
                scripts.world().objects[&station].flags,
                before.objects[&station].flags
            );
            assert!(scripts.drain_outbox().is_empty());
            let native = support::run_text(&scripts, &config, gunner, 1, "contacts b");
            assert!(!native.contains("Hangar"), "{native}");
            assert_eq!(scripts.world().btech, before.btech);
            assert_eq!(
                scripts.world().objects[&gunner].location,
                before.objects[&gunner].location
            );
            assert_eq!(
                scripts.world().objects[&station].flags,
                before.objects[&station].flags
            );
        }
    }
}
