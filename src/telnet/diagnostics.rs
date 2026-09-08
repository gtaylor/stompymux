//! Bounded, escaped protocol diagnostics; no client-supplied bytes become terminal controls.
use super::{Decoder, q::Side, transport::Snapshot};
/// Escape bytes exactly as C's telnet_append_escaped, including non-ASCII UTF-8 bytes.
pub fn escape(bytes: &[u8]) -> String {
    let mut s = String::new();
    for &b in bytes {
        match b {
            b'\\' => s.push_str("\\\\"),
            b'"' => s.push_str("\\\""),
            0x20..=0x7e => s.push(b as char),
            _ => s.push_str(&format!("\\x{b:02X}")),
        }
    }
    s
}
/// Reserve a truncation notice and enforce the encoded message limit during construction.
pub struct Report {
    text: String,
    limit: usize,
    truncated: bool,
}
impl Report {
    pub fn new(limit: usize) -> Self {
        Self {
            text: String::new(),
            limit,
            truncated: false,
        }
    }
    pub fn line(&mut self, s: &str) {
        if self.truncated {
            return;
        }
        if self.text.len() + s.len() + 2 > self.limit.saturating_sub(24) {
            self.truncated = true;
            return;
        }
        self.text.push_str(s);
        self.text.push_str("\r\n");
    }
    pub fn full(&self) -> bool {
        self.truncated
    }
    pub fn finish(mut self) -> Vec<u8> {
        if self.truncated {
            self.text.push_str("***Output truncated***\r\n");
        }
        super::bounded_error(self.text.trim_end_matches(['\r', '\n']), self.limit)
    }
}
/// Render live protocol state and a sampled writer state for one session.
pub fn telnet(r: &mut Report, name: &str, player: i64, id: u64, d: &Decoder, t: &Snapshot) {
    r.line(&format!(
        "Telnet state for {}(#{player}), session {id}:",
        escape(name.as_bytes())
    ));
    for (option, label, side) in [
        (super::TTYPE, "TTYPE / MTTS", Side::Remote),
        (super::NAWS, "NAWS", Side::Remote),
        (super::CHARSET, "CHARSET", Side::Local),
        (super::NEW_ENVIRON, "NEW-ENVIRON", Side::Remote),
        (super::GMCP, "GMCP", Side::Local),
        (super::MSSP, "MSSP", Side::Local),
        (super::MCCP2, "MCCP2", Side::Local),
        (super::ECHO, "ECHO", Side::Local),
    ] {
        r.line(&format!("  {label}:"));
        r.line(&format!(
            "    Negotiated: {}",
            if d.option_state(option, side).enabled() {
                "yes"
            } else {
                "no"
            }
        ));
        r.line(&format!(
            "    Q local: {:?}; remote: {:?}",
            d.option_state(option, Side::Local),
            d.option_state(option, Side::Remote)
        ));
        match option {
            super::TTYPE => {
                r.line(&format!("    Client: \"{}\"", escape(&d.client_raw)));
                r.line(&format!(
                    "    Terminal type: \"{}\"",
                    escape(&d.terminal_raw)
                ));
                r.line(&format!("    Responses: {}", d.ttype_responses));
                r.line(&format!(
                    "    Color depth (advertised): {}",
                    match d.color_depth {
                        0 => "off",
                        24 => "truecolor",
                        256 => "256",
                        _ => "16",
                    }
                ));
                r.line(&format!(
                    "    Screen reader: {}",
                    if d.screen_reader { "yes" } else { "no" }
                ));
            }
            super::NAWS => r.line(&format!("    Window size: {}x{}", d.width, d.height)),
            super::CHARSET => {
                r.line(&format!(
                    "    Encoding: {}",
                    if d.charset_utf8 {
                        "UTF-8"
                    } else {
                        "unsupported"
                    }
                ));
                r.line(&format!(
                    "    Request pending: {}",
                    if d.charset_pending { "yes" } else { "no" }
                ));
            }
            super::NEW_ENVIRON => {
                r.line(if d.environment.0.is_empty() {
                    "    Variables: (none)"
                } else {
                    "    Variables:"
                });
                for ((kind, name), value) in &d.environment.0 {
                    if r.full() {
                        break;
                    }
                    r.line(&format!(
                        "      {} \"{}\" = \"{}\"",
                        if *kind == super::environment::Kind::Var {
                            "VAR"
                        } else {
                            "USERVAR"
                        },
                        escape(name),
                        escape(value)
                    ));
                }
            }
            super::MCCP2 => {
                r.line(&format!(
                    "    Compression: {}",
                    match t.compression {
                        0 => "inactive",
                        1 => "starting",
                        _ => "active",
                    }
                ));
                r.line(&format!("    Wire output bytes: {}", t.wire_output));
            }
            super::ECHO => r.line(&format!(
                "    Client echo (requested): {}",
                if d.echo_suppressed {
                    "suppressed"
                } else {
                    "enabled"
                }
            )),
            _ => {}
        }
    }
}

/// C connection display: days and HH:MM, without losing long uptimes.
pub fn connected_time(seconds: u64) -> String {
    let clock = format!("{:02}:{:02}", (seconds / 3600) % 24, (seconds / 60) % 60);
    if seconds >= 86400 {
        format!("{}d {clock}", seconds / 86400)
    } else {
        clock
    }
}
/// C connection display uses the largest nonzero day/hour/minute unit, then seconds.
pub fn idle_time(seconds: u64) -> String {
    if seconds >= 86400 {
        format!("{}d", seconds / 86400)
    } else if seconds >= 3600 {
        format!("{}h", seconds / 3600)
    } else if seconds >= 60 {
        format!("{}m", seconds / 60)
    } else {
        format!("{seconds}s")
    }
}
