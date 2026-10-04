//! Cockpit claims through normal entry/exit, persisted pilots and callback rollback.
use crate::support;
use stompymux_rs::{
    BattleTemplate, Flag, Kind, MapAsset, ObjectId, Scripts, assign_battle_pilot,
    create_battle_map, create_battle_unit, dbck, persistence, place_battle_unit,
};
const JENNER: &str = include_str!("fixtures/btech/mechs/JR7-D.toml");

#[tokio::test]
async fn enter_pilot_restart_and_leave_preserve_ordinary_movement() {
    let (_dir, config, mut world) = support::isolated_world().await;
    let map = world.create(&config, "Battlefield".into(), Kind::Room);
    create_battle_map(
        &mut world,
        map,
        "test.map",
        MapAsset::from_cells("2 1\n.0.0\n").unwrap(),
    )
    .unwrap();
    support::seed_object_dice(&mut world, map, support::FIXTURE_DICE_SEED);
    world
        .objects
        .get_mut(&ObjectId(2))
        .unwrap()
        .flags
        .remove(Flag::Wizard);
    let mut ids = Vec::new();
    for (player, name, x) in [(ObjectId(1), "Alpha", 0), (ObjectId(2), "Bravo", 1)] {
        let id = world.create(&config, name.into(), Kind::Thing);
        world.objects.get_mut(&id).unwrap().home = Some(ObjectId(config.home()));
        create_battle_unit(
            &mut world,
            id,
            BattleTemplate::parse("JR7-D", JENNER).unwrap(),
        )
        .unwrap();
        support::seed_object_dice(&mut world, id, support::FIXTURE_DICE_SEED);
        place_battle_unit(&mut world, id, map, x, 0).unwrap();
        world.objects.get_mut(&player).unwrap().location = Some(map);
        ids.push(id);
    }
    let scripts = Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(world))).unwrap();
    for (player, id) in [(ObjectId(1), ids[0]), (ObjectId(2), ids[1])] {
        let text = support::run_text(
            &scripts,
            &config,
            player,
            player.0 as u64,
            &format!("enter #{}", id.0),
        );
        assert_eq!(
            scripts.world().objects[&player].location,
            Some(id),
            "{text}"
        );
        let text = support::run_text(&scripts, &config, player, player.0 as u64, "pilot");
        assert!(text.contains("take the cockpit"), "{text}");
    }
    let candidate = scripts.world().clone();
    persistence::save(&config.database(), &candidate)
        .await
        .unwrap();
    let loaded = persistence::load(&config.database()).await.unwrap();
    assert_eq!(loaded.btech, candidate.btech);
    assert_eq!(
        loaded.btech.constructed_units()[&ids[0]].pilot(),
        Some(ObjectId(1))
    );
    assert_eq!(
        loaded.btech.constructed_units()[&ids[1]].pilot(),
        Some(ObjectId(2))
    );
    let scripts = Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(loaded))).unwrap();
    let text = support::run_text(&scripts, &config, ObjectId(1), 1, "unpilot");
    assert!(text.contains("release the cockpit"), "{text}");
    assert_eq!(scripts.world().objects[&ObjectId(1)].location, Some(ids[0]));
    support::run_text(&scripts, &config, ObjectId(1), 1, "pilot");
    let text = support::run_text(&scripts, &config, ObjectId(1), 1, "leave");
    assert_eq!(
        scripts.world().objects[&ObjectId(1)].location,
        Some(map),
        "{text}"
    );
    assert_eq!(
        scripts.world().btech.constructed_units()[&ids[0]].pilot(),
        None
    );
    assert_eq!(
        scripts.world().btech.constructed_units()[&ids[1]].pilot(),
        Some(ObjectId(2))
    );
    let candidate = scripts.world().clone();
    persistence::save(&config.database(), &candidate)
        .await
        .unwrap();
    assert_eq!(
        persistence::load(&config.database()).await.unwrap().btech,
        candidate.btech
    );
}

