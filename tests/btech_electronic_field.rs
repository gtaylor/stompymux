//! Electronic field boundaries, opposing strength, retained disturbance and guided-weapon policy.
use stompymux_rs::*;

/// One emitter with independent ordinary, Angel and personal modes.
fn source(
    team: i32,
    distance: f64,
    guardian: ElectronicMode,
    angel: ElectronicMode,
    personal: ElectronicMode,
) -> ElectronicSource {
    ElectronicSource {
        team,
        distance,
        guardian,
        angel,
        personal,
    }
}

/// Resolve a newly observed field without previous disturbance flags.
fn fresh(sources: Vec<ElectronicSource>) -> ElectronicField {
    resolve_battle_electronic_field(ElectronicField::default(), 1, sources, false).unwrap()
}

/// Exact range boundaries include six hexes for suites and half a hex for personal emissions.
#[test]
fn field_range_boundaries_and_invalid_distances() {
    use ElectronicMode::{Ecm, Off};
    for (distance, guardian, personal) in [
        (0.0, true, true),
        (0.5, true, true),
        (0.500001, true, false),
        (6.0, true, false),
        (6.000001, false, false),
    ] {
        assert_eq!(
            fresh(vec![source(1, distance, Ecm, Off, Off)]).protected,
            guardian
        );
        assert_eq!(
            fresh(vec![source(1, distance, Off, Ecm, Off)]).angel_protected,
            guardian
        );
        assert_eq!(
            fresh(vec![source(1, distance, Off, Off, Ecm)]).protected,
            personal
        );
        assert_eq!(
            fresh(vec![source(2, distance, Ecm, Off, Off)]).disturbed,
            guardian
        );
    }
    for distance in [-0.01, f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert!(
            resolve_battle_electronic_field(
                ElectronicField::default(),
                1,
                [source(1, distance, Ecm, Off, Off)],
                false
            )
            .is_err()
        );
    }
}

/// Only opposing ECCM cancels protection; equal strength cancels, and Angel contributes two.
#[test]
fn field_opposing_strength_and_angel_weight() {
    use ElectronicMode::{Eccm, Ecm, Off};
    let ordinary = source(1, 0.0, Ecm, Off, Off);
    let enemy_counter = source(2, 1.0, Eccm, Off, Off);
    assert!(fresh(vec![ordinary]).protected);
    let tied = fresh(vec![ordinary, enemy_counter]);
    assert!(tied.countered);
    assert!(!tied.protected);
    assert!(fresh(vec![ordinary, ordinary, enemy_counter]).protected);
    let angel = source(1, 0.0, Off, Ecm, Off);
    assert!(fresh(vec![angel, enemy_counter]).angel_protected);
    assert!(!fresh(vec![angel, enemy_counter, enemy_counter]).angel_protected);
    // Friendly ECCM neither counters friendly ECM nor protects against hostile ECCM.
    assert!(fresh(vec![ordinary, source(1, 0.0, Eccm, Off, Off)]).protected);
    assert!(fresh(vec![enemy_counter]).countered);
    let hostile = source(2, 0.0, Off, Ecm, Off);
    let counter = source(1, 0.0, Eccm, Off, Off);
    assert!(fresh(vec![hostile, counter]).angel_disturbed);
    assert!(!fresh(vec![hostile, counter, counter]).blocks_outgoing_guidance());
}

/// Protection and disturbance are independent: opposing ECM does not cancel friendly ECM.
#[test]
fn field_simultaneous_effects_and_guidance() {
    use ElectronicMode::{Ecm, Off};
    let field = fresh(vec![
        source(1, 0.0, Ecm, Ecm, Off),
        source(2, 0.0, Ecm, Ecm, Off),
    ]);
    assert!(field.protected && field.angel_protected && field.disturbed && field.angel_disturbed);
    assert!(!field.countered);
    assert!(field.blocks_incoming_guidance() && field.blocks_outgoing_guidance());
    let reversed = resolve_battle_electronic_field(
        ElectronicField::default(),
        2,
        [source(1, 0.0, Ecm, Off, Off)],
        false,
    )
    .unwrap();
    assert!(reversed.disturbed && !reversed.protected);
    let saved: ElectronicField =
        serde_json::from_value(serde_json::to_value(field).unwrap()).unwrap();
    assert_eq!(saved, field);
}

