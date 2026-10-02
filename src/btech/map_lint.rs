//! Strict map-asset checks layered on the game's own map decoder.
//!
//! The decoder tolerates things a clean asset should not contain: row text past the declared
//! width, unknown terrain (loaded as grassland), CRLF endings, and trailing lines after the grid.
//! [`check_map_source`] reports those alongside every error that would stop the map loading, and
//! [`tidy_map_source`] removes the purely mechanical problems without touching terrain.
use super::BattleMapAsset;

/// One problem found in a map asset; `line` is 1-based, or `None` for whole-file problems.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MapCheckIssue {
    pub line: Option<usize>,
    pub message: String,
}

impl std::fmt::Display for MapCheckIssue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.line {
            Some(line) => write!(f, "line {line}: {}", self.message),
            None => f.write_str(&self.message),
        }
    }
}

/// Report every problem in `source`, or nothing when the asset is clean.
/// A source that fails to decode reports only the decoder's error, since later checks
/// depend on the declared dimensions being right.
pub fn check_map_source(source: &[u8]) -> Vec<MapCheckIssue> {
    let issue = |line: Option<usize>, message: String| MapCheckIssue { line, message };
    let (map, warnings) = match BattleMapAsset::from_cells_diagnostics(source, 0) {
        Ok(decoded) => decoded,
        Err(error) => return vec![issue(None, format!("{error:#}"))],
    };
    let mut issues: Vec<_> = warnings
        .iter()
        .map(|warning| {
            issue(
                Some(usize::from(warning.y) + 2),
                format!(
                    "unknown terrain {:?} at {},{} loads as grassland",
                    warning.symbol, warning.x, warning.y
                ),
            )
        })
        .collect();
    if source.contains(&b'\r') {
        issues.push(issue(None, "CRLF line endings".into()));
    }
    if !source.ends_with(b"\n") {
        issues.push(issue(None, "missing final newline".into()));
    }
    let text = String::from_utf8_lossy(source);
    let lines: Vec<_> = text.lines().collect();
    let width = usize::from(map.width) * 2;
    let height = usize::from(map.height);
    for (index, row) in lines.iter().enumerate().skip(1).take(height) {
        if row.len() > width {
            issues.push(issue(
                Some(index + 1),
                format!(
                    "{} bytes past the declared width of {} are ignored",
                    row.len() - width,
                    map.width
                ),
            ));
        }
    }
    let mut trailing = lines.iter().enumerate().skip(height + 1);
    if let Some((index, line)) = trailing.next()
        && super::map::parse_metadata(line.as_bytes()).is_none()
    {
        issues.push(issue(
            Some(index + 1),
            "unexpected data after the grid; only a `flags: gravity temperature` line may follow"
                .into(),
        ));
    }
    if let Some((index, _)) = trailing.find(|(_, line)| !line.is_empty()) {
        issues.push(issue(
            Some(index + 1),
            "unexpected data after the map settings line".into(),
        ));
    }
    issues
}

/// Remove CRLF endings, DOS end-of-file padding, trailing spaces and blank lines. When the
/// result decodes, also drop every line after the grid except a valid settings line directly
/// below it, which is the only one the decoder reads. Lines are all kept when the map does not
/// decode, or when a full-width row follows the grid, since then the header cannot be trusted
/// to say where the grid ends. Terrain cells are never changed.
pub fn tidy_map_source(source: &[u8]) -> Vec<u8> {
    let text = String::from_utf8_lossy(source).replace('\r', "");
    let mut lines: Vec<_> = text
        .split('\n')
        .map(|line| line.trim_end_matches([' ', '\t', '\u{1a}']))
        .filter(|line| !line.is_empty())
        .collect();
    let mut cleaned = lines.join("\n").into_bytes();
    cleaned.push(b'\n');
    let Ok((map, _)) = BattleMapAsset::from_cells_diagnostics(&cleaned, 0) else {
        return cleaned;
    };
    let height = usize::from(map.height);
    let width = usize::from(map.width) * 2;
    if lines.len() > height + 1 && lines[height + 1..].iter().all(|line| line.len() != width) {
        let settings = Some(lines[height + 1])
            .filter(|line| super::map::parse_metadata(line.as_bytes()).is_some());
        lines.truncate(height + 1);
        lines.extend(settings);
    }
    let mut tidy = lines.join("\n").into_bytes();
    tidy.push(b'\n');
    tidy
}

#[cfg(test)]
mod tests {
    use super::*;

    fn messages(source: &[u8]) -> Vec<String> {
        check_map_source(source)
            .iter()
            .map(ToString::to_string)
            .collect()
    }

    #[test]
    fn clean_maps_have_no_issues() {
        assert!(messages(b"2 1\n.0#1\n").is_empty());
        assert!(messages(b"2 1\n.0#1\n2: 50 -40\n").is_empty());
        assert!(messages(b"2 1\n'0&1\n").is_empty());
    }

    #[test]
    fn decoder_failures_are_reported_alone() {
        let issues = messages(b"3 1\n.0#1\n-1\n");
        assert_eq!(issues.len(), 1);
        assert!(
            issues[0].contains("Height not loaded properly"),
            "{issues:?}"
        );
        assert_eq!(messages(b"1 1\n.x\n").len(), 1);
    }

    #[test]
    fn tolerated_problems_are_reported() {
        assert_eq!(
            messages(b"1 2\r\n.0\r\nS0~1\r\n-1\r\n-1 -1 -1 -1\r\n"),
            [
                "line 3: unknown terrain 'S' at 0,1 loads as grassland",
                "CRLF line endings",
                "line 3: 2 bytes past the declared width of 1 are ignored",
                "line 4: unexpected data after the grid; only a `flags: gravity temperature` line may follow",
                "line 5: unexpected data after the map settings line",
            ]
        );
        assert_eq!(messages(b"1 1\n.0"), ["missing final newline"]);
    }

    #[test]
    fn tidy_removes_only_mechanical_junk() {
        assert_eq!(
            tidy_map_source(b"2 1\r\n\r\n.0#1  \r\n8: 100 20\r\n-1 -1\r\n\x1a\x1a"),
            b"2 1\n.0#1\n8: 100 20\n"
        );
        // A settings line below other junk was never read, so it is dropped too.
        assert_eq!(
            tidy_map_source(b"2 1\n.0#1\n-1\n8: 100 20\n"),
            b"2 1\n.0#1\n"
        );
        // Rows past the declared width are terrain, so tidying leaves them for review.
        let wide = b"1 1\n.0#1\n";
        assert_eq!(tidy_map_source(wide), wide);
        // A header that does not match the grid cannot say where the grid ends.
        let swapped = b"1 2\n.0#1\n-1\n";
        assert_eq!(tidy_map_source(swapped), swapped);
        let extra_rows = b"1 1\n.0\n#1\n-1\n";
        assert_eq!(tidy_map_source(extra_rows), extra_rows);
        for source in [&b"2 1\n.0#1\n"[..], b"2 1\n.0#1\n2: 50 -40\n"] {
            assert_eq!(tidy_map_source(source), source);
        }
    }
}
