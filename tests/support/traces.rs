//! Read the debug traces a test transaction staged for commit.
use stompymux_rs::{
    Scripts,
    logging::{TraceRecord, TraceTopic},
};

/// Drain every staged debug trace and keep the messages for one topic, in staging order.
pub fn drain_traces(scripts: &Scripts, topic: TraceTopic) -> Vec<String> {
    scripts
        .drain_traces_for_inspection()
        .into_iter()
        .filter(|record: &TraceRecord| record.topic == topic)
        .map(|record| record.message)
        .collect()
}
