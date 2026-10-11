//! Outfit name grammar: a pattern is literal text with `{slot}` placeholders, and each slot
//! names a word list or a derived value such as an ordinal or a possessive family name.
use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};

/// A placeholder in an outfit pattern.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Slot {
    /// A descriptive word such as "Iron" or "Silent".
    Adjective,
    /// A creature, object or idea such as "Hawk" or "Anvil".
    Noun,
    /// A unit type such as "Irregulars" or "Lancers".
    Suffix,
    /// A heraldic color such as "Crimson".
    Color,
    /// A world or region name such as "Tarsus".
    Place,
    /// A regiment number such as "21st".
    Ordinal,
    /// A commander's family name in possessive form such as "Reyes'".
    Possessive,
}

impl Slot {
    /// Parse a placeholder name as written inside braces.
    fn parse(name: &str) -> Option<Self> {
        Some(match name {
            "adjective" => Self::Adjective,
            "noun" => Self::Noun,
            "suffix" => Self::Suffix,
            "color" => Self::Color,
            "place" => Self::Place,
            "ordinal" => Self::Ordinal,
            "possessive" => Self::Possessive,
            _ => return None,
        })
    }
}

/// One piece of a parsed pattern.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Piece {
    /// Text copied as written.
    Literal(String),
    /// A placeholder filled at generation time.
    Slot(Slot),
}

/// A parsed outfit pattern.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pattern {
    /// The template as written in the word lists, kept for reporting.
    pub template: String,
    pieces: Vec<Piece>,
}

impl Pattern {
    /// Parse `template`, rejecting unknown placeholders and unbalanced braces.
    pub fn parse(template: &str) -> Result<Self> {
        let mut pieces = Vec::new();
        let mut literal = String::new();
        let mut rest = template;
        while let Some(open) = rest.find('{') {
            literal.push_str(&rest[..open]);
            let Some(close) = rest[open..].find('}') else {
                bail!("Pattern {template:?} has an unclosed brace");
            };
            let name = &rest[open + 1..open + close];
            let Some(slot) = Slot::parse(name) else {
                bail!("Pattern {template:?} uses unknown slot {name:?}");
            };
            if !literal.is_empty() {
                pieces.push(Piece::Literal(std::mem::take(&mut literal)));
            }
            pieces.push(Piece::Slot(slot));
            rest = &rest[open + close + 1..];
        }
        if rest.contains('}') {
            bail!("Pattern {template:?} has a stray closing brace");
        }
        literal.push_str(rest);
        if !literal.is_empty() {
            pieces.push(Piece::Literal(literal));
        }
        if !pieces.iter().any(|piece| matches!(piece, Piece::Slot(_))) {
            bail!("Pattern {template:?} has no slots");
        }
        Ok(Self {
            template: template.to_string(),
            pieces,
        })
    }

    /// The pieces in order.
    pub fn pieces(&self) -> &[Piece] {
        &self.pieces
    }

    /// Fill every slot with `fill` and join the result.
    pub fn render(&self, mut fill: impl FnMut(Slot) -> String) -> String {
        let mut text = String::new();
        for piece in &self.pieces {
            match piece {
                Piece::Literal(literal) => text.push_str(literal),
                Piece::Slot(slot) => text.push_str(&fill(*slot)),
            }
        }
        text
    }
}

/// English ordinal suffix: 1st, 2nd, 3rd, 4th, 11th, 21st.
pub fn ordinal(number: u32) -> String {
    let suffix = match (number % 10, number % 100) {
        (_, 11..=13) => "th",
        (1, _) => "st",
        (2, _) => "nd",
        (3, _) => "rd",
        _ => "th",
    };
    format!("{number}{suffix}")
}

/// Possessive form of a name: "Reyes'" for names ending in s, otherwise "Voss's" style
/// is avoided in favour of the plain apostrophe convention used by regiment names.
pub fn possessive(name: &str) -> String {
    if name.ends_with('s') || name.ends_with('z') {
        format!("{name}'")
    } else {
        format!("{name}'s")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_literals_and_slots() {
        let pattern = Pattern::parse("{ordinal} {place} {suffix}").unwrap();
        assert_eq!(
            pattern.pieces(),
            &[
                Piece::Slot(Slot::Ordinal),
                Piece::Literal(" ".into()),
                Piece::Slot(Slot::Place),
                Piece::Literal(" ".into()),
                Piece::Slot(Slot::Suffix),
            ]
        );
        assert_eq!(
            pattern.render(|slot| format!("<{slot:?}>")),
            "<Ordinal> <Place> <Suffix>"
        );
    }

    #[test]
    fn rejects_bad_templates() {
        assert!(Pattern::parse("{nope} Guard").is_err());
        assert!(Pattern::parse("{noun Guard").is_err());
        assert!(Pattern::parse("noun} Guard").is_err());
        assert!(Pattern::parse("Guard").is_err());
    }

    #[test]
    fn ordinals_and_possessives() {
        assert_eq!(ordinal(1), "1st");
        assert_eq!(ordinal(2), "2nd");
        assert_eq!(ordinal(3), "3rd");
        assert_eq!(ordinal(11), "11th");
        assert_eq!(ordinal(12), "12th");
        assert_eq!(ordinal(21), "21st");
        assert_eq!(ordinal(100), "100th");
        assert_eq!(possessive("Reyes"), "Reyes'");
        assert_eq!(possessive("Voss"), "Voss'");
        assert_eq!(possessive("Halloran"), "Halloran's");
    }
}
