//! Shared artillery sighting checks for both Mech and vehicle station fixtures.
use crate::support;
use std::{cell::RefCell, rc::Rc};
use stompymux_rs::*;

/// Native and Lua sighting agree, roll back together and change only the physical shooter's dice.
pub fn check_artillery(
    config: &Config,
    world: &World,
    station: ObjectId,
    gunner: ObjectId,
    index: usize,
) -> i32 {
    let parent = world.btech.gunner_stations()[&station].parent;
    let make = || Scripts::new(config, Rc::new(RefCell::new(world.clone()))).unwrap();
    let lua = make();
    let native = make();
    let query = format!("btech.gunner.sight({},{},{index})", station.0, gunner.0);
    assert!(
        lua.eval_callback::<()>(&format!("{query}; error('abort')"))
            .is_err()
    );
    assert_eq!(lua.world().btech, world.btech);
    assert!(lua.drain_outbox().is_empty());
    let aim: i32 = lua
        .eval_callback(&format!("return {query}.target_number"))
        .unwrap();
    let text = support::run_text(&native, config, gunner, 1, &format!("sight {index}"));
    assert!(text.contains(&format!("BTH: {aim}")), "{text}");
    assert_eq!(native.world().btech, lua.world().btech);
    let before = serde_json::to_value(&world.btech).unwrap();
    let mut after = serde_json::to_value(&lua.world().btech).unwrap();
    let key = if world.btech.vehicles().contains_key(&parent) {
        "vehicles"
    } else {
        "constructed"
    };
    assert_ne!(
        before[key][parent.0.to_string()]["dice"],
        after[key][parent.0.to_string()]["dice"]
    );
    after[key][parent.0.to_string()]["dice"] = before[key][parent.0.to_string()]["dice"].clone();
    assert_eq!(before, after);
    aim
}
