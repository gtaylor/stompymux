//! Reads MegaMek `.blk` vehicle files into drafts.
//!
//! A `.blk` file is a series of `<tag>` blocks, one value per line. Only combat tanks and VTOLs
//! are accepted; support vehicles, naval units, aerospace craft, infantry and the rest are
//! refused. MegaMek's body location has no stompymux counterpart, so body ammunition joins the
//! location of a weapon it feeds and other body equipment fills the first location with room.
use crate::Converted;
use crate::draft::{Draft, Section};
use crate::equipment::{self, Critical, TechBase};
use crate::layout::{self, WeaponSpan};
use anyhow::{Context, Result, bail, ensure};
use std::collections::BTreeMap;

/// MegaMek equipment blocks with a stompymux location, and the template heading each fills.
const LOCATIONS: [(&str, &str); 6] = [
    ("front equipment", "front_side"),
    ("right equipment", "right_side"),
    ("left equipment", "left_side"),
    ("rear equipment", "aft_side"),
    ("turret equipment", "turret"),
    ("rotor equipment", "rotor"),
];

/// Template headings in output order.
const ORDER: [&str; 6] = [
    "left_side",
    "right_side",
    "front_side",
    "aft_side",
    "turret",
    "rotor",
];

/// The raw contents of a `.blk` file: lowercase tags and their non-blank lines.
struct BlockFile {
    blocks: BTreeMap<String, Vec<String>>,
}

impl BlockFile {
    /// Split a file into tagged blocks.
    fn read(source: &str) -> Self {
        let mut blocks: BTreeMap<String, Vec<String>> = BTreeMap::new();
        let mut current: Option<String> = None;
        for line in source.lines() {
            let line = line.trim();
            if line.starts_with('<') && line.ends_with('>') {
                let tag = line[1..line.len() - 1].trim();
                current = match tag.strip_prefix('/') {
                    Some(_) => None,
                    None => {
                        let tag = tag.to_ascii_lowercase();
                        blocks.entry(tag.clone()).or_default();
                        Some(tag)
                    }
                };
                continue;
            }
            match &current {
                Some(tag) if !line.is_empty() => blocks
                    .get_mut(tag)
                    .expect("block opened")
                    .push(line.to_owned()),
                _ => {}
            }
        }
        Self { blocks }
    }

    /// A block's lines, empty when the block is absent.
    fn lines(&self, tag: &str) -> &[String] {
        self.blocks.get(tag).map_or(&[], Vec::as_slice)
    }

    /// A single-value block, if present.
    fn value(&self, tag: &str) -> Option<&str> {
        self.lines(tag).first().map(String::as_str)
    }

    /// A single-value block that must be present.
    fn required(&self, tag: &str) -> Result<&str> {
        self.value(tag).with_context(|| format!("missing <{tag}>"))
    }

    /// A single whole-number block, if present.
    fn integer(&self, tag: &str) -> Result<Option<i64>> {
        self.value(tag)
            .map(|value| {
                value
                    .parse()
                    .with_context(|| format!("<{tag}> {value} is not a whole number"))
            })
            .transpose()
    }
}

