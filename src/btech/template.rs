//! Bounded shared template syntax and typed BattleMech decoding; equipment awaits catalogue validation.
use anyhow::{Context, Result, bail, ensure};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Stable BattleMech attachment identity in persisted order.
/// Limb identifiers use biped names; use the chassis for quad names and leg roles.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum BattleSection {
    LeftArm,
    RightArm,
    LeftTorso,
    RightTorso,
    CenterTorso,
    LeftLeg,
    RightLeg,
    Head,
}

impl BattleSection {
    /// All eight sections in storage order.
    pub const ALL: [Self; 8] = [
        Self::LeftArm,
        Self::RightArm,
        Self::LeftTorso,
        Self::RightTorso,
        Self::CenterTorso,
        Self::LeftLeg,
        Self::RightLeg,
        Self::Head,
    ];

    /// Decode a template section heading.
    pub fn parse(name: &str) -> Result<Self> {
        Self::ALL
            .into_iter()
            .find(|section| section.name().eq_ignore_ascii_case(name))
            .with_context(|| format!("unsupported section {name}"))
    }

    /// Stable template-file spelling.
    pub fn name(self) -> &'static str {
        match self {
            Self::LeftArm => "Left_Arm",
            Self::RightArm => "Right_Arm",
            Self::LeftTorso => "Left_Torso",
            Self::RightTorso => "Right_Torso",
            Self::CenterTorso => "Center_Torso",
            Self::LeftLeg => "Left_Leg",
            Self::RightLeg => "Right_Leg",
            Self::Head => "Head",
        }
    }
}

/// One occupied template slot, with asset-level data and modes retained verbatim.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CriticalDefinition {
    pub equipment: String,
    pub data: String,
    pub modes: Vec<String>,
    pub brand: Option<u8>,
}

/// Unresolved armor and slot layout shared by unit-specific template decoders.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SectionDefinition {
    pub armor: u16,
    pub internal: u16,
    pub rear: u16,
    /// Zero-based critical positions. Unoccupied slots are absent.
    pub criticals: BTreeMap<u8, CriticalDefinition>,
    pub configuration: Option<String>,
}

/// A decoded BattleMech asset; successful parsing does not imply simulation support.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BattleTemplate {
    pub name: String,
    pub reference: String,
    pub tons: u16,
    pub max_speed: f64,
    pub jump_speed: f64,
    pub heat_sinks: u16,
    pub sections: BTreeMap<BattleSection, SectionDefinition>,
    /// All unit-level fields, including features awaiting domain implementation.
    pub attributes: BTreeMap<String, String>,
}

impl BattleTemplate {
    /// Clan chassis always use double-efficiency heat sinks.
    pub fn has_double_heat_sinks(&self) -> bool {
        self.has_special("Clan") || self.has_special("DoubleHS")
    }

    /// Number of contiguous critical slots occupied by one installed heat sink.
    pub fn heat_sink_slots(&self) -> usize {
        if self.has_special("Clan") {
            return 2;
        }
        if self.has_special("DoubleHS") {
            return 3;
        }
        1
    }

    /// Test a whitespace-separated chassis feature using the asset's case-insensitive spelling.
    pub(crate) fn has_special(&self, name: &str) -> bool {
        self.attributes.get("specials").is_some_and(|value| {
            value
                .split_ascii_whitespace()
                .any(|flag| flag.eq_ignore_ascii_case(name))
        })
    }

