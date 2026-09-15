//! Scenario-transferred jumps retain their route and resolve the destination map's boundaries on update.
use stompymux_rs::*;
#[path = "support/btech_firing.rs"]
mod firing;
mod support;

/// Both jump-capable anatomies survive changes in map size and wrapping, including saved continuation.
#[tokio::test]
async fn transferred_jumps_rebind_boundaries_and_replay_to_landing() {
    let templates = firing::templates();
    let sources = [
        templates[0].as_str(),
        include_str!("../game/mechs/StalkingSpider-1"),
    ];
    for source in sources {
        for source_wrap in [false, true] {
            for destination_wrap in [false, true] {
                for height in [2, 12] {
                    let (_dir, config, mut world, unit, _, _) =
                        firing::fixture_with_target(source, None, source).await;
                    let source_map = world.objects[&unit].location.unwrap();
                    set_battle_map_wrapping(&mut world, source_map, source_wrap).unwrap();
                    launch_battle_jump(&mut world, unit, ObjectId(1), 0, 3.0).unwrap();
                    advance_battle_jumps(&mut world, BattleMovementRules::STANDARD).unwrap();
                    let flight = world.btech.constructed_units()[&unit].flight().unwrap();
                    let destination = world.create(&config, "Destination".into(), Kind::Room);
                    create_battle_map(
                        &mut world,
                        destination,
                        "destination",
                        BattleMapAsset::parse(&format!("1 {height}\n{}", ".0\n".repeat(height)))
                            .unwrap(),
                    )
                    .unwrap();
                    set_battle_map_wrapping(&mut world, destination, destination_wrap).unwrap();
                    let report =
                        reassign_battle_map(&mut world, unit, destination, Some("XY")).unwrap();
                    assert_eq!(report.reset_origin, height == 2);
                    let moved = world.btech.constructed_units()[&unit].flight().unwrap();
                    assert_eq!(moved.path(), flight.path());
                    assert_eq!(moved.travelled(), flight.travelled());
                    assert_eq!(moved.sample().elevation, flight.sample().elevation);
                    assert_eq!(
                        moved.sample().point,
                        world.btech.constructed_units()[&unit]
                            .motion()
                            .unwrap()
                            .point
                    );
                    world.validate(&config).unwrap();
                    persistence::save(&config.database(), &world).await.unwrap();
                    let mut restored = persistence::load(&config.database()).await.unwrap();
                    assert_eq!(restored.btech, world.btech);
                    let mut hit_edge = false;
                    for _ in 0..100 {
                        let notices =
                            advance_battle_jumps(&mut world, BattleMovementRules::STANDARD)
                                .unwrap();
                        assert_eq!(
                            notices,
                            advance_battle_jumps(&mut restored, BattleMovementRules::STANDARD)
                                .unwrap()
                        );
                        assert_eq!(world.btech, restored.btech);
                        world.validate(&config).unwrap();
                        hit_edge |= notices
                            .iter()
                            .any(|notice| notice.text == "You cannot move off this map!");
                        if world.btech.constructed_units()[&unit].flight().is_none() {
                            break;
                        }
                    }
                    assert!(world.btech.constructed_units()[&unit].flight().is_none());
                    assert_eq!(hit_edge, height == 2 && !destination_wrap);
                    let position = world.btech.constructed_units()[&unit].position().unwrap();
                    assert_eq!(position.map, destination);
                    assert!(usize::from(position.y) < height);
                    if hit_edge {
                        assert_eq!((position.x, position.y), (0, 1));
                    }
                }
            }
        }
    }
}

/// Native/Lua reassignment share the rebound cursor and restore it on callback failure.
#[tokio::test]
async fn rebound_jump_assignment_uses_host_rollback() {
    use std::{cell::RefCell, rc::Rc};
    let source = &firing::templates()[0];
    let (_dir, config, mut world, unit, _, _) =
        firing::fixture_with_target(source, None, source).await;
    let old = world.objects[&unit].location.unwrap();
    set_battle_map_wrapping(&mut world, old, true).unwrap();
    launch_battle_jump(&mut world, unit, ObjectId(1), 0, 3.0).unwrap();
    advance_battle_jumps(&mut world, BattleMovementRules::STANDARD).unwrap();
    let map = world.create(&config, "Small map".into(), Kind::Room);
    create_battle_map(
        &mut world,
        map,
        "small",
        BattleMapAsset::parse("1 2\n.0\n.0\n").unwrap(),
    )
    .unwrap();
    let native = Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
    let lua = Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
    assert!(
        lua.eval_callback::<()>(&format!(
            "btech.unit.setmapindex(1, {}, {}, 'XY'); error('reject')",
            unit.0, map.0
        ))
        .is_err()
    );
    assert_eq!(lua.world().btech, world.btech);
    assert!(lua.drain_outbox().is_empty());
    let text = support::run_text(
        &native,
        &config,
        ObjectId(1),
        1,
        &format!("setmapindx {} XY", map.0),
    );
    assert!(text.contains("Pos changed to 0,0"), "{text}");
    lua.eval_callback::<mlua::Table>(&format!(
        "return btech.unit.setmapindex(1, {}, {}, 'XY')",
        unit.0, map.0
    ))
    .unwrap();
    assert_eq!(native.world().btech, lua.world().btech);
    let _ = advance_battle_jumps_action(&native, &config, BattleMovementRules::STANDARD).unwrap();
    assert!(
        native.world().btech.constructed_units()[&unit]
            .flight()
            .is_none()
    );
    assert!(
        native
            .drain_outbox()
            .iter()
            .any(|(_, message)| message.source().contains("You cannot move off this map!"))
    );
    native.world().validate(&config).unwrap();
}
