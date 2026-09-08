//! Maintenance repair, preservation, dependency and restart regressions on isolated databases.
use sqlx::{Connection, Row, SqliteConnection};
use std::{collections::BTreeMap, path::Path};
use stompymux_rs::{
    config::Config,
    dbck::{self, Links},
    flags::Flag,
    persistence,
    world::*,
};
/// Isolate every mutation from the supplied game database.
fn copy(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for e in std::fs::read_dir(from).unwrap() {
        let e = e.unwrap();
        if e.path().is_dir() {
            copy(&e.path(), &to.join(e.file_name()));
        } else {
            std::fs::copy(e.path(), to.join(e.file_name())).unwrap();
        }
    }
}
/// Load the populated fixture and an operation-scoped SQL connection.
async fn fixture() -> (tempfile::TempDir, Config, World, SqliteConnection) {
    let d = tempfile::tempdir().unwrap();
    copy(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/game"),
        d.path(),
    );
    let c = Config::load(d.path()).unwrap();
    let w = persistence::load(&c.database()).await.unwrap();
    let sql = SqliteConnection::connect_with(
        &sqlx::sqlite::SqliteConnectOptions::new()
            .filename(c.database())
            .foreign_keys(false),
    )
    .await
    .unwrap();
    (d, c, w, sql)
}
/// Add a placed object with an ordinary safe home.
fn add(w: &mut World, c: &Config, kind: Kind, location: i64) -> ObjectId {
    let id = w.create(c, format!("Repair {}", w.next_id), kind);
    let o = w.objects.get_mut(&id).unwrap();
    if kind != Kind::Room {
        o.location = Some(ObjectId(location));
    }
    if matches!(kind, Kind::Thing | Kind::Player) {
        o.home = Some(ObjectId(c.home()));
    }
    if kind == Kind::Exit {
        o.destination = Some(ObjectId(c.start()));
    }
    if kind == Kind::Player {
        w.accounts.insert(id, Account::default());
    }
    id
}
/// Raw list slots used to verify legacy readers see the repaired memberships.
async fn raw(c: &mut SqliteConnection) -> Links {
    sqlx::query("SELECT dbref,contents,exits,next FROM objects")
        .fetch_all(c)
        .await
        .unwrap()
        .into_iter()
        .map(|r| {
            (
                ObjectId(r.get("dbref")),
                [r.get("contents"), r.get("exits"), r.get("next")],
            )
        })
        .collect()
}
#[tokio::test]
async fn purges_all_types_evacuates_contents_and_preserves_unknown_data() {
    let (_d, c, mut w, mut sql) = fixture().await;
    let room = add(&mut w, &c, Kind::Room, 0);
    let thing = add(&mut w, &c, Kind::Thing, room.0);
    let player = add(&mut w, &c, Kind::Player, thing.0);
    let exit = add(&mut w, &c, Kind::Exit, room.0);
    let survivor = add(&mut w, &c, Kind::Thing, thing.0);
    w.objects.get_mut(&survivor).unwrap().home = Some(room);
    w.objects.get_mut(&player).unwrap().pending_destroyer = Some(ObjectId(1));
    persistence::save(&c.database(), &w).await.unwrap();
    assert_eq!(
        persistence::load(&c.database()).await.unwrap().objects[&player].pending_destroyer,
        None
    );
    sqlx::raw_sql("ALTER TABLE objects ADD COLUMN unknown_flag INTEGER DEFAULT 7; CREATE TABLE future_data (value BLOB); INSERT INTO future_data VALUES(x'001234');").execute(&mut sql).await.unwrap();
    sqlx::query("INSERT INTO btech_economy_parts VALUES(?,1,1,5)")
        .bind(thing.0)
        .execute(&mut sql)
        .await
        .unwrap();
    sqlx::query("INSERT INTO commac_entries VALUES(?,0,0,-1,-1,-1,-1)")
        .bind(player.0)
        .execute(&mut sql)
        .await
        .unwrap();
    sqlx::query("INSERT INTO macro_sets VALUES(0,?,0,'owned')")
        .bind(player.0)
        .execute(&mut sql)
        .await
        .unwrap();
    sqlx::query("INSERT INTO macro_entries VALUES(0,0,'x','y')")
        .execute(&mut sql)
        .await
        .unwrap();
    sqlx::query("INSERT INTO comsys_channel_users VALUES('Public',99,?,1)")
        .bind(player.0)
        .execute(&mut sql)
        .await
        .unwrap();
    for id in [room, thing, player, exit, ObjectId(1), ObjectId(c.start())] {
        w.objects.get_mut(&id).unwrap().flags.insert(Flag::Going);
    }
    let report = persistence::repair(&c.database(), 5000, |raw| dbck::plan(&w, raw, &c))
        .await
        .unwrap();
    assert_eq!(report.plan.purges.len(), 4);
    assert!(report.plan.detachments.contains(&player));
    let loaded = persistence::load(&c.database()).await.unwrap();
    loaded.validate(&c).unwrap();
    assert_eq!(loaded.next_id, w.next_id);
    for id in [room, thing, player, exit] {
        let o = &loaded.objects[&id];
        assert_eq!(o.kind, Kind::Garbage);
        assert_eq!(o.flags, [Flag::Going].into_iter().collect());
        assert!(o.state.is_empty());
    }
    assert!(!loaded.accounts.contains_key(&player));
    assert_eq!(
        loaded.objects[&survivor].location,
        Some(ObjectId(c.mux.default_home))
    );
    assert!(!loaded.objects[&ObjectId(1)].flags.contains(Flag::Going));
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT unknown_flag FROM objects WHERE dbref=?")
            .bind(thing.0)
            .fetch_one(&mut sql)
            .await
            .unwrap(),
        7
    );
    for table in [
        "btech_economy_parts",
        "commac_entries",
        "macro_sets",
        "macro_entries",
    ] {
        let n: i64 =
            sqlx::query_scalar(sqlx::AssertSqlSafe(format!("SELECT count(*) FROM {table}")))
                .fetch_one(&mut sql)
                .await
                .unwrap();
        assert_eq!(n, 0, "{table}");
    }
    assert_eq!(
        sqlx::query_scalar::<_, Vec<u8>>("SELECT value FROM future_data")
            .fetch_one(&mut sql)
            .await
            .unwrap(),
        [0, 0x12, 0x34]
    );
    let slots = raw(&mut sql).await;
    assert_eq!(slots, dbck::rebuild_links(&loaded, &slots));
    let bytes = std::fs::read(c.database()).unwrap();
    let second = persistence::repair(&c.database(), 5000, |raw| dbck::plan(&loaded, raw, &c))
        .await
        .unwrap();
    assert!(second.plan.purges.is_empty());
    assert!(second.plan.fields.is_empty());
    assert!(second.plan.list_changes.is_empty());
    assert_eq!(bytes, std::fs::read(c.database()).unwrap());
    sql.close().await.unwrap();
}
#[tokio::test]
async fn malformed_lists_cycles_references_and_droptos_repair_deterministically() {
    let (_d, c, mut w, mut sql) = fixture().await;
    let a = add(&mut w, &c, Kind::Thing, c.start());
    let b = add(&mut w, &c, Kind::Thing, c.start());
    let exit = add(&mut w, &c, Kind::Exit, c.start());
    persistence::save(&c.database(), &w).await.unwrap();
    sqlx::query("UPDATE objects SET next=dbref WHERE dbref=?")
        .bind(a.0)
        .execute(&mut sql)
        .await
        .unwrap();
    let mut slots = raw(&mut sql).await;
    w.objects.get_mut(&a).unwrap().location = Some(b);
    w.objects.get_mut(&b).unwrap().location = Some(a);
    slots.get_mut(&a).unwrap()[0] = b.0;
    slots.get_mut(&b).unwrap()[0] = a.0;
    w.objects.get_mut(&a).unwrap().home = Some(ObjectId(99999));
    w.objects.get_mut(&b).unwrap().zone = Some(ObjectId(99999));
    w.objects.get_mut(&b).unwrap().affiliation = Some(ObjectId(99999));
    w.objects.get_mut(&exit).unwrap().destination = Some(ObjectId(99999));
    w.objects.get_mut(&ObjectId(c.start())).unwrap().dropto = Some(ObjectId(c.home()));
    let (fixed, report) = dbck::plan(&w, &slots, &c).unwrap();
    fixed.validate(&c).unwrap();
    assert!(report.plan.purges.contains(&exit));
    assert_eq!(fixed.objects[&b].zone, None);
    assert_eq!(fixed.objects[&b].affiliation, None);
    assert_eq!(
        fixed.objects[&ObjectId(c.start())].dropto,
        Some(ObjectId(c.home()))
    );
    assert_eq!(
        report.plan.links,
        dbck::rebuild_links(&fixed, &report.plan.links)
    );
    persistence::repair(&c.database(), 5000, |_| Ok((fixed, report)))
        .await
        .unwrap();
    let loaded = persistence::load(&c.database()).await.unwrap();
    assert_eq!(loaded.objects[&ObjectId(c.start())].location, None);
    assert_eq!(
        loaded.objects[&ObjectId(c.start())].dropto,
        Some(ObjectId(c.home()))
    );
    sql.close().await.unwrap();
}
#[tokio::test]
async fn unknown_dependency_and_write_failure_leave_every_row_unchanged() {
    let (_d, c, mut w, mut sql) = fixture().await;
    let p = add(&mut w, &c, Kind::Player, c.start());
    persistence::save(&c.database(), &w).await.unwrap();
    w.objects.get_mut(&p).unwrap().flags.insert(Flag::Going);
    sqlx::query("CREATE TABLE future_owner (owner INTEGER REFERENCES objects(dbref))")
        .execute(&mut sql)
        .await
        .unwrap();
    sqlx::query("INSERT INTO future_owner VALUES(?)")
        .bind(p.0)
        .execute(&mut sql)
        .await
        .unwrap();
    let bytes = std::fs::read(c.database()).unwrap();
    let error = persistence::repair(&c.database(), 5000, |raw| dbck::plan(&w, raw, &c))
        .await
        .unwrap_err();
    assert!(error.to_string().contains("unknown dependency"));
    assert_eq!(bytes, std::fs::read(c.database()).unwrap());
    sqlx::query("DROP TABLE future_owner")
        .execute(&mut sql)
        .await
        .unwrap();
    sqlx::query("CREATE TRIGGER fail_purge BEFORE UPDATE ON objects BEGIN SELECT RAISE(FAIL,'write blocked'); END").execute(&mut sql).await.unwrap();
    let bytes = std::fs::read(c.database()).unwrap();
    assert!(
        persistence::repair(&c.database(), 5000, |raw| dbck::plan(&w, raw, &c))
            .await
            .is_err()
    );
    assert_eq!(bytes, std::fs::read(c.database()).unwrap());
    assert!(
        persistence::load(&c.database())
            .await
            .unwrap()
            .accounts
            .contains_key(&p)
    );
    sql.close().await.unwrap();
}
#[tokio::test]
async fn unrepairable_foundations_and_accounts_are_not_guessed() {
    let (_d, c, mut w, sql) = fixture().await;
    let slots = dbck::rebuild_links(&w, &BTreeMap::new());
    let name = w.objects[&ObjectId(2)].name.clone();
    w.objects.get_mut(&ObjectId(2)).unwrap().name = w.objects[&ObjectId(1)].name.clone();
    w.objects
        .get_mut(&ObjectId(2))
        .unwrap()
        .flags
        .insert(Flag::Going);
    assert!(
        dbck::plan(&w, &slots, &c)
            .unwrap_err()
            .to_string()
            .contains("duplicate player name")
    );
    w.objects.get_mut(&ObjectId(2)).unwrap().name = name;
    w.objects.remove(&ObjectId(c.start()));
    assert!(dbck::plan(&w, &slots, &c).is_err());
    sql.close().await.unwrap();
}

/// Normal startup must refuse broken bookkeeping without performing a repair or writing hooks.
#[tokio::test]
async fn startup_rejects_broken_lists_without_side_effects() {
    let (_d, c, _w, mut sql) = fixture().await;
    sqlx::query("UPDATE objects SET contents=999999 WHERE dbref=0")
        .execute(&mut sql)
        .await
        .unwrap();
    let bytes = std::fs::read(c.database()).unwrap();
    assert!(stompymux_rs::server::prepare(&c).await.is_err());
    assert_eq!(bytes, std::fs::read(c.database()).unwrap());
    sql.close().await.unwrap();
}

/// Small response budgets retain the completion marker whenever the marker fits.
#[test]
fn bounded_reports_keep_done() {
    let report = dbck::DbCheckReport::default();
    for limit in 0..128 {
        let bytes = report.response(limit);
        assert!(bytes.len() <= limit);
        assert!(std::str::from_utf8(&bytes).is_ok());
        if limit >= 7 {
            assert!(bytes.ends_with(b"Done.\r\n"));
        }
    }
}
