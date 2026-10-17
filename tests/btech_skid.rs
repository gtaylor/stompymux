//! Mech skid checks on pavement: only a running 'Mech that turned rolls before it moves on, and
//! a skid's combat modifiers for the rest of the turn.
use crate::btech_motion_common::{RULES, fixture_assets, optical_aim_rules, pass_seconds};
use stompymux_rs::*;

/// Drive the 6/9 Phoenix Hawk south across open pavement at `speed`, bear off by a small turn
/// once it is moving, and report whether any hex it left after turning asked for a skid roll.
async fn turns_into_skid_check(speed: f64) -> bool {
    let source = format!("30 30\n{}", format!("{}\n", "_0".repeat(30)).repeat(30));
    let template =
        MechTemplate::parse("PXH-1", include_str!("fixtures/btech/units/PXH-1.toml")).unwrap();
    let (_dir, _config, mut world, id) = fixture_assets(&source, template).await;
    assert_eq!(world.btech.constructed_units()[&id].walking_speed(), 64.5);
    set_battle_heading(&mut world, id, ObjectId(1), 180.0).unwrap();
    for _ in 0..60 {
        advance_battle_motion(&mut world, RULES).unwrap();
    }
    set_battle_speed(&mut world, id, ObjectId(1), speed).unwrap();
    for _ in 0..30 {
        advance_battle_motion(&mut world, RULES).unwrap();
    }
    assert_eq!(
        world.btech.constructed_units()[&id].motion().unwrap().speed,
        speed
    );
    set_battle_heading(&mut world, id, ObjectId(1), 170.0).unwrap();
    let start = world.btech.constructed_units()[&id].position().unwrap();
    for _ in 0..40 {
        let notices = advance_battle_motion(&mut world, RULES).unwrap();
        if notices
            .iter()
            .any(|notice| notice.text.contains("Your footing slips"))
        {
            return true;
        }
    }
    let end = world.btech.constructed_units()[&id].position().unwrap();
    assert!(
        end.y >= start.y + 2,
        "the 'Mech crossed hexes after turning"
    );
    false
}

/// A 'Mech turning on pavement at or below its walking speed keeps its footing without a roll;
/// above walking speed it is running and must roll before it moves on (Total Warfare pp. 60,
/// 62).
#[tokio::test]
async fn pavement_skid_checks_start_above_walking_speed() {
    // 4.5 MP and walking speed itself are walking; 8 MP and 9 MP are running for a 6/9.
    assert!(!turns_into_skid_check(48.375).await);
    assert!(!turns_into_skid_check(64.5).await);
    assert!(turns_into_skid_check(86.0).await);
    assert!(turns_into_skid_check(96.75).await);
}

/// Hexes a running Phoenix Hawk skids after turning on open pavement, with light woods filling
/// every hex south of it when `woods`.
async fn skid_hexes(woods: bool) -> usize {
    let source = format!("30 30\n{}", format!("{}\n", "_0".repeat(30)).repeat(30));
    let template =
        MechTemplate::parse("PXH-1", include_str!("fixtures/btech/units/PXH-1.toml")).unwrap();
    let (_dir, _config, mut world, id) = fixture_assets(&source, template).await;
    set_battle_heading(&mut world, id, ObjectId(1), 180.0).unwrap();
    for _ in 0..60 {
        advance_battle_motion(&mut world, RULES).unwrap();
    }
    set_battle_speed(&mut world, id, ObjectId(1), 96.75).unwrap();
    for _ in 0..30 {
        advance_battle_motion(&mut world, RULES).unwrap();
    }
    let position = world.btech.constructed_units()[&id].position().unwrap();
    if woods {
        world
            .btech
            .rewrite_map_record(position.map, |record| {
                for y in usize::from(position.y) + 1..30 {
                    for x in 0..30 {
                        crate::support::set_hex_terrain(
                            &mut record["terrain"][y * 30 + x],
                            Terrain::LightWoods,
                        );
                    }
                }
            })
            .unwrap();
    }
    // A roll of two fails the skid check.
    let seed = (0..=255)
        .find(|seed| Dice::seeded([*seed; 32]).two_d6() == 2)
        .unwrap();
    world
        .btech
        .set_unit_dice(id, Dice::seeded([seed; 32]))
        .unwrap();
    set_battle_heading(&mut world, id, ObjectId(1), 170.0).unwrap();
    for _ in 0..40 {
        let notices = advance_battle_motion(&mut world, RULES).unwrap();
        if notices
            .iter()
            .any(|notice| notice.text.contains("lose your footing and skid"))
        {
            return notices
                .iter()
                .filter(|notice| notice.text.starts_with("You skid into"))
                .count();
        }
    }
    panic!("the 'Mech never skidded");
}

