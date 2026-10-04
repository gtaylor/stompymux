//! Embedded server startup shared by TCP integration scenarios.

use crate::Heartbeats;
use std::{cell::Cell, rc::Rc};
use stompymux_rs::{Config, ShutdownRequest, prepare_server, run_with_schedule_clock};
use tokio::{net::TcpListener, sync::oneshot};

/// Start an isolated server using a deterministic schedule wall clock, and return once
/// its loop runs. The [`Heartbeats`] handle drives and observes its periodic work.
pub async fn start(
    config: &Config,
    clock: Rc<Cell<i64>>,
) -> (
    std::net::SocketAddr,
    oneshot::Sender<ShutdownRequest>,
    tokio::task::JoinHandle<anyhow::Result<()>>,
    mlua::Lua,
    Heartbeats,
) {
    crate::init_logging();
    let scripts = prepare_server(config).await.unwrap();
    let lua = scripts.inspect_lua();
    let mut heartbeats = Heartbeats::new(scripts.progress(), config);
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let (shutdown, request) = oneshot::channel();
    let config = config.clone();
    let task = tokio::task::spawn_local(async move {
        run_with_schedule_clock(
            config,
            scripts,
            listener,
            async { request.await.unwrap_or(ShutdownRequest::Sigterm) },
            move || clock.get(),
        )
        .await
    });
    heartbeats.ready().await;
    (address, shutdown, task, lua, heartbeats)
}
