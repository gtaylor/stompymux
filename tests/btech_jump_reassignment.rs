//! Scenario-transferred jumps retain their route and resolve the destination map's boundaries on update.
use crate::support;
use crate::support::btech_firing as firing;
use stompymux_rs::*;

/// Both jump-capable anatomies survive changes in map size, including saved continuation.
#[tokio::test]
async fn transferred_jumps_rebind_boundaries_and_replay_to_landing() {
    let templates = firing::templates();
    let sources = [
        templates[0].as_str(),
        include_str!("../game/mechs/StalkingSpider-1.toml"),
    ];
    for source in sources {
        for height in [2, 12] {
            let (_dir, config, mut world, unit, _, _) =
                firing::fixture_with_target(source, None, source).await;
            launch_battle_jump(&mut world, unit, ObjectId(1), 0, 3.0).unwrap();
            advance_battle_jumps(&mut world, MovementRules::STANDARD).unwrap();
            let flight = world.btech.constructed_units()[&unit].flight().unwrap();
            let destination = world.create(&config, "Destination".into(), Kind::Room);
            create_battle_map(
                &mut world,
                destination,
                "destination",
                MapAsset::from_cells(&format!("1 {height}\n{}", ".0\n".repeat(height))).unwrap(),
            )
            .unwrap();
            support::seed_object_dice(&mut world, destination, support::FIXTURE_DICE_SEED);
            let report = reassign_battle_map(&mut world, unit, destination, Some("XY")).unwrap();
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
                let notices = advance_battle_jumps(&mut world, MovementRules::STANDARD).unwrap();
                assert_eq!(
                    notices,
                    advance_battle_jumps(&mut restored, MovementRules::STANDARD).unwrap()
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
            assert_eq!(hit_edge, height == 2);
            let position = world.btech.constructed_units()[&unit].position().unwrap();
            assert_eq!(position.map, destination);
            assert!(usize::from(position.y) < height);
            if hit_edge {
                assert_eq!((position.x, position.y), (0, 1));
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
    launch_battle_jump(&mut world, unit, ObjectId(1), 0, 3.0).unwrap();
    advance_battle_jumps(&mut world, MovementRules::STANDARD).unwrap();
    let map = world.create(&config, "Small map".into(), Kind::Room);
    create_battle_map(
        &mut world,
        map,
        "small",
        MapAsset::from_cells("1 2\n.0\n.0\n").unwrap(),
    )
    .unwrap();
    support::seed_object_dice(&mut world, map, support::FIXTURE_DICE_SEED);
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
    let _ = advance_battle_jumps_action(&native, &config, MovementRules::STANDARD).unwrap();
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
