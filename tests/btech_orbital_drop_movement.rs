//! Live orbital descent shares the airborne transaction, durable dice and existing physical consequences.
use crate::support::btech_firing as firing;
use std::{cell::RefCell, rc::Rc};
use stompymux_rs::*;

/// Saved scenario setup exercises production deserialization without adding an unguarded launch API.
fn prepare(world: &mut World, id: ObjectId, elevation: i32, safe: bool, roll: u8) {
    let dice = (0..=255)
        .map(|seed| BattleDice::seeded([seed; 32]))
        .find(|dice| dice.clone().two_d6() == roll)
        .unwrap();
    firing::edit(world, id, |state| {
        state["orbital_drop"] =
            serde_json::to_value(BattleOrbitalDrop::new(35 * 1024, elevation).unwrap()).unwrap();
        state["combat_safe"] = safe.into();
        state["ground_elevation"] = serde_json::Value::Null;
        state["dice"] = serde_json::to_value(dice).unwrap();
    });
}

/// Compare material state and randomness without depending on anatomy storage.
fn saved_unit(world: &World, id: ObjectId) -> serde_json::Value {
    if let Some(unit) = world.btech.constructed_units().get(&id) {
        serde_json::to_value(unit).unwrap()
    } else {
        serde_json::to_value(&world.btech.vehicles()[&id]).unwrap()
    }
}

#[tokio::test]
async fn stopped_safe_drops_advance_once_per_airborne_tick_and_land_without_dice() {
    for source in firing::templates().into_iter().take(6) {
        let (_dir, config, mut world, unit, _, _) =
            firing::fixture_with_target(&source, None, &source).await;
        prepare(&mut world, unit, 10, true, 2);
        firing::edit(&mut world, unit, |state| {
            state["power"] = serde_json::to_value(BattlePower::Off).unwrap();
            state["target_lock"] = serde_json::Value::Null;
        });
        let original = saved_unit(&world, unit);
        advance_battle_jumps(&mut world, BattleMovementRules::STANDARD).unwrap();
        assert_eq!(battle_unit_elevation(&world, unit).unwrap(), Some(8));
        persistence::save(&config.database(), &world).await.unwrap();
        let mut restored = persistence::load(&config.database()).await.unwrap();
        for expected in [6, 4, 2, 0] {
            let notices = advance_battle_jumps(&mut world, BattleMovementRules::STANDARD).unwrap();
            assert_eq!(
                notices,
                advance_battle_jumps(&mut restored, BattleMovementRules::STANDARD).unwrap()
            );
            assert_eq!(world.btech, restored.btech);
            assert_eq!(battle_unit_elevation(&world, unit).unwrap(), Some(expected));
            assert_eq!(
                notices
                    .iter()
                    .any(|notice| notice.text == "Your unit touches down!"),
                expected == 0
            );
            world.validate(&config).unwrap();
        }
        let landed = saved_unit(&world, unit);
        assert!(landed["orbital_drop"].is_null());
        assert_eq!(landed["dice"], original["dice"]);
        assert_eq!(landed["sections"], original["sections"]);
        let before = world.btech.clone();
        assert!(
            advance_battle_jumps(&mut world, BattleMovementRules::STANDARD)
                .unwrap()
                .is_empty()
        );
        assert_eq!(world.btech, before);
    }
}

#[tokio::test]
async fn failed_landings_apply_shared_fall_damage_and_replay() {
    for source in firing::templates().into_iter().take(6) {
        let (_dir, config, mut world, unit, _, _) =
            firing::fixture_with_target(&source, None, &source).await;
        prepare(&mut world, unit, 2, false, 2);
        let before = saved_unit(&world, unit);
        persistence::save(&config.database(), &world).await.unwrap();
        let mut restored = persistence::load(&config.database()).await.unwrap();
        let notices = advance_battle_jumps(&mut world, BattleMovementRules::STANDARD).unwrap();
        assert_eq!(
            notices,
            advance_battle_jumps(&mut restored, BattleMovementRules::STANDARD).unwrap()
        );
        assert_eq!(world.btech, restored.btech);
        assert!(
            notices.iter().any(|notice| notice.unit == unit
                && notice.text.contains("unable to control your momentum"))
        );
        let after = saved_unit(&world, unit);
        assert!(after["orbital_drop"].is_null());
        assert_ne!(after["sections"], before["sections"]);
        assert_eq!(battle_unit_elevation(&world, unit).unwrap(), Some(0));
        world.validate(&config).unwrap();
    }
}

