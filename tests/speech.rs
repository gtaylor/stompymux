//! C speech command and notification graph compatibility, on isolated worlds.
use crate::support;
use std::{cell::RefCell, path::Path, rc::Rc};
use stompymux_rs::{
    Account, Config, Flag, Kind, ObjectId, Scripts, World,
    commands::{self, Action},
    notification::{Policy, Request},
    persistence,
    text::{Document, RenderOptions},
};
use support::copy;
fn create(w: &mut World, c: &Config, name: &str, kind: Kind, loc: Option<ObjectId>) -> ObjectId {
    let p = w.create(c, name.into(), kind);
    let o = w.objects.get_mut(&p).unwrap();
    o.location = loc;
    if matches!(kind, Kind::Thing | Kind::Player) {
        o.home = Some(ObjectId(c.home()));
    }
    if kind == Kind::Player {
        w.accounts.insert(p, Account::default());
    }
    p
}
struct Ids {
    room: ObjectId,
    far: ObjectId,
    alice: ObjectId,
    bob: ObjectId,
    remote: ObjectId,
    bag: ObjectId,
    exit: ObjectId,
}
async fn fixture(settings: &str) -> (tempfile::TempDir, Config, Scripts, Ids) {
    let d = tempfile::tempdir().unwrap();
    copy(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/game"),
        d.path(),
    );
    let path = d.path().join("stompymux.toml");
    let text = std::fs::read_to_string(&path).unwrap();
    std::fs::write(&path, text.replace("[lua]", &format!("[lua]\n{settings}"))).unwrap();
    let c = Config::load(d.path()).unwrap();
    let mut w = persistence::load(&c.database()).await.unwrap();
    let room = create(&mut w, &c, "Near", Kind::Room, None);
    let far = create(&mut w, &c, "Far", Kind::Room, None);
    let alice = create(&mut w, &c, "Alice", Kind::Player, Some(room));
    let bob = create(&mut w, &c, "Bob", Kind::Player, Some(room));
    let remote = create(&mut w, &c, "Remote", Kind::Player, Some(far));
    for p in [alice, bob, remote] {
        w.objects.get_mut(&p).unwrap().flags.insert(Flag::Connected);
    }
    w.objects.get_mut(&ObjectId(1)).unwrap().location = Some(room);
    w.objects
        .get_mut(&ObjectId(1))
        .unwrap()
        .flags
        .insert(Flag::Connected);
    w.objects.get_mut(&ObjectId(2)).unwrap().location = Some(far);
    w.objects
        .get_mut(&ObjectId(2))
        .unwrap()
        .flags
        .insert(Flag::Connected);
    let bag = create(&mut w, &c, "Box", Kind::Thing, Some(room));
    let exit = create(&mut w, &c, "Farward", Kind::Exit, Some(room));
    w.objects.get_mut(&exit).unwrap().destination = Some(far);
    let s = Scripts::new(&c, Rc::new(RefCell::new(w))).unwrap();
    (
        d,
        c,
        s,
        Ids {
            room,
            far,
            alice,
            bob,
            remote,
            bag,
            exit,
        },
    )
}
fn take(s: &Scripts) -> Vec<(ObjectId, String)> {
    s.drain_outbox()
        .into_iter()
        .map(|(id, d)| {
            (
                id,
                d.spans(s.palette(), &RenderOptions::default())
                    .into_iter()
                    .map(|s| s.text)
                    .collect(),
            )
        })
        .collect()
}
fn run(
    s: &Scripts,
    c: &Config,
    p: ObjectId,
    line: &str,
) -> anyhow::Result<Vec<(ObjectId, String)>> {
    let before = s.world().clone();
    let pending = s.outbox().clone();
    match commands::run(s, c, p, 1, line) {
        Ok(Action::Report(commands::Report::Reply(t))) => anyhow::bail!(t),
        Ok(Action::Report(commands::Report::Inspection(t))) => Ok(vec![(p, t)]),
        Ok(_) => Ok(take(s)),
        Err(e) => {
            *s.world_mut() = before;
            s.replace_outbox(pending);
            Err(e)
        }
    }
}
#[tokio::test(flavor = "current_thread")]
async fn speech_prefixes_switches_authority_and_formatting() {
    let (_d, c, s, i) = fixture("").await;
    for row in include_str!("fixtures/speech_formats.tsv")
        .lines()
        .filter(|l| !l.starts_with('#') && !l.is_empty())
    {
        let (line, text) = row.split_once('\t').unwrap();
        let out = run(&s, &c, i.alice, line).unwrap();
        assert!(out.contains(&(i.bob, text.into())), "{line}: {out:?}");
    }
    assert!(
        run(&s, &c, i.alice, "@emit denied")
            .unwrap()
            .iter()
            .any(|(_, s)| s.contains("Permission denied"))
    );
    for bad in [
        "pose/default/nospace nope",
        "@emit/unknown nope",
        "@wall/emit/pose nope",
        "@pemit/silent/object me=x",
    ] {
        assert!(run(&s, &c, ObjectId(1), bad).is_err(), "{bad}");
    }
    s.world_mut()
        .objects
        .get_mut(&i.alice)
        .unwrap()
        .flags
        .insert(Flag::Gagged);
    assert!(
        run(&s, &c, i.alice, ":waves")
            .unwrap()
            .iter()
            .all(|(id, _)| *id == i.alice)
    );
    let out = run(&s, &c, ObjectId(1), &format!("@fsay #{}=forced", i.alice.0)).unwrap();
    assert!(out.contains(&(i.bob, "Alice says \"forced\"".into())));
    // This fork ORs the nospace bit into 5; @fpose/nospace remains spaced.
    let out = run(
        &s,
        &c,
        ObjectId(1),
        &format!("@fpose/n #{}=waves", i.alice.0),
    )
    .unwrap();
    assert!(out.contains(&(i.bob, "Alice waves".into())));
}
#[tokio::test(flavor = "current_thread")]
async fn targets_lists_exclusions_and_broadcasts() {
    let (_d, c, s, i) = fixture("").await;
    let out = run(
        &s,
        &c,
        ObjectId(1),
        &format!("@pemit/list #{} #{}=message", i.bob.0, i.bob.0),
    )
    .unwrap();
    assert_eq!(out.iter().filter(|(id, _)| *id == i.bob).count(), 2);
    let out = run(&s, &c, ObjectId(1), &format!("@oemit #{}=except", i.bob.0)).unwrap();
    assert!(!out.iter().any(|(id, _)| *id == i.bob));
    assert!(out.iter().any(|(id, _)| *id == i.alice));
    assert!(run(&s, &c, ObjectId(2), "@pemit #1=remote god").is_err());
    let out = run(&s, &c, ObjectId(1), "@wall/wizard/pose greets").unwrap();
    assert!(
        out.iter()
            .all(|(id, _)| [ObjectId(1), ObjectId(2)].contains(id))
    );
    assert!(out.iter().all(|(_, t)| t.starts_with("Broadcast: ")));
    let out = run(&s, &c, ObjectId(1), "@wall/admin/wizard notice").unwrap();
    assert!(out.iter().all(|(_, t)| t.starts_with("Admin: ")));
    let out = run(&s, &c, ObjectId(1), "@wall/emit/no_prefix all").unwrap();
    assert_eq!(out.len(), 5);
    assert!(out.iter().all(|(_, t)| t == "all"));
    let out = run(&s, &c, ObjectId(1), "@pemit/list #9999 me=mixed").unwrap();
    assert!(out.iter().any(|(_, t)| t == "mixed"));
    create(
        &mut s.world_mut(),
        &c,
        "Duplicate",
        Kind::Thing,
        Some(i.room),
    );
    create(
        &mut s.world_mut(),
        &c,
        "Duplicate",
        Kind::Thing,
        Some(i.room),
    );
    assert!(run(&s, &c, ObjectId(1), "@pemit Duplicate=ambiguous").is_err());
}
#[tokio::test(flavor = "current_thread")]
async fn container_audibility_and_rich_lua_delivery() {
    let (_d, c, s, i) = fixture("").await;
    s.world_mut().objects.get_mut(&i.alice).unwrap().location = Some(i.bag);
    let out = run(&s, &c, i.alice, "say inside").unwrap();
    assert!(!out.iter().any(|(id, _)| *id == i.bob));
    s.world_mut()
        .objects
        .get_mut(&i.bag)
        .unwrap()
        .flags
        .insert(Flag::Audible);
    let out = run(&s, &c, i.alice, "say inside").unwrap();
    assert!(out.contains(&(i.bob, "From Box, Alice says \"inside\"".into())));
    s.world_mut()
        .objects
        .get_mut(&i.exit)
        .unwrap()
        .flags
        .insert(Flag::Audible);
    let out = run(&s, &c, i.alice, ":waves").unwrap();
    assert!(
        out.iter()
            .any(|(id, t)| *id == i.remote && t.contains("From a distance, From Box, Alice waves"))
    );
    s.eval_callback::<()>(&format!(
        "mux.world.pemit({},mux.text.markdown('**bold** `[red]literal[/]`'))",
        i.room.0
    ))
    .unwrap();
    let out = take(&s);
    assert!(out.iter().any(|(id, t)| *id == i.remote
        && t.contains("From a distance, ")
        && t.contains("[red]literal[/]")));
    // Direct messages do not implement the absent listener-only downward branch.
    s.eval_callback::<()>(&format!("mux.world.pemit({},'private')", i.bag.0))
        .unwrap();
    assert!(take(&s).is_empty());
    let out = run(
        &s,
        &c,
        ObjectId(1),
        &format!("@pemit/contents #{}=inside", i.bag.0),
    )
    .unwrap();
    assert!(out.iter().any(|(id, _)| *id == i.alice));
    let out = run(&s, &c, ObjectId(1), "@emit/here/room once").unwrap();
    assert_eq!(
        out.iter()
            .filter(|(id, t)| *id == i.bob && t == "once")
            .count(),
        1
    );
    let out = run(
        &s,
        &c,
        ObjectId(1),
        &format!("@femit/room #{}=enclosing", i.alice.0),
    )
    .unwrap();
    assert!(out.contains(&(i.bob, "enclosing".into())));
    let out = run(
        &s,
        &c,
        ObjectId(1),
        &format!("@oemit #{}=exit-destination", i.exit.0),
    )
    .unwrap();
    assert!(out.contains(&(i.remote, "exit-destination".into())));
    s.world_mut().objects.get_mut(&i.room).unwrap().dropto = Some(i.far);
    let out = run(
        &s,
        &c,
        ObjectId(1),
        &format!("@oemit #{}=room-dropto", i.room.0),
    )
    .unwrap();
    assert!(out.contains(&(i.remote, "room-dropto".into())));
    assert!(run(&s, &c, ObjectId(1), "@npemit me").unwrap().is_empty());
    assert_ne!(i.room, i.far);
}
#[tokio::test(flavor = "current_thread")]
async fn atomic_limits_recursion_and_lock_rollback() {
    let (_d, c, s, i) = fixture("output_entry_limit=2").await;
    assert!(run(&s, &c, i.alice, "say excessive").is_err());
    assert!(s.outbox().is_empty());
    s.eval_callback::<()>(&format!(
        "local ok=pcall(mux.world.pemit,{},string.rep('x',70000));assert(not ok)",
        i.alice.0
    ))
    .unwrap();
    assert!(s.outbox().is_empty());
    let (_d, c, s, i) = fixture("").await;
    s.world_mut().objects.get_mut(&i.bag).unwrap().location = Some(i.bag);
    s.world_mut()
        .objects
        .get_mut(&i.bag)
        .unwrap()
        .flags
        .insert(Flag::Audible);
    s.send_notification(
        &c,
        Request {
            target: i.bag,
            sender: i.alice,
            document: Document::Literal("loop".into()),
            policy: Policy::ROOM,
            exclusions: None,
        },
    )
    .unwrap();
    assert!(take(&s).is_empty());
}
