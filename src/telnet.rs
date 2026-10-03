//! Incremental Telnet decoding. Negotiation never enters the command stream.
use anyhow::{Result, bail};
pub mod diagnostics;
pub mod environment;
pub mod q;
pub mod transport;
use q::{Negotiator, Side, Verb};
/// Supported option numbers and subnegotiation framing bytes.
pub const TTYPE: u8 = 24;
pub const NAWS: u8 = 31;
pub const CHARSET: u8 = 42;
pub const NEW_ENVIRON: u8 = 39;
pub const MSSP: u8 = 70;
pub const MCCP2: u8 = 86;
pub const GMCP: u8 = 201;
const SB: u8 = 250;
const SE: u8 = 240;
/// C telnet_handler.c TELNET_OPTIONS: unsolicited acceptance is direction-specific.
const OPTIONS: &[(u8, Side)] = &[
    (TTYPE, Side::Remote),
    (NAWS, Side::Remote),
    (NEW_ENVIRON, Side::Remote),
    (MSSP, Side::Local),
    (MCCP2, Side::Local),
    (CHARSET, Side::Local),
    (GMCP, Side::Local),
];
pub const IAC: u8 = 255;
pub const ECHO: u8 = 1;
/// C's 64-byte NUL-terminated client and terminal buffers retain 63 payload bytes.
const TTYPE_TEXT_BYTES: usize = 63;
#[derive(Debug, Clone)]
enum State {
    Data,
    Iac,
    Option(u8),
    SubOption,
    Sub(u8, Vec<u8>, bool),
    /// Drain an oversized environment message without retaining its payload.
    DiscardSub(bool),
}
#[derive(Debug)]
pub enum Input {
    Line(String),
    Reply(Vec<u8>),
    InvalidUtf8,
    /// Writer control and world-owner status requests.
    StartCompression,
    StatusRequest,
    /// A peer protocol problem: the option involved and a description.
    Problem(&'static str, String),
    /// Negotiation state changed; option effects have already been applied.
    Negotiated(q::Change),
}
#[derive(Debug, Clone)]
pub struct Decoder {
    state: State,
    negotiation: Negotiator,
    initialized: bool,
    pub ttype_responses: u64,
    pub charset_pending: bool,
    pub environment: environment::Environment,
    pub client: String,
    /// Exact bounded C-string bytes retained for escaped diagnostics.
    pub client_raw: Vec<u8>,
    pub color_depth: u16,
    pub screen_reader: bool,
    pub echo_suppressed: bool,
    /// Result of CHARSET negotiation; input remains UTF-8 regardless.
    pub charset_utf8: bool,
    line: Vec<u8>,
    discarded: u64,
    after_cr: bool,
    input_line_limit: usize,
    subnegotiation_limit: usize,
    pub width: u16,
    pub height: u16,
    pub ansi: bool,
    pub terminal: String,
    /// Exact bounded C-string bytes retained for escaped diagnostics.
    pub terminal_raw: Vec<u8>,
}
impl Default for Decoder {
    fn default() -> Self {
        Self::new(&crate::config::RuntimeConfig::default())
    }
}
impl Decoder {
    pub fn new(config: &crate::config::RuntimeConfig) -> Self {
        Self {
            state: State::Data,
            negotiation: Negotiator::default(),
            initialized: false,
            ttype_responses: 0,
            charset_pending: false,
            environment: Default::default(),
            client: String::new(),
            client_raw: Vec::new(),
            color_depth: 16,
            screen_reader: false,
            echo_suppressed: false,
            charset_utf8: true,
            line: Vec::new(),
            discarded: 0,
            after_cr: false,
            width: 80,
            height: 25,
            ansi: true,
            terminal: "vt100".into(),
            terminal_raw: b"vt100".to_vec(),
            input_line_limit: config.input_line_limit,
            subnegotiation_limit: config.telnet_subnegotiation_limit,
        }
    }
    /// Begin supported negotiations once, using the C server's option directions.
    pub fn initial(&mut self) -> Vec<Input> {
        if self.initialized {
            return Vec::new();
        }
        self.initialized = true;
        let mut out = Vec::new();
        for &(option, side) in OPTIONS {
            out.extend(self.negotiate(option, side, true));
        }
        out
    }
    /// Application echo requests are explicit stimuli; the Q engine handles repeats and reversals.
    pub fn echo(&mut self, secret: bool) -> Vec<Input> {
        self.echo_suppressed = secret;
        self.negotiate(ECHO, Side::Local, secret)
    }
    /// Query either endpoint's current option state.
    pub fn option_state(&self, option: u8, side: Side) -> q::QState {
        self.negotiation.state(option, side)
    }
    /// Request supported behavior; ECHO is server-initiated only, as in the C handler.
    pub fn negotiate(&mut self, option: u8, side: Side, enable: bool) -> Vec<Input> {
        if enable && !OPTIONS.contains(&(option, side)) && (option, side) != (ECHO, Side::Local) {
            return Vec::new();
        }
        let events = self.negotiation.request(option, side, enable);
        self.effects(events)
    }
    /// Apply option effects only on real transitions, after negotiation output.
    fn effects(&mut self, events: Vec<q::Event>) -> Vec<Input> {
        let mut out = Vec::new();
        for event in events {
            match event {
                q::Event::Send(verb, option) => {
                    out.push(Input::Reply(vec![IAC, verb as u8, option]))
                }
                q::Event::Changed(change) => {
                    out.push(Input::Negotiated(change));
                    if change.after.enabled() && !change.before.enabled() {
                        match (change.option, change.side) {
                            (NEW_ENVIRON, Side::Remote) => {
                                self.environment = Default::default();
                                out.push(Self::sub_reply(NEW_ENVIRON, &[1]));
                            }
                            (MSSP, Side::Local) => out.push(Input::StatusRequest),
                            (MCCP2, Side::Local) => out.push(Input::StartCompression),
                            (TTYPE, Side::Remote) => out.push(Self::sub_reply(TTYPE, &[1])),
                            (CHARSET, Side::Local) => {
                                self.charset_pending = true;
                                out.push(Self::sub_reply(CHARSET, b"\x01;UTF-8"));
                            }
                            _ => {}
                        }
                    } else if !change.after.enabled() {
                        match (change.option, change.side) {
                            (TTYPE, Side::Remote) => {
                                self.terminal = "vt100".into();
                                self.terminal_raw = b"vt100".to_vec();
                            }
                            (NAWS, Side::Remote) => {
                                self.width = 80;
                                self.height = 25;
                            }
                            (CHARSET, Side::Local) => self.charset_pending = false,
                            (NEW_ENVIRON, Side::Remote) => self.environment = Default::default(),
                            _ => {}
                        }
                    }
                }
            }
        }
        out
    }
    /// Encode bounded option payloads, escaping IAC independently of text encoding.
    pub fn sub_reply(option: u8, payload: &[u8]) -> Input {
        let mut bytes = vec![IAC, SB, option];
        for &b in payload {
            bytes.push(b);
            if b == IAC {
                bytes.push(b);
            }
        }
        bytes.extend([IAC, SE]);
        Input::Reply(bytes)
    }
    /// Decode one byte so the session owner can act on a line before later negotiations.
    pub fn feed_byte(&mut self, byte: u8) -> Result<Vec<Input>> {
        self.feed(&[byte])
    }
    /// Bulk convenience API for callers without interleaved application actions.
    pub fn feed(&mut self, bytes: &[u8]) -> Result<Vec<Input>> {
        let mut out = Vec::new();
        for &b in bytes {
            let state = std::mem::replace(&mut self.state, State::Data);
            match state {
                State::Data => {
                    if b == IAC {
                        self.state = State::Iac;
                    } else {
                        self.data(b, &mut out)?;
                    }
                }
                State::Iac => match b {
                    IAC => self.data(b, &mut out)?,
                    251..=254 => self.state = State::Option(b),
                    250 => self.state = State::SubOption,
                    247 => {
                        self.erase_character();
                    }
                    248 => self.line.clear(),
                    _ => {}
                },
                State::Option(cmd) => {
                    let verb = Verb::from_byte(cmd).expect("framing validated verb");
                    let side = if matches!(verb, Verb::Will | Verb::Wont) {
                        Side::Remote
                    } else {
                        Side::Local
                    };
                    let events = self
                        .negotiation
                        .receive(b, verb, OPTIONS.contains(&(b, side)));
                    out.extend(self.effects(events));
                }
                State::DiscardSub(escaped) => {
                    if !(escaped && b == SE) {
                        self.state = State::DiscardSub(!escaped && b == IAC);
                    }
                }
                State::SubOption => self.state = State::Sub(b, Vec::new(), false),
                State::Sub(option, mut payload, escaped) => {
                    if escaped && b == 240 {
                        self.sub(option, &payload, &mut out);
                    } else if !escaped && b == IAC {
                        self.state = State::Sub(option, payload, true);
                    } else {
                        if !escaped || b == IAC {
                            payload.push(b);
                        }
                        if payload.len() > self.subnegotiation_limit {
                            if option == NEW_ENVIRON {
                                out.push(Input::Problem(
                                    "ENVIRON",
                                    "sent an invalid or oversized NEW-ENVIRON update".into(),
                                ));
                                self.state = State::DiscardSub(false);
                                continue;
                            }
                            bail!("Telnet subnegotiation too long");
                        }
                        self.state = State::Sub(option, payload, false);
                    }
                }
            }
        }
        Ok(out)
    }
    /// Buffered text awaiting a line terminator.
    /// Drain invalid/oversized decoded input accounting into the session counters.
    pub fn take_discarded(&mut self) -> u64 {
        std::mem::take(&mut self.discarded)
    }
    pub fn pending_text(&self) -> usize {
        self.line.len()
    }
    fn erase_character(&mut self) {
        while let Some(byte) = self.line.pop() {
            if byte & 0xc0 != 0x80 {
                break;
            }
        }
    }
    fn data(&mut self, b: u8, out: &mut Vec<Input>) -> Result<()> {
        if self.after_cr {
            self.after_cr = false;
            if b == b'\n' || b == 0 {
                return Ok(());
            }
        }
        match b {
            b'\r' | b'\n' => {
                let line = std::mem::take(&mut self.line);
                out.push(match String::from_utf8(line) {
                    Ok(s) => Input::Line(s),
                    Err(e) => {
                        self.discarded += e.as_bytes().len() as u64;
                        Input::InvalidUtf8
                    }
                });
                self.after_cr = b == b'\r';
            }
            8 | 127 => {
                self.erase_character();
            }
            0 => {}
            _ => {
                self.line.push(b);
                if self.line.len() > self.input_line_limit {
                    self.discarded += self.line.len() as u64;
                    self.line.clear();
                    bail!("input line exceeds {} bytes", self.input_line_limit);
                }
            }
        }
        Ok(())
    }
    fn sub(&mut self, option: u8, p: &[u8], out: &mut Vec<Input>) {
        match option {
            NEW_ENVIRON if self.option_state(NEW_ENVIRON, Side::Remote).enabled() => {
                if let Err(e) = self.environment.update(p) {
                    out.push(Input::Problem(
                        "ENVIRON",
                        format!("sent an invalid or oversized NEW-ENVIRON update: {e}"),
                    ));
                }
            }
            GMCP if self.option_state(GMCP, Side::Local).enabled()
                && (p == b"Core.Ping" || p.starts_with(b"Core.Ping ")) =>
            {
                out.push(Self::sub_reply(GMCP, b"Core.Ping"))
            }
            NAWS if self.option_state(NAWS, Side::Remote).enabled() && p.len() == 4 => {
                self.width = u16::from_be_bytes([p[0], p[1]]);
                self.height = u16::from_be_bytes([p[2], p[3]]);
            }
            TTYPE if self.option_state(TTYPE, Side::Remote).enabled() && p.first() == Some(&0) => {
                // libtelnet exposes a C string: embedded NUL ends parsing and display storage.
                let name = p[1..].split(|byte| *byte == 0).next().unwrap_or_default();
                let bits = mtts_bits(name);
                if let Some(bits) = bits {
                    self.screen_reader = bits & 64 != 0;
                    self.color_depth = if bits & 256 != 0 {
                        24
                    } else if bits & 8 != 0 {
                        256
                    } else if bits & 1 != 0 {
                        16
                    } else {
                        0
                    };
                } else {
                    let displayed = &name[..name.len().min(TTYPE_TEXT_BYTES)];
                    if self.ttype_responses == 0 {
                        self.client_raw = displayed.to_vec();
                        self.client = String::from_utf8_lossy(displayed).into_owned();
                    }
                    self.terminal_raw = displayed.to_vec();
                    self.terminal = String::from_utf8_lossy(displayed).into_owned();
                    // Capability inference sees the full original C string before display truncation.
                    self.color_depth = if contains_ascii_case_insensitive(name, b"TRUECOLOR") {
                        24
                    } else if contains_ascii_case_insensitive(name, b"256COLOR")
                        || name.eq_ignore_ascii_case(b"XTERM")
                    {
                        256
                    } else if name.eq_ignore_ascii_case(b"DUMB") {
                        0
                    } else {
                        16
                    };
                }
                self.ansi = self.color_depth != 0 && !self.screen_reader;
                // C handler counts responses across the connection, including re-negotiations.
                self.ttype_responses = self.ttype_responses.saturating_add(1);
                if self.ttype_responses < 3 {
                    out.push(Self::sub_reply(TTYPE, &[1]));
                }
            }
            CHARSET if self.option_state(CHARSET, Side::Local).enabled() && !p.is_empty() => {
                match p[0] {
                    2 => {
                        self.charset_pending = false;
                        self.charset_utf8 = p[1..].eq_ignore_ascii_case(b"UTF-8");
                        if !self.charset_utf8 {
                            out.push(Input::Problem(
                                "CHARSET",
                                "accepted unsupported charset".into(),
                            ));
                        }
                    }
                    3 => self.charset_pending = false,
                    1 if p.len() > 2 && !self.charset_pending => {
                        let accepted = p[2..]
                            .split(|b| *b == p[1])
                            .any(|s| s.eq_ignore_ascii_case(b"UTF-8"));
                        if accepted {
                            self.charset_utf8 = true;
                        }
                        out.push(Self::sub_reply(
                            CHARSET,
                            if accepted { b"\x02UTF-8" } else { b"\x03" },
                        ));
                    }
                    _ => out.push(Self::sub_reply(CHARSET, &[3])),
                }
            }
            _ => {}
        }
    }
}

/// Parse the deliberately permissive nonnegative `strtol` form used by C MTTS.
fn mtts_bits(name: &[u8]) -> Option<i64> {
    if name.len() < 5 || !name[..5].eq_ignore_ascii_case(b"MTTS ") {
        return None;
    }
    let suffix = &name[5..];
    if suffix.is_empty() {
        return Some(0);
    }
    let mut at = suffix
        .iter()
        .position(|byte| !matches!(byte, b'\t'..=b'\r' | b' '))
        .unwrap_or(0);
    if suffix
        .iter()
        .all(|byte| matches!(byte, b'\t'..=b'\r' | b' '))
    {
        return None;
    }
    let negative = match suffix[at] {
        b'+' => {
            at += 1;
            false
        }
        b'-' => {
            at += 1;
            true
        }
        _ => false,
    };
    let start = at;
    let mut value = 0i64;
    while at < suffix.len() && suffix[at].is_ascii_digit() {
        value = value
            .checked_mul(10)?
            .checked_add(i64::from(suffix[at] - b'0'))?;
        at += 1;
    }
    if at == start || at != suffix.len() || (negative && value != 0) {
        None
    } else {
        Some(value)
    }
}

fn contains_ascii_case_insensitive(haystack: &[u8], needle: &[u8]) -> bool {
    haystack
        .windows(needle.len())
        .any(|window| window.eq_ignore_ascii_case(needle))
}
pub fn encode(text: &str) -> Vec<u8> {
    let normalized = text.replace("\r\n", "\n").replace('\n', "\r\n");
    let mut out = Vec::new();
    for b in normalized.bytes() {
        out.push(b);
        if b == IAC {
            out.push(b);
        }
    }
    out
}

/// Encode a line within even very small output limits without splitting UTF-8.
pub fn bounded_error(message: &str, limit: usize) -> Vec<u8> {
    let encoded = encode(&format!("{message}\r\n"));
    let mut end = encoded.len().min(limit);
    while end > 0 && std::str::from_utf8(&encoded[..end]).is_err() {
        end -= 1;
    }
    encoded[..end].to_vec()
}