#[tokio::test]
async fn cockpit_occupancy_authority_callback_rollback_and_pilot_purge() {
    let (_dir, config, mut world) = support::isolated_world().await;
    let id = world.create(&config, "Jenner".into(), Kind::Thing);
    world.objects.get_mut(&id).unwrap().location = Some(ObjectId(config.start()));
    world.objects.get_mut(&id).unwrap().home = Some(ObjectId(config.home()));
    create_battle_unit(
        &mut world,
        id,
        BattleTemplate::parse("JR7-D", JENNER).unwrap(),
    )
    .unwrap();
    assert!(assign_battle_pilot(&mut world, id, ObjectId(1)).is_err());
    for player in [ObjectId(1), ObjectId(2)] {
        world.objects.get_mut(&player).unwrap().location = Some(id);
    }
    world
        .objects
        .get_mut(&ObjectId(2))
        .unwrap()
        .flags
        .remove(Flag::Wizard);
    let scripts = Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(world))).unwrap();
    scripts
        .inspect_lua()
        .globals()
        .set(
            "_parents",
            scripts
                .inspect_lua()
                .named_registry_value::<mlua::Table>("mux.parents")
                .unwrap(),
        )
        .unwrap();
    let parent = scripts.world().objects[&id].lua_parent.clone();
    scripts
        .eval_callback::<()>(&format!(
            "_parents[{parent:?}].locks={{use=function(ctx) return false end}}"
        ))
        .unwrap();
    let text = support::run_text(&scripts, &config, ObjectId(2), 2, "pilot");
    assert!(text.contains("can't pilot"), "{text}");
    scripts.eval_callback::<()>(&format!("_parents[{parent:?}].locks.use=function(ctx) mux.world.teleport_object{{object=2,destination={}}}; return true end",config.start())).unwrap();
    let text = support::run_text(&scripts, &config, ObjectId(2), 2, "pilot");
    assert!(text.contains("Enter the unit"), "{text}");
    assert_eq!(scripts.world().objects[&ObjectId(2)].location, Some(id));
    let before = scripts.world().btech.clone();
    assert!(
        scripts
            .eval_callback::<()>(&format!("btech.unit.pilot({},1); error('abort')", id.0))
            .is_err()
    );
    assert_eq!(scripts.world().btech, before);
    scripts
        .eval_callback::<()>(&format!("btech.unit.pilot({},2)", id.0))
        .unwrap();
    assert!(assign_battle_pilot(&mut scripts.world_mut(), id, ObjectId(1)).is_err());
    assert!(
        scripts
            .eval_callback::<()>(&format!(
                "mux.world.teleport_object{{object=2,destination={}}}; error('abort departure')",
                config.start()
            ))
            .is_err()
    );
    assert_eq!(
        scripts.world().btech.constructed_units()[&id].pilot(),
        Some(ObjectId(2))
    );
    assert_eq!(scripts.world().objects[&ObjectId(2)].location, Some(id));

    let candidate = scripts.world().clone();
    persistence::save(&config.database(), &candidate)
        .await
        .unwrap();
    let mut world = persistence::load(&config.database()).await.unwrap();
    world
        .objects
        .get_mut(&ObjectId(2))
        .unwrap()
        .flags
        .insert(Flag::Going);
    persistence::save(&config.database(), &world).await.unwrap();
    persistence::repair(&config.database(), config.database.busy_timeout_ms, |raw| {
        dbck::plan(&world, raw, &config)
    })
    .await
    .unwrap();
    let loaded = persistence::load(&config.database()).await.unwrap();
    assert_eq!(loaded.btech.constructed_units()[&id].pilot(), None);
    let mut loaded = loaded;
    assign_battle_pilot(&mut loaded, id, ObjectId(1)).unwrap();
    loaded
        .objects
        .get_mut(&id)
        .unwrap()
        .flags
        .insert(Flag::Going);
    persistence::save(&config.database(), &loaded)
        .await
        .unwrap();
    persistence::repair(&config.database(), config.database.busy_timeout_ms, |raw| {
        dbck::plan(&loaded, raw, &config)
    })
    .await
    .unwrap();
    let loaded = persistence::load(&config.database()).await.unwrap();
    assert!(!loaded.btech.constructed_units().contains_key(&id));
    assert_ne!(loaded.objects[&ObjectId(1)].location, Some(id));
}

