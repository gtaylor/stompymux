//! Reads MegaMek `.mtf` BattleMech files into drafts.
//!
//! An `.mtf` file is a list of `key:value` lines plus one block of critical slots per location,
//! each headed by the location's name (`Left Arm:`) and holding up to twelve equipment names.
//! Only biped and quad BattleMechs built from technology stompymux implements are accepted.
use crate::Converted;
use crate::draft::{Draft, Section, SplitMount};
use crate::equipment::{self, Critical, HeatSinkKind, TechBase};
use crate::layout::{self, WeaponSpan};
use anyhow::{Context, Result, bail, ensure};
use std::collections::BTreeMap;

/// One location: MegaMek heading, template heading, armor key and rear armor key.
type Location = (
    &'static str,
    &'static str,
    &'static str,
    Option<&'static str>,
);

/// Biped locations in template order.
const BIPED: [Location; 8] = [
    ("left arm", "left_arm", "la", None),
    ("right arm", "right_arm", "ra", None),
    ("left torso", "left_torso", "lt", Some("rtl")),
    ("right torso", "right_torso", "rt", Some("rtr")),
    ("center torso", "center_torso", "ct", Some("rtc")),
    ("left leg", "left_leg", "ll", None),
    ("right leg", "right_leg", "rl", None),
    ("head", "head", "hd", None),
];

/// Quad locations in template order.
const QUAD: [Location; 8] = [
    ("front left leg", "front_left_leg", "fll", None),
    ("front right leg", "front_right_leg", "frl", None),
    ("left torso", "left_torso", "lt", Some("rtl")),
    ("right torso", "right_torso", "rt", Some("rtr")),
    ("center torso", "center_torso", "ct", Some("rtc")),
    ("rear left leg", "rear_left_leg", "rll", None),
    ("rear right leg", "rear_right_leg", "rrl", None),
    ("head", "head", "hd", None),
];

/// Heat dissipation stompymux assumes for a mech template that states none.
const DEFAULT_HEAT_SINKS: u32 = 10;

/// Every location heading an `.mtf` file may use, including ones stompymux refuses.
const HEADINGS: &[&str] = &[
    "left arm",
    "right arm",
    "left torso",
    "right torso",
    "center torso",
    "head",
    "left leg",
    "right leg",
    "front left leg",
    "front right leg",
    "rear left leg",
    "rear right leg",
    "center leg",
];

/// Locations a split weapon may span, as unordered pairs of template headings.
const ADJACENT: &[(&str, &str)] = &[
    ("left_torso", "left_arm"),
    ("left_torso", "left_leg"),
    ("left_torso", "center_torso"),
    ("right_torso", "right_arm"),
    ("right_torso", "right_leg"),
    ("right_torso", "center_torso"),
    ("left_torso", "front_left_leg"),
    ("left_torso", "rear_left_leg"),
    ("right_torso", "front_right_leg"),
    ("right_torso", "rear_right_leg"),
];

/// The raw contents of an `.mtf` file.
struct MechFile {
    /// Lowercase keys and trimmed values; a repeated key keeps its last value.
    fields: BTreeMap<String, String>,
    /// Lowercase location headings and their equipment lines.
    locations: BTreeMap<String, Vec<String>>,
    /// Lowercase design quirks, from the repeated `quirk:` lines.
    quirks: Vec<String>,
}

impl MechFile {
    /// Split a file into fields and location blocks.
    fn read(source: &str) -> Self {
        let mut file = Self {
            fields: BTreeMap::new(),
            locations: BTreeMap::new(),
            quirks: Vec::new(),
        };
        let mut current: Option<String> = None;
        for line in source.lines() {
            let line = line.trim();
            let heading = line
                .strip_suffix(':')
                .map(str::to_ascii_lowercase)
                .filter(|heading| HEADINGS.contains(&heading.as_str()));
            if let Some(heading) = heading {
                file.locations.entry(heading.clone()).or_default();
                current = Some(heading);
                continue;
            }
            if let Some(location) = &current {
                let slots = file.locations.get_mut(location).expect("location opened");
                if !line.is_empty() && slots.len() < 12 {
                    slots.push(line.to_owned());
                    continue;
                }
                current = None;
            }
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if let Some((key, value)) = line.split_once(':') {
                let key = key.trim().to_ascii_lowercase();
                if key == "quirk" {
                    file.quirks.push(value.trim().to_ascii_lowercase());
                    continue;
                }
                file.fields.insert(key, value.trim().to_owned());
            }
        }
        file
    }

