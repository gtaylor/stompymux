//! Object-owned stock persists and rolls back without coupling rooms or chassis to installed parts.
use crate::support;
use std::{cell::RefCell, rc::Rc};
use stompymux_rs::*;

/// Native and Lua stock edits share authority, validation, ordering and callback rollback.
#[tokio::test]
async fn stock_controls_share_transactions_and_reject_invalid_values() {
    let (_dir, config, world) = support::isolated_world().await;
    let native = Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
    let lua = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
    for (part, quantity) in [(50, 8), (1, 5), (51, 4), (50, 0)] {
        let command = format!("@btech inventory-set #0 {part} {quantity}");
        let callback = format!("btech.inventory.set(1,0,{part},{quantity})");
        let before = lua.world().btech.clone();
        assert!(
            lua.eval_callback::<()>(&format!("{callback}; error('abort')"))
                .is_err()
        );
        assert_eq!(lua.world().btech, before);
        assert!(lua.drain_outbox().is_empty());
        let output = support::run_text(&native, &config, ObjectId(1), 1, &command);
        assert!(
            output.contains(&format!("quantity {quantity}.")),
            "{output}"
        );
        lua.eval_callback::<()>(&callback).unwrap();
        assert_eq!(lua.world().btech, native.world().btech);
    }
    let before = lua.world().btech.clone();
    for arguments in [
        "4,0,1,5",
        "1,999999,1,5",
        "1,0,-1,5",
        "1,0,1,-1",
        "1,0,1,2147483648",
    ] {
        assert!(
            lua.eval_callback::<()>(&format!("btech.inventory.set({arguments})"))
                .is_err()
        );
        assert_eq!(lua.world().btech, before);
    }
    let detached: mlua::Table = lua.eval_callback("return btech.inventory.read(0)").unwrap();
    let first: mlua::Table = detached.get(1).unwrap();
    first.set("quantity", 999).unwrap();
    assert_eq!(lua.world().btech, before);
    let output = support::run_text(&native, &config, ObjectId(1), 1, "@btech inventory #0");
    assert!(output.contains("Part 1: 5"), "{output}");
    assert!(!output.contains("Part 50:"), "{output}");
    lua.world().validate(&config).unwrap();
}

/// The existing game table is authoritative; updates preserve unrelated row columns and replay.
#[tokio::test]
async fn stock_loads_existing_rows_and_selectively_persists_corrections() {
    use sqlx::Connection;
    let (_dir, config, mut world) = support::isolated_world().await;
    let mut holders = vec![ObjectId(0)];
    for source in [
        include_str!("fixtures/btech/mechs/JR7-D.toml"),
        include_str!("../game/mechs/Demolisher.toml"),
        include_str!("../game/mechs/Kestrel.toml"),
    ] {
        let id = world.create(&config, "Stock holder".into(), Kind::Thing);
        UnitTemplate::parse("test", source)
            .unwrap()
            .create(&mut world, id)
            .unwrap();
        holders.push(id);
    }
    persistence::save(&config.database(), &world).await.unwrap();
    let mut db = sqlx::SqliteConnection::connect_with(
        &sqlx::sqlite::SqliteConnectOptions::new().filename(config.database()),
    )
    .await
    .unwrap();
    sqlx::query(
        "ALTER TABLE btech_economy_parts ADD COLUMN annotation TEXT NOT NULL DEFAULT 'keep'",
    )
    .execute(&mut db)
    .await
    .unwrap();
    sqlx::query("INSERT INTO btech_economy_parts(object_dbref,part_id,quantity) VALUES(0,50,7)")
        .execute(&mut db)
        .await
        .unwrap();
    world = persistence::load(&config.database()).await.unwrap();
    assert_eq!(
        battle_inventory(&world, ObjectId(0)).unwrap()[0].quantity,
        7
    );
    for &holder in &holders {
        set_battle_inventory_quantity(&mut world, ObjectId(1), holder, 50, 9).unwrap();
        set_battle_inventory_quantity(&mut world, ObjectId(1), holder, 51, i32::MAX).unwrap();
    }
    world.validate(&config).unwrap();
    persistence::save(&config.database(), &world).await.unwrap();
    let loaded = persistence::load(&config.database()).await.unwrap();
    assert_eq!(loaded.btech, world.btech);
    let annotation: String = sqlx::query_scalar(
        "SELECT annotation FROM btech_economy_parts WHERE object_dbref=0 AND part_id=50",
    )
    .fetch_one(&mut db)
    .await
    .unwrap();
    assert_eq!(annotation, "keep");
    for &holder in &holders {
        set_battle_inventory_quantity(&mut world, ObjectId(1), holder, 50, 0).unwrap();
        set_battle_inventory_quantity(&mut world, ObjectId(1), holder, 51, 0).unwrap();
        assert!(battle_inventory(&world, holder).unwrap().is_empty());
    }
    persistence::save(&config.database(), &world).await.unwrap();
    assert_eq!(
        persistence::load(&config.database()).await.unwrap().btech,
        world.btech
    );
    sqlx::query("INSERT INTO btech_economy_parts(object_dbref,part_id,quantity) VALUES(0,-1,7)")
        .execute(&mut db)
        .await
        .unwrap();
    assert!(persistence::load(&config.database()).await.is_err());
}