/// Evacuation uses movement reconciliation, exempts wizards, and keeps XP and relocation atomic.
#[tokio::test]
async fn evacuation_moves_crew_retains_xp_and_rolls_back() {
    use stompymux_rs::*;
    let (_dir, config, mut world) = support::isolated_world().await;
    let afterlife = ObjectId(config.battletech.afterlife_dbref);
    let path = config.root.join("stompymux.toml");
    let mut document: toml::Value =
        toml::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    document
        .as_table_mut()
        .unwrap()
        .entry("battletech")
        .or_insert_with(|| toml::Value::Table(Default::default()));
    document["battletech"]
        .as_table_mut()
        .unwrap()
        .insert("xploss".into(), toml::Value::Integer(500));
    document["battletech"]
        .as_table_mut()
        .unwrap()
        .insert("ic".into(), toml::Value::Integer(1));
    std::fs::write(&path, toml::to_string(&document).unwrap()).unwrap();
    let config = Config::load(&config.root).unwrap();
    let unit = world.create(&config, "Evacuation unit".into(), Kind::Thing);
    world.objects.get_mut(&unit).unwrap().home = Some(ObjectId(config.home()));
    create_battle_unit(
        &mut world,
        unit,
        BattleTemplate::parse("JR7-D", JENNER).unwrap(),
    )
    .unwrap();
    support::seed_object_dice(&mut world, unit, support::FIXTURE_DICE_SEED);
    world
        .objects
        .get_mut(&unit)
        .unwrap()
        .flags
        .insert(Flag::InCharacter);
    let pilot = ObjectId(2);
    world
        .objects
        .get_mut(&pilot)
        .unwrap()
        .flags
        .remove(Flag::Wizard);
    world.objects.get_mut(&pilot).unwrap().location = Some(unit);
    world.objects.get_mut(&ObjectId(1)).unwrap().location = Some(unit);
    set_battle_character(
        &mut world,
        pilot,
        BattleCharacter {
            build: 5,
            reflexes: 5,
            intuition: 5,
            learn: 5,
            charisma: 5,
            bruise: 0,
            lethal: 0,
        },
    )
    .unwrap();
    support::seed_object_dice(&mut world, pilot, support::FIXTURE_DICE_SEED);
    set_battle_character_value(
        &mut world,
        pilot,
        "Piloting-Biped",
        BattleCharacterValue {
            value: 4,
            experience: 16_777_216 + 4000,
            last_used: 123,
        },
    )
    .unwrap();
    assign_battle_pilot(&mut world, unit, pilot).unwrap();
    support::seed_object_dice(&mut world, pilot, support::FIXTURE_DICE_SEED);
    let scripts = Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(world))).unwrap();
    let before = scripts.world().clone();
    assert!(
        scripts
            .eval_callback::<()>(&format!("btech.unit.evacuate({},2)", unit.0))
            .is_err()
    );
    assert_eq!(scripts.world().btech, before.btech);
    assert!(
        scripts
            .eval_callback::<()>(&format!(
                "btech.unit.evacuate({},1); error('abort')",
                unit.0
            ))
            .is_err()
    );
    assert_eq!(scripts.world().objects[&pilot].location, Some(unit));
    assert_eq!(scripts.world().btech, before.btech);
    set_battle_character_value(
        &mut scripts.world_mut(),
        pilot,
        "Unknown",
        BattleCharacterValue {
            experience: 1,
            ..Default::default()
        },
    )
    .unwrap();
    // Custom values are outside the reference's selected stat set and must
    // not prevent crew relocation or lose their own data.
    let moved: usize = scripts
        .eval_callback(&format!("return btech.unit.evacuate({},1)", unit.0))
        .unwrap();
    assert_eq!(moved, 1);
    assert_eq!(scripts.world().objects[&pilot].location, Some(afterlife));
    assert_eq!(scripts.world().objects[&ObjectId(1)].location, Some(unit));
    assert_eq!(
        scripts.world().btech.constructed_units()[&unit].pilot(),
        None
    );
    let value = scripts.world().btech.character_values()[&pilot]["Piloting-Biped"];
    assert_eq!(value.experience, 2000);
    assert_eq!(value.last_used, 123);
    assert_eq!(
        scripts.world().btech.character_values()[&pilot]["Unknown"].experience,
        1
    );
    let moved: usize = scripts
        .eval_callback(&format!("return btech.unit.evacuate({},1)", unit.0))
        .unwrap();
    assert_eq!(moved, 0);
    let candidate = scripts.world().clone();
    candidate.validate(&config).unwrap();
    persistence::save(&config.database(), &candidate)
        .await
        .unwrap();
    let loaded = persistence::load(&config.database()).await.unwrap();
    assert_eq!(loaded.btech, candidate.btech);
    assert_eq!(loaded.objects[&pilot].location, Some(afterlife));
}

