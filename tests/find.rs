//! Compatibility fixtures for C world/walkdb.c, objects/flags.c and match_helpers.c.
use stompymux_rs::{
    find::{self, SearchRange},
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
/// Capture a complete report independently of transport chunk size.
fn search(w: &World, actor: i64, query: &str) -> String {
    String::from_utf8(stompymux_rs::telnet::encode(&format!(
        "{}\n",
        find::report(w, ObjectId(actor), query, 65536).unwrap()
    )))
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
    assert_eq!(SearchRange::new(",#2,4,5", 9).upper, 9);
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
fn full_reports_are_bounded_and_explicit_about_omissions() {
    let mut w = world();
    for o in w.objects.values_mut() {
        o.name = "é👩‍🚀".repeat(1000);
    }
    let report = find::report(&w, ObjectId(2), "", 200).unwrap();
    assert!(report.contains("Report truncated"));
    assert!(report.ends_with("***End of List***"));
    assert!(stompymux_rs::telnet::encode(&report).len() <= 200);
    assert!(find::report(&w, ObjectId(2), "", 10).is_err());
    for limit in 1..40 {
        assert!(find::bounded_error("é error", limit).len() <= limit);
    }
}

#[test]
fn c_search_filters_grouping_and_statistics() {
    use stompymux_rs::search::{Criteria, report, statistics};
    let mut w = world();
    w.objects.get_mut(&ObjectId(8)).unwrap().zone = Some(ObjectId(4));
    w.objects
        .get_mut(&ObjectId(8))
        .unwrap()
        .powers
        .insert(stompymux_rs::powers::Power::Idle);
    let text = report(&w, ObjectId(2), "", 65536).unwrap();
    assert!(text.contains("GOD(#1:PW)"));
    assert!(text.contains("Other Wizard"));
    assert!(text.contains("Trash(#7:-)"));
    assert!(text.contains("out(#6:E) [from NOWHERE to NOWHERE]"));
    assert!(text.contains("Rooms...2  Exits...1  Objects...3  Players...3  Garbage...1"));
    let expected = [
        ("name=apple", vec![]),
        ("name=red", vec![8]),
        ("name=", vec![]),
        ("players=", vec![]),
        ("type=p", vec![1, 2, 3]),
        ("p=W", vec![2]),
        ("flags=DG", vec![5]),
        ("flags=RP", vec![1, 2, 3]),
        ("flags=+", vec![]),
        ("power=IDLE", vec![8]),
        ("zone=#4", vec![8]),
        ("name=*", vec![9]),
        ("type=,4,5", vec![4, 5]),
        ("name=red,9,3", vec![]),
    ];
    for (query, expected) in expected {
        let criteria = Criteria::parse(&w, ObjectId(2), query).unwrap();
        assert_eq!(
            w.objects
                .values()
                .filter(|o| criteria.matches(&w, o))
                .map(|o| o.id.0)
                .collect::<Vec<_>>(),
            expected,
            "{query}"
        );
    }
    for query in [
        "oops=",
        "flags=~",
        "power=nope",
        "type=invalid",
        "zone=#999",
    ] {
        assert!(Criteria::parse(&w, ObjectId(2), query).is_err(), "{query}");
    }
    assert!(report(&w, ObjectId(5), "", 65536).is_err());
    w.next_id = 12;
    w.objects
        .get_mut(&ObjectId(4))
        .unwrap()
        .flags
        .insert(Flag::Going);
    assert_eq!(
        statistics(&w),
        "12 objects = 2 rooms, 1 exits, 2 things, 3 players. (4 garbage)"
    );
    let short = report(&w, ObjectId(2), "", 200).unwrap();
    assert!(short.contains("truncated"));
    assert!(short.contains("Rooms...2"));
}
