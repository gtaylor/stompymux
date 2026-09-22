//! Canonical database snapshots for live-server equality checks.
use stompymux_rs::persistence;

/// Serialize the persisted world with the BattleTech tick counters pinned.
///
/// A live server commits the shared turn phase once per real second even on
/// otherwise idle worlds (`Server::btech_tick`), so raw database bytes are
/// never stable across a live connection. Read-only scenarios assert the
/// strongest stable invariant instead: logical world equality with only the
/// committed simulation counters normalized.
pub async fn stable_world(database: &std::path::Path) -> serde_json::Value {
    let world = persistence::load(database).await.unwrap();
    let mut value = serde_json::to_value(&world).unwrap();
    let btech = value
        .get_mut("btech")
        .expect("serialized btech state")
        .as_object_mut()
        .expect("btech state object");
    btech.insert("turn_clock".into(), 0.into());
    btech.insert("simulation_seconds".into(), 0.into());
    if let Some(reactor) = btech.get_mut("reactor").and_then(|r| r.as_object_mut()) {
        reactor.insert("startup_remaining".into(), 0.into());
    }
    value
}