    /// A field's value, if present and not blank.
    fn field(&self, key: &str) -> Option<&str> {
        self.fields
            .get(key)
            .map(String::as_str)
            .filter(|value| !value.is_empty())
    }

    /// A field that must be present.
    fn required(&self, key: &str) -> Result<&str> {
        self.field(key).with_context(|| format!("missing {key}"))
    }

    /// A field holding a whole number.
    fn number(&self, key: &str) -> Result<u32> {
        let value = self.required(key)?;
        value
            .parse()
            .with_context(|| format!("{key} {value} is not a whole number"))
    }
}

/// A weapon run that holds only part of a weapon, waiting for its other half.
struct Fragment {
    location: usize,
    first: usize,
    last: usize,
    critical: Critical,
}

impl Fragment {
    /// Slots this fragment occupies.
    fn len(&self) -> usize {
        self.last - self.first + 1
    }
}

/// Convert an `.mtf` file.
pub fn convert(source: &str) -> Result<Converted> {
    let file = MechFile::read(source);
    let chassis = file.required("chassis")?.to_owned();
    let model = file.field("model").unwrap_or_default().to_owned();
    let config = file.required("config")?;
    let lower_config = config.to_ascii_lowercase();
    let omni = lower_config.contains("omnimech") || lower_config.contains("omnimek");
    let movement = match lower_config
        .replace("omnimech", "")
        .replace("omnimek", "")
        .trim()
    {
        "biped" => "biped",
        "quad" => "quad",
        _ => bail!("unsupported unit type: {config} BattleMech"),
    };
    let tech = tech_base(file.required("techbase")?)?;
    let tons: u16 = file
        .number("mass")?
        .try_into()
        .context("mass is out of range")?;
    let mut construction = Vec::new();
    if tech == TechBase::Clan {
        construction.push(("tech_base", "clan"));
    }
    let engine_header = file.required("engine")?;
    let engine_family = engine(engine_header)?;
    construction.extend(engine_family.map(|engine| ("engine", engine)));
    construction.extend(gyro(file.field("gyro"))?.map(|gyro| ("gyro", gyro)));
    construction.extend(cockpit(file.field("cockpit"))?.map(|cockpit| ("cockpit", cockpit)));
    let structure_header = file.required("structure")?;
    let structure_family = structure(structure_header)?;
    construction.extend(structure_family.map(|kind| ("structure", kind)));
    let armor_header = file.required("armor")?;
    let armor_family = armor(armor_header)?;
    construction.extend(armor_family.map(|armor| ("armor", armor)));
    // A mixed-technology unit may take its engine, structure or armor from the other base;
    // record it where the technology base changes slots or mass.
    for (key, family, component, tech_dependent) in [
        (
            "engine_tech",
            engine_family,
            engine_tech(engine_header),
            &["xl", "xxl", "light"][..],
        ),
        (
            "structure_tech",
            structure_family,
            structure_tech(structure_header),
            &["endo_steel"][..],
        ),
        (
            "armor_tech",
            armor_family,
            armor_tech(armor_header),
            &["ferro_fibrous", "laser_reflective"][..],
        ),
    ] {
        if let Some(component) = component
            && component != tech
            && family.is_some_and(|family| tech_dependent.contains(&family))
        {
            construction.push((
                key,
                match component {
                    TechBase::InnerSphere => "inner_sphere",
                    TechBase::Clan => "clan",
                },
            ));
        }
    }
    let (sink_count, sinks) = heat_sinks(file.required("heat sinks")?, tech)?;
    match sinks {
        HeatSinkKind::Double(TechBase::InnerSphere) => construction.push(("heat_sinks", "double")),
        HeatSinkKind::Laser => construction.push(("heat_sinks", "laser")),
        _ => {}
    }
    construction.extend(myomer(file.field("myomer"))?.map(|myomer| ("myomer", myomer)));
    let walk = file.number("walk mp")?;
    let jump = match file.field("jump mp") {
        Some(_) => file.number("jump mp")?,
        None => 0,
    };
    let locations = if movement == "quad" { &QUAD } else { &BIPED };
    let mut specials = Vec::new();
    if omni {
        specials.push("OmniMech_Tech");
    }
    if file.quirks.iter().any(|quirk| quirk == "searchlight") {
        specials.push("SearchLight");
    }
    let mut criticals = Vec::new();
    for (mtf, heading, _, _) in locations {
        let lines = file
            .locations
            .get(*mtf)
            .with_context(|| format!("missing {mtf} critical slots"))?;
        let mut resolved = Vec::new();
        for (slot, line) in lines.iter().enumerate() {
            let critical =
                equipment::parse(line).with_context(|| format!("{heading} slot {}", slot + 1))?;
            match critical {
                Critical::HeatSink(kind) if !same_family(kind, sinks) => {
                    bail!(
                        "{heading} slot {} holds {line}, but the mech declares {sink_count} {:?} heat sinks",
                        slot + 1,
                        sinks
                    )
                }
                Critical::System(item)
                    if equipment::system_tech(line).is_some_and(|item_tech| item_tech != tech) =>
                {
                    bail!(
                        "{heading} slot {}: {line} is mixed technology on this {} chassis, which stompymux cannot represent ({item})",
                        slot + 1,
                        tech.label()
                    )
                }
                Critical::Searchlight => {
                    if !specials.contains(&"SearchLight") {
                        specials.push("SearchLight");
                    }
                    resolved.push(Critical::Empty);
                }
                critical => resolved.push(critical),
            }
        }
        criticals.push(resolved);
    }
    feed_other_tech_launchers(&mut criticals);
    let split_mounts = split_mounts(&mut criticals, locations)?;
    let mut sections = Vec::new();
    for ((_, heading, armor_key, rear_key), criticals) in locations.iter().zip(&criticals) {
        let armor = armor_value(&file, armor_key)?;
        let rear = match rear_key {
            Some(key) => armor_value(&file, key)?,
            None => 0,
        };
        let mut section = Section::new(heading, armor, rear, true);
        section.slots =
            layout::slots(criticals, WeaponSpan::Catalogue).with_context(|| heading.to_string())?;
        sections.push(section);
    }
    let full_reference = if model.is_empty() {
        chassis.clone()
    } else {
        format!("{}-{model}", chassis.replace(' ', ""))
    };
    let reference = if is_designation(&model) {
        model.clone()
    } else {
        full_reference.clone()
    };
    Ok(Converted {
        draft: Draft {
            name: chassis,
            class: "mech",
            movement,
            tons,
            max_mp: (walk * 3).div_ceil(2),
            jump_mp: jump,
            heat_sinks: Some(match sinks {
                HeatSinkKind::Single => sink_count,
                HeatSinkKind::Double(_) | HeatSinkKind::Laser => sink_count * 2,
            })
            .filter(|dissipation| *dissipation != DEFAULT_HEAT_SINKS),
            construction,
            specials,
            sections,
            split_mounts,
        },
        reference,
        full_reference,
        warnings: Vec::new(),
    })
}

