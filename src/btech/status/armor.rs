//! Data-driven silhouettes with typed armor cells and section-dependent outlines.
use crate::btech::{MechChassis, VehicleSection};
use crate::{ObjectId, World};
use anyhow::{Context, Result};
use serde::Deserialize;
use std::{collections::BTreeMap, sync::LazyLock};

/// A row is composed of literal strokes or a live protection value.
#[derive(Deserialize)]
#[serde(untagged)]
enum Cell {
    Stroke {
        text: String,
        when: u16,
    },
    Value {
        section: usize,
        face: Face,
        width: usize,
    },
}

/// Front, rear and internal protection use the same numeric and color rules.
#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Face {
    Front,
    Rear,
    Internal,
}

/// Immutable presentation assets; no game rules or executable template language.
static DIAGRAMS: LazyLock<BTreeMap<String, Vec<Vec<Cell>>>> = LazyLock::new(|| {
    serde_json::from_str(include_str!("diagrams.json")).expect("validated status diagrams")
});

/// Render one silhouette, retaining fixed columns when a section is destroyed.
pub(super) fn render(world: &World, id: ObjectId) -> Result<String> {
    render_mode(world, id, false)
}

/// Enemy scans share the silhouette and protection thresholds without disclosing numeric values.
pub(super) fn scan(world: &World, id: ObjectId) -> Result<String> {
    render_mode(world, id, true)
}

/// Chassis adapters select artwork once for both owned and adversarial presentation.
fn render_mode(world: &World, id: ObjectId, adversarial: bool) -> Result<String> {
    let mut current = [[0_u16; 3]; 8];
    let mut original = current;
    let name = if let Some(unit) = world.btech.constructed_units().get(&id) {
        for (&section, state) in unit.sections() {
            let slot = crate::btech::damage_field::mech_section(section) as usize;
            let baseline = &unit.definition().sections[&section];
            current[slot] = [state.armor, state.rear, state.internal];
            original[slot] = [baseline.armor, baseline.rear, baseline.internal];
        }
        if unit.chassis() == MechChassis::Quad {
            "quad"
        } else {
            match unit.definition().tons {
                0..=35 => "light",
                36..=55 => "medium",
                56..=75 => "heavy",
                _ => "assault",
            }
        }
    } else {
        let unit = world
            .btech
            .vehicles()
            .get(&id)
            .context("Unit is unavailable")?;
        for (&section, state) in unit.sections() {
            let slot = crate::btech::damage_field::vehicle_section(section) as usize;
            let baseline = &unit.definition().sections[&section];
            current[slot] = [state.armor, state.rear, state.internal];
            original[slot] = [baseline.armor, baseline.rear, baseline.internal];
        }
        if unit.vtol_flight().is_some() {
            "vtol"
        } else if unit
            .definition()
            .sections
            .get(&VehicleSection::Turret)
            .is_some_and(|s| s.internal > 0)
        {
            "vehicle"
        } else {
            "turretless"
        }
    };
    let keys: &[u8] = match name {
        "vehicle" | "turretless" => &[1, 2, 3, 4, 5, 6, 7],
        "vtol" => &[7, 7, 1, 2, 3, 4, 5, 6, 7, 7],
        _ => &[7, 1, 2, 3, 4, 5, 6],
    };
    Ok(draw(
        &DIAGRAMS[name],
        current,
        original,
        adversarial.then_some(keys),
    ))
}

/// Resolve all rows without consulting or mutating simulation state.
fn draw(
    rows: &[Vec<Cell>],
    current: [[u16; 3]; 8],
    original: [[u16; 3]; 8],
    keys: Option<&[u8]>,
) -> String {
    let intact = current.iter().enumerate().fold(0_u16, |mask, (i, values)| {
        mask | (u16::from(values[2] > 0) << i)
    });
    rows.iter()
        .enumerate()
        .map(|(row_index, row)| {
            let mut line = keys.map_or_else(String::new, |keys| legend(keys[row_index]));
            for cell in row {
                match cell {
                    Cell::Stroke { text, when } => {
                        if *when == 0 || intact & when != 0 {
                            line.push_str(&crate::text::escape(text));
                        } else {
                            line.push_str(&" ".repeat(text.len()));
                        }
                    }
                    Cell::Value {
                        section,
                        face,
                        width,
                    } => {
                        if current[*section][2] == 0 {
                            line.push_str(&" ".repeat(*width));
                            continue;
                        }
                        let column = match face {
                            Face::Front => 0,
                            Face::Rear => 1,
                            Face::Internal => 2,
                        };
                        let value = current[*section][column];
                        let (symbol, style) = protection(value, original[*section][column]);
                        let number = if keys.is_some() {
                            symbol.to_string().repeat(*width)
                        } else {
                            format!("{value:>width$}")
                        };
                        // The diagram's two-column cells retain the low digits for larger values.
                        line.push_str(&format!(
                            "[fg={style}]{}[reset]",
                            &number[number.len() - width..]
                        ));
                    }
                }
            }
            line
        })
        .collect::<Vec<_>>()
        .join("\r\n")
}

/// Shared integer damage bands for both color and qualitative disclosure.
fn protection(value: u16, original: u16) -> (char, &'static str) {
    if value == 0 {
        return ('*', "black bold");
    }
    match (u32::from(value) + 1) * 100 / (u32::from(original) + 1) {
        0..=45 => ('X', "red"),
        46..=70 => ('x', "yellow bold"),
        71..=90 => ('o', "green"),
        _ => ('O', "green bold"),
    }
}

/// Three-column scan legend follows each silhouette's authored row keys.
fn legend(key: u8) -> String {
    let (symbol, style) = match key {
        1 => return "Key".into(),
        2 => ('*', "black bold"),
        3 => ('X', "red"),
        4 => ('x', "yellow bold"),
        5 => ('o', "green"),
        6 => ('O', "green bold"),
        _ => return "   ".into(),
    };
    format!("[fg={style}]{symbol}{symbol} [reset]")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every asset uses supported sections and fixed-width cells, including destroyed outlines.
    #[test]
    fn diagram_assets_are_bounded_and_keep_columns_on_destruction() {
        assert_eq!(DIAGRAMS.len(), 8);
        for rows in DIAGRAMS.values() {
            for cell in rows.iter().flatten() {
                if let Cell::Value { section, width, .. } = cell {
                    assert!(*section < 8);
                    assert_eq!(*width, 2);
                }
            }
            let intact = crate::text::plain(&draw(rows, [[99; 3]; 8], [[99; 3]; 8], None));
            let destroyed = crate::text::plain(&draw(rows, [[0; 3]; 8], [[99; 3]; 8], None));
            assert_eq!(
                intact.lines().map(str::len).collect::<Vec<_>>(),
                destroyed.lines().map(str::len).collect::<Vec<_>>()
            );
        }
    }
}

#[cfg(test)]
mod disclosure_tests {
    use super::protection;

    #[test]
    fn armor_disclosure_preserves_integer_band_boundaries() {
        for (remaining, expected) in [
            (0, '*'),
            (1, 'X'),
            (44, 'X'),
            (45, 'x'),
            (69, 'x'),
            (70, 'o'),
            (89, 'o'),
            (90, 'O'),
            (99, 'O'),
        ] {
            assert_eq!(protection(remaining, 99).0, expected);
        }
        assert_eq!(protection(0, 0).0, '*');
        assert_eq!(protection(u16::MAX, u16::MAX).0, 'O');
    }
}
