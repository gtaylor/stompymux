//! Shared TCP client for fixture-world integration scenarios.
use std::time::Duration;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
};

/// Socket client retaining unread response bytes across assertions.
pub struct Client {
    pub socket: TcpStream,
    pub pending: Vec<u8>,
}

impl Client {
    /// Log in to the fixture world using its configured test password.
    pub async fn connect(address: std::net::SocketAddr, player: i64) -> Self {
        let mut client = Self {
            socket: TcpStream::connect(address).await.unwrap(),
            pending: Vec::new(),
        };
        client.until("Who are you? ").await;
        client.send(&format!("#{player}")).await;
        client.until("Password: ").await;
        client.send("secret").await;
        client.until("Staff Nexus").await;
        client
    }

    /// Send one CRLF-terminated input line.
    pub async fn send(&mut self, text: &str) {
        self.socket
            .write_all(format!("{text}\r\n").as_bytes())
            .await
            .unwrap();
    }

    /// Remove and return everything through `needle` if it has already arrived.
    fn take(&mut self, needle: &str) -> Option<String> {
        let index = self
            .pending
            .windows(needle.len())
            .position(|b| b == needle.as_bytes())?;
        let bytes = self
            .pending
            .drain(..index + needle.len())
            .collect::<Vec<_>>();
        Some(String::from_utf8_lossy(&bytes).into_owned())
    }

    /// Consume through an expected marker, retaining any following bytes.
    pub async fn until(&mut self, needle: &str) -> String {
        tokio::time::timeout(Duration::from_secs(10), async {
            loop {
                if let Some(text) = self.take(needle) {
                    return text;
                }
                let mut bytes = [0; 8192];
                let count = self.socket.read(&mut bytes).await.unwrap();
                assert!(
                    count > 0,
                    "closed waiting for {needle}: {:?}",
                    String::from_utf8_lossy(&self.pending)
                );
                self.pending.extend_from_slice(&bytes[..count]);
            }
        })
        .await
        .unwrap_or_else(|_| {
            panic!(
                "timeout waiting for {needle}: {:?}",
                String::from_utf8_lossy(&self.pending)
            )
        })
    }

    /// Consume through a marker that only a later server heartbeat produces, firing each
    /// heartbeat through `heartbeats` instead of waiting out real seconds.
    ///
    /// Before each attempt a [`Self::fence`] collects everything the server has already
    /// sent, so a marker from one heartbeat is always seen before the next one runs.
    /// `count` bounds the attempts.
    pub async fn until_heartbeats(
        &mut self,
        needle: &str,
        heartbeats: &mut crate::Heartbeats,
        count: usize,
    ) -> String {
        for _ in 0..count {
            self.fence().await;
            if let Some(text) = self.take(needle) {
                return text;
            }
            heartbeats.attempt().await;
        }
        self.fence().await;
        self.take(needle).unwrap_or_else(|| {
            panic!(
                "{needle} not seen after {count} heartbeats: {:?}",
                String::from_utf8_lossy(&self.pending)
            )
        })
    }

    /// Wait until everything the server queued for this session so far has arrived.
    ///
    /// The server refuses the unsupported telnet TIMING-MARK option with `WONT`, and that
    /// reply follows all earlier output through the session's ordered output queue. The
    /// reply itself is dropped so it never shows up in later text.
    pub async fn fence(&mut self) {
        const DO_TIMING_MARK: [u8; 3] = [255, 253, 6];
        const WONT_TIMING_MARK: [u8; 3] = [255, 252, 6];
        self.socket.write_all(&DO_TIMING_MARK).await.unwrap();
        tokio::time::timeout(Duration::from_secs(10), async {
            loop {
                if let Some(index) = self
                    .pending
                    .windows(WONT_TIMING_MARK.len())
                    .position(|bytes| bytes == WONT_TIMING_MARK)
                {
                    self.pending.drain(index..index + WONT_TIMING_MARK.len());
                    return;
                }
                let mut bytes = [0; 8192];
                let count = self.socket.read(&mut bytes).await.unwrap();
                assert!(
                    count > 0,
                    "closed waiting for fence: {:?}",
                    String::from_utf8_lossy(&self.pending)
                );
                self.pending.extend_from_slice(&bytes[..count]);
            }
        })
        .await
        .unwrap_or_else(|_| {
            panic!(
                "timeout waiting for fence: {:?}",
                String::from_utf8_lossy(&self.pending)
            )
        })
    }
}
