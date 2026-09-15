//! Bounded read-only terminal menus using the shared styled-text width and truncation rules.

/// Render a fixed-width menu cell, reserving its final column for spacing.
pub(super) fn cell(text: &str, width: usize, centered: bool) -> String {
    let text = crate::text::truncate(text, width.saturating_sub(1));
    let visible = crate::text::width(&text);
    let left = if centered { (width - visible) / 2 } else { 0 };
    let content = if centered {
        format!("[fg=blue bold]{text}[reset]")
    } else {
        text
    };
    format!(
        "{}{content}{}",
        " ".repeat(left),
        " ".repeat(width - visible - left)
    )
}

/// Full-width separator shared by information and categorized command menus.
pub(super) fn rule() -> String {
    format!("[fg=blue]{}[reset]", "-".repeat(78))
}

/// A single-column information menu, with the header above the middle rule.
/// Callers supply escaped/styled text; every visible row occupies 78 columns.
pub(super) fn information(
    title: &str,
    header: &str,
    rows: impl IntoIterator<Item = String>,
) -> String {
    const WIDTH: usize = 78;
    let rule = rule();
    let title = cell(title, WIDTH, true);
    let fit = |text: &str| cell(text, WIDTH, false);
    let mut lines = vec![rule.clone(), title, fit(header), rule.clone()];
    lines.extend(rows.into_iter().map(|row| fit(&row)));
    lines.push(rule);
    lines.join("\r\n")
}

/// Two-column report with full-width rules and an optional single-cell footer.
/// Inputs are escaped or styled by the caller; partial rows retain their own cell width.
pub(super) fn pairs(
    rows: impl IntoIterator<Item = (String, String)>,
    footer: Option<String>,
) -> String {
    let mut lines = vec![rule()];
    lines.extend(
        rows.into_iter()
            .map(|(left, right)| format!("{}{}", cell(&left, 39, false), cell(&right, 39, false))),
    );
    lines.push(rule());
    if let Some(footer) = footer {
        lines.push(cell(&footer, 39, false));
        lines.push(rule());
    }
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Styling and wide titles cannot overflow or move the fixed-width menu columns.
    #[test]
    fn information_bounds_and_centers_visible_text() {
        for title in [
            "Short".to_owned(),
            "界".repeat(100),
            crate::text::escape("[fg=red]literal"),
        ] {
            let menu = information(&title, "[fg=green]Header[reset]", ["x".repeat(100)]);
            assert!(menu.lines().all(|line| crate::text::width(line) == 78));
            assert_eq!(menu.lines().count(), 6);
            assert_eq!(
                crate::text::plain(menu.lines().nth(4).unwrap())
                    .trim_end()
                    .len(),
                77
            );
        }
        let menu = crate::text::plain(&information("Title", "Header", []));
        assert_eq!(
            menu.lines().nth(1).unwrap(),
            format!("{}Title{}", " ".repeat(36), " ".repeat(37))
        );
    }
}
