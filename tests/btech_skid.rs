//! Mech skid checks on pavement: only a running 'Mech that turned rolls before it moves on.
use crate::btech_motion_common::{RULES, fixture_assets};
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
