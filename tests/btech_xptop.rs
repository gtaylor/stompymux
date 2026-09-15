//! XP leaderboard selection, balance arithmetic, limits and transactional publication.
use std::{cell::RefCell, rc::Rc};
use stompymux_rs::*;
mod support;

fn player(world: &mut World, config: &Config, name: &str, experience: u32) -> ObjectId {
    let id = world.create(config, name.into(), Kind::Player);
    set_battle_character(
        world,
        id,
        BattleCharacter {
            bruise: 0,
            lethal: 0,
            build: 5,
            reflexes: 4,
            intuition: 3,
            learn: 2,
            charisma: 1,
        },
    )
    .unwrap();
    set_battle_character_value(
        world,
        id,
        "Piloting-Biped",
        BattleCharacterValue {
            value: 4,
            experience,
            last_used: 123,
        },
    )
    .unwrap();
    id
}

fn output(scripts: &Scripts) -> String {
    scripts
        .drain_outbox()
        .into_iter()
        .map(|(_, line)| text::plain(line.source()))
        .collect::<Vec<_>>()
        .join("\n")
}

#[tokio::test]
async fn leaderboard_totals_ties_native_lua_and_restart() {
    let (_dir, config, mut world) = support::isolated_world().await;
    let mut ordinary = Vec::new();
    for xp in 1..=18 {
        ordinary.push(player(&mut world, &config, &format!("Player {xp}"), xp));
    }
    let first = player(&mut world, &config, "[fg=red]海[reset]", 42);
    let second = player(&mut world, &config, "Second tie", 42 + 16_777_216);
    let wizard = player(&mut world, &config, "Wizard", 9999);
    world
        .objects
        .get_mut(&wizard)
        .unwrap()
        .flags
        .insert(Flag::Wizard);
    player(&mut world, &config, "No XP", 0);
    player(&mut world, &config, "Only levels", 16_777_216);
    let before = world.btech.clone();
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
    let report = battle_xp_ranking_action(&scripts, &config, ObjectId(1), "PilBip").unwrap();
    assert_eq!(report.skill, "Piloting-Biped");
    assert_eq!(report.counted, 21);
    assert_eq!(report.total, 255);
    assert_eq!(report.entries.len(), 16);
    let expected: Vec<_> = [first, second]
        .into_iter()
        .chain(ordinary.iter().rev().take(14).copied())
        .collect();
    assert_eq!(
        report.entries.iter().map(|e| e.player).collect::<Vec<_>>(),
        expected
    );
    assert_eq!(report.entries[0].experience, 42);
    assert_eq!(report.entries[1].experience, 42);
    assert!((report.entries[0].percentage - 100.0 * 42.0 / 255.0).abs() < 1e-10);
    assert!(report.text.contains("16.471 %"));
    assert!(report.text.contains("Grand total: 255 points"));
    assert_eq!(output(&scripts), report.text);
    assert_eq!(
        text::plain(&support::run_text(
            &scripts,
            &config,
            ObjectId(1),
            1,
            "xptop piloting-biped"
        )),
        report.text
    );
    let lua: String = scripts
        .eval_callback("return btech.character.xptop(1,'PilBip').text")
        .unwrap();
    assert_eq!(lua, report.text);
    assert_eq!(output(&scripts), report.text);
    scripts
        .eval_callback::<()>("local r=btech.character.xptop(1,'PilBip');r.entries[1].experience=0")
        .unwrap();
    output(&scripts);
    assert_eq!(scripts.world().btech, before);
    let saved = scripts.world().clone();
    persistence::save(&config.database(), &saved).await.unwrap();
    let loaded = persistence::load(&config.database()).await.unwrap();
    assert_eq!(loaded.btech, before);
    let restart = Scripts::new(&config, Rc::new(RefCell::new(loaded))).unwrap();
    assert_eq!(
        battle_xp_ranking_action(&restart, &config, ObjectId(1), "PilBip").unwrap(),
        report
    );
}

