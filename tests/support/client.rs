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

    /// Consume through a marker that only a later server heartbeat produces, driving each
    /// heartbeat with [`crate::attempt_heartbeat`] instead of waiting out real seconds.
    ///
    /// Between attempts this reads only bytes that have already arrived, so the paused
    /// clock is always resumed before the next read. `heartbeats` bounds the attempts.
    pub async fn until_heartbeats(&mut self, needle: &str, heartbeats: usize) -> String {
        for _ in 0..heartbeats {
            self.read_available();
            if let Some(text) = self.take(needle) {
                return text;
            }
            crate::attempt_heartbeat().await;
        }
        self.read_available();
        self.take(needle).unwrap_or_else(|| {
            panic!(
                "{needle} not seen after {heartbeats} heartbeats: {:?}",
                String::from_utf8_lossy(&self.pending)
            )
        })
    }

    /// Append every byte the socket already holds without waiting for more.
    fn read_available(&mut self) {
        let mut bytes = [0; 8192];
        loop {
            match self.socket.try_read(&mut bytes) {
                Ok(0) => panic!(
                    "closed while reading: {:?}",
                    String::from_utf8_lossy(&self.pending)
                ),
                Ok(count) => self.pending.extend_from_slice(&bytes[..count]),
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => return,
                Err(error) => panic!("socket read failed: {error}"),
            }
        }
    }
}