/// Lethal material damage and its evacuation commit together, including failure after dice and damage.
#[tokio::test]
async fn casualty_impact_action_rolls_back_damage_and_moves() {
    use stompymux_rs::*;
    let (_dir, config, mut world) = support::isolated_world().await;
    let path = config.root.join("stompymux.toml");
    let mut document: toml::Value =
        toml::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    let table = document
        .as_table_mut()
        .unwrap()
        .entry("battletech")
        .or_insert_with(|| toml::Value::Table(Default::default()))
        .as_table_mut()
        .unwrap();
    table.insert("xploss".into(), toml::Value::Integer(500));
    table.insert("ic".into(), toml::Value::Integer(1));
    std::fs::write(&path, toml::to_string(&document).unwrap()).unwrap();
    let config = Config::load(&config.root).unwrap();
    let unit = world.create(&config, "Casualty unit".into(), Kind::Thing);
    world.objects.get_mut(&unit).unwrap().home = Some(ObjectId(config.home()));
    create_battle_unit(
        &mut world,
        unit,
        BattleTemplate::parse("JR7-D", JENNER).unwrap(),
    )
    .unwrap();
    support::seed_object_dice(&mut world, unit, support::FIXTURE_DICE_SEED);
    world
        .objects
        .get_mut(&unit)
        .unwrap()
        .flags
        .insert(Flag::InCharacter);
    let pilot = ObjectId(2);
    world
        .objects
        .get_mut(&pilot)
        .unwrap()
        .flags
        .remove(Flag::Wizard);
    world.objects.get_mut(&pilot).unwrap().location = Some(unit);
    set_battle_character(
        &mut world,
        pilot,
        BattleCharacter {
            build: 5,
            reflexes: 5,
            intuition: 5,
            learn: 5,
            charisma: 5,
            bruise: 0,
            lethal: 0,
        },
    )
    .unwrap();
    support::seed_object_dice(&mut world, pilot, support::FIXTURE_DICE_SEED);
    for (name, experience) in [
        ("Toughness", 16_777_227),
        ("Lives", 11),
        ("Acrobatics", u32::MAX),
        ("Custom", 17),
    ] {
        set_battle_character_value(
            &mut world,
            pilot,
            name,
            BattleCharacterValue {
                value: 0,
                experience,
                last_used: 123,
            },
        )
        .unwrap();
    }
    assign_battle_pilot(&mut world, unit, pilot).unwrap();
    support::seed_object_dice(&mut world, pilot, support::FIXTURE_DICE_SEED);
    let baseline = world.clone();
    let scripts = Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(world))).unwrap();
    let hit = |section| BattleHit {
        section,
        rear_armor: false,
        through_armor_critical: false,
        crew_stun: false,
    };
    let report = resolve_battle_impact_action(
        &scripts,
        &config,
        unit,
        hit(BattleSection::CenterTorso),
        100,
    )
    .unwrap();
    assert!(report.destroyed);
    assert!(report.crew_casualty().is_none());
    assert_eq!(scripts.world().objects[&pilot].location, Some(unit));
    *scripts.world_mut() = baseline.clone();
    let afterlife = ObjectId(config.battletech.afterlife_dbref);
    scripts.world_mut().objects.remove(&afterlife);
    let invalid = scripts.world().clone();
    assert!(
        resolve_battle_impact_action(&scripts, &config, unit, hit(BattleSection::Head), 100)
            .is_err()
    );
    assert_eq!(scripts.world().btech, invalid.btech);
    assert_eq!(scripts.world().objects[&pilot].location, Some(unit));
    *scripts.world_mut() = baseline.clone();
    let parents: mlua::Table = scripts
        .inspect_lua()
        .named_registry_value("mux.parents")
        .unwrap();
    let parent: mlua::Table = parents
        .get(scripts.world().objects[&unit].lua_parent.as_str())
        .unwrap();
    let previous: mlua::Value = parent.get("events").unwrap();
    let events = scripts.inspect_lua().create_table().unwrap();
    let failure = scripts
        .inspect_lua()
        .create_function(|_, _: mlua::Value| -> mlua::Result<()> {
            Err(mlua::Error::external("casualty departure failed"))
        })
        .unwrap();
    events.set("on_leave", failure).unwrap();
    parent.set("events", events).unwrap();
    assert!(
        resolve_battle_impact_action(&scripts, &config, unit, hit(BattleSection::Head), 100)
            .is_err()
    );
    assert_eq!(scripts.world().btech, baseline.btech);
    assert_eq!(scripts.world().objects[&pilot].location, Some(unit));
    parent.set("events", previous).unwrap();
    *scripts.world_mut() = baseline;
    let report =
        resolve_battle_impact_action(&scripts, &config, unit, hit(BattleSection::Head), 100)
            .unwrap();
    for (name, experience) in [
        ("Toughness", 5),
        ("Lives", 5),
        ("Acrobatics", 0),
        ("Custom", 17),
    ] {
        let value = scripts.world().btech.character_values()[&pilot][name];
        assert_eq!(value.experience, experience, "{name}");
        assert_eq!(value.last_used, 123);
    }
    assert_eq!(
        report.crew_casualty(),
        Some(BattleCrewCasualty::HeadDestroyed)
    );
    assert_eq!(scripts.world().objects[&pilot].location, Some(afterlife));
    assert_eq!(
        scripts.world().btech.constructed_units()[&unit].pilot(),
        None
    );
    let candidate = scripts.world().clone();
    persistence::save(&config.database(), &candidate)
        .await
        .unwrap();
    let loaded = persistence::load(&config.database()).await.unwrap();
    assert_eq!(loaded.btech, candidate.btech);
    assert_eq!(loaded.objects[&pilot].location, Some(afterlife));
}

