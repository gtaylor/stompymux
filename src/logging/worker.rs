//! One bounded output worker owns `@log`/`mux.log` append handles and expires them using
//! monotonic time. Server diagnostics go through `tracing`, not this worker.
use crate::config::Config;
use anyhow::{Context, Result, ensure};
use std::{
    collections::BTreeMap,
    fs::{File, OpenOptions},
    io::Write,
    path::PathBuf,
    sync::{
        Arc, Mutex, OnceLock,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};
use tokio::sync::{mpsc, oneshot};
const IDLE: Duration = Duration::from_secs(300);
/// C log buffer includes its terminating NUL; reserve the newline in the Rust record.
const MESSAGE_BYTES: usize = 4094;
#[derive(Clone, Debug)]
/// An already validated, bounded append request.
pub struct FileRequest {
    pub(crate) filename: String,
    pub(crate) message: Vec<u8>,
}
impl FileRequest {
    pub fn filename(&self) -> &str {
        &self.filename
    }

    pub fn message(&self) -> &[u8] {
        &self.message
    }

    pub fn new(filename: &str, message: &str) -> Result<Self> {
        Self::new_bytes(filename, message.as_bytes())
    }

    /// Construct a C-compatible request from a binary-safe Lua message.
    pub fn new_bytes(filename: &str, message: &[u8]) -> Result<Self> {
        ensure!(
            !filename.is_empty()
                && filename.len() <= 200
                && !filename.contains(['/', '\0'])
                && !filename.contains(".."),
            "Invalid logfile."
        );
        ensure!(
            !message.contains(&0),
            "Log message contains an embedded NUL byte"
        );
        let n = message.len().min(MESSAGE_BYTES);
        let mut bounded = Vec::with_capacity(n + 1);
        bounded.extend_from_slice(&message[..n]);
        bounded.push(b'\n');
        Ok(Self {
            filename: filename.into(),
            message: bounded,
        })
    }
}
/// Cloneable configuration dependency; no thread or file is opened until a write is requested.
#[derive(Clone, Debug, Default)]
pub struct Logger(Arc<Inner>);
#[derive(Debug, Default)]
struct Inner {
    sender: OnceLock<mpsc::Sender<Request>>,
    lost: Arc<AtomicU64>,
    stopped: AtomicBool,
    files: Arc<Mutex<BTreeMap<String, Instant>>>,
}
#[derive(Debug)]
enum Request {
    File(FileRequest, Instant, Option<oneshot::Sender<Result<()>>>),
    Stop(oneshot::Sender<()>),
}
struct Cached {
    file: File,
    last: Instant,
}
struct Cache {
    root: PathBuf,
    max: usize,
    files: BTreeMap<String, Cached>,
}
impl Cache {
    fn expire(&mut self, now: Instant) {
        self.files.retain(|_, f| now.duration_since(f.last) < IDLE);
    }
    fn write(&mut self, r: &FileRequest, now: Instant) -> Result<()> {
        self.expire(now);
        let root = std::fs::canonicalize(&self.root).context("Log directory unavailable")?;
        ensure!(
            !std::fs::symlink_metadata(&self.root)?
                .file_type()
                .is_symlink(),
            "Log directory must not be a symlink"
        );
        #[cfg(target_os = "linux")]
        let directory = {
            use std::os::unix::fs::OpenOptionsExt;
            const NOFOLLOW: i32 = 0o400000;
            const DIRECTORY: i32 = 0o200000;
            OpenOptions::new()
                .read(true)
                .custom_flags(NOFOLLOW | DIRECTORY)
                .open(&self.root)?
        };
        #[cfg(target_os = "linux")]
        let path = {
            use std::os::fd::AsRawFd;
            PathBuf::from(format!("/proc/self/fd/{}", directory.as_raw_fd())).join(&r.filename)
        };
        #[cfg(not(target_os = "linux"))]
        let path = root.join(&r.filename);
        let _ = root;
        let metadata = std::fs::symlink_metadata(&path).context("Logfile must already exist")?;
        ensure!(
            metadata.is_file() && !metadata.file_type().is_symlink(),
            "Logfile must be a regular file"
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = metadata.permissions().mode();
            ensure!(
                mode & 0o444 != 0 && mode & 0o222 != 0,
                "Logfile must be readable and writable"
            );
        }
        if !self.files.contains_key(&r.filename) {
            if self.files.len() >= self.max
                && let Some(key) = self
                    .files
                    .iter()
                    .min_by_key(|(_, f)| f.last)
                    .map(|(k, _)| k.clone())
            {
                self.files.remove(&key);
            }
            let mut options = OpenOptions::new();
            options.read(true).append(true);
            // Linux open flags prevent symlink and FIFO replacement races; this server targets Unix.
            #[cfg(target_os = "linux")]
            {
                use std::os::unix::fs::OpenOptionsExt;
                const NOFOLLOW: i32 = 0o400000;
                const NONBLOCK: i32 = 0o4000;
                options.custom_flags(NOFOLLOW | NONBLOCK);
            }
            let file = options.open(&path)?;
            ensure!(file.metadata()?.is_file(), "Logfile must be a regular file");
            #[cfg(unix)]
            {
                use std::os::unix::fs::MetadataExt;
                let opened = file.metadata()?;
                ensure!(
                    metadata.dev() == opened.dev() && metadata.ino() == opened.ino(),
                    "Logfile changed while opening"
                );
            }
            self.files
                .insert(r.filename.clone(), Cached { file, last: now });
        }
        let entry = self.files.get_mut(&r.filename).unwrap();
        if let Err(e) = entry.file.write_all(&r.message) {
            self.files.remove(&r.filename);
            return Err(e.into());
        }
        entry.last = now;
        Ok(())
    }
}
impl Logger {
    fn sender(&self, c: &Config) -> &mpsc::Sender<Request> {
        self.0.sender.get_or_init(|| {
            let (tx, mut rx) = mpsc::channel(c.runtime.event_queue_capacity);
            let lost = self.0.lost.clone();
            let files = self.0.files.clone();
            let mut cache = Cache {
                root: c.root.join("logs"),
                max: c.runtime.max_connections,
                files: BTreeMap::new(),
            };
            std::thread::Builder::new()
                .name("mux-logging".into())
                .spawn(move || {
                    loop {
                        // Polling also expires cache entries while no requests arrive.
                        let request = match rx.try_recv() {
                            Ok(r) => r,
                            Err(mpsc::error::TryRecvError::Empty) => {
                                cache.expire(Instant::now());
                                publish(&cache, &files);
                                std::thread::sleep(Duration::from_millis(20));
                                continue;
                            }
                            Err(_) => break,
                        };
                        let omitted = lost.swap(0, Ordering::Relaxed);
                        if omitted > 0 {
                            tracing::warn!(omitted, "script log requests omitted");
                        }
                        match request {
                            Request::File(r, deadline, reply) => {
                                let result = if Instant::now() > deadline {
                                    Err(anyhow::anyhow!("Log request expired before write"))
                                } else {
                                    cache.write(&r, Instant::now())
                                };
                                if let Err(error) = &result {
                                    tracing::error!(
                                        filename = ?r.filename,
                                        error = %error,
                                        "log file write failed"
                                    );
                                }
                                publish(&cache, &files);
                                if let Some(reply) = reply {
                                    let _ = reply.send(result);
                                }
                            }
                            Request::Stop(reply) => {
                                cache.files.clear();
                                publish(&cache, &files);
                                let _ = reply.send(());
                                break;
                            }
                        }
                        publish(&cache, &files);
                    }
                })
                .expect("start logging worker");
            tx
        })
    }
    /// Accepted script writes are best effort after commit, with explicit overload diagnostics.
    pub fn submit(&self, c: &Config, r: FileRequest) {
        let r_name = r.filename.clone();
        let deadline = Instant::now() + Duration::from_millis(c.runtime.write_timeout_ms);
        if self.0.stopped.load(Ordering::Acquire)
            || self
                .sender(c)
                .try_send(Request::File(r, deadline, None))
                .is_err()
        {
            self.0.lost.fetch_add(1, Ordering::Relaxed);
            tracing::warn!(
                filename = ?r_name,
                "script log request omitted: output queue unavailable"
            );
        }
    }
    /// Explicit @log waits for acknowledgment, never reports a timed-out write as successful.
    pub async fn write(&self, c: &Config, r: FileRequest) -> Result<()> {
        ensure!(
            !self.0.stopped.load(Ordering::Acquire),
            "Logging is stopped"
        );
        let deadline = Instant::now() + Duration::from_millis(c.runtime.write_timeout_ms);
        let (tx, rx) = oneshot::channel();
        tokio::time::timeout_at(deadline.into(), async {
            self.sender(c)
                .send(Request::File(r, deadline, Some(tx)))
                .await
                .context("Logging unavailable")?;
            rx.await.context("Logging worker stopped")?
        })
        .await
        .context("Log write timed out; outcome may be unknown")?
    }
    pub fn report(&self) -> String {
        let files = self.0.files.lock().unwrap();
        let now = Instant::now();
        let mut rows = vec!["/--------------------------- Open Logfiles".into()];
        if files.is_empty() {
            rows.push("- There are no open logfile handles.".into());
        } else {
            rows.push("Filename                               Timeout".into());
            rows.extend(files.iter().map(|(name, at)| {
                format!(
                    "{:<40}{}",
                    super::clean(name),
                    IDLE.saturating_sub(now.duration_since(*at)).as_secs()
                )
            }));
        }
        rows.join("\n")
    }
    pub async fn shutdown(&self, c: &Config) -> Result<()> {
        if self.0.stopped.swap(true, Ordering::AcqRel) {
            return Ok(());
        }
        let Some(sender) = self.0.sender.get() else {
            return Ok(());
        };
        let (tx, rx) = oneshot::channel();
        tokio::time::timeout(
            Duration::from_millis(c.runtime.shutdown_timeout_ms),
            async {
                sender
                    .send(Request::Stop(tx))
                    .await
                    .context("Logging worker stopped")?;
                rx.await.context("Logging worker stopped")
            },
        )
        .await
        .context("Logging shutdown did not drain before deadline")?
    }
}
fn publish(cache: &Cache, files: &Mutex<BTreeMap<String, Instant>>) {
    *files.lock().unwrap() = cache
        .files
        .iter()
        .map(|(n, f)| (n.clone(), f.last))
        .collect();
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cache_expiry_eviction_and_safe_paths() {
        let d = tempfile::tempdir().unwrap();
        for name in ["a", "b"] {
            std::fs::write(d.path().join(name), "").unwrap();
        }
        let mut cache = Cache {
            root: d.path().into(),
            max: 1,
            files: BTreeMap::new(),
        };
        let now = Instant::now();
        cache
            .write(&FileRequest::new("a", "one").unwrap(), now)
            .unwrap();
        cache
            .write(
                &FileRequest::new("b", "two").unwrap(),
                now + Duration::from_secs(1),
            )
            .unwrap();
        assert!(!cache.files.contains_key("a"));
        cache.expire(now + Duration::from_secs(300));
        assert!(cache.files.contains_key("b"));
        cache.expire(now + Duration::from_secs(301));
        assert!(cache.files.is_empty());
    }
    #[test]
    fn binary_file_request_preserves_bytes_and_truncates_before_newline() {
        let request = FileRequest::new_bytes("binary", &[0xff, b'x']).unwrap();
        assert_eq!(request.message, vec![0xff, b'x', b'\n']);
        let request = FileRequest::new_bytes("bounded", &vec![b'x'; MESSAGE_BYTES + 20]).unwrap();
        assert_eq!(request.message.len(), MESSAGE_BYTES + 1);
        assert_eq!(request.message.last(), Some(&b'\n'));
    }
    /// Missing creation-time macro defaults produce a contextual warning without blocking allocation.
    #[test]
    fn default_player_macros_warn_only_when_missing_at_creation() {
        let (capture, _guard) = crate::logging::Capture::install("warn");
        let d = tempfile::tempdir().unwrap();
        std::fs::write(d.path().join("stompymux.toml"), "").unwrap();
        let c = Config::load(d.path()).unwrap();
        let mut world = crate::World::default();
        let early = world.create(&c, "Early player".into(), crate::Kind::Player);
        let warnings = capture.lines_containing("default macro set not found");
        assert_eq!(warnings.len(), 1, "{}", capture.text());
        assert!(warnings[0].contains(&format!("player={}", early.0)));
        assert!(warnings[0].contains("set=0"));
        assert!(world.objects.contains_key(&early));
        world.macros.sets.push(crate::MacroSet {
            id: Default::default(),
            origin: Default::default(),
            owner: early,
            modes: Default::default(),
            description: "created by bootstrap".into(),
            entries: vec![],
        });
        let later = world.create(&c, "Later player".into(), crate::Kind::Player);
        assert_eq!(world.macros.players[&later].slots[0], Some(0));
        assert_eq!(capture.lines_containing("WARN").len(), 1);
    }

    #[tokio::test]
    async fn saturation_and_shutdown_deadline_are_bounded() {
        let d = tempfile::tempdir().unwrap();
        std::fs::write(
            d.path().join("stompymux.toml"),
            "[runtime]\nshutdown_timeout_ms=10\nwrite_timeout_ms=10\n",
        )
        .unwrap();
        let c = Config::load(d.path()).unwrap();
        let (tx, _rx) = mpsc::channel(1);
        c.logger.0.sender.set(tx).unwrap();
        c.logger
            .submit(&c, FileRequest::new("test", "one").unwrap());
        c.logger
            .submit(&c, FileRequest::new("test", "two").unwrap());
        assert_eq!(c.logger.0.lost.load(Ordering::Relaxed), 1);
        assert!(
            c.logger
                .write(&c, FileRequest::new("test", "blocked").unwrap())
                .await
                .is_err()
        );
        assert!(c.logger.shutdown(&c).await.is_err());
    }
}