#[tokio::test]
async fn successful_water_landings_use_chassis_support_and_waterproofing() {
    let sources = firing::templates();
    let waterproof = crate::support::templates::with_flags(&sources[2], &["Waterproof_Tech"]);
    for source in [&sources[2], &sources[4], &waterproof] {
        for safe in [false, true] {
            let (_dir, config, mut world, unit, _, _) =
                firing::fixture_with_target(source, None, source).await;
            prepare(&mut world, unit, -8, safe, 12);
            let map = world.objects[&unit].location.unwrap();
            let mut state = serde_json::to_value(&world.btech).unwrap();
            state["maps"][map.0.to_string()]["terrain"][11] =
                serde_json::to_value(BattleHex::new(Terrain::Water, 3)).unwrap();
            world.btech = serde_json::from_value(state).unwrap();
            let before = saved_unit(&world, unit);
            let notices = advance_battle_jumps(&mut world, BattleMovementRules::STANDARD).unwrap();
            let vehicle = &world.btech.vehicles()[&unit];
            let hover = vehicle.definition().movement == BattleVehicleMovement::Hover;
            assert_eq!(
                battle_unit_elevation(&world, unit).unwrap(),
                Some(if hover { 0 } else { -3 })
            );
            let flooded = !safe && !hover && !source.contains("Waterproof_Tech");
            assert_eq!(vehicle.flooded(), flooded);
            assert_eq!(
                notices
                    .iter()
                    .any(|notice| notice.text.contains("Water floods")),
                flooded
            );
            assert_eq!(saved_unit(&world, unit)["sections"], before["sections"]);
            world.validate(&config).unwrap();
        }
    }
}

#[tokio::test]
async fn host_landing_publication_failure_restores_drop_damage_dice_and_output() {
    let source = &firing::templates()[0];
    let (dir, _, mut world, unit, _, _) = firing::fixture_with_target(source, None, source).await;
    prepare(&mut world, unit, 2, false, 2);
    let path = dir.path().join("stompymux.toml");
    let mut table: toml::Table = std::fs::read_to_string(&path).unwrap().parse().unwrap();
    table
        .entry("lua")
        .or_insert(toml::Value::Table(toml::Table::new()))
        .as_table_mut()
        .unwrap()
        .insert("output_entry_limit".into(), 2.into());
    std::fs::write(path, toml::to_string(&table).unwrap()).unwrap();
    let config = Config::load(dir.path()).unwrap();
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
    let before = scripts.world().clone();
    let error =
        advance_battle_jumps_action(&scripts, &config, BattleMovementRules::STANDARD).unwrap_err();
    assert!(error.to_string().contains("output limit"), "{error:#}");
    assert_eq!(scripts.world().btech, before.btech);
    assert_eq!(
        serde_json::to_value(&scripts.world().objects).unwrap(),
        serde_json::to_value(&before.objects).unwrap()
    );
    assert!(scripts.drain_outbox().is_empty());
}

