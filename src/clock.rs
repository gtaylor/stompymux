//! Wall-clock timestamps for persisted records and externally visible times.
//!
//! Runtime deadlines and elapsed durations use `std::time::Instant` or
//! `tokio::time::Instant` at their call sites instead.

/// Current Unix timestamp in whole seconds.
pub fn wall_time() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}