/// Whether a model is a self-standing designation such as `AS7-D` or `HER-4K`, letters and
/// digits around a hyphen, rather than a variant label such as `Prime`, `2` or `C 2` that only
/// makes sense beside the chassis name.
fn is_designation(model: &str) -> bool {
    let Some((prefix, suffix)) = model.split_once('-') else {
        return false;
    };
    prefix.chars().any(|c| c.is_ascii_alphabetic())
        && model.chars().any(|c| c.is_ascii_digit())
        && !suffix.is_empty()
        && !model.contains(' ')
}

/// MegaMek shares some ammunition between Inner Sphere and Clan launchers (Narc pods are
/// written as `ISNarc Pods` on Clan units). Feed a bin whose launcher the unit lacks to the
/// other technology base's matching launcher when the unit carries that one instead.
fn feed_other_tech_launchers(criticals: &mut [Vec<Critical>]) {
    let launchers: Vec<_> = criticals
        .iter()
        .flatten()
        .filter_map(|critical| match critical {
            Critical::Weapon { weapon, .. } => Some(*weapon),
            _ => None,
        })
        .collect();
    for critical in criticals.iter_mut().flatten() {
        let Critical::Ammo { weapon, rounds, .. } = critical else {
            continue;
        };
        if launchers.contains(weapon) {
            continue;
        }
        let Some((namespace, label)) = weapon.name().split_once('.') else {
            continue;
        };
        let other = if namespace == "IS" { "CL" } else { "IS" };
        let Ok(fed) = stompymux_rs::BattleWeapon::parse(&format!("{other}.{label}")) else {
            continue;
        };
        if !launchers.contains(&fed) {
            continue;
        }
        let per_ton = weapon.profile().ammunition_per_ton;
        let fed_per_ton = fed.profile().ammunition_per_ton;
        if per_ton > 0 {
            *rounds =
                u16::try_from(u32::from(*rounds) * u32::from(fed_per_ton) / u32::from(per_ton))
                    .unwrap_or(*rounds);
        }
        *weapon = fed;
    }
}

