//! MUD eXtension Protocol (MXP): Telnet option 91, line-mode control, the
//! `<VERSION>`/`<SUPPORT>` handshake, and the secure tags the renderer emits.
//!
//! The server keeps every MXP connection in *locked* mode, so ordinary game
//! text is never parsed for tags and needs no entity escaping. Each tag the
//! server sends is preceded by the one-shot *temp secure* mode switch, which
//! parses exactly one tag and then returns the client to locked mode.
use std::collections::BTreeSet;

/// Telnet option number assigned to MXP.
pub const OPTION: u8 = 91;

/// Lock the client in locked mode: `<`, `>` and `&` in text are literal.
pub const LOCK_LOCKED: &str = "\x1b[7z";

/// Parse exactly the next tag securely, then return to the locked mode.
pub const TEMP_SECURE: &str = "\x1b[4z";

/// Prefix clients place before protocol replies such as `<VERSION>`.
pub const SECURE_LINE: &str = "\x1b[1z";

/// MXP tags are bounded to keep client-supplied lists small.
const MAX_SUPPORTED_TAGS: usize = 256;

/// Maximum retained bytes for one tag name, attribute name or attribute value.
const MAX_TOKEN_BYTES: usize = 63;

/// What a client reported about itself and the tags it accepts.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Client {
    /// `CLIENT` attribute of the `<VERSION>` reply.
    pub name: String,
    /// `VERSION` attribute of the `<VERSION>` reply: the client's own version.
    pub version: String,
    /// `MXP` attribute of the `<VERSION>` reply: the protocol version implemented.
    pub mxp_version: String,
    /// `REGISTERED` attribute of the `<VERSION>` reply, when supplied.
    pub registered: Option<bool>,
    /// Whether a `<VERSION>` reply has arrived.
    pub version_received: bool,
    /// Whether a `<SUPPORTS>` reply has arrived.
    pub supports_received: bool,
    /// Lower-cased tag and `tag.attribute` names the client advertised with `+`.
    pub supports: BTreeSet<String>,
}

impl Client {
    /// Whether the client advertised support for a tag or `tag.attribute` name.
    pub fn supports(&self, tag: &str) -> bool {
        self.supports.contains(&tag.to_ascii_lowercase())
    }

    /// Apply one protocol reply line (without its secure-line prefix).
    ///
    /// Unknown or malformed replies are ignored: they carry no authority and the
    /// connection keeps working without them.
    pub fn receive(&mut self, line: &str) -> Option<()> {
        let tag = Tag::parse(line)?;
        match tag.name.as_str() {
            "version" => {
                self.version_received = true;
                for (name, value) in &tag.attributes {
                    let value = value.as_deref().unwrap_or("");
                    match name.as_str() {
                        "client" => self.name = value.to_owned(),
                        "version" => self.version = value.to_owned(),
                        "mxp" => self.mxp_version = value.to_owned(),
                        "registered" => {
                            self.registered = Some(value.eq_ignore_ascii_case("yes"));
                        }
                        _ => {}
                    }
                }
            }
            "supports" => {
                self.supports_received = true;
                self.supports.clear();
                for (name, value) in &tag.attributes {
                    if value.is_some() {
                        continue;
                    }
                    if let Some(tag) = name.strip_prefix('+') {
                        if !tag.is_empty() && self.supports.len() < MAX_SUPPORTED_TAGS {
                            self.supports.insert(tag.to_owned());
                        }
                    } else if let Some(tag) = name.strip_prefix('-') {
                        self.supports.remove(tag);
                    }
                }
            }
            _ => return None,
        }
        Some(())
    }
}

/// One parsed MXP tag: a lower-cased name and its attributes in order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tag {
    pub name: String,
    /// Lower-cased attribute names with their unquoted values; bare flags have none.
    pub attributes: Vec<(String, Option<String>)>,
}

impl Tag {
    /// Parse `<NAME attr=value attr="quoted value" +flag>` from one reply line.
    pub fn parse(line: &str) -> Option<Self> {
        let inner = line.trim().strip_prefix('<')?.strip_suffix('>')?;
        let mut rest = inner.trim_start();
        let end = rest
            .find(|c: char| c.is_ascii_whitespace())
            .unwrap_or(rest.len());
        let name = bounded(&rest[..end]).to_ascii_lowercase();
        if name.is_empty() || !name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'!') {
            return None;
        }
        rest = rest[end..].trim_start();
        let mut attributes = Vec::new();
        while !rest.is_empty() {
            let end = rest
                .find(|c: char| c.is_ascii_whitespace() || c == '=')
                .unwrap_or(rest.len());
            if end == 0 {
                // A stray `=` or quote: skip the character to guarantee progress.
                rest = &rest[rest.chars().next().map_or(1, char::len_utf8)..];
                continue;
            }
            let attribute = bounded(&rest[..end]).to_ascii_lowercase();
            rest = &rest[end..];
            let mut value = None;
            if let Some(tail) = rest.strip_prefix('=') {
                rest = tail;
                let (parsed, tail) = match rest.chars().next() {
                    Some(quote @ ('"' | '\'')) => {
                        let body = &rest[1..];
                        match body.find(quote) {
                            Some(close) => (&body[..close], &body[close + 1..]),
                            None => (body, ""),
                        }
                    }
                    _ => {
                        let end = rest
                            .find(|c: char| c.is_ascii_whitespace())
                            .unwrap_or(rest.len());
                        (&rest[..end], &rest[end..])
                    }
                };
                value = Some(bounded(parsed).to_owned());
                rest = tail;
            }
            attributes.push((attribute, value));
            rest = rest.trim_start();
        }
        Some(Self { name, attributes })
    }
}

