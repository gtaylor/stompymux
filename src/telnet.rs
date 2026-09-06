//! Incremental Telnet decoding. Negotiation never enters the command stream.
use anyhow::{Result, bail};
pub const IAC: u8 = 255;
pub const ECHO: u8 = 1;
#[derive(Debug, Clone)]
enum State {
    Data,
    Iac,
    Option(u8),
    SubOption,
    Sub(u8, Vec<u8>, bool),
}
#[derive(Debug)]
pub enum Input {
    Line(String),
    Reply(Vec<u8>),
    InvalidUtf8,
}
#[derive(Debug, Clone)]
pub struct Decoder {
    state: State,
    line: Vec<u8>,
    after_cr: bool,
    input_line_limit: usize,
    subnegotiation_limit: usize,
    pub width: u16,
    pub height: u16,
    pub ansi: bool,
    pub terminal: String,
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
            line: Vec::new(),
            after_cr: false,
            width: 80,
            height: 25,
            ansi: true,
            terminal: "vt100".into(),
            input_line_limit: config.input_line_limit,
            subnegotiation_limit: config.telnet_subnegotiation_limit,
        }
    }
    pub fn initial() -> Vec<u8> {
        vec![IAC, 253, 24, IAC, 253, 31, IAC, 253, 42]
    }
    pub fn echo(secret: bool) -> Vec<u8> {
        vec![IAC, if secret { 251 } else { 252 }, ECHO]
    }
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
                State::Option(cmd) => match (cmd, b) {
                    (251, 24) => out.push(Input::Reply(vec![IAC, 250, 24, 1, IAC, 240])),
                    (251, 31) => {}
                    (251, 42) => out.push(Input::Reply(vec![
                        IAC, 250, 42, 1, b';', b'U', b'T', b'F', b'-', b'8', IAC, 240,
                    ])),
                    (252, 31) => {
                        self.width = 80;
                        self.height = 25;
                    }
                    (253, 1) => {}
                    (251, _) => out.push(Input::Reply(vec![IAC, 254, b])),
                    (253, _) => out.push(Input::Reply(vec![IAC, 252, b])),
                    _ => {}
                },
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
                            bail!("Telnet subnegotiation too long");
                        }
                        self.state = State::Sub(option, payload, false);
                    }
                }
            }
        }
        Ok(out)
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
                    Err(_) => Input::InvalidUtf8,
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
                    bail!("input line exceeds {} bytes", self.input_line_limit);
                }
            }
        }
        Ok(())
    }
    fn sub(&mut self, option: u8, p: &[u8], out: &mut Vec<Input>) {
        match option {
            31 if p.len() == 4 => {
                self.width = u16::from_be_bytes([p[0], p[1]]);
                self.height = u16::from_be_bytes([p[2], p[3]]);
            }
            24 if p.first() == Some(&0) => {
                self.terminal = String::from_utf8_lossy(&p[1..]).into_owned();
                self.ansi = !self.terminal.eq_ignore_ascii_case("DUMB");
                if let Some(bits) = self
                    .terminal
                    .strip_prefix("MTTS ")
                    .and_then(|s| s.parse::<u32>().ok())
                {
                    self.ansi = bits & 64 == 0 && bits & 1 != 0;
                }
            }
            42 if p.first() == Some(&1) && p.len() > 2 => {
                let accepted = p[2..]
                    .split(|b| *b == p[1])
                    .any(|s| s.eq_ignore_ascii_case(b"UTF-8"));
                let mut reply = vec![IAC, 250, 42, if accepted { 2 } else { 3 }];
                if accepted {
                    reply.extend(b"UTF-8");
                }
                reply.extend([IAC, 240]);
                out.push(Input::Reply(reply));
            }
            _ => {}
        }
    }
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
