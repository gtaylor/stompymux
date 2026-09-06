//! Legacy matching, range, permissions and byte-bounded search regression tests.
use stompymux_rs::{
    find::{self, FindCursor},
    flags::Flag,
    world::{Kind, Object, ObjectId, World},
};

/// Small deterministic world with explicit flags and sparse identities.
fn world() -> World {
    let mut world = World::default();
    for (id, name, kind) in [
        (0, "Limbo", Kind::Room),
        (1, "GOD", Kind::Player),
        (2, "Wizard", Kind::Player),
        (3, "Other Wizard", Kind::Player),
        (4, "Staff Nexus", Kind::Room),
        (5, "Plain", Kind::Thing),
        (6, "out", Kind::Exit),
        (7, "Trash", Kind::Garbage),
        (8, "[bold]Red[/] \x1b[31mApple\x1b[0m", Kind::Thing),
        (9, "*literal", Kind::Thing),
    ] {
        let mut flags = stompymux_rs::flags::FlagSet::default();
        if (1..=3).contains(&id) {
            flags.insert(Flag::Wizard);
        }
        if id == 5 {
            flags.insert(Flag::Dark);
            flags.insert(Flag::Going);
        }
        world.objects.insert(
            ObjectId(id),
            Object {
                id: ObjectId(id),
                name: name.into(),
                kind,
                flags,
                powers: Default::default(),
                location: None,
                zone: None,
                home: None,
                affiliation: None,
                destination: None,
                dropto: None,
                description: None,
                internal_description: None,
                lua_parent: String::new(),
                state: Default::default(),
                generation: Default::default(),
            },
        );
    }
    world.next_id = 10;
    world
}
/// Decode a complete large page for assertions.
fn search(w: &World, actor: i64, query: &str) -> String {
    String::from_utf8(
        find::page(w, ObjectId(actor), &FindCursor::new(query, w), 20, 65536)
            .unwrap()
            .bytes,
    )
    .unwrap()
}
#[test]
fn legacy_matching_and_ranges() {
    let w = world();
    assert!(find::matches("Staff Nexus", "nEX"));
    assert!(!find::matches("Staff Nexus", "ex"));
    assert!(find::matches("Staff--Nexus", "nex"));
    assert!(!find::matches("Staff Nexus", "*"));
    assert!(search(&w, 1, "apple").starts_with("Red Apple(#8)\r\n"));
    assert_eq!(search(&w, 1, "*"), "*literal(#9)\r\n***End of List***\r\n");
    assert_eq!(
        search(&w, 1, ",#4,#4"),
        "Staff Nexus(#4:R)\r\n***End of List***\r\n"
    );
    for query in [",bad,bad", ",-5,999", ",,", ""] {
        assert_eq!(search(&w, 1, query), search(&w, 1, ""));
    }
    for query in [",9,4", ",,-1", ",100", "missing"] {
        assert_eq!(search(&w, 1, query), "***End of List***\r\n");
    }
    assert_eq!(FindCursor::new(",#2,4,5", &w).upper, 9);
}
#[test]
fn control_filtering_and_exact_format() {
    let w = world();
    let output = search(&w, 2, "");
    assert_eq!(
        output,
        "Limbo(#0:R)\r\nWizard(#2:PW)\r\nStaff Nexus(#4:R)\r\nPlain(#5:DG)\r\nRed Apple(#8)\r\n*literal(#9)\r\n***End of List***\r\n"
    );
    assert!(search(&w, 1, "").contains("Other Wizard(#3:PW)"));
    assert_eq!(search(&w, 5, ""), "***End of List***\r\n");
}
#[test]
fn pages_recheck_world_and_capture_upper_bound() {
    let mut w = world();
    let first = find::page(&w, ObjectId(2), &FindCursor::new("", &w), 2, 65536).unwrap();
    assert!(
        String::from_utf8(first.bytes)
            .unwrap()
            .ends_with("***Use @find/next for more***\r\n")
    );
    let cursor = first.cursor.unwrap();
    assert_eq!(cursor.next, 4);
    w.objects
        .get_mut(&ObjectId(4))
        .unwrap()
        .flags
        .insert(Flag::Wizard);
    w.objects.get_mut(&ObjectId(5)).unwrap().name = "Changed".into();
    let mut extra = w.objects[&ObjectId(8)].clone();
    extra.id = ObjectId(10);
    w.objects.insert(extra.id, extra);
    let second = find::page(&w, ObjectId(2), &cursor, 20, 65536).unwrap();
    assert_eq!(
        String::from_utf8(second.bytes).unwrap(),
        "Changed(#5:DG)\r\nRed Apple(#8)\r\n*literal(#9)\r\n***End of List***\r\n"
    );
    assert!(second.cursor.is_none());
    w.objects
        .get_mut(&ObjectId(2))
        .unwrap()
        .flags
        .remove(Flag::Wizard);
    assert!(!stompymux_rs::flags::is_wizard(&w, ObjectId(2)));
}
#[test]
fn small_pages_truncate_without_lost_or_repeated_objects() {
    let mut w = world();
    for object in w.objects.values_mut() {
        object.name = "é".repeat(1000);
    }
    let mut cursor = Some(FindCursor::new("", &w));
    let mut ids = Vec::new();
    while let Some(current) = cursor {
        let page = find::page(&w, ObjectId(2), &current, 20, 64).unwrap();
        assert!(page.bytes.len() <= 64);
        let output = String::from_utf8(page.bytes).unwrap();
        for row in output.lines().filter(|s| s.starts_with('é')) {
            ids.push(
                row.split("(#")
                    .nth(1)
                    .unwrap()
                    .split([':', ')'])
                    .next()
                    .unwrap()
                    .parse::<i64>()
                    .unwrap(),
            );
        }
        cursor = page.cursor;
    }
    assert_eq!(ids, [0, 2, 4, 5, 8, 9]);
    let cursor = FindCursor::new("", &w);
    let original = cursor.clone();
    assert!(find::page(&w, ObjectId(2), &cursor, 20, 10).is_err());
    assert_eq!(cursor, original);
    for limit in 1..40 {
        assert!(find::bounded_error("é error", limit).len() <= limit);
    }
    assert!(find::page(&w, ObjectId(2), &FindCursor::new("missing", &w), 20, 1).is_err());
}
