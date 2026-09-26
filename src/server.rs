//! Serialized world owner, connection lifecycle and common graceful shutdown coordinator.
mod administration;
mod authentication;
mod btech;
mod configuration;
mod connection;
mod diagnostics;
mod execution;
mod heartbeat_harness;
mod help;
pub use heartbeat_harness::{HeartbeatHarness, HeartbeatMetrics};
mod logging;
mod lua_admin;
mod maintenance;
mod operations;
mod presence;
mod queue;
mod runtime;
mod shutdown;
mod startup;
mod transaction;
use crate::{
    accounts,
    commands::{self, Action},
    config::Config,
    lua::Scripts,
    persistence,
    sessions::*,
    telnet::{self, Input},
    world::*,
};
use anyhow::{Context, Result};
use connection::connection;
use execution::ReplyDestination;
use presence::TransitionKind;
pub use runtime::{run, run_with_clocks, run_with_schedule_clock};
pub use startup::{prepare, serve};
use std::{
    cell::RefCell,
    collections::BTreeMap,
    future::Future,
    net::IpAddr,
    rc::Rc,
    time::{Duration, Instant},
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::mpsc,
};
use zeroize::Zeroizing;

// login_hash_limit is measured per second; this is its unit, not a tunable.
const HASH_RATE_WINDOW: Duration = Duration::from_secs(1);

/// Origin of a request handled exclusively by the world owner.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShutdownRequest {
    /// Interrupt signal.
    Sigint,
    /// Termination signal.
    Sigterm,
    /// Authenticated administrator.
    Player(ObjectId),
}

enum Event {
    AdminHashed(administration::Job, Result<String>),
    Bytes(SessionId, Vec<u8>),
    Gone(SessionId),
    Authenticated(authentication::Outcome),
}
struct Server {
    config: Config,
    scripts: Scripts,
    sessions: BTreeMap<SessionId, Session>,
    events: mpsc::Sender<Event>,
    authentication: authentication::State,
    /// Runtime identity for MSSP replies.
    started_at: i64,
    listen_port: u16,
    /// Accepted request; also prevents further command and authentication dispatch.
    shutdown: Option<ShutdownRequest>,
    /// A shutdown write failed, even if a later snapshot succeeds.
    shutdown_failed: bool,
    /// Runtime commands survive disconnect and Lua reload, but never restart.
    command_queue: commands::queue::Queue,
    cleaning: crate::cleaning::Cleaning,
    controls: crate::controls::Controls,
    idle_recheck: bool,
    message_cache: crate::message_cache::MessageCache,
    /// The world as the database last stored it, used as the baseline for the next save.
    /// `None` whenever that is uncertain, for example after a failed commit or a write
    /// outside the ordinary commit path; the next save then reads the stored world.
    durable: Option<World>,
    /// Keeps the database open between saves once the first save has run.
    database_anchor: Option<persistence::DatabaseAnchor>,
}