/// RPG health survives more than six injury points; fatal injury and evacuation share rollback.
#[tokio::test]
async fn character_pilot_health_recovery_and_fatal_evacuation() {
    use stompymux_rs::*;
    let (_dir, config, mut world) = support::isolated_world().await;
    let unit = world.create(&config, "Character injury unit".into(), Kind::Thing);
    world.objects.get_mut(&unit).unwrap().home = Some(ObjectId(config.home()));
    create_battle_unit(
        &mut world,
        unit,
        BattleTemplate::parse("JR7-D", JENNER).unwrap(),
    )
    .unwrap();
    support::seed_object_dice(&mut world, unit, support::FIXTURE_DICE_SEED);
    world
        .objects
        .get_mut(&unit)
        .unwrap()
        .flags
        .insert(Flag::InCharacter);
    let pilot = ObjectId(2);
    world
        .objects
        .get_mut(&pilot)
        .unwrap()
        .flags
        .remove(Flag::Wizard);
    world.objects.get_mut(&pilot).unwrap().location = Some(unit);
    set_battle_character(
        &mut world,
        pilot,
        BattleCharacter {
            build: 5,
            reflexes: 5,
            intuition: 5,
            learn: 5,
            charisma: 5,
            bruise: 0,
            lethal: 0,
        },
    )
    .unwrap();
    assign_battle_pilot(&mut world, unit, pilot).unwrap();
    let seed = (0..=255)
        .find(|seed| BattleDice::seeded([*seed; 32]).two_d6() == 2)
        .unwrap();
    let mut state = serde_json::to_value(&world.btech).unwrap();
    state["recoveries"][pilot.0.to_string()]["dice"] =
        serde_json::to_value(BattleDice::seeded([seed; 32])).unwrap();
    world.btech = serde_json::from_value(state).unwrap();
    let before = world.btech.clone();
    injure_battle_character_pilot(&mut world, unit, 0, false).unwrap();
    assert_eq!(world.btech, before);
    let report = injure_battle_character_pilot(&mut world, unit, 6, false).unwrap();
    assert!(!report.injury.fatal);
    assert!(!report.consciousness.unwrap().conscious);
    assert!(world.btech.unconscious(pilot));
    assert!(!world.btech.constructed_units()[&unit].is_destroyed());
    assert_eq!(
        world.btech.constructed_units()[&unit]
            .character_pilot_status()
            .unwrap()
            .injuries,
        6
    );
    assert_eq!(world.btech.constructed_units()[&unit].pilot_injuries(), 6);
    let recovery = world.btech.recoveries()[&pilot].clone();
    let report = injure_battle_character_pilot(&mut world, unit, 1, false).unwrap();
    assert_eq!(report.consciousness, None);
    assert_eq!(world.btech.recoveries()[&pilot], recovery);
    persistence::save(&config.database(), &world).await.unwrap();
    let loaded = persistence::load(&config.database()).await.unwrap();
    assert_eq!(loaded.btech, world.btech);
    let scripts = Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(loaded))).unwrap();
    let baseline = scripts.world().clone();
    let afterlife = ObjectId(config.battletech.afterlife_dbref);
    scripts.world_mut().objects.remove(&afterlife);
    assert!(injure_battle_character_pilot_action(&scripts, &config, unit, 3, false).is_err());
    assert_eq!(scripts.world().btech, baseline.btech);
    assert_eq!(scripts.world().objects[&pilot].location, Some(unit));
    *scripts.world_mut() = baseline;
    injure_battle_character_pilot(&mut scripts.world_mut(), unit, 2, false).unwrap();
    let report = resolve_battle_impact_action(
        &scripts,
        &config,
        unit,
        BattleHit {
            section: BattleSection::Head,
            rear_armor: false,
            through_armor_critical: false,
            crew_stun: false,
        },
        1,
    )
    .unwrap();
    assert_eq!(
        report.crew_casualty(),
        Some(BattleCrewCasualty::CharacterInjury)
    );
    assert_eq!(report.character_injuries.len(), 1);
    assert!(report.character_injuries[0].injury.fatal);
    assert_eq!(report.character_injuries[0].consciousness, None);
    assert_eq!(scripts.world().objects[&pilot].location, Some(afterlife));
    let candidate = scripts.world().clone();
    let status = candidate.btech.constructed_units()[&unit]
        .character_pilot_status()
        .unwrap();
    assert!(status.killed);
    assert_eq!(status.injuries, 9);
    assert!(candidate.btech.constructed_units()[&unit].is_destroyed());
    assert_eq!(candidate.btech.constructed_units()[&unit].pilot(), None);
    assert!(!candidate.btech.unconscious(pilot));
    candidate.validate(&config).unwrap();
    persistence::save(&config.database(), &candidate)
        .await
        .unwrap();
    assert_eq!(
        persistence::load(&config.database()).await.unwrap().btech,
        candidate.btech
    );
}