/// Pair weapon runs that hold only part of a weapon with their remainder in an adjacent
/// location, removing both from the location slots and returning them as split mounts.
fn split_mounts(
    criticals: &mut [Vec<Critical>],
    locations: &[Location],
) -> Result<Vec<SplitMount>> {
    let mut fragments = Vec::new();
    for (location, slots) in criticals.iter().enumerate() {
        let mut index = 0;
        while index < slots.len() {
            let Critical::Weapon { weapon, .. } = &slots[index] else {
                index += 1;
                continue;
            };
            let width = usize::from(weapon.profile().critical_slots.max(1));
            let mut end = index + 1;
            while slots.get(end) == Some(&slots[index]) {
                end += 1;
            }
            let remainder = (end - index) % width;
            if remainder > 0 {
                fragments.push(Fragment {
                    location,
                    first: end - remainder,
                    last: end - 1,
                    critical: slots[index].clone(),
                });
            }
            index = end;
        }
    }
    let mut mounts = Vec::new();
    while let Some(fragment) = fragments.pop() {
        let Critical::Weapon { weapon, .. } = fragment.critical.clone() else {
            unreachable!("fragments hold weapons");
        };
        let width = usize::from(weapon.profile().critical_slots);
        let heading = locations[fragment.location].1;
        let partner = fragments.iter().position(|other| {
            other.critical == fragment.critical
                && other.len() + fragment.len() == width
                && adjacent(heading, locations[other.location].1)
        });
        let Some(partner) = partner else {
            bail!(
                "{heading} has {} slot(s) of {} left over after whole weapons ({width} slots each \
                 in stompymux), and no adjacent location holds the rest of a split weapon",
                fragment.len(),
                weapon.name()
            );
        };
        let other = fragments.remove(partner);
        let (primary, extension) = if other.len() >= fragment.len() {
            (other, fragment)
        } else {
            (fragment, other)
        };
        for part in [&primary, &extension] {
            criticals[part.location][part.first..=part.last].fill(Critical::Empty);
        }
        mounts.push(SplitMount {
            item: weapon.name().to_owned(),
            modes: primary.critical.modes(),
            placements: [
                (locations[primary.location].1, primary.first, primary.last),
                (
                    locations[extension.location].1,
                    extension.first,
                    extension.last,
                ),
            ],
        });
    }
    Ok(mounts)
}

/// Whether a split weapon may span two locations.
fn adjacent(first: &str, second: &str) -> bool {
    ADJACENT
        .iter()
        .any(|pair| *pair == (first, second) || *pair == (second, first))
}

/// Whether a heat sink slot belongs to the declared heat sink family.
fn same_family(slot: HeatSinkKind, declared: HeatSinkKind) -> bool {
    matches!(
        (slot, declared),
        (HeatSinkKind::Single, HeatSinkKind::Single)
            | (HeatSinkKind::Double(_), HeatSinkKind::Double(_))
            | (HeatSinkKind::Laser, HeatSinkKind::Laser)
    )
}

/// Read one location's armor points.
fn armor_value(file: &MechFile, key: &str) -> Result<u16> {
    let key = format!("{key} armor");
    file.number(&key)?
        .try_into()
        .with_context(|| format!("{key} is out of range"))
}