/// Keep at most the retained token size without splitting a UTF-8 character.
fn bounded(s: &str) -> &str {
    if s.len() <= MAX_TOKEN_BYTES {
        return s;
    }
    let mut end = MAX_TOKEN_BYTES;
    while !s.is_char_boundary(end) {
        end -= 1;
    }
    &s[..end]
}

/// Bytes sent once the client agrees to MXP: lock the client in locked mode,
/// then ask what it is and which tags it accepts.
pub fn activation() -> Vec<u8> {
    format!(
        "{LOCK_LOCKED}{}{}",
        secure_tag("<VERSION>"),
        secure_tag("<SUPPORT>")
    )
    .into_bytes()
}

/// Wrap one complete tag so the client parses it securely and then relocks.
pub fn secure_tag(tag: &str) -> String {
    format!("{TEMP_SECURE}{tag}")
}

/// Quote an attribute value, encoding the characters that would end or
/// confuse the quoted value. Control characters are dropped.
pub fn quote(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 2);
    out.push('"');
    for c in value.chars() {
        match c {
            '"' => out.push_str("&quot;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            c if c.is_control() => {}
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// Whether the reply line a client sent is an MXP protocol reply rather than a command.
pub fn reply_body(line: &str) -> Option<&str> {
    let body = line.strip_prefix(SECURE_LINE)?;
    body.trim_start().starts_with('<').then_some(body)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_and_supports_replies_populate_client_state() {
        let mut client = Client::default();
        assert!(
            client
                .receive("<VERSION MXP=1.0 CLIENT=Mudlet VERSION=\"4.17.2\" REGISTERED=YES>")
                .is_some()
        );
        assert_eq!(client.name, "Mudlet");
        assert_eq!(client.version, "4.17.2");
        assert_eq!(client.mxp_version, "1.0");
        assert_eq!(client.registered, Some(true));
        assert!(client.version_received);
        assert!(!client.supports_received);
        client
            .receive("<SUPPORTS +image +image.url +send -frame +COLOR>")
            .unwrap();
        assert!(client.supports_received);
        assert!(client.supports("IMAGE"));
        assert!(client.supports("image.url"));
        assert!(client.supports("color"));
        assert!(!client.supports("frame"));
        client.receive("<SUPPORTS +send>").unwrap();
        assert!(!client.supports("image"));
        assert!(client.supports("send"));
    }

    #[test]
    fn malformed_and_unknown_replies_are_ignored() {
        let mut client = Client::default();
        for line in [
            "",
            "<",
            "VERSION",
            "<>",
            "<!ELEMENT x>",
            "<unknown a=b>",
            "<ve rsion>",
        ] {
            assert!(client.receive(line).is_none(), "{line:?}");
        }
        assert_eq!(client, Client::default());
    }

    #[test]
    fn tag_parser_handles_quotes_flags_and_bounds() {
        let tag = Tag::parse("<Image  fn='a b' URL=\"http://x/\" ISMAP x==y>").unwrap();
        assert_eq!(tag.name, "image");
        assert_eq!(
            tag.attributes,
            vec![
                ("fn".into(), Some("a b".into())),
                ("url".into(), Some("http://x/".into())),
                ("ismap".into(), None),
                ("x".into(), Some("=y".into())),
            ]
        );
        let long = format!("<SUPPORTS +{}>", "é".repeat(100));
        let tag = Tag::parse(&long).unwrap();
        assert!(tag.attributes[0].0.len() <= MAX_TOKEN_BYTES + 1);
        assert!(Tag::parse("<SUPPORTS +a=\"unterminated>").is_some());
        let mut client = Client::default();
        let many: String = (0..400).map(|n| format!(" +t{n}")).collect();
        client.receive(&format!("<SUPPORTS{many}>")).unwrap();
        assert_eq!(client.supports.len(), MAX_SUPPORTED_TAGS);
    }

    #[test]
    fn activation_locks_then_queries_and_values_are_quoted() {
        assert_eq!(
            activation(),
            b"\x1b[7z\x1b[4z<VERSION>\x1b[4z<SUPPORT>".to_vec()
        );
        assert_eq!(quote("a\"b<c>\u{7}d"), "\"a&quot;b&lt;c&gt;d\"");
        assert_eq!(reply_body("\x1b[1z<VERSION>"), Some("<VERSION>"));
        assert_eq!(reply_body("\x1b[1z look"), None);
        assert_eq!(reply_body("<VERSION>"), None);
    }
}