/// Convert a `.blk` file.
pub fn convert(source: &str) -> Result<Converted> {
    let file = BlockFile::read(source);
    let unit_type = file.required("unittype")?;
    let (class, vtol) = match unit_type.to_ascii_lowercase().as_str() {
        "tank" => ("vehicle", false),
        "vtol" => ("vtol", true),
        _ => bail!("unsupported unit type: {unit_type}"),
    };
    let motion = file.required("motion_type")?;
    let movement = match (vtol, motion.to_ascii_lowercase().as_str()) {
        (false, "tracked") => "track",
        (false, "wheeled") => "wheel",
        (false, "hover") => "hover",
        (true, "vtol") => "vtol",
        _ => bail!("unsupported motion type: {motion} {unit_type}"),
    };
    let name = file.required("name")?.to_owned();
    let model = file.value("model").unwrap_or_default().to_owned();
    let tonnage = file.required("tonnage")?;
    let tons: f64 = tonnage
        .parse()
        .with_context(|| format!("tonnage {tonnage} is not a number"))?;
    ensure!(
        tons.fract() == 0.0 && tons >= 1.0,
        "unsupported tonnage {tonnage}: vehicles weigh whole tons"
    );
    ensure!(
        tons <= 100.0,
        "unsupported unit type: superheavy {unit_type} ({tonnage} tons)"
    );
    let tons = tons as u16;
    let tech = tech_from_rules(file.required("type")?)?;
    ensure!(
        file.integer("jumpingmp")?.unwrap_or(0) == 0,
        "jumping vehicles are unsupported"
    );
    for (tag, what) in [
        ("trailer", "trailers"),
        ("hasnocontrolsystems", "vehicles without control systems"),
    ] {
        ensure!(
            file.value(tag)
                .is_none_or(|value| matches!(value, "false" | "0")),
            "{what} are unsupported"
        );
    }
    let cruise = file.integer("cruisemp")?.context("missing <cruiseMP>")?;
    let cruise = u32::try_from(cruise).context("cruiseMP is out of range")?;
    let mut construction = Vec::new();
    if tech == TechBase::Clan {
        construction.push(("tech_base", "clan"));
    }
    let engine = match file
        .integer("engine_type")?
        .context("missing <engine_type>")?
    {
        0 => None,
        1 => Some("ice"),
        2 => Some("xl"),
        3 => Some("xxl"),
        4 => Some("light"),
        5 => Some("compact"),
        6 => bail!("unsupported engine: fuel cell"),
        7 => bail!("unsupported engine: fission"),
        code => bail!("unsupported engine type {code}"),
    };
    construction.extend(engine.map(|engine| ("engine", engine)));
    let structure = match file.integer("internal_type")?.unwrap_or(0) {
        -1 | 0 => None,
        2 => Some("endo_steel"),
        4 => Some("reinforced"),
        5 => Some("composite"),
        code => bail!("unsupported structure type {code}"),
    };
    construction.extend(structure.map(|structure| ("structure", structure)));
    let armor = match file.integer("armor_type")?.unwrap_or(0) {
        0 => None,
        1 => Some("ferro_fibrous"),
        3 => Some("laser_reflective"),
        4 => Some("hardened"),
        5 => Some("light_ferro_fibrous"),
        6 => Some("heavy_ferro_fibrous"),
        code => bail!("unsupported armor type {code}"),
    };
    construction.extend(armor.map(|armor| ("armor", armor)));

    let armor_values = file
        .lines("armor")
        .iter()
        .map(|value| {
            value
                .parse::<u16>()
                .with_context(|| format!("armor value {value} is not a whole number"))
        })
        .collect::<Result<Vec<_>>>()?;
    let headings: &[&str] = match (vtol, armor_values.len()) {
        (false, 4) => &["front_side", "right_side", "left_side", "aft_side"],
        (false, 5) => &[
            "front_side",
            "right_side",
            "left_side",
            "aft_side",
            "turret",
        ],
        (true, 5) => &["front_side", "right_side", "left_side", "aft_side", "rotor"],
        (true, 6) => &[
            "front_side",
            "right_side",
            "left_side",
            "aft_side",
            "rotor",
            "turret",
        ],
        (false, count) => {
            bail!(
                "unsupported tank layout with {count} armor locations (dual turrets or superheavy)"
            )
        }
        (true, count) => bail!("unsupported VTOL layout with {count} armor locations"),
    };
    let armor: BTreeMap<&str, u16> = headings.iter().copied().zip(armor_values).collect();

    for (tag, lines) in &file.blocks {
        ensure!(
            !tag.ends_with(" equipment")
                || lines.is_empty()
                || tag == "body equipment"
                || LOCATIONS.iter().any(|(known, _)| known == tag),
            "unsupported equipment location <{tag}>"
        );
    }
    let mut specials = Vec::new();
    let mut located: BTreeMap<&str, Vec<Critical>> = BTreeMap::new();
    for (tag, heading) in LOCATIONS {
        let lines = file.lines(tag);
        if lines.is_empty() {
            continue;
        }
        ensure!(
            armor.contains_key(heading),
            "<{tag}> lists equipment, but the unit has no {heading}"
        );
        ensure!(heading != "rotor", "rotor-mounted equipment is unsupported");
        for line in lines {
            let critical = vehicle_critical(line).with_context(|| format!("<{tag}>"))?;
            if critical == Critical::Searchlight {
                push_special(&mut specials, "SearchLight");
                continue;
            }
            located.entry(heading).or_default().push(critical);
        }
    }
    for line in file.lines("body equipment") {
        let critical = vehicle_critical(line).context("<body equipment>")?;
        let heading = match &critical {
            Critical::Searchlight => {
                push_special(&mut specials, "SearchLight");
                continue;
            }
            Critical::Ammo { weapon, .. } => ORDER.into_iter().find(|heading| {
                located.get(heading).is_some_and(|criticals| {
                    criticals.iter().any(|candidate| {
                        matches!(candidate, Critical::Weapon { weapon: fed, .. } if fed == weapon)
                    })
                })
            }),
            _ => None,
        };
        let heading = heading
            .filter(|heading| located[heading].len() < 12)
            .or_else(|| {
                [
                    "front_side",
                    "right_side",
                    "left_side",
                    "aft_side",
                    "turret",
                ]
                .into_iter()
                .find(|heading| {
                    armor.contains_key(heading)
                        && located.get(heading).is_none_or(|items| items.len() < 12)
                })
            })
            .with_context(|| format!("no location has room for body equipment {line}"))?;
        located.entry(heading).or_default().push(critical);
    }

    let energy_heat: u32 = located
        .values()
        .flatten()
        .filter_map(|critical| match critical {
            Critical::Weapon { weapon, .. } if weapon.is_energy() => {
                Some(u32::from(weapon.profile().heat))
            }
            _ => None,
        })
        .sum();
    // Energy weapons need heat sinks for their heat; fusion engines carry ten for free.
    // A template that states no heat sinks gets exactly that free allowance.
    let efficiency = if tech == TechBase::Clan { 2 } else { 1 };
    let free = if engine == Some("ice") { 0 } else { 10 };
    let heat_sinks = energy_heat.div_ceil(efficiency).max(free) * efficiency;

    let mut sections = Vec::new();
    for heading in ORDER {
        let Some(&armor) = armor.get(heading) else {
            continue;
        };
        let mut section = Section::new(heading, armor, 0, false);
        let criticals = located.get(heading).map_or(&[][..], Vec::as_slice);
        section.slots =
            layout::slots(criticals, WeaponSpan::Single).with_context(|| heading.to_string())?;
        sections.push(section);
    }
    let mut warnings = Vec::new();
    if !file.lines("transporters").is_empty() {
        warnings.push("ignoring transporter (troop and cargo) capacity".to_owned());
    }
    let reference = if model.is_empty() {
        name.clone()
    } else {
        format!("{name} {model}")
    };
    Ok(Converted {
        full_reference: reference.clone(),
        reference,
        draft: Draft {
            name,
            class,
            movement,
            tons,
            max_mp: (cruise * 3).div_ceil(2),
            jump_mp: 0,
            heat_sinks: (heat_sinks != free).then_some(heat_sinks),
            construction,
            specials,
            sections,
            split_mounts: Vec::new(),
        },
        warnings,
    })
}