/// The chassis technology base; mixed-technology units follow their chassis.
fn tech_base(value: &str) -> Result<TechBase> {
    Ok(match value.trim().to_ascii_lowercase().as_str() {
        "inner sphere" | "mixed (is chassis)" => TechBase::InnerSphere,
        "clan" | "mixed (clan chassis)" => TechBase::Clan,
        _ => bail!("unsupported technology base {value}"),
    })
}

/// The technology base an engine header names (`XL (Clan) Engine`, `XL Engine(IS)`), if any.
fn engine_tech(value: &str) -> Option<TechBase> {
    let lower = value.to_ascii_lowercase();
    if lower.contains("(clan)") {
        Some(TechBase::Clan)
    } else if lower.contains("(is)") || lower.contains("(inner sphere)") {
        Some(TechBase::InnerSphere)
    } else {
        None
    }
}

/// The technology base a structure header names (`IS Endo Steel`, `Clan Endo Steel`), if any.
fn structure_tech(value: &str) -> Option<TechBase> {
    let lower = value.to_ascii_lowercase();
    if lower.starts_with("clan ") {
        Some(TechBase::Clan)
    } else if lower.starts_with("is ") {
        Some(TechBase::InnerSphere)
    } else {
        None
    }
}

/// The technology base an armor header names (`Ferro-Fibrous(Clan)`), if any.
fn armor_tech(value: &str) -> Option<TechBase> {
    engine_tech(value)
}

/// The engine family, or `None` for a standard fusion engine.
fn engine(value: &str) -> Result<Option<&'static str>> {
    let kind = value
        .trim_start_matches(|c: char| c.is_ascii_digit())
        .trim()
        .to_ascii_lowercase();
    for unsupported in [
        "fuel cell",
        "fuel-cell",
        "fission",
        "primitive",
        "battery",
        "solar",
        "steam",
        "maglev",
        "none",
    ] {
        ensure!(!kind.contains(unsupported), "unsupported engine {value}");
    }
    Ok(Some(if kind.contains("xxl") {
        "xxl"
    } else if kind.contains("xl") {
        "xl"
    } else if kind.contains("light") {
        "light"
    } else if kind.contains("compact") {
        "compact"
    } else if kind.contains("ice") || kind.contains("i.c.e") {
        "ice"
    } else if kind.contains("fusion") {
        return Ok(None);
    } else {
        bail!("unsupported engine {value}")
    }))
}

/// The gyro family, or `None` for a standard gyro.
fn gyro(value: Option<&str>) -> Result<Option<&'static str>> {
    let Some(value) = value else {
        return Ok(None);
    };
    Ok(
        match value.to_ascii_lowercase().replace('-', " ").as_str() {
            "standard gyro" | "standard" => None,
            "xl gyro" => Some("xl"),
            "compact gyro" => Some("compact"),
            "heavy duty gyro" => Some("heavy_duty"),
            _ => bail!("unsupported gyro {value}"),
        },
    )
}

/// The cockpit family, or `None` for a standard cockpit.
fn cockpit(value: Option<&str>) -> Result<Option<&'static str>> {
    let Some(value) = value else {
        return Ok(None);
    };
    Ok(match value.to_ascii_lowercase().as_str() {
        "standard cockpit" | "standard" => None,
        "small cockpit" => Some("small"),
        _ => bail!("unsupported cockpit {value}"),
    })
}

/// The internal structure family, or `None` for standard structure.
fn structure(value: &str) -> Result<Option<&'static str>> {
    let lower = value.to_ascii_lowercase().replace('-', " ");
    let kind = lower
        .strip_prefix("is ")
        .or_else(|| lower.strip_prefix("clan "))
        .unwrap_or(&lower);
    Ok(match kind.trim() {
        "standard" => None,
        "endo steel" => Some("endo_steel"),
        "composite" => Some("composite"),
        "reinforced" => Some("reinforced"),
        _ => bail!("unsupported structure {value}"),
    })
}