/// A continuing disturbance retains its family until clearance; a later disturbance is classified anew.
#[test]
fn field_disturbance_transition_and_clearance() {
    use ElectronicMode::{Eccm, Ecm, Off};
    let old = fresh(vec![source(2, 1.0, Ecm, Off, Off)]);
    let angel = source(2, 1.0, Off, Ecm, Off);
    let continued = resolve_battle_electronic_field(old, 1, [angel], false).unwrap();
    assert!(continued.disturbed && !continued.angel_disturbed);
    let cleared = resolve_battle_electronic_field(
        continued,
        1,
        [angel, source(1, 0.0, Off, Eccm, Off)],
        false,
    )
    .unwrap();
    assert!(!cleared.blocks_outgoing_guidance());
    let renewed = resolve_battle_electronic_field(cleared, 1, [angel], false).unwrap();
    assert!(renewed.angel_disturbed && !renewed.disturbed);
    assert_eq!(
        resolve_battle_electronic_field(renewed, 1, [], false).unwrap(),
        ElectronicField::default()
    );
}

/// Self-interference is a large ordinary contribution, rather than an uncancellable special case.
#[test]
fn field_self_interference_and_personal_contributions() {
    use ElectronicMode::{Eccm, Ecm, Off};
    let affected =
        resolve_battle_electronic_field(ElectronicField::default(), 1, [], true).unwrap();
    assert!(affected.disturbed && !affected.protected);
    let counters = vec![source(1, 0.0, Off, Eccm, Off); 500];
    assert!(
        !resolve_battle_electronic_field(affected, 1, counters, true)
            .unwrap()
            .disturbed
    );
    let combined = fresh(vec![
        source(1, 0.0, Ecm, Off, Ecm),
        source(2, 0.5, Eccm, Off, Off),
    ]);
    assert!(combined.protected);
}

/// Selecting the other mode replaces it; selecting the current mode disables the suite.
#[test]
fn electronic_modes_are_exclusive_and_round_trip() {
    use ElectronicMode::{Eccm, Ecm, Off};
    assert_eq!(Off.toggle(Ecm), Ecm);
    assert_eq!(Ecm.toggle(Eccm), Eccm);
    assert_eq!(Eccm.toggle(Eccm), Off);
    assert_eq!(Ecm.toggle(Off), Off);
    for mode in [Off, Ecm, Eccm] {
        assert_eq!(
            serde_json::from_value::<ElectronicMode>(serde_json::to_value(mode).unwrap()).unwrap(),
            mode
        );
    }
}

/// Countering and disturbance produce separate notices only at their transitions.
#[test]
fn electronic_field_notices_follow_transitions() {
    use ElectronicMode::{Eccm, Ecm, Off};
    let clear = ElectronicField::default();
    let blocked = fresh(vec![source(2, 1.0, Ecm, Eccm, Off)]);
    let notices = blocked.notices(clear, ObjectId(7), true);
    assert_eq!(notices.len(), 2);
    assert!(notices[0].text.contains("ready light turns red"));
    assert!(notices[1].text.contains("static"));
    assert!(notices.iter().all(|notice| notice.unit == ObjectId(7)));
    assert_eq!(blocked.notices(clear, ObjectId(7), false).len(), 1);
    assert!(blocked.notices(blocked, ObjectId(7), true).is_empty());
    let restored = clear.notices(blocked, ObjectId(7), true);
    assert_eq!(restored.len(), 2);
    assert!(restored[0].text.contains("ready light turns green"));
    assert!(restored[1].text.contains("back to normal"));
}