/// Whole-world validation rejects duplicate, unordered and orphaned stock before publication.
#[tokio::test]
async fn malformed_stock_snapshots_and_purged_owners_are_checked() {
    let (_dir, config, mut world) = support::isolated_world().await;
    let holder = world.create(&config, "Stock crate".into(), Kind::Thing);
    set_battle_inventory_quantity(&mut world, ObjectId(1), holder, 50, 9).unwrap();
    let original = serde_json::to_value(&world.btech).unwrap();
    for rows in [
        serde_json::json!([]),
        serde_json::json!([{"part_id":50,"quantity":0}]),
        serde_json::json!([{"part_id":50,"quantity":9},{"part_id":50,"quantity":4}]),
        serde_json::json!([{"part_id":51,"quantity":9},{"part_id":50,"quantity":4}]),
    ] {
        let mut changed = original.clone();
        changed["inventories"][holder.0.to_string()] = rows;
        let mut invalid = world.clone();
        invalid.btech = serde_json::from_value(changed).unwrap();
        assert!(invalid.validate(&config).is_err());
    }
    let mut orphan = world.clone();
    orphan.objects.remove(&holder);
    assert!(orphan.validate(&config).is_err());
    world
        .objects
        .get_mut(&holder)
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
    assert_eq!(loaded.objects[&holder].kind, Kind::Garbage);
    assert!(
        serde_json::to_value(&loaded.btech).unwrap()["inventories"]
            .as_object()
            .unwrap()
            .is_empty()
    );
    assert!(battle_inventory(&loaded, holder).is_err());
    loaded.validate(&config).unwrap();
}

/// A database rejection rolls back every preceding stock update in the same save.
#[tokio::test]
async fn stock_database_failure_rolls_back_the_entire_change() {
    use sqlx::Connection;
    let (_dir, config, mut world) = support::isolated_world().await;
    for part in [50, 51] {
        set_battle_inventory_quantity(&mut world, ObjectId(1), ObjectId(0), part, 7).unwrap();
    }
    persistence::save(&config.database(), &world).await.unwrap();
    let baseline = persistence::load(&config.database()).await.unwrap();
    let mut db = sqlx::SqliteConnection::connect_with(
        &sqlx::sqlite::SqliteConnectOptions::new().filename(config.database()),
    )
    .await
    .unwrap();
    sqlx::raw_sql("CREATE TRIGGER reject_inventory BEFORE UPDATE ON btech_economy_parts WHEN NEW.quantity=99 BEGIN SELECT RAISE(ABORT,'stock rejected'); END;").execute(&mut db).await.unwrap();
    set_battle_inventory_quantity(&mut world, ObjectId(1), ObjectId(0), 50, 8).unwrap();
    set_battle_inventory_quantity(&mut world, ObjectId(1), ObjectId(0), 51, 99).unwrap();
    assert!(persistence::save(&config.database(), &world).await.is_err());
    assert_eq!(
        persistence::load(&config.database()).await.unwrap().btech,
        baseline.btech
    );
}
