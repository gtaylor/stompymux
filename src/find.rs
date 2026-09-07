//! Legacy word-prefix matching and read-only object reports.
use crate::{
    flags, telnet, text,
    world::{Kind, Object, ObjectId, World},
};

/// A name and inclusive legacy database range; this is not session state.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SearchRange {
    pub query: String,
    pub lower: i64,
    pub upper: i64,
}
impl SearchRange {
    /// Apply legacy defaults without allocating missing database slots.
    pub fn new(args: &str, maximum: i64) -> Self {
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
        let lower = fields.next().and_then(bound).unwrap_or(0).max(0);
        let upper = fields
            .next()
            .and_then(bound)
            .unwrap_or(maximum)
            .min(maximum);
        Self {
            query,
            lower,
            upper,
        }
    }
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
fn display_name(object: &Object, palette: &text::Palette) -> String {
    text::plain_with(palette, &object.name)
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect()
}
/// Format the stable dbref/type/flag portion using the shared catalog order.
pub(crate) fn suffix(object: &Object) -> String {
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
/// Capture a complete read-only find report, bounded independently of transport chunks.
pub fn report(world: &World, actor: ObjectId, args: &str, limit: usize) -> anyhow::Result<String> {
    let maximum = world.objects.keys().next_back().map_or(0, |id| id.0);
    let range = SearchRange::new(args, maximum);
    let mut report = crate::reports::Report::new(limit, "***End of List***")?;
    for o in world
        .objects
        .values()
        .filter(|o| o.id.0 >= range.lower && o.id.0 <= range.upper)
    {
        if !matches!(o.kind, Kind::Exit | Kind::Garbage)
            && flags::controls(world, actor, o.id)
            && matches(&display_name(o, &world.palette), &range.query)
        {
            report.row(&identity(world, Some(o.id)));
        }
    }
    report.finish()
}
/// Shared styled-name-free object identity for read-only reports.
pub fn identity(world: &World, id: Option<ObjectId>) -> String {
    id.and_then(|id| world.objects.get(&id)).map_or_else(
        || "NOWHERE".into(),
        |o| format!("{}{}", display_name(o, &world.palette), suffix(o)),
    )
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