#[tokio::test]
async fn character_landings_award_the_drop_reason_and_publish_the_pilot_roll() {
    let sources = firing::templates();
    for (source, skill) in [
        (&sources[0], "Piloting-Biped"),
        (&sources[2], "Piloting-Tracked"),
    ] {
        let (_dir, config, mut world, unit, _, _) =
            firing::fixture_with_target(source, None, source).await;
        let pilot = ObjectId(1);
        world
            .objects
            .get_mut(&pilot)
            .unwrap()
            .flags
            .insert(Flag::Connected);
        world
            .objects
            .get_mut(&unit)
            .unwrap()
            .flags
            .insert(Flag::InCharacter);
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
        crate::support::seed_object_dice(&mut world, pilot, crate::support::FIXTURE_DICE_SEED);
        set_battle_character_value(
            &mut world,
            pilot,
            skill,
            BattleCharacterValue {
                value: 0,
                experience: 0,
                last_used: 0,
            },
        )
        .unwrap();
        prepare(&mut world, unit, 2, false, 12);
        assert_eq!(battle_unit_piloting_target(&world, unit, true).unwrap(), 8);
        let before = world.btech.clone();
        assert!(advance_battle_jumps(&mut world, BattleMovementRules::STANDARD).is_err());
        assert_eq!(world.btech, before);
        let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
        advance_battle_jumps_action(&scripts, &config, BattleMovementRules::STANDARD).unwrap();
        assert_eq!(
            scripts.world().btech.character_values()[&pilot][skill].experience,
            10
        );
        let output = scripts.drain_outbox();
        assert!(output.iter().any(|(recipient, text)| {
            *recipient == pilot
                && text
                    .source()
                    .contains("Modified Pilot Skill: BTH 8\tRoll: 12")
        }));
        let messages: Vec<_> = output
            .iter()
            .filter(|(recipient, _)| *recipient == pilot)
            .map(|(_, text)| text.source())
            .collect();
        let touchdown = messages
            .iter()
            .position(|text| text.contains("Your unit touches down!"))
            .unwrap();
        let roll = messages
            .iter()
            .position(|text| text.contains("Modified Pilot Skill:"))
            .unwrap();
        assert!(touchdown < roll);
        assert!(saved_unit(&scripts.world(), unit)["orbital_drop"].is_null());
        scripts.world().validate(&config).unwrap();
    }
}