/// Resolve one vehicle equipment line, refusing mech-only equipment.
fn vehicle_critical(line: &str) -> Result<Critical> {
    let critical = equipment::parse(line)?;
    match &critical {
        Critical::Fixed(_) | Critical::HeatSink(_) => {
            bail!("{line} cannot be mounted on a vehicle")
        }
        Critical::Weapon { rear: true, .. } => {
            bail!("rear-mounted vehicle weapons are unsupported: {line}")
        }
        _ => Ok(critical),
    }
}

/// Technology base from a rules line such as `Clan Level 2` or `Mixed (IS Chassis) Level 3`.
fn tech_from_rules(rules: &str) -> Result<TechBase> {
    let lower = rules.to_ascii_lowercase();
    if lower.starts_with("mixed (clan chassis)") || lower.starts_with("clan") {
        return Ok(TechBase::Clan);
    }
    if lower.starts_with("mixed (is chassis)") || lower.starts_with("is") {
        return Ok(TechBase::InnerSphere);
    }
    bail!("unsupported technology base {rules}")
}

/// Record a chassis special once.
fn push_special(specials: &mut Vec<&'static str>, special: &'static str) {
    if !specials.contains(&special) {
        specials.push(special);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A tracked tank with a turret and body ammunition.
    const BULLDOG: &str = include_str!("../../../tests/fixtures/megamek/Bulldog Medium Tank.blk");

    /// Convert and validate a `.blk` source, returning the reference and template.
    fn template(source: &str) -> Result<(String, String)> {
        let converted = convert(source)?;
        let template = crate::finish(&converted.reference, &converted.draft)?;
        Ok((converted.reference, template))
    }

    #[test]
    fn a_turreted_tank_converts_with_body_ammunition_beside_its_weapons() {
        let (reference, template) = template(BULLDOG).unwrap();
        assert_eq!(reference, "Bulldog Medium Tank");
        for expected in [
            "class = \"vehicle\"\nmovement = \"track\"\ntons = 60\nwalk_mp = 6\nheat_sinks = 8\n",
            "[construction]\nengine = \"ice\"\n",
            "[sections.front_side]\narmor = 24\nslots = [\n    { at = 1, item = \"IS.MachineGun\" },\n    \
             { at = 2, item = \"Ammo_IS.MachineGun\", rounds = 100, modes = [\"Halfton\"] },\n]",
            "[sections.turret]\narmor = 20\nslots = [\n    { at = \"1-2\", item = \"IS.SRM-4\" },\n    \
             { at = 3, item = \"IS.LargeLaser\" },\n    { at = \"4-5\", item = \"Ammo_IS.SRM-4\", rounds = 25 },\n]",
        ] {
            assert!(
                template.contains(expected),
                "{expected} missing from\n{template}"
            );
        }
    }

    #[test]
    fn a_vtol_gets_a_rotor_and_fusion_engines_keep_default_cooling() {
        let source = BULLDOG
            .replace("<UnitType>\nTank", "<UnitType>\nVTOL")
            .replace("Tracked", "VTOL")
            .replace("<engine_type>\n1", "<engine_type>\n0")
            .replace("60.0", "30.0")
            .replace("SRM 4\nSRM 4\nLarge Laser\n", "")
            .replace("IS Ammo SRM-4\nIS Ammo SRM-4\n", "")
            .replace("<armor>\n24\n20\n20\n20\n20", "<armor>\n10\n8\n8\n6\n2");
        let (_, template) = template(&source).unwrap();
        assert!(template.contains("class = \"vtol\"\nmovement = \"vtol\""));
        assert!(template.contains("[sections.rotor]\narmor = 2\n"));
        assert!(!template.contains("heat_sinks"));
        assert!(!template.contains("[construction]"));
    }

    #[test]
    fn unsupported_vehicles_technology_and_equipment_are_refused() {
        for (from, to, expected) in [
            (
                "<UnitType>\nTank",
                "<UnitType>\nSupportTank",
                "unsupported unit type: SupportTank",
            ),
            (
                "<UnitType>\nTank",
                "<UnitType>\nNaval",
                "unsupported unit type: Naval",
            ),
            ("Tracked", "WiGE", "unsupported motion type"),
            ("60.0", "150.0", "superheavy"),
            ("<engine_type>\n1", "<engine_type>\n6", "fuel cell"),
            (
                "<armor>\n24",
                "<armor_type>\n22\n</armor_type>\n<armor>\n24",
                "unsupported armor type 22",
            ),
            ("<armor>\n24", "<armor>\n10\n24", "dual turrets"),
            (
                "Machine Gun\n</Front",
                "Paramedic Equipment\n</Front",
                "unsupported equipment",
            ),
            (
                "Machine Gun\n</Front",
                "Medium Laser (R)\n</Front",
                "rear-mounted vehicle weapons",
            ),
            (
                "IS Ammo SRM-4\n",
                "IS Ammo SRM-4 Thunder\n",
                "unsupported ammunition",
            ),
            (
                "<Rear Equipment>",
                "<Front Left Equipment>\nMachine Gun\n</Front Left Equipment>\n<Rear Equipment>",
                "unsupported equipment location",
            ),
            (
                "<tonnage>",
                "<trailer>\ntrue\n</trailer>\n<tonnage>",
                "trailers",
            ),
            (
                "<tonnage>",
                "<jumpingMP>\n2\n</jumpingMP>\n<tonnage>",
                "jumping",
            ),
        ] {
            let source = BULLDOG.replacen(from, to, 1);
            assert_ne!(source, BULLDOG, "{from}");
            let error = format!("{:#}", template(&source).unwrap_err());
            assert!(error.contains(expected), "{to}: {error}");
        }
    }
}