    /// Parse bounded brace-delimited fields and section-local critical ranges.
    pub fn parse(source: &str) -> Result<Self> {
        let ParsedTemplate { fields, sections } = ParsedTemplate::parse(source)?;
        let required = |name: &str| -> Result<String> {
            fields
                .get(name)
                .filter(|value| !value.is_empty())
                .cloned()
                .with_context(|| format!("missing template field {name}"))
        };
        ensure!(
            required("type")?.eq_ignore_ascii_case("Mech"),
            "only BattleMech templates are supported"
        );
        let chassis = super::BattleMechChassis::parse(&required("move_type")?)?;
        let sections: BTreeMap<_, _> = sections
            .into_iter()
            .map(|(name, layout)| Ok((chassis.parse_section(&name)?, layout)))
            .collect::<Result<_>>()?;
        let speed = |name: &str| -> Result<f64> {
            let value: f64 = fields
                .get(name)
                .map(String::as_str)
                .unwrap_or("0")
                .parse()
                .with_context(|| format!("invalid {name}"))?;
            ensure!(value.is_finite() && value >= 0.0, "invalid {name}");
            Ok(value)
        };
        ensure!(
            sections.len() == 8 && sections.values().all(|section| section.internal > 0),
            "all eight sections require positive internals"
        );
        let tons = required("tons")?.parse().context("invalid tonnage")?;
        ensure!(tons > 0, "tonnage must be positive");
        let mut template = Self {
            name: required("name")?,
            reference: required("reference")?,
            tons,
            max_speed: speed("max_speed")?,
            jump_speed: speed("jump_speed")?,
            heat_sinks: fields
                .get("heat_sinks")
                .map(String::as_str)
                .unwrap_or("10")
                .parse()
                .context("invalid heat sinks")?,
            sections,
            attributes: fields,
        };
        // The reference load always forces the tonnage-chart internal structure
        // (mech_int_check) while reading the template file, overriding authored
        // Internals lines for both the original and the live value. Keeping the
        // rewrite here, where C's parse and finalize are one operation, leaves
        // saved definitions and later construction edits restored verbatim.
        let quad = chassis == super::BattleMechChassis::Quad;
        for section in BattleSection::ALL {
            if let Some(expected) = Self::chart_internal(template.tons, section, quad)
                && let Some(layout) = template.sections.get_mut(&section)
            {
                layout.internal = expected;
            }
        }
        Ok(template)
    }

    /// Expected mech internal structure from the reference tonnage chart.
    ///
    /// Rows are `[tons, center torso, side torsos, arms, legs]`; quad chassis
    /// use the leg column for arms. Head structure is always three and unknown
    /// tonnage leaves authored internals untouched, matching mech_int_check.
    fn chart_internal(tons: u16, section: BattleSection, quad: bool) -> Option<u16> {
        const STRUCTURE: [[i16; 5]; 20] = [
            [10, 4, 3, 1, 2],
            [15, 5, 4, 2, 3],
            [20, 6, 5, 3, 4],
            [25, 8, 6, 4, 6],
            [30, 10, 7, 5, 7],
            [35, 11, 8, 6, 8],
            [40, 12, 10, 6, 10],
            [45, 14, 11, 7, 11],
            [50, 16, 12, 8, 12],
            [55, 18, 13, 9, 13],
            [60, 20, 14, 10, 14],
            [65, 21, 15, 10, 15],
            [70, 22, 15, 11, 15],
            [75, 23, 16, 12, 16],
            [80, 25, 17, 13, 17],
            [85, 27, 18, 14, 18],
            [90, 29, 19, 15, 19],
            [95, 30, 20, 16, 20],
            [100, 31, 21, 17, 21],
            [-1, 0, 0, 0, 0],
        ];
        let row = STRUCTURE.iter().find(|row| row[0] == tons as i16)?;
        let column = match section {
            BattleSection::Head => return Some(3),
            BattleSection::CenterTorso => 1,
            BattleSection::LeftTorso | BattleSection::RightTorso => 2,
            BattleSection::LeftArm | BattleSection::RightArm if quad => 4,
            BattleSection::LeftArm | BattleSection::RightArm => 3,
            BattleSection::LeftLeg | BattleSection::RightLeg => 4,
        };
        u16::try_from(row[column]).ok()
    }
}

/// Shared bounded syntax, before unit-specific section and metadata validation.
pub(super) struct ParsedTemplate {
    pub fields: BTreeMap<String, String>,
    pub sections: BTreeMap<String, SectionDefinition>,
}