#[tokio::test]
async fn landing_callback_precedes_dice_and_changes_the_shared_landing_rules() {
    for source in firing::templates().into_iter().take(6) {
        let (dir, config, mut world, unit, _, _) =
            firing::fixture_with_target(&source, None, &source).await;
        prepare(&mut world, unit, 4, false, 2);
        world.objects.get_mut(&unit).unwrap().lua_parent = "drop_event.lua".into();
        std::fs::write(
            dir.path().join("lua/object_logic/drop_event.lua"),
            r#"
return {events={on_ood_land=function(ctx)
    assert(ctx.object==ctx.enactor and ctx.object==ctx.cause)
    assert(ctx.operation=='ood_land' and ctx.event=='on_ood_land')
    assert(ctx.source==nil and ctx.destination==nil and #ctx.args==0)
    local unit=btech.unit.state(ctx.object)
    assert(unit.orbital_drop~=nil)
    local state=mux.world.object(ctx.object):state('drop')
    state:set('count',(state:get('count') or 0)+1)
    btech.unit.combat_safe(ctx.object,true)
    mux.world.pemit(1,'DROP_CALLBACK')
    if fail_drop then error('landing callback abort') end
end}}
"#,
        )
        .unwrap();
        let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
        let original = saved_unit(&scripts.world(), unit);
        advance_battle_jumps_action(&scripts, &config, BattleMovementRules::STANDARD).unwrap();
        assert_eq!(
            battle_unit_elevation(&scripts.world(), unit).unwrap(),
            Some(2)
        );
        assert!(scripts.drain_outbox().is_empty());
        scripts.inspect_lua().load("fail_drop=true").exec().unwrap();
        scripts
            .eval_callback::<()>("mux.world.pemit(1,'PRIOR')")
            .unwrap();
        let before = serde_json::to_value(&*scripts.world()).unwrap();
        let error = advance_battle_jumps_action(&scripts, &config, BattleMovementRules::STANDARD)
            .unwrap_err();
        assert!(format!("{error:#}").contains("landing callback abort"));
        assert_eq!(serde_json::to_value(&*scripts.world()).unwrap(), before);
        let output = scripts.drain_outbox();
        assert_eq!(output.len(), 1);
        assert_eq!(output[0].1.source(), "PRIOR");
        scripts
            .inspect_lua()
            .load("fail_drop=false")
            .exec()
            .unwrap();
        advance_battle_jumps_action(&scripts, &config, BattleMovementRules::STANDARD).unwrap();
        let landed = saved_unit(&scripts.world(), unit);
        assert!(landed["orbital_drop"].is_null());
        assert_eq!(landed["dice"], original["dice"]);
        assert_eq!(landed["sections"], original["sections"]);
        assert_eq!(
            battle_unit_elevation(&scripts.world(), unit).unwrap(),
            Some(0)
        );
        let output = scripts.drain_outbox();
        let text: Vec<_> = output
            .iter()
            .filter(|(id, _)| *id == ObjectId(1))
            .map(|(_, doc)| doc.source())
            .collect();
        let touchdown = text
            .iter()
            .position(|text| text.contains("Your unit touches down!"))
            .unwrap();
        let callback = text
            .iter()
            .position(|text| text.contains("DROP_CALLBACK"))
            .unwrap();
        assert!(touchdown < callback);
        scripts.world().validate(&config).unwrap();
        let saved = scripts.world().clone();
        persistence::save(&config.database(), &saved).await.unwrap();
        let restored = persistence::load(&config.database()).await.unwrap();
        let restarted = Scripts::new(&config, Rc::new(RefCell::new(restored))).unwrap();
        advance_battle_jumps_action(&restarted, &config, BattleMovementRules::STANDARD).unwrap();
        let count: i64 = restarted
            .eval_callback(&format!(
                "return mux.world.object({}):state('drop'):get('count')",
                unit.0
            ))
            .unwrap();
        assert_eq!(count, 1);
        assert!(restarted.drain_outbox().is_empty());
    }
}

#[tokio::test]
async fn later_landing_callback_failure_restores_earlier_arrivals_in_the_tick() {
    let sources = firing::templates();
    let (dir, config, mut world, first, second, _) =
        firing::fixture_with_target(&sources[0], None, &sources[2]).await;
    for unit in [first, second] {
        prepare(&mut world, unit, 2, true, 2);
        world.objects.get_mut(&unit).unwrap().lua_parent = "drop_batch.lua".into();
    }
    std::fs::write(
        dir.path().join("lua/object_logic/drop_batch.lua"),
        format!(
            r#"
return {{events={{on_ood_land=function(ctx)
    local state=mux.world.object(ctx.object):state('drop')
    state:set('count',(state:get('count') or 0)+1)
    mux.world.pemit(1,'LANDED '..ctx.object)
    if ctx.object=={} then error('second arrival abort') end
end}}}}
"#,
            second.0
        ),
    )
    .unwrap();
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
    let before = serde_json::to_value(&*scripts.world()).unwrap();
    let error =
        advance_battle_jumps_action(&scripts, &config, BattleMovementRules::STANDARD).unwrap_err();
    assert!(format!("{error:#}").contains("second arrival abort"));
    assert_eq!(serde_json::to_value(&*scripts.world()).unwrap(), before);
    assert!(scripts.drain_outbox().is_empty());
}

#[tokio::test]
async fn landing_callback_can_detach_a_unit_without_resuming_stale_geometry() {
    for source in firing::templates().into_iter().take(6) {
        let (dir, config, mut world, unit, _, _) =
            firing::fixture_with_target(&source, None, &source).await;
        prepare(&mut world, unit, 2, false, 2);
        let before = saved_unit(&world, unit);
        world.objects.get_mut(&unit).unwrap().lua_parent = "drop_remove.lua".into();
        std::fs::write(
            dir.path().join("lua/object_logic/drop_remove.lua"),
            r#"
return {events={on_ood_land=function(ctx)
    btech.unit.setmapindex(1,ctx.object,-1)
end}}
"#,
        )
        .unwrap();
        let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
        advance_battle_jumps_action(&scripts, &config, BattleMovementRules::STANDARD).unwrap();
        let after = saved_unit(&scripts.world(), unit);
        assert_eq!(after["orbital_drop"], before["orbital_drop"]);
        assert_eq!(after["dice"], before["dice"]);
        assert_eq!(after["sections"], before["sections"]);
        assert_eq!(after["detached"], true);
        scripts.world().validate(&config).unwrap();
        advance_battle_units(&mut scripts.world_mut(), 0);
        assert!(saved_unit(&scripts.world(), unit)["orbital_drop"].is_null());
        scripts.world().validate(&config).unwrap();
    }
}