/// The armor family, or `None` for standard armor.
pub fn armor(value: &str) -> Result<Option<&'static str>> {
    let lower = value.to_ascii_lowercase();
    let kind = ["(inner sphere)", "(clan)", "(is)"]
        .iter()
        .find_map(|suffix| lower.strip_suffix(suffix))
        .unwrap_or(&lower);
    Ok(match kind.trim() {
        "standard" => None,
        "ferro-fibrous" => Some("ferro_fibrous"),
        "light ferro-fibrous" => Some("light_ferro_fibrous"),
        "heavy ferro-fibrous" => Some("heavy_ferro_fibrous"),
        "stealth" => Some("stealth"),
        "hardened" => Some("hardened"),
        "reflective" | "laser reflective" | "laser-reflective" => Some("laser_reflective"),
        _ => bail!("unsupported armor {value}"),
    })
}

/// The myomer family, or `None` for standard myomer.
fn myomer(value: Option<&str>) -> Result<Option<&'static str>> {
    let Some(value) = value else {
        return Ok(None);
    };
    Ok(
        match value.to_ascii_lowercase().replace('-', " ").as_str() {
            "standard" => None,
            "triple strength" => Some("triple_strength"),
            _ => bail!("unsupported myomer {value}"),
        },
    )
}

/// The heat sink count and family, refusing families stompymux cannot mount on this chassis.
fn heat_sinks(value: &str, tech: TechBase) -> Result<(u32, HeatSinkKind)> {
    let (count, kind) = value
        .trim()
        .split_once(' ')
        .unwrap_or((value.trim(), "single"));
    let count = count
        .parse()
        .with_context(|| format!("heat sink count {count} is not a whole number"))?;
    let lower = kind.to_ascii_lowercase();
    let words: Vec<&str> = lower.split_whitespace().collect();
    let kind = if words.contains(&"compact") {
        bail!("unsupported heat sinks {value}")
    } else if words.contains(&"laser") {
        HeatSinkKind::Laser
    } else if words.contains(&"double") {
        HeatSinkKind::Double(if words.contains(&"clan") {
            TechBase::Clan
        } else if words.contains(&"is") {
            TechBase::InnerSphere
        } else {
            tech
        })
    } else if words.is_empty() || words == ["single"] {
        HeatSinkKind::Single
    } else {
        bail!("unsupported heat sinks {value}")
    };
    match (tech, kind) {
        (TechBase::Clan, HeatSinkKind::Single) => {
            bail!("Clan chassis with single heat sinks are unsupported")
        }
        (TechBase::Clan, HeatSinkKind::Double(TechBase::InnerSphere)) => {
            bail!("Clan chassis with Inner Sphere double heat sinks are unsupported")
        }
        (TechBase::InnerSphere, HeatSinkKind::Double(TechBase::Clan) | HeatSinkKind::Laser) => {
            bail!("Inner Sphere chassis with Clan heat sinks are unsupported")
        }
        _ => Ok((count, kind)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A complete standard Inner Sphere biped.
    const ATLAS: &str = include_str!("../../../tests/fixtures/megamek/Atlas AS7-D.mtf");

    /// Convert and validate an `.mtf` source, returning the reference and template.
    fn template(source: &str) -> Result<(String, String)> {
        let converted = convert(source)?;
        let template = crate::finish(&converted.reference, &converted.draft)?;
        Ok((converted.reference, template))
    }

    /// Replace the line `offset` lines below a location heading.
    fn replace_line(source: &str, heading: &str, offset: usize, line: &str) -> String {
        let mut lines: Vec<&str> = source.lines().collect();
        let at = lines
            .iter()
            .position(|candidate| *candidate == heading)
            .unwrap();
        lines[at + offset] = line;
        lines.join("\n")
    }

    #[test]
    fn a_standard_biped_converts_to_a_constructible_template() {
        let (reference, template) = template(ATLAS).unwrap();
        assert_eq!(reference, "AS7-D");
        for line in [
            "name = \"Atlas\"\n",
            "class = \"mech\"\n",
            "walk_mp = 5\n",
            "heat_sinks = 20\n",
            "rear = 14\n",
            "{ at = \"1-10\", item = \"IS.AC/20\" },",
            "{ at = \"11-12\", item = \"Ammo_IS.AC/20\", rounds = 5 },",
            "{ at = \"11-12\", item = \"IS.MediumLaser\", modes = [\"RearMount\"] },",
        ] {
            assert!(template.contains(line), "{line} missing from\n{template}");
        }
        assert!(!template.contains("Engine"));
        assert!(!template.contains("[construction]"));
    }

    #[test]
    fn weapons_that_cross_locations_become_split_mounts() {
        let mut source = ATLAS.to_owned();
        for offset in [9, 10] {
            source = replace_line(&source, "Right Torso:", offset, "-Empty-");
        }
        for offset in [8, 9] {
            source = replace_line(&source, "Right Arm:", offset, "Autocannon/20");
        }
        let (_, template) = template(&source).unwrap();
        assert!(
            template.contains(
                "[[split_mounts]]\nitem = \"IS.AC/20\"\nplacements = [\n    \
                 { section = \"right_torso\", at = \"1-8\" },\n    \
                 { section = \"right_arm\", at = \"8-9\" },\n]"
            ),
            "{template}"
        );
        let stranded = replace_line(ATLAS, "Right Torso:", 10, "-Empty-");
        assert!(template_error(&stranded).contains("left over"));
    }

    #[test]
    fn references_use_designations_and_qualify_variant_labels() {
        assert!(is_designation("AS7-D"));
        assert!(is_designation("HER-4K"));
        for label in ["2", "C 2", "Prime", "IIC", "GDR-", "-D"] {
            assert!(!is_designation(label), "{label}");
        }
        let numbered = ATLAS.replace("model:AS7-D", "model:2");
        let converted = convert(&numbered).unwrap();
        assert_eq!(converted.reference, "Atlas-2");
        assert_eq!(convert(ATLAS).unwrap().full_reference, "Atlas-AS7-D");
    }

    #[test]
    fn omnimechs_and_searchlight_quirks_become_specials() {
        let source = ATLAS
            .replace("Config:Biped", "Config:Biped OmniMek")
            .replace(
                "model:AS7-D",
                "model:AS7-D\nquirk:command_mech\nquirk:searchlight",
            );
        let draft = convert(&source).unwrap().draft;
        assert_eq!(draft.specials, ["OmniMech_Tech", "SearchLight"]);
    }

    #[test]
    fn mixed_technology_components_are_recorded_or_refused() {
        let clan_engine = ATLAS.replace("300 Fusion Engine(IS)", "300 XL (Clan) Engine");
        let draft = convert(&clan_engine).unwrap().draft;
        assert!(draft.construction.contains(&("engine", "xl")));
        assert!(draft.construction.contains(&("engine_tech", "clan")));
        // A standard fusion engine is the same whichever base built it.
        let clan_fusion = ATLAS.replace("300 Fusion Engine(IS)", "300 Fusion (Clan) Engine");
        let draft = convert(&clan_fusion).unwrap().draft;
        assert!(
            !draft
                .construction
                .iter()
                .any(|(key, _)| *key == "engine_tech")
        );
        let clan_case = replace_line(ATLAS, "Left Torso:", 12, "CLCASEII");
        assert!(template_error(&clan_case).contains("mixed technology"));
        assert_eq!(equipment::system_tech("CLCASEII"), Some(TechBase::Clan));
        assert_eq!(
            equipment::system_tech("BeagleActiveProbe (OMNIPOD)"),
            Some(TechBase::InnerSphere)
        );
        assert_eq!(equipment::system_tech("CASE II"), None);
        assert_eq!(equipment::system_tech("Medium Laser"), None);
    }

    #[test]
    fn shared_ammunition_feeds_the_other_base_launcher() {
        let narc = |weapon| Critical::Weapon {
            weapon,
            rear: false,
            one_shot: false,
        };
        let pods = |weapon| Critical::Ammo {
            weapon,
            modes: Vec::new(),
            rounds: 6,
        };
        let mut criticals = vec![vec![
            narc(stompymux_rs::BattleWeapon::ClanNarcBeacon),
            pods(stompymux_rs::BattleWeapon::NarcBeacon),
        ]];
        feed_other_tech_launchers(&mut criticals);
        assert_eq!(
            criticals[0][1],
            pods(stompymux_rs::BattleWeapon::ClanNarcBeacon)
        );
        // Ammunition for a launcher the unit carries is left alone.
        let mut matched = vec![vec![
            narc(stompymux_rs::BattleWeapon::NarcBeacon),
            pods(stompymux_rs::BattleWeapon::NarcBeacon),
        ]];
        feed_other_tech_launchers(&mut matched);
        assert_eq!(matched[0][1], pods(stompymux_rs::BattleWeapon::NarcBeacon));
    }

    /// The full error chain from converting an `.mtf` source.
    fn template_error(source: &str) -> String {
        format!("{:#}", template(source).unwrap_err())
    }

    #[test]
    fn unsupported_units_technology_and_equipment_are_refused() {
        for (from, to, expected) in [
            ("Config:Biped", "Config:LAM", "unsupported unit type: LAM"),
            (
                "Config:Biped",
                "Config:Tripod",
                "unsupported unit type: Tripod",
            ),
            (
                "300 Fusion Engine(IS)",
                "300 Fuel Cell Engine",
                "unsupported engine",
            ),
            (
                "structure:IS Standard",
                "structure:IS Industrial",
                "unsupported structure",
            ),
            (
                "armor:Standard(Inner Sphere)",
                "armor:Patchwork",
                "unsupported armor",
            ),
            (
                "myomer:Standard",
                "myomer:Industrial Triple-Strength",
                "unsupported myomer",
            ),
            (
                "heat sinks:20 Single",
                "heat sinks:20 Compact",
                "unsupported heat sinks",
            ),
            (
                "techbase:Inner Sphere",
                "techbase:Clan",
                "Clan chassis with single heat sinks",
            ),
            (
                "Autocannon/20\nIS Ammo",
                "ISImprovedHeavyGaussRifle\nIS Ammo",
                "unsupported equipment",
            ),
            (
                "IS Ammo SRM-6",
                "IS Ammo SRM-6 Tandem-Charge",
                "unsupported ammunition",
            ),
            (
                "Hand Actuator\nHeat Sink\nMedium",
                "Hand Actuator\nISDoubleHeatSink\nMedium",
                "heat sinks",
            ),
            (
                "Medium Laser (R)",
                "Medium Laser (T)",
                "mech turrets are unsupported",
            ),
            (
                "Upper Arm Actuator",
                "Upper Arm Actuator (ARMORED)",
                "armored components",
            ),
        ] {
            let source = ATLAS.replacen(from, to, 1);
            assert_ne!(source, ATLAS, "{from}");
            let error = template_error(&source);
            assert!(error.contains(expected), "{to}: {error}");
        }
    }

    #[test]
    fn construction_headers_map_to_document_choices() {
        assert_eq!(engine("300 Fusion Engine(IS)").unwrap(), None);
        assert_eq!(engine("250 XL Engine").unwrap(), Some("xl"));
        assert_eq!(engine("220 XL (Clan) Engine").unwrap(), Some("xl"));
        assert_eq!(engine("400 XXL Engine(IS)").unwrap(), Some("xxl"));
        assert_eq!(engine("100 I.C.E.").unwrap(), Some("ice"));
        assert!(engine("100 Fuel Cell Engine").is_err());
        assert_eq!(structure("IS Endo Steel").unwrap(), Some("endo_steel"));
        assert_eq!(structure("Endo-Steel").unwrap(), Some("endo_steel"));
        assert!(structure("Endo-Composite").is_err());
        assert!(structure("IS Industrial").is_err());
        assert_eq!(armor("Ferro-Fibrous(Clan)").unwrap(), Some("ferro_fibrous"));
        assert_eq!(armor("Standard(Inner Sphere)").unwrap(), None);
        assert!(armor("Reactive(Inner Sphere)").is_err());
        assert!(armor("Ferro-Lamellor(Clan)").is_err());
        assert!(cockpit(Some("Torso-Mounted Cockpit")).is_err());
        assert_eq!(gyro(Some("Heavy Duty Gyro")).unwrap(), Some("heavy_duty"));
        assert_eq!(
            heat_sinks("10 Double", TechBase::Clan).unwrap(),
            (10, HeatSinkKind::Double(TechBase::Clan))
        );
        assert_eq!(
            heat_sinks("14 IS Double", TechBase::InnerSphere).unwrap(),
            (14, HeatSinkKind::Double(TechBase::InnerSphere))
        );
        assert!(heat_sinks("10 Single", TechBase::Clan).is_err());
        assert!(heat_sinks("10 Laser", TechBase::InnerSphere).is_err());
        assert!(heat_sinks("10 Compact", TechBase::InnerSphere).is_err());
    }
}
