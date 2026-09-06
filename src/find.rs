//! Read-only, session-scoped MUX object searches with bounded Telnet pages.
use crate::{
    flags, telnet, text,
    world::{Kind, Object, ObjectId, World},
};

/// Parsed search operation; switches are validated before execution.
#[derive(Debug)]
pub enum FindRequest {
    /// Start a search with legacy name/range syntax.
    Search(String),
    /// Continue the session's pending search.
    Next,
    /// A syntax error to report only to the invoking session.
    Error(String),
}
impl FindRequest {
    /// Interpret the supported continuation switch and reject extraneous arguments.
    pub fn parse(args: &str, switch: Option<&str>) -> Self {
        match switch {
            None => Self::Search(args.trim().into()),
            Some("next") if args.trim().is_empty() => Self::Next,
            Some("next") => Self::Error("@find/next takes no arguments.".into()),
            Some(_) => Self::Error("Unsupported @find switch.".into()),
        }
    }
}
/// Search state contains no cached objects; every page uses current permissions.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FindCursor {
    /// Case-insensitive name prefix to match.
    pub query: String,
    /// Inclusive upper bound captured when the search began.
    pub upper: i64,
    /// First dbref not yet consumed.
    pub next: i64,
}
impl FindCursor {
    /// Apply legacy range defaults and clamp to the current database maximum.
    pub fn new(args: &str, world: &World) -> Self {
        let maximum = world.objects.keys().next_back().map_or(0, |id| id.0);
        let mut fields = args.splitn(3, ',');
        let query = fields.next().unwrap_or_default().trim().into();
        let bound = |s: &str| {
            s.trim()
                .strip_prefix('#')
                .unwrap_or(s.trim())
                .trim()
                .parse::<i64>()
                .ok()
        };
        let next = fields.next().and_then(bound).unwrap_or(0).max(0);
        let upper = fields
            .next()
            .and_then(bound)
            .unwrap_or(maximum)
            .min(maximum);
        Self { query, upper, next }
    }
}
/// A single encoded message and the cursor to install only after successful delivery.
pub struct FindPage {
    /// Telnet-ready bytes, including a continuation or completion footer.
    pub bytes: Vec<u8>,
    /// None marks a completed search.
    pub cursor: Option<FindCursor>,
}
/// Legacy matching tests the beginning and each alphanumeric word boundary.
pub fn matches(name: &str, query: &str) -> bool {
    let name = name.as_bytes();
    let query = query.as_bytes();
    (0..=name.len()).any(|i| {
        (i == 0
            || i < name.len()
                && name[i].is_ascii_alphanumeric()
                && !name[i - 1].is_ascii_alphanumeric())
            && name[i..]
                .get(..query.len())
                .is_some_and(|prefix| prefix.eq_ignore_ascii_case(query))
    })
}
/// Strip display styling and control characters from object names.
fn display_name(object: &Object) -> String {
    text::markup(&text::plain(&object.name))
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect()
}
/// Format the stable dbref/type/flag portion using the shared catalog order.
fn suffix(object: &Object) -> String {
    let mut letters = match object.kind {
        Kind::Room => "R",
        Kind::Player => "P",
        Kind::Exit => "E",
        Kind::Garbage => "-",
        Kind::Thing => "",
    }
    .to_string();
    for flag in flags::ALL {
        if object.flags.contains(flag) {
            letters.push(flag.letter());
        }
    }
    format!(
        "(#{}{}{})",
        object.id.0,
        if letters.is_empty() { "" } else { ":" },
        letters
    )
}
/// Build a page without changing world or cursor. Errors leave the search retryable.
pub fn page(
    world: &World,
    actor: ObjectId,
    cursor: &FindCursor,
    size: usize,
    limit: usize,
) -> Result<FindPage, &'static str> {
    const END: &str = "***End of List***\r\n";
    const MORE: &str = "***Use @find/next for more***\r\n";
    const ERROR: &str = "@find output limit too small.";
    let mut found = world
        .objects
        .range(ObjectId(cursor.next)..)
        .map(|(_, object)| object)
        .take_while(|object| object.id.0 <= cursor.upper)
        .filter(|o| {
            !matches!(o.kind, Kind::Exit | Kind::Garbage)
                && flags::controls(world, actor, o.id)
                && matches(&display_name(o), &cursor.query)
        })
        .peekable();
    // Names contain no controls and valid UTF-8 cannot contain Telnet IAC (0xff).
    // With explicit CRLF separators, encoding preserves this exact byte budget.
    let mut output = String::new();
    let mut next = cursor.clone();
    let mut count = 0;
    while let Some(object) = found.next() {
        let footer = if found.peek().is_some() { MORE } else { END };
        let name = display_name(object);
        let suffix = suffix(object);
        let available = limit.saturating_sub(output.len() + footer.len());
        let full = name.len() + suffix.len() + 2;
        if full > available && count > 0 {
            next.next = object.id.0;
            break;
        }
        // Retain at least one character for nonempty names, and always the full identity.
        let minimum = name.chars().next().map_or(0, char::len_utf8) + suffix.len() + 2;
        if minimum > available {
            return Err(ERROR);
        }
        let mut end = name.len().min(available - suffix.len() - 2);
        while !name.is_char_boundary(end) {
            end -= 1;
        }
        output.push_str(&name[..end]);
        output.push_str(&suffix);
        output.push_str("\r\n");
        count += 1;
        match found.peek() {
            Some(o) => next.next = o.id.0,
            None => {
                output.push_str(END);
                return Ok(FindPage {
                    bytes: telnet::encode(&output),
                    cursor: None,
                });
            }
        }
        if count >= size {
            break;
        }
    }
    let remaining = count > 0;
    output.push_str(if remaining { MORE } else { END });
    if output.len() > limit {
        return Err(ERROR);
    }
    Ok(FindPage {
        bytes: telnet::encode(&output),
        cursor: remaining.then_some(next),
    })
}
/// Encode an error within even very small message limits without evicting the client.
pub fn bounded_error(message: &str, limit: usize) -> Vec<u8> {
    let encoded = telnet::encode(&format!("{message}\r\n"));
    let mut end = encoded.len().min(limit);
    while end > 0 && std::str::from_utf8(&encoded[..end]).is_err() {
        end -= 1;
    }
    encoded[..end].to_vec()
}
