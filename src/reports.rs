//! Bounded literal report construction, independent of per-message transport chunk sizes.
use anyhow::{Result, ensure};

const OMITTED: &str = "***Report truncated: additional results omitted***";

/// Reserve both completion and omission notices before accepting complete rows.
pub struct Report {
    body: String,
    footer: String,
    remaining: usize,
    truncated: bool,
    notice_fits: bool,
}
impl Report {
    /// Reserve footer and truncation bytes using encoded Telnet sizes.
    pub fn new(limit: usize, footer: &str) -> Result<Self> {
        let reserved = crate::telnet::encode(footer).len() + OMITTED.len() + 6;
        ensure!(
            limit >= crate::telnet::encode(footer).len() + 2,
            "Report output limit too small."
        );
        Ok(Self {
            body: String::new(),
            footer: footer.into(),
            remaining: limit.saturating_sub(reserved),
            truncated: false,
            notice_fits: limit >= reserved,
        })
    }
    /// Keep a prefix of complete rows. Client controls are rendered literally, never as markup.
    pub fn row(&mut self, row: &str) {
        let size = crate::telnet::encode(row).len() + 2;
        if self.truncated || size > self.remaining {
            self.truncated = true;
            return;
        }
        self.remaining -= size;
        self.body.push_str(row);
        self.body.push('\n');
    }
    /// Complete a report or fail if required truncation diagnostics cannot fit.
    pub fn finish(mut self) -> Result<String> {
        ensure!(
            !self.truncated || self.notice_fits,
            "Report output limit too small."
        );
        if self.truncated {
            self.body.push_str(OMITTED);
            self.body.push('\n');
        }
        self.body.push_str(&self.footer);
        Ok(self.body)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn completion_and_notice_reservations_include_wire_bytes() {
        let report = Report::new(5, "end").unwrap().finish().unwrap();
        assert_eq!(report, "end");
        let mut report = Report::new(5, "end").unwrap();
        report.row("oversized");
        assert!(report.finish().is_err());
        for limit in 70..180 {
            let mut report = Report::new(limit, "Done.").unwrap();
            for _ in 0..100 {
                report.row("é👩‍🚀 [literal] \r\n");
            }
            let text = report.finish().unwrap();
            assert!(text.contains("truncated") && text.ends_with("Done."));
            assert!(crate::telnet::encode(&format!("{text}\n")).len() <= limit);
        }
    }
}
