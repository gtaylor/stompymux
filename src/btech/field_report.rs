//! Shared wizard field-list layout and prefix filtering for maps and weapon stations.

/// Decode the common one/two/four-column selector and case-insensitive field prefix.
pub(super) fn options(arguments: &str) -> (usize, String) {
    let (columns, filter) = if let Some(filter) = arguments.strip_prefix('1') {
        (1, filter)
    } else if let Some(filter) = arguments.strip_prefix('4') {
        (4, filter)
    } else {
        (2, arguments)
    };
    (columns, filter.trim().to_ascii_lowercase())
}

/// Render literal field rows; callers escape text at the publication boundary.
pub(super) fn render<'a>(
    name: &str,
    kind: &str,
    columns: usize,
    fields: impl IntoIterator<Item = (&'a str, Option<&'a str>)>,
) -> String {
    let width = 74 / columns;
    let cells: Vec<_> = fields
        .into_iter()
        .map(|(name, value)| {
            let label: String = name.chars().take(width / 3).collect();
            format!(
                "{label:<label_width$}{value:>value_width$}",
                label_width = width / 3,
                value_width = width * 2 / 3,
                value = value.unwrap_or("n/a")
            )
        })
        .collect();
    let mut lines = vec![
        "-".repeat(74),
        format!("Data for {name} ({kind})"),
        "-".repeat(74),
    ];
    lines.extend(cells.chunks(columns).map(|row| row.join(" ")));
    lines.push("-".repeat(74));
    lines.join("\n")
}

/// Split a field name from its complete value, preserving embedded spaces in textual fields.
pub(super) fn assignment(arguments: &str) -> anyhow::Result<(&str, &str)> {
    use anyhow::{Context, ensure};
    let (field, value) = arguments
        .trim()
        .split_once(char::is_whitespace)
        .context("Invalid arguments!")?;
    ensure!(!value.trim().is_empty(), "Invalid arguments!");
    Ok((field, value.trim()))
}
