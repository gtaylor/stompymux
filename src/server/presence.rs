//! Observational session transitions and transactional suspect-channel notifications.
use super::*;
use crate::flags::Flag;

/// Actual session transitions are independent of callback rollback.
#[derive(Clone, Copy)]
pub(super) enum TransitionKind {
    Connected,
    Reconnected,
    PartialDisconnect,
    Disconnected,
}

/// Capture identity and suspicion before callbacks can change or remove an object.
pub(super) struct Transition {
    name: String,
    kind: TransitionKind,
    dark: bool,
    suspect: bool,
    site: crate::sites::Classification,
    peer: IpAddr,
}
impl Transition {
    /// Preserve a removed object's identity for committed destruction notifications.
    pub(super) fn capture(
        world: &World,
        player: ObjectId,
        kind: TransitionKind,
        peer: IpAddr,
        site: crate::sites::Classification,
    ) -> Option<Self> {
        let object = world.objects.get(&player)?;
        Some(Self {
            name: object.name.clone(),
            kind,
            dark: object.flags.contains(Flag::Dark),
            suspect: object.flags.contains(Flag::Suspect),
            site,
            peer,
        })
    }
}
impl Server {
    /// Snapshot a real attachment/detachment before callbacks can change its identity.
    pub(super) fn transition(
        &self,
        player: ObjectId,
        kind: TransitionKind,
        peer: IpAddr,
        site: crate::sites::Classification,
    ) -> Option<Transition> {
        Transition::capture(&self.scripts.world.borrow(), player, kind, peer, site)
    }

    /// Notify live monitors independently, then commit channel output in its own transaction.
    pub(super) async fn announce_transition(&mut self, transition: Option<Transition>) {
        let Some(t) = transition else {
            return;
        };
        let verb = match t.kind {
            TransitionKind::Connected if t.dark => "DARK-connected",
            TransitionKind::Connected => "connected",
            TransitionKind::Reconnected => "reconnected",
            TransitionKind::PartialDisconnect => "partially disconnected",
            TransitionKind::Disconnected => "disconnected",
        };
        self.config.log(
            &[crate::logging::Category::Logins],
            "CON",
            "EVENT",
            format!("{} has {verb} from {}", t.name, t.peer),
        );
        let message = format!("GAME: {} has {verb}.\r\n", t.name);
        {
            let world = self.scripts.world.borrow();
            for session in self.sessions.values() {
                if let Some(object) = session.player.and_then(|p| world.objects.get(&p))
                    && object.flags.contains(Flag::Monitor)
                {
                    session.text(&message, object.flags.contains(Flag::Ansi));
                }
            }
        }
        if !(t.suspect || t.site.suspect)
            || self
                .scripts
                .communication(&self.config)
                .name("Suspect")
                .is_err()
        {
            return;
        }
        let before = self.scripts.world.borrow().clone();
        let result = (|| -> Result<()> {
            self.snapshots()?;
            let service = self.scripts.communication(&self.config);
            let verb = if matches!(
                t.kind,
                TransitionKind::Connected | TransitionKind::Reconnected
            ) {
                "connected"
            } else {
                "disconnected"
            };
            if t.suspect {
                service.emit("Suspect", &format!("{} has {verb}.", t.name), false)?;
            }
            if t.site.suspect {
                service.emit(
                    "Suspect",
                    &format!(
                        "{} {} has {verb}.",
                        crate::text::escape(&format!("[Suspect site: {}]", t.peer)),
                        t.name
                    ),
                    false,
                )?;
            }
            Ok(())
        })();
        if let Err(error) = result {
            self.config.log(
                &[crate::logging::Category::Problems],
                "SRV",
                "ERROR",
                format!("Suspect connection notification: {error:#}"),
            );
            *self.scripts.world.borrow_mut() = before;
            self.reconcile_connections();
            self.scripts.effects.rollback();
            return;
        }
        if !self.commit(before).await {
            self.config.log(
                &[crate::logging::Category::Problems],
                "SRV",
                "ERROR",
                "Suspect connection notification was not saved",
            );
        }
        self.flush();
    }
}

/// Denied peers receive no protocol initialization and never enter the session registry.
pub(super) async fn reject_site(
    stream: TcpStream,
    body: String,
    palette: std::sync::Arc<crate::text::Palette>,
    config: Config,
) {
    let (mut read, mut write) = stream.into_split();
    let delivery = async {
        let options = crate::text::RenderOptions::default();
        let body = if body.is_empty() {
            "Connection refused.\n".into()
        } else {
            format!("{body}\n")
        };
        let spans = crate::text::Document::Styled(body).spans(&palette, &options);
        let chunks = crate::text::telnet_chunks(
            &spans,
            &palette,
            &options,
            config.runtime.output_message_limit,
            config.lua.output_byte_limit,
        );
        match chunks {
            Ok(chunks) => {
                for chunk in chunks {
                    write.write_all(&chunk).await?;
                }
            }
            Err(error) => {
                config.log(
                    &[crate::logging::Category::Problems],
                    "SRV",
                    "ERROR",
                    format!("Bad-site message rendering: {error:#}"),
                );
                let fallback = crate::telnet::bounded_error(
                    "Connection refused.",
                    config.runtime.output_message_limit,
                );
                write.write_all(&fallback).await?;
            }
        }
        write.shutdown().await
    };
    // Discard pipelined login bytes without entering framing or authentication.
    let drain = async {
        const DISCARD_BUFFER_BYTES: usize = 1024;
        let mut buffer = [0; DISCARD_BUFFER_BYTES];
        while matches!(read.read(&mut buffer).await, Ok(n) if n > 0) {}
        std::future::pending::<()>().await;
    };
    let exchange = async {
        tokio::select! {
            result = delivery => result,
            _ = drain => unreachable!("discard waits until delivery completes"),
        }
    };
    match tokio::time::timeout(
        Duration::from_millis(config.runtime.write_timeout_ms),
        exchange,
    )
    .await
    {
        Ok(Ok(())) => {}
        Ok(Err(error)) => config.log(
            &[crate::logging::Category::Network],
            "NET",
            "ERROR",
            format!("Bad-site message delivery: {error}"),
        ),
        Err(_) => config.log(
            &[crate::logging::Category::Network],
            "NET",
            "ERROR",
            "Bad-site message delivery timed out",
        ),
    }
}
