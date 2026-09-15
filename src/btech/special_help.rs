//! Read-only special-object help derived from the same catalogue as command admission.
use super::{BattleCommandClass, BattleSpecialCommand, BattleSpecialType, menu};
use crate::text;

impl BattleSpecialType {
    /// Render the selected object's visible command menu or exact named help category.
    /// Authority and object selection belong to the caller; this never changes game state.
    pub fn help(self, class: Option<BattleCommandClass>, privileged: bool, topic: &str) -> String {
        let entries: Vec<_> = self.visible_commands(class, privileged).collect();
        let mut starts: Vec<_> = entries
            .iter()
            .enumerate()
            .filter_map(|(i, entry)| entry.category.then_some(i))
            .collect();
        if starts.is_empty() {
            starts.push(0);
        }
        let categorized = starts.len() > 1;
        starts.push(entries.len());
        let sections: Vec<_> = starts
            .windows(2)
            .map(|range| &entries[range[0]..range[1]])
            .collect();
        let mut lines = vec![menu::rule()];
        if topic.is_empty() {
            let mut count = 0;
            for section in &sections {
                if categorized {
                    lines.push(category(&section[0].description));
                } else {
                    lines.push(menu::cell(
                        &format!("{} command listing: ", self.label()),
                        78,
                        true,
                    ));
                }
                let commands = &section[usize::from(categorized)..];
                count += commands.len();
                for row in commands.chunks(4) {
                    lines.push(
                        row.iter()
                            .map(|entry| menu::cell(&text::escape(entry.name()), 19, false))
                            .collect(),
                    );
                }
            }
            if count == 0 {
                lines.push(menu::cell(
                    "There are no commands you are authorized to use here.",
                    78,
                    false,
                ));
            } else {
                lines.push(menu::rule());
                lines.push(menu::cell(
                    if categorized {
                        "Additional info available with 'HELP SUBTOPIC'"
                    } else {
                        "Additional info available with 'HELP ALL'"
                    },
                    78,
                    false,
                ));
            }
        } else {
            let selected = if topic.eq_ignore_ascii_case("all") {
                if categorized {
                    Err("ALL not available for objects with subcategories.")
                } else {
                    Ok(0)
                }
            } else if !categorized {
                Err("This object doesn't have any other detailed help than 'HELP ALL'")
            } else {
                sections
                    .iter()
                    .position(|section| section[0].description.eq_ignore_ascii_case(topic))
                    .ok_or("Subcategory not found.")
            };
            match selected {
                Err(message) => lines.push(menu::cell(message, 78, false)),
                Ok(index) => {
                    let section = sections[index];
                    if categorized {
                        lines.push(category(&section[0].description));
                    }
                    for entry in &section[usize::from(categorized)..] {
                        detail(&mut lines, entry);
                    }
                }
            }
        }
        lines.push(menu::rule());
        lines.join("\r\n")
    }

    /// Reference type name used by ungrouped command menus.
    fn label(self) -> &'static str {
        match self {
            Self::Mech => "MECH",
            Self::Debug => "DEBUG",
            Self::Map => "MAP",
            Self::Autopilot => "AUTOPILOT",
            Self::Turret => "TURRET",
        }
    }
}

/// Category titles use their own 70-column centering inside a 78-column menu row.
fn category(title: &str) -> String {
    menu::cell(
        &format!(
            "[fg=green]{}{}[reset]",
            " ".repeat(70_usize.saturating_sub(title.len()) / 2),
            text::escape(title)
        ),
        78,
        false,
    )
}

/// Split authored ASCII prose at the final space before the reference width boundary.
fn split_line(value: &str, width: usize) -> (&str, &str) {
    let boundary = if value.len() <= width {
        value.len()
    } else {
        value[..width]
            .rfind(' ')
            .filter(|&i| i != 0)
            .unwrap_or(width)
    };
    let (line, remainder) = value.split_at(boundary);
    (line, remainder.strip_prefix(' ').unwrap_or(remainder))
}

/// Syntax starts with the reference colored command word; descriptions are indented three spaces.
fn detail(lines: &mut Vec<String>, entry: &BattleSpecialCommand) {
    let mut remaining = entry.syntax.as_str();
    let mut first = true;
    while !remaining.is_empty() {
        let (line, rest) = split_line(remaining, 73);
        let line = if first {
            // The first syntax row colors the full syntax before the menu clips its visible width.
            match remaining.split_once(' ') {
                Some((name, arguments)) => format!(
                    "[fg=blue bold]{}[reset] {}",
                    text::escape(name),
                    text::escape(arguments)
                ),
                None => format!("[fg=cyan]{}[reset]", text::escape(remaining)),
            }
        } else {
            text::escape(line)
        };
        lines.push(menu::cell(&line, 78, false));
        remaining = rest;
        first = false;
    }
    remaining = &entry.description;
    while !remaining.is_empty() {
        let (line, rest) = split_line(remaining, 71);
        lines.push(menu::cell(&format!("   {}", text::escape(line)), 78, false));
        remaining = rest;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Public maps expose only STORES, with an incomplete four-column row retaining its width.
    #[test]
    fn ungrouped_public_help_has_reference_rows() {
        let help = text::plain(&BattleSpecialType::Map.help(None, false, ""));
        let lines: Vec<_> = help.lines().collect();
        assert_eq!(lines.len(), 6);
        assert_eq!(lines[2], format!("STORES{}", " ".repeat(13)));
        assert_eq!(
            lines[1],
            format!("{}MAP command listing: {}", " ".repeat(28), " ".repeat(29))
        );
        assert!(lines[4].contains("'HELP ALL'"));
        let detail = BattleSpecialType::Map.help(None, false, "aLl");
        assert!(detail.contains(&text::truncate("[fg=cyan]STORES[reset]", 78)));
        assert!(!detail.contains("LOADMAP"));
        assert!(detail.lines().all(|line| text::width(line) == 78));
    }

    /// Mech categories retain authored order, privilege filtering and exact topic matching.
    #[test]
    fn categorized_help_obeys_class_and_authority() {
        for privileged in [false, true] {
            for class in [BattleCommandClass::Mech, BattleCommandClass::Ground] {
                let help = BattleSpecialType::Mech.help(Some(class), privileged, "");
                assert!(help.find("Movement").unwrap() < help.find("Radio").unwrap());
                assert_eq!(help.contains("Physical"), class == BattleCommandClass::Mech);
                assert_eq!(help.contains("Restricted"), privileged);
                assert!(help.contains("HELP SUBTOPIC"));
                assert!(
                    BattleSpecialType::Mech
                        .help(Some(class), privileged, "ALL")
                        .contains("ALL not available")
                );
                assert!(
                    BattleSpecialType::Mech
                        .help(Some(class), privileged, "Mov")
                        .contains("Subcategory not found.")
                );
                let detail = BattleSpecialType::Mech.help(Some(class), privileged, "mOvEmEnT");
                assert!(detail.contains(&text::truncate("[fg=blue bold]HEADING[reset]", 78)));
                assert!(!detail.contains("HELP SUBTOPIC"));
                assert!(detail.lines().all(|line| text::width(line) == 78));
            }
        }
        assert!(
            BattleSpecialType::Mech
                .help(None, true, "")
                .contains("There are no commands")
        );
        assert!(
            BattleSpecialType::Map
                .help(None, true, "missing")
                .contains("doesn't have any other detailed help")
        );
    }
}
