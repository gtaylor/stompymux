//! Wizard catalog reports share character definitions and preserve fixed display columns.
use crate::{CommandAction, CommandContext, CommandInput, CommandReport};
use anyhow::Result;

use super::character_names::VALUES;

/// The help text is presentation data, including its advertised but unaccepted prefix.
const HELP: &str = "Valid arguments:  [char_]allvalues, [char_]values, [char_]skills, [char_]advantages, [char_]attributes, btechvalues [scode]";

/// List metadata without initializing character state or changing runtime values.
pub(crate) fn command(_ctx: &CommandContext<'_>, input: &CommandInput) -> Result<CommandAction> {
    // The two-argument command ignores the right-hand side of an equals sign.
    let category = input
        .args
        .split_once('=')
        .map_or(input.args.as_str(), |(left, _)| left);
    Ok(CommandAction::Report(CommandReport::Inspection(render(
        category.trim(),
    ))))
}

/// Format catalog order and three 24-column cells, with a leading space after row one.
fn render(category: &str) -> String {
    let category = category.to_ascii_lowercase();
    if category.is_empty() {
        return HELP.to_owned();
    }
    if category == "btechvalues" {
        // This is the reference field-name inventory, not a promise that each
        // raw field setter is implemented. Runtime field admission stays shared.
        return include_str!("special_value_names.txt")
            .trim_end_matches('\n')
            .to_owned();
    }
    let advantages = || super::BATTLE_ADVANTAGES.iter().map(|value| value.name);
    let skills = || super::BATTLE_SKILLS.iter().map(|value| value.name);
    let attributes = super::character_list::ATTRIBUTES;
    let (heading, names): (&str, Vec<&str>) = match category.as_str() {
        "allvalues" => (
            "charvalues",
            VALUES
                .iter()
                .copied()
                .chain(advantages())
                .chain(attributes.iter().copied())
                .chain(skills())
                .collect(),
        ),
        "values" => ("Char_value", VALUES.to_vec()),
        "advantages" => ("Char_advantage", advantages().collect()),
        "attributes" => ("Char_attribute", attributes.to_vec()),
        "skills" => ("Char_skill", skills().collect()),
        _ => return "Invalid arguments to +show command!".to_owned(),
    };
    let mut lines = vec![format!("List of {heading} available:")];
    for (row, entries) in names.chunks(3).enumerate() {
        let mut line = if row == 0 {
            String::new()
        } else {
            " ".to_owned()
        };
        for name in entries {
            let cell = format!("{name:<23} ");
            line.extend(cell.chars().take(24));
        }
        lines.push(line);
    }
    lines.push(" ".to_owned());
    lines.push(format!("Total of {} things found.", names.len()));
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Independent literal rows pin padding, category spelling and the final partial row.
    #[test]
    fn attribute_columns_and_category_admission() {
        assert_eq!(
            render("ATTRIBUTES"),
            concat!(
                "List of Char_attribute available:\n",
                "Build                   Reflexes                Intuition               \n",
                " Learn                   Charisma                \n",
                " \nTotal of 5 things found."
            )
        );
        for name in ["a", "char_attributes", "btechvalues mech"] {
            assert_eq!(render(name), "Invalid arguments to +show command!");
        }
        assert_eq!(render(""), HELP);
        assert!(render("allvalues").ends_with("Total of 117 things found."));
        assert!(render("values").ends_with("Total of 14 things found."));
        assert!(render("skills").ends_with("Total of 78 things found."));
        assert!(render("advantages").ends_with("Total of 20 things found."));
    }
}
