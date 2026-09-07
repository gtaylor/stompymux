//! Bounded MCCP2 output and live transport accounting, owned by the socket writer.
use flate2::{Compress, Compression, FlushCompress, Status};
use std::sync::atomic::{AtomicU8, AtomicU64, Ordering::Relaxed};
use tokio::io::{AsyncWrite, AsyncWriteExt};
/// Logical queue counters and actual socket bytes. Compression: 0 inactive, 1 starting, 2 active.
#[derive(Debug, Default)]
pub struct Stats {
    /// Accepted interactive commands; only the world owner increments this counter.
    pub commands: AtomicU64,
    pub input_total: AtomicU64,
    pub input_pending: AtomicU64,
    pub input_lost: AtomicU64,
    pub output_total: AtomicU64,
    pub output_pending: AtomicU64,
    pub output_lost: AtomicU64,
    pub wire_output: AtomicU64,
    pub compression: AtomicU8,
}
/// Immutable diagnostic view; counters are sampled, never reset by inspection.
#[derive(Debug, Default)]
pub struct Snapshot {
    pub input: [u64; 3],
    pub output: [u64; 3],
    pub wire_output: u64,
    pub compression: u8,
}
impl Stats {
    pub fn snapshot(&self) -> Snapshot {
        Snapshot {
            input: [
                self.input_pending.load(Relaxed),
                self.input_lost.load(Relaxed),
                self.input_total.load(Relaxed),
            ],
            output: [
                self.output_pending.load(Relaxed),
                self.output_lost.load(Relaxed),
                self.output_total.load(Relaxed),
            ],
            wire_output: self.wire_output.load(Relaxed),
            compression: self.compression.load(Relaxed),
        }
    }
    /// Saturation also permits direct server-input fixtures that bypass the socket task.
    pub fn consumed(&self, n: u64) {
        let _ = self
            .input_pending
            .fetch_update(Relaxed, Relaxed, |v| Some(v.saturating_sub(n)));
    }
}
/// One persistent zlib stream; compressed data is never converted back to plaintext midconnection.
#[derive(Default)]
pub struct Writer {
    compressor: Option<Compress>,
}
impl Writer {
    pub async fn start<W: AsyncWrite + Unpin>(
        &mut self,
        w: &mut W,
        s: &Stats,
    ) -> anyhow::Result<()> {
        if self.compressor.is_some() {
            return Ok(());
        }
        let compressor = Compress::new(Compression::default(), true);
        self.wire(w, s, &[255, 250, 86, 255, 240]).await?;
        self.compressor = Some(compressor);
        s.compression.store(2, Relaxed);
        Ok(())
    }
    async fn wire<W: AsyncWrite + Unpin>(
        &self,
        w: &mut W,
        s: &Stats,
        mut p: &[u8],
    ) -> anyhow::Result<()> {
        while !p.is_empty() {
            let n = w.write(p).await?;
            anyhow::ensure!(n > 0, "socket write returned zero");
            s.wire_output.fetch_add(n as u64, Relaxed);
            p = &p[n..];
        }
        Ok(())
    }
    pub async fn bytes<W: AsyncWrite + Unpin>(
        &mut self,
        w: &mut W,
        s: &Stats,
        p: &[u8],
    ) -> anyhow::Result<()> {
        if self.compressor.is_none() {
            return self.wire(w, s, p).await;
        }
        self.compress(w, s, p, FlushCompress::Sync).await
    }
    /// Fixed output buffer bounds expansion even for incompressible messages.
    async fn compress<W: AsyncWrite + Unpin>(
        &mut self,
        w: &mut W,
        s: &Stats,
        mut p: &[u8],
        flush: FlushCompress,
    ) -> anyhow::Result<()> {
        loop {
            let c = self.compressor.as_mut().unwrap();
            let before = (c.total_in(), c.total_out());
            let mut out = [0; 4096];
            let status = c.compress(p, &mut out, flush)?;
            let used = (c.total_in() - before.0) as usize;
            let n = (c.total_out() - before.1) as usize;
            p = &p[used..];
            self.wire(w, s, &out[..n]).await?;
            if status == Status::StreamEnd
                || (flush != FlushCompress::Finish && p.is_empty() && n < out.len())
            {
                break;
            }
            anyhow::ensure!(used + n > 0, "compression made no progress");
        }
        Ok(())
    }
    pub async fn finish<W: AsyncWrite + Unpin>(
        &mut self,
        w: &mut W,
        s: &Stats,
    ) -> anyhow::Result<()> {
        if self.compressor.is_some() {
            self.compress(w, s, &[], FlushCompress::Finish).await?;
        }
        Ok(())
    }
}