/// Each hex of a skid uses up the movement points its terrain costs to enter, so light woods
/// at two MP a hex halve the skid (Total Warfare p. 62).
#[tokio::test]
async fn terrain_shortens_a_skid() {
    let pavement = skid_hexes(false).await;
    assert!(pavement >= 2, "{pavement}");
    assert_eq!(skid_hexes(true).await, pavement.div_ceil(2));
}

/// For the turn it skids, a skidding 'Mech fires at +1 and is hit at +2 (Total Warfare p. 62);
/// a turn later both are gone.
#[tokio::test]
async fn a_skid_costs_one_to_fire_and_gives_two_to_be_hit_for_a_turn() {
    let source = format!("30 30\n{}", format!("{}\n", "_0".repeat(30)).repeat(30));
    let template =
        MechTemplate::parse("PXH-1", include_str!("fixtures/btech/units/PXH-1.toml")).unwrap();
    let (_dir, config, mut world, id) = fixture_assets(&source, template.clone()).await;
    let map = world.btech.constructed_units()[&id].position().unwrap().map;
    let watcher = world.create(&config, "Watcher".into(), Kind::Thing);
    world.objects.get_mut(&watcher).unwrap().home = Some(ObjectId(config.home()));
    create_battle_unit(&mut world, watcher, template).unwrap();
    place_battle_unit(&mut world, watcher, map, 8, 5).unwrap();
    set_battle_heading(&mut world, id, ObjectId(1), 180.0).unwrap();
    for _ in 0..60 {
        advance_battle_motion(&mut world, RULES).unwrap();
    }
    set_battle_speed(&mut world, id, ObjectId(1), 96.75).unwrap();
    for _ in 0..30 {
        advance_battle_motion(&mut world, RULES).unwrap();
    }
    let seed = (0..=255)
        .find(|seed| Dice::seeded([*seed; 32]).two_d6() == 2)
        .unwrap();
    world
        .btech
        .set_unit_dice(id, Dice::seeded([seed; 32]))
        .unwrap();
    set_battle_heading(&mut world, id, ObjectId(1), 170.0).unwrap();
    let skidded = (0..40).any(|_| {
        advance_battle_motion(&mut world, RULES)
            .unwrap()
            .iter()
            .any(|notice| notice.text.contains("lose your footing and skid"))
    });
    assert!(skidded, "the 'Mech never skidded");
    let skidding = |world: &World| {
        battle_unit_target_movement(world, id, 3.0)
            .unwrap()
            .skidding
    };
    let movement = |world: &World| {
        battle_aim_modifiers(world, id, watcher, 0, 4, optical_aim_rules())
            .unwrap()
            .attacker_movement
    };
    assert_eq!(skidding(&world), 2);
    let skidded_movement = movement(&world);
    pass_seconds(&mut world, 29);
    assert_eq!(skidding(&world), 2);
    pass_seconds(&mut world, 1);
    assert_eq!(skidding(&world), 0);
    assert_eq!(movement(&world) + 1, skidded_movement);
}