impl ParsedTemplate {
    /// Decode fields without conflating BattleMech and vehicle anatomy.
    pub fn parse(source: &str) -> Result<Self> {
        ensure!(source.len() <= 1_048_576, "template exceeds size limit");
        let mut fields = BTreeMap::new();
        let mut sections = BTreeMap::<String, SectionDefinition>::new();
        let mut active = None;
        let mut pending = String::new();
        let mut pending_line = 0;
        for (index, line) in source.lines().enumerate() {
            let line = line.trim();
            if pending.is_empty() && (line.is_empty() || line.starts_with('#')) {
                continue;
            }
            if pending.is_empty() {
                pending_line = index + 1;
            }
            if !pending.is_empty() {
                pending.push(' ');
            }
            pending.push_str(line);
            if pending.contains('{') && !pending.contains('}') {
                continue;
            }
            (|| -> Result<()> {
                if !pending.contains('{') {
                    let section = pending.to_ascii_lowercase();
                    ensure!(
                        !sections.contains_key(&section),
                        "duplicate section {pending}"
                    );
                    sections.insert(section.clone(), SectionDefinition::default());
                    active = Some(section);
                    return Ok(());
                }
                let (name, rest) = pending.split_once('{').unwrap();
                let (value, tail) = rest.split_once('}').context("unclosed template field")?;
                ensure!(
                    !value.contains('{') && tail.trim().is_empty(),
                    "invalid template field delimiters"
                );
                let name = name.trim().to_ascii_lowercase();
                ensure!(!name.is_empty(), "missing field name");
                let value = value.trim();
                let local = matches!(name.as_str(), "armor" | "internals" | "rear" | "config")
                    || name.starts_with("crit_");
                if !local {
                    // Comments carry no construction state; retain repeated notes in source order.
                    if name == "comment" {
                        fields
                            .entry(name)
                            .and_modify(|notes: &mut String| {
                                notes.push('\n');
                                notes.push_str(value);
                            })
                            .or_insert_with(|| value.to_owned());
                        return Ok(());
                    }
                    if name == "specials" {
                        let combined = fields.entry(name).or_insert_with(String::new);
                        if combined == "-" {
                            combined.clear();
                        }
                        for flag in value.split_ascii_whitespace().filter(|flag| *flag != "-") {
                            if combined
                                .split_ascii_whitespace()
                                .any(|existing| existing.eq_ignore_ascii_case(flag))
                            {
                                continue;
                            }
                            if !combined.is_empty() {
                                combined.push(' ');
                            }
                            combined.push_str(flag);
                        }
                        if combined.is_empty() {
                            combined.push('-');
                        }
                        return Ok(());
                    }
                    ensure!(
                        fields.insert(name.clone(), value.to_owned()).is_none(),
                        "duplicate field {name}"
                    );
                    return Ok(());
                }
                let section = active.as_ref().context("section field outside a section")?;
                let layout = sections.get_mut(section).unwrap();
                match name.as_str() {
                    "armor" => layout.armor = value.parse().context("invalid armor")?,
                    "internals" => layout.internal = value.parse().context("invalid internals")?,
                    "rear" => layout.rear = value.parse().context("invalid rear armor")?,
                    "config" => layout.configuration = Some(value.to_owned()),
                    _ => {
                        let (first, last) = critical_range(&name)?;
                        let critical = parse_equipment(value)?;
                        for slot in first..=last {
                            ensure!(
                                !layout.criticals.contains_key(&slot),
                                "overlapping critical slot {}",
                                slot + 1
                            );
                            layout.criticals.insert(slot, critical.clone());
                        }
                    }
                }
                Ok(())
            })()
            .with_context(|| format!("template line {pending_line}"))?;
            pending.clear();
        }
        ensure!(
            pending.is_empty(),
            "unclosed template field on line {pending_line}"
        );
        Ok(Self { fields, sections })
    }

    /// Require a nonempty unit-level field while preserving its spelling.
    pub fn required(&self, name: &str) -> Result<String> {
        self.fields
            .get(name)
            .filter(|value| !value.is_empty())
            .cloned()
            .with_context(|| format!("missing template field {name}"))
    }
}