#[tokio::test]
async fn empty_zero_total_and_failed_publication_are_safe() {
    let (dir, config, mut world) = support::isolated_world().await;
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
    let empty = battle_xp_ranking_action(&scripts, &config, ObjectId(1), "PilBip").unwrap();
    assert_eq!((empty.counted, empty.total, empty.entries.len()), (0, 0, 0));
    assert!(!empty.text.contains("Grand total"));
    player(&mut world, &config, "Only levels", 16_777_216);
    let before = world.btech.clone();
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
    let zero = battle_xp_ranking_action(&scripts, &config, ObjectId(1), "PilBip").unwrap();
    assert_eq!((zero.counted, zero.total), (1, 0));
    assert_eq!(zero.entries[0].percentage, 0.0);
    assert!(zero.text.contains("0 (0.000 %)"));
    output(&scripts);
    for skill in ["", "Build", "Unknown", "PilBip extra"] {
        assert!(battle_xp_ranking_action(&scripts, &config, ObjectId(1), skill).is_err());
    }
    assert!(battle_xp_ranking_action(&scripts, &config, ObjectId(4), "PilBip").is_err());
    assert!(
        scripts
            .eval_callback::<()>("btech.character.xptop(1,'PilBip');error('abort')")
            .is_err()
    );
    assert!(scripts.drain_outbox().is_empty());
    assert_eq!(scripts.world().btech, before);
    let path = dir.path().join("stompymux.toml");
    let mut table: toml::Table = std::fs::read_to_string(&path).unwrap().parse().unwrap();
    table
        .entry("lua")
        .or_insert(toml::Value::Table(toml::Table::new()))
        .as_table_mut()
        .unwrap()
        .insert("output_entry_limit".into(), 1.into());
    std::fs::write(path, toml::to_string(&table).unwrap()).unwrap();
    let config = Config::load(dir.path()).unwrap();
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(scripts.world().clone()))).unwrap();
    assert!(battle_xp_ranking_action(&scripts, &config, ObjectId(1), "PilBip").is_err());
    assert!(scripts.drain_outbox().is_empty());
    assert_eq!(scripts.world().btech, before);
}

#[tokio::test]
async fn player_limit_precedes_ranking_and_totals_do_not_overflow() {
    let (_dir, config, mut world) = support::isolated_world().await;
    for index in 0..10_000 {
        player(&mut world, &config, &format!("Rank {index}"), 16_777_215);
    }
    let excluded = player(&mut world, &config, "Beyond limit", 1);
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
    let report = battle_xp_ranking_action(&scripts, &config, ObjectId(1), "PilBip").unwrap();
    assert_eq!(report.counted, 10_000);
    assert_eq!(report.total, 167_772_150_000);
    assert_eq!(report.entries.len(), 16);
    assert!(report.entries.iter().all(|entry| entry.player != excluded));
}

/// A later leader displaces equal balances in reference exchange order; display cells stay fixed.
#[tokio::test]
async fn reference_exchange_order_and_two_column_menu() {
    let (_dir, config, mut world) = support::isolated_world().await;
    let first = player(&mut world, &config, "First tie", 5);
    let second = player(&mut world, &config, "Second tie", 5);
    let leader = player(&mut world, &config, "Leader", 6);
    let before = world.btech.clone();
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
    let report = battle_xp_ranking_action(&scripts, &config, ObjectId(1), "PilBip").unwrap();
    assert_eq!(
        report
            .entries
            .iter()
            .map(|entry| entry.player)
            .collect::<Vec<_>>(),
        [leader, second, first]
    );
    let expected = [
        "-".repeat(78),
        format!("{:<39}{:<39}", "  1. Leader", "6 (37.500 %)"),
        format!("{:<39}{:<39}", "  2. Second tie", "5 (31.250 %)"),
        format!("{:<39}{:<39}", "  3. First tie", "5 (31.250 %)"),
        "-".repeat(78),
        format!("{:<39}", "Grand total: 16 points"),
        "-".repeat(78),
    ]
    .join("\n");
    assert_eq!(report.text, expected);
    let notices = scripts.drain_outbox();
    assert_eq!(notices.len(), 7);
    assert_eq!(
        notices[0].1.source(),
        format!("[fg=blue]{}[reset]", "-".repeat(78))
    );
    assert_eq!(scripts.world().btech, before);
    // Player-authored style-looking text stays literal and cannot move the XP column.
    scripts.world_mut().objects.get_mut(&leader).unwrap().name =
        format!("[fg=red]{}", "L".repeat(80));
    let report = battle_xp_ranking_action(&scripts, &config, ObjectId(1), "PilBip").unwrap();
    let row = report.text.lines().nth(1).unwrap();
    assert_eq!(row.len(), 78);
    assert_eq!(&row[39..], format!("{:<39}", "6 (37.500 %)"));
    assert!(row.starts_with("  1. [fg=red]"));
    assert_eq!(output(&scripts), report.text);
}
