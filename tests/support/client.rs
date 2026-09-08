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

    /// Consume through an expected marker, retaining any following bytes.
    pub async fn until(&mut self, needle: &str) -> String {
        tokio::time::timeout(Duration::from_secs(10), async {
            loop {
                if let Some(index) = self
                    .pending
                    .windows(needle.len())
                    .position(|b| b == needle.as_bytes())
                {
                    let bytes = self
                        .pending
                        .drain(..index + needle.len())
                        .collect::<Vec<_>>();
                    return String::from_utf8_lossy(&bytes).into_owned();
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
}