/// System entries may omit metadata; ammunition flags may use whitespace or pipes.
/// Weapon entries retain their explicit data and single mode field.
/// A final numeric field is a brand, and ammunition may instead end with a dash placeholder.
fn parse_equipment(value: &str) -> Result<CriticalDefinition> {
    let words: Vec<_> = value.split_whitespace().collect();
    if words.len() < 3
        && words
            .first()
            .is_some_and(|name| super::BattleSystem::parse(name).is_ok())
    {
        return Ok(CriticalDefinition {
            equipment: words[0].to_owned(),
            data: words.get(1).unwrap_or(&"-").to_string(),
            modes: Vec::new(),
            brand: None,
        });
    }
    ensure!(
        words.len() >= 3,
        "expected equipment, data, modes, and optional brand"
    );
    let ammunition = super::equipment::strip_name_prefix(words[0], "Ammo_").is_some();
    ensure!(
        ammunition || words.len() <= 4,
        "expected equipment, data, modes, and optional brand"
    );
    let mut end = words.len();
    let mut brand = None;
    if words.len() > 3 {
        let last = words[end - 1];
        let numeric = last.strip_prefix(['+', '-']).unwrap_or(last);
        if ammunition && last == "-" {
            end -= 1;
        } else if !ammunition
            || (!numeric.is_empty() && numeric.bytes().all(|c| c.is_ascii_digit()))
        {
            brand = Some(last.parse::<u8>().context("invalid equipment brand")?);
            end -= 1;
        }
    }
    Ok(CriticalDefinition {
        equipment: words[0].to_owned(),
        data: words[1].to_owned(),
        modes: words[2..end]
            .iter()
            .filter(|word| **word != "-")
            .flat_map(|word| word.split('|'))
            .map(str::to_owned)
            .collect(),
        brand,
    })
}

/// Decode one-based asset positions into a bounded, inclusive zero-based range.
fn critical_range(name: &str) -> Result<(u8, u8)> {
    let range = name
        .strip_prefix("crit_")
        .context("invalid critical heading")?;
    let (first, last) = range.split_once('-').unwrap_or((range, range));
    let first: u8 = first.parse().context("invalid first critical position")?;
    let last: u8 = last.parse().context("invalid last critical position")?;
    if first == 0 || first > last || last > 12 {
        bail!("critical positions must be between 1 and 12 in ascending order");
    }
    Ok((first - 1, last - 1))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ammunition_mode_words_preserve_flags_and_validate_brands() {
        for (row, brand) in [
            ("Ammo_IS.MachineGun 100 Hotload Halfton -", None),
            ("Ammo_IS.MachineGun 100 Hotload|Halfton", None),
            ("Ammo_IS.MachineGun 100 Hotload Halfton", None),
            ("Ammo_IS.MachineGun 100 Hotload Halfton 4", Some(4)),
        ] {
            for name in [
                "Ammo_IS.MachineGun",
                "ammo_is.machinegun",
                "AMMO_IS.MACHINEGUN",
            ] {
                let row = row.replace("Ammo_IS.MachineGun", name);
                let definition = parse_equipment(&row).unwrap();
                assert_eq!(definition.equipment, name);
                assert_eq!(definition.data, "100");
                assert_eq!(definition.modes, ["Hotload", "Halfton"]);
                assert_eq!(definition.brand, brand);
            }
        }
        for row in [
            "Ammo_IS.MachineGun 100",
            "Ammo_IS.MachineGun 100 Halfton 256",
            "Ammo_IS.MachineGun 100 Halfton -1",
            "IS.MediumLaser - Hotload nope",
            "IS.MediumLaser - - 256",
            "IS.MediumLaser - Heat Extra 1",
        ] {
            assert!(parse_equipment(row).is_err(), "{row}");
        }
        assert!(
            parse_equipment("Ammo_IS.MachineGun 100 - -")
                .unwrap()
                .modes
                .is_empty()
        );
        assert_eq!(
            parse_equipment("IS.MediumLaser - Heat 4").unwrap().brand,
            Some(4)
        );
    }

    #[test]
    fn abbreviated_system_rows_default_only_missing_metadata() {
        assert_eq!(
            parse_equipment("Ecm").unwrap(),
            parse_equipment("Ecm - -").unwrap()
        );
        assert_eq!(
            parse_equipment("ArtemisIV 2").unwrap(),
            parse_equipment("ArtemisIV 2 -").unwrap()
        );
        for row in ["", "UnknownSystem", "IS.MediumLaser", "Ammo_IS.MachineGun"] {
            assert!(parse_equipment(row).is_err(), "{row}");
        }
    }

    #[test]
    fn critical_ranges_reject_invalid_positions() {
        assert_eq!(critical_range("crit_2-4").unwrap(), (1, 3));
        for name in ["crit_0", "crit_2-1", "crit_13", "crit_1-2-3", "crit_256"] {
            assert!(critical_range(name).is_err());
        }
    }
}
