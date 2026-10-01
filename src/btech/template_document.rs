//! TOML unit template documents: the on-disk syntax shared by every unit class.
//!
//! A document decodes into [`ParsedTemplate`], the class-neutral field and
//! section layout that the mech, vehicle and raw decoders validate further.
//! Rendering goes the other way, so saved units use the same syntax as the
//! stock assets.
//!
//! ```toml
//! name = "Zeus"
//! class = "mech"
//! movement = "biped"
//! tons = 80
//! max_speed = 64.5
//! specials = ["DoubleHS"]
//!
//! [sections.left_arm]
//! armor = 22
//! internals = 13
//! slots = [
//!     { at = 1, item = "ShoulderOrHip", brand = 3 },
//!     { at = "4-6", item = "IS.ERPPC", brand = 3 },
//! ]
//!
//! [[split_mounts]]
//! item = "IS.AC/20"
//! placements = [
//!     { section = "center_torso", at = "11-12" },
//!     { section = "left_torso", at = "1-8" },
//! ]
//! ```
use super::{BattleSection, CriticalDefinition, RawMovement, RawUnitClass, SectionDefinition};
use anyhow::{Context, Result, bail, ensure};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::fmt::Write as _;

/// Largest template source accepted from disk or scripts.
pub(super) const TEMPLATE_SIZE_LIMIT: usize = 1_048_576;

/// Equipment name of the internal slot that extends a split mount into a left-side section.
pub(super) const SPLIT_LEFT: &str = "SplitCrit_Left";

/// Equipment name of the internal slot that extends a split mount into a right-side section.
pub(super) const SPLIT_RIGHT: &str = "SplitCrit_Right";

/// How a unit-level document value is typed.
#[derive(Clone, Copy)]
enum Kind {
    Integer,
    Float,
    Text,
    Flags,
}

/// Unit-level document keys, their internal attribute names and types, in rendering order.
const FIELDS: &[(&str, &str, Kind)] = &[
    ("name", "name", Kind::Text),
    ("class", "type", Kind::Text),
    ("movement", "move_type", Kind::Text),
    ("tons", "tons", Kind::Integer),
    ("max_speed", "max_speed", Kind::Float),
    ("jump_speed", "jump_speed", Kind::Float),
    ("template_speed", "template_speed", Kind::Float),
    ("heat_sinks", "heat_sinks", Kind::Integer),
    ("hs_engine_override", "hsengoverride", Kind::Integer),
    ("computer", "computer", Kind::Integer),
    ("radio", "radio", Kind::Integer),
    ("radio_type", "radiotype", Kind::Integer),
    ("radio_range", "radio_range", Kind::Integer),
    ("tac_range", "tac_range", Kind::Integer),
    ("lrs_range", "lrs_range", Kind::Integer),
    ("scan_range", "scan_range", Kind::Integer),
    ("si", "si", Kind::Integer),
    ("fuel", "fuel", Kind::Integer),
    ("cargo_space", "cargo_space", Kind::Integer),
    ("max_suits", "max_suits", Kind::Integer),
    ("max_ton", "max_ton", Kind::Integer),
    ("carrier_maximum_tonnage", "carrier_maximum_tonnage", Kind::Integer),
    ("unit_era", "unit_era", Kind::Text),
    ("unit_tro", "unit_tro", Kind::Text),
    ("specials", "specials", Kind::Flags),
    ("infantry_specials", "infantryspecials", Kind::Flags),
];

/// Internal attributes that a rendered document carries elsewhere or not at all.
const UNRENDERED: &[&str] = &[
    "reference",
    "comment",
    "administrative_tonnage",
    "administrative_unit_type",
    "administrative_movement_type",
];

/// One section's protection and occupied slots.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SectionDocument {
    #[serde(default)]
    armor: u16,
    #[serde(default)]
    internals: u16,
    #[serde(default)]
    rear: u16,
    config: Option<String>,
    #[serde(default)]
    slots: Vec<SlotDocument>,
}

/// One item occupying a contiguous run of slots in a single section.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SlotDocument {
    at: SlotRange,
    item: String,
    rounds: Option<u32>,
    link: Option<i32>,
    #[serde(default)]
    modes: Vec<String>,
    brand: Option<u8>,
}

/// A weapon whose slots continue from its primary section into one adjacent section.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SplitMountDocument {
    item: String,
    #[serde(default)]
    modes: Vec<String>,
    brand: Option<u8>,
    placements: Vec<PlacementDocument>,
}

/// The slots one split mount occupies in one section.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PlacementDocument {
    section: String,
    at: SlotRange,
}

/// A one-based slot or an inclusive `"first-last"` run.
#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum SlotRange {
    One(i64),
    Run(String),
}

impl SlotRange {
    /// Decode into an inclusive zero-based range within a twelve-slot section.
    fn bounds(&self) -> Result<(u8, u8)> {
        let (first, last) = match self {
            Self::One(slot) => (*slot, *slot),
            Self::Run(run) => {
                let (first, last) = run.split_once('-').unwrap_or((run, run));
                (
                    first.trim().parse().context("invalid first slot")?,
                    last.trim().parse().context("invalid last slot")?,
                )
            }
        };
        ensure!(
            (1..=12).contains(&first) && (first..=12).contains(&last),
            "slots must be between 1 and 12 in ascending order"
        );
        Ok((first as u8 - 1, last as u8 - 1))
    }
}

/// Class-neutral fields and section layouts, before unit-specific validation.
///
/// Field keys use the internal attribute names in [`FIELDS`]; section keys are
/// lowercase template headings such as `left_arm` or `front_left_leg`.
pub(super) struct ParsedTemplate {
    pub fields: BTreeMap<String, String>,
    pub sections: BTreeMap<String, SectionDefinition>,
}

impl ParsedTemplate {
    /// Decode a TOML document, recording `reference` as the unit's identity.
    pub fn parse(reference: &str, source: &str) -> Result<Self> {
        ensure!(
            source.len() <= TEMPLATE_SIZE_LIMIT,
            "template exceeds size limit"
        );
        let mut document: toml::Table = toml::from_str(source)?;
        let sections = document.remove("sections");
        let split_mounts = document.remove("split_mounts");
        let mut fields = BTreeMap::new();
        if !reference.is_empty() {
            fields.insert("reference".to_owned(), reference.to_owned());
        }
        for (key, value) in document {
            let (_, attribute, kind) = FIELDS
                .iter()
                .find(|(name, _, _)| *name == key)
                .with_context(|| format!("unsupported template field {key}"))?;
            if let Some(value) = decode_field(&key, *kind, value)? {
                fields.insert((*attribute).to_owned(), value);
            }
        }
        let mut parsed = Self {
            fields,
            sections: BTreeMap::new(),
        };
        if let Some(sections) = sections {
            let toml::Value::Table(sections) = sections else {
                bail!("sections must be a table");
            };
            for (name, section) in sections {
                parsed
                    .add_section(&name, section)
                    .with_context(|| format!("section {name}"))?;
            }
        }
        if let Some(mounts) = split_mounts {
            let mounts: Vec<SplitMountDocument> =
                mounts.try_into().context("invalid split_mounts")?;
            for mount in mounts {
                let item = mount.item.clone();
                parsed
                    .add_split_mount(mount)
                    .with_context(|| format!("split mount {item}"))?;
            }
        }
        Ok(parsed)
    }

    /// Require a nonempty unit-level field while preserving its spelling.
    pub fn required(&self, name: &str) -> Result<String> {
        self.fields
            .get(name)
            .filter(|value| !value.is_empty())
            .cloned()
            .with_context(|| format!("missing template field {name}"))
    }

    /// Decode one `[sections.<name>]` table and its single-section slots.
    fn add_section(&mut self, name: &str, section: toml::Value) -> Result<()> {
        let key = name.to_ascii_lowercase();
        ensure!(!self.sections.contains_key(&key), "duplicate section");
        let section: SectionDocument = section.try_into()?;
        let mut layout = SectionDefinition {
            armor: section.armor,
            internal: section.internals,
            rear: section.rear,
            criticals: BTreeMap::new(),
            configuration: section.config,
        };
        for slot in section.slots {
            let (first, last) = slot
                .at
                .bounds()
                .with_context(|| format!("item {}", slot.item))?;
            let critical = slot_critical(slot)?;
            occupy(&mut layout, first, last, &critical)?;
        }
        self.sections.insert(key, layout);
        Ok(())
    }

    /// Expand a split mount into its primary run plus linked extension slots.
    fn add_split_mount(&mut self, mount: SplitMountDocument) -> Result<()> {
        ensure!(
            self.fields
                .get("type")
                .is_some_and(|class| class.eq_ignore_ascii_case("Mech")),
            "only mechs support split mounts"
        );
        let [primary, extension] = mount.placements.as_slice() else {
            bail!("split mounts span exactly two sections");
        };
        let primary_section = mech_section(&primary.section)?;
        let extension_section = mech_section(&extension.section)?;
        ensure!(
            split_adjacent(primary_section, extension_section),
            "{} and {} are not adjacent",
            primary.section,
            extension.section
        );
        ensure!(
            !mount
                .item
                .get(..5)
                .is_some_and(|head| head.eq_ignore_ascii_case("Ammo_")),
            "ammunition cannot be split"
        );
        let (first, last) = primary.at.bounds()?;
        let weapon = CriticalDefinition {
            equipment: mount.item,
            data: "-".into(),
            modes: mount.modes,
            brand: mount.brand,
        };
        let proxy = CriticalDefinition {
            equipment: split_proxy_name(primary_section, extension_section).into(),
            data: split_link_data(primary_section, first),
            modes: Vec::new(),
            brand: weapon.brand,
        };
        let layout = self
            .sections
            .get_mut(&primary.section.to_ascii_lowercase())
            .with_context(|| format!("missing section {}", primary.section))?;
        occupy(layout, first, last, &weapon)?;
        let (first, last) = extension.at.bounds()?;
        let layout = self
            .sections
            .get_mut(&extension.section.to_ascii_lowercase())
            .with_context(|| format!("missing section {}", extension.section))?;
        occupy(layout, first, last, &proxy)
    }
}

/// Convert one typed document value into its internal attribute spelling.
fn decode_field(key: &str, kind: Kind, value: toml::Value) -> Result<Option<String>> {
    Ok(Some(match (kind, value) {
        (Kind::Integer, toml::Value::Integer(value)) => i32::try_from(value)
            .with_context(|| format!("{key} is out of range"))?
            .to_string(),
        (Kind::Float, toml::Value::Integer(value)) => (value as f64).to_string(),
        (Kind::Float, toml::Value::Float(value)) => {
            ensure!(value.is_finite(), "{key} must be finite");
            value.to_string()
        }
        (Kind::Text, toml::Value::String(value)) => match key {
            "class" => {
                ensure!(value == value.to_ascii_lowercase(), "class must be lowercase");
                RawUnitClass::parse(&value)?.name().to_owned()
            }
            "movement" => {
                ensure!(
                    value == value.to_ascii_lowercase(),
                    "movement must be lowercase"
                );
                RawMovement::parse(&value)?.name().to_owned()
            }
            _ => value,
        },
        (Kind::Flags, toml::Value::Array(values)) => {
            let mut flags: Vec<String> = Vec::new();
            for value in values {
                let toml::Value::String(flag) = value else {
                    bail!("{key} entries must be strings");
                };
                ensure!(
                    !flag.is_empty() && !flag.contains(char::is_whitespace),
                    "{key} entries must be single words"
                );
                if !flags.iter().any(|known| known.eq_ignore_ascii_case(&flag)) {
                    flags.push(flag);
                }
            }
            if flags.is_empty() {
                return Ok(None);
            }
            flags.join(" ")
        }
        (Kind::Integer, _) => bail!("{key} must be an integer"),
        (Kind::Float, _) => bail!("{key} must be a number"),
        (Kind::Text, _) => bail!("{key} must be a string"),
        (Kind::Flags, _) => bail!("{key} must be an array of strings"),
    }))
}

/// Build the stored critical for one single-section slot entry.
fn slot_critical(slot: SlotDocument) -> Result<CriticalDefinition> {
    let ammunition = slot
        .item
        .get(..5)
        .is_some_and(|head| head.eq_ignore_ascii_case("Ammo_"));
    ensure!(
        !slot.item.eq_ignore_ascii_case(SPLIT_LEFT) && !slot.item.eq_ignore_ascii_case(SPLIT_RIGHT),
        "use split_mounts for weapons that span sections"
    );
    ensure!(
        ammunition || slot.rounds.is_none(),
        "only ammunition takes rounds"
    );
    ensure!(
        !ammunition || slot.link.is_none(),
        "ammunition cannot take a link"
    );
    ensure!(
        slot.modes
            .iter()
            .all(|mode| !mode.is_empty() && !mode.contains(char::is_whitespace)),
        "modes must be single words"
    );
    let data = slot
        .rounds
        .map(|rounds| rounds.to_string())
        .or_else(|| slot.link.map(|link| link.to_string()))
        .unwrap_or_else(|| "-".into());
    Ok(CriticalDefinition {
        equipment: slot.item,
        data,
        modes: slot.modes,
        brand: slot.brand,
    })
}

/// Install one critical across an inclusive slot range, refusing overlaps.
fn occupy(
    layout: &mut SectionDefinition,
    first: u8,
    last: u8,
    critical: &CriticalDefinition,
) -> Result<()> {
    for slot in first..=last {
        ensure!(
            layout
                .criticals
                .insert(slot, critical.clone())
                .is_none(),
            "overlapping slot {}",
            slot + 1
        );
    }
    Ok(())
}

/// Resolve a biped or quad section heading to its stable mech section.
fn mech_section(heading: &str) -> Result<BattleSection> {
    super::BattleMechChassis::Biped
        .parse_section(heading)
        .or_else(|_| super::BattleMechChassis::Quad.parse_section(heading))
}

/// Whether a split mount may continue from one section into the other.
pub(super) fn split_adjacent(primary: BattleSection, extension: BattleSection) -> bool {
    use BattleSection::*;
    matches!(
        (primary, extension),
        (LeftArm | LeftLeg | CenterTorso, LeftTorso)
            | (LeftTorso, LeftArm | LeftLeg | CenterTorso)
            | (RightArm | RightLeg | CenterTorso, RightTorso)
            | (RightTorso, RightArm | RightLeg | CenterTorso)
    )
}

/// The extension marker names the side of the body the mount occupies.
pub(super) fn split_proxy_name(primary: BattleSection, extension: BattleSection) -> &'static str {
    use BattleSection::*;
    if [primary, extension]
        .iter()
        .any(|section| matches!(section, LeftArm | LeftTorso | LeftLeg))
    {
        SPLIT_LEFT
    } else {
        SPLIT_RIGHT
    }
}

/// Encode an extension slot's link to the primary section and its first zero-based slot.
pub(super) fn split_link_data(section: BattleSection, slot: u8) -> String {
    format!("{}:{slot}", section.name())
}

/// Decode an extension slot's link to the primary section and its first zero-based slot.
pub(super) fn parse_split_link(data: &str) -> Result<(BattleSection, u8)> {
    let (section, slot) = data
        .split_once(':')
        .context("Invalid split critical parent")?;
    Ok((
        BattleSection::parse(section)?,
        slot.parse().context("Invalid split critical parent slot")?,
    ))
}

/// One section to render, in document order.
pub(super) struct RenderSection<'a> {
    /// Lowercase document heading such as `left_arm`.
    pub heading: String,
    /// The stable mech section, so split links can name their sections.
    pub mech: Option<BattleSection>,
    pub layout: &'a SectionDefinition,
}

/// Render internal attributes and section layouts as a TOML template document.
pub(super) fn render(
    attributes: &BTreeMap<String, String>,
    sections: &[RenderSection<'_>],
) -> Result<String> {
    for key in attributes.keys() {
        ensure!(
            UNRENDERED.contains(&key.as_str())
                || FIELDS.iter().any(|(_, attribute, _)| attribute == key),
            "unsupported template attribute {key}"
        );
    }
    let mut output = String::new();
    for (name, attribute, kind) in FIELDS {
        let Some(value) = attributes.get(*attribute) else {
            continue;
        };
        let rendered = match kind {
            Kind::Integer => value
                .trim()
                .parse::<i64>()
                .with_context(|| format!("invalid {attribute} {value}"))?
                .to_string(),
            Kind::Float => {
                let value: f64 = value
                    .trim()
                    .parse()
                    .with_context(|| format!("invalid {attribute} {value}"))?;
                toml::Value::Float(value).to_string()
            }
            Kind::Text if matches!(*name, "class" | "movement") => quote(&value.to_ascii_lowercase()),
            Kind::Text => quote(value),
            Kind::Flags => {
                let flags: Vec<_> = value
                    .split_ascii_whitespace()
                    .filter(|flag| *flag != "-")
                    .map(str::to_owned)
                    .collect();
                if flags.is_empty() {
                    continue;
                }
                quoted_list(&flags)
            }
        };
        writeln!(output, "{name} = {rendered}")?;
    }
    let mounts = split_mounts(sections)?;
    for section in sections {
        render_section(&mut output, section, &mounts)?;
    }
    for mount in &mounts {
        render_split_mount(&mut output, mount, sections)?;
    }
    Ok(output)
}

/// A split mount recovered from its primary run and linked extension slots.
struct RenderedMount {
    primary: BattleSection,
    first: u8,
    last: u8,
    extension: BattleSection,
    extension_slots: Vec<u8>,
}

/// Group linked extension slots with the primary runs they continue.
fn split_mounts(sections: &[RenderSection<'_>]) -> Result<Vec<RenderedMount>> {
    let mut mounts: Vec<RenderedMount> = Vec::new();
    for section in sections {
        for (&slot, critical) in &section.layout.criticals {
            if !is_split_proxy(&critical.equipment) {
                continue;
            }
            let extension = section
                .mech
                .context("split mount outside a mech section")?;
            let (primary, first) = parse_split_link(&critical.data)?;
            if let Some(mount) = mounts
                .iter_mut()
                .find(|mount| mount.primary == primary && mount.first == first)
            {
                ensure!(
                    mount.extension == extension,
                    "split mount spans more than two sections"
                );
                mount.extension_slots.push(slot);
                continue;
            }
            let layout = sections
                .iter()
                .find(|candidate| candidate.mech == Some(primary))
                .map(|candidate| candidate.layout)
                .context("split mount primary section is missing")?;
            let weapon = layout
                .criticals
                .get(&first)
                .context("split mount primary slot is empty")?;
            let mut last = first;
            let local = primary_slot_count(weapon, sections, primary, first);
            while layout.criticals.get(&(last + 1)) == Some(weapon)
                && local.is_none_or(|count| last + 1 < first + count)
            {
                last += 1;
            }
            mounts.push(RenderedMount {
                primary,
                first,
                last,
                extension,
                extension_slots: vec![slot],
            });
        }
    }
    Ok(mounts)
}

/// Slots a split weapon occupies in its primary section, when its profile is known.
fn primary_slot_count(
    weapon: &CriticalDefinition,
    sections: &[RenderSection<'_>],
    primary: BattleSection,
    first: u8,
) -> Option<u8> {
    let name = super::loadout::unbranded_weapon_name(&weapon.equipment).unwrap_or(&weapon.equipment);
    let total = super::BattleWeapon::parse(name).ok()?.profile().critical_slots;
    let link = split_link_data(primary, first);
    let extension = sections
        .iter()
        .flat_map(|section| section.layout.criticals.values())
        .filter(|critical| is_split_proxy(&critical.equipment) && critical.data == link)
        .count();
    total.checked_sub(u8::try_from(extension).ok()?)
}

/// Whether a stored critical is a split-mount extension marker.
pub(super) fn is_split_proxy(equipment: &str) -> bool {
    equipment.eq_ignore_ascii_case(SPLIT_LEFT) || equipment.eq_ignore_ascii_case(SPLIT_RIGHT)
}

/// Write one section table, compressing identical neighbouring slots into runs.
fn render_section(
    output: &mut String,
    section: &RenderSection<'_>,
    mounts: &[RenderedMount],
) -> Result<()> {
    let layout = section.layout;
    writeln!(output, "\n[sections.{}]", section.heading)?;
    writeln!(output, "armor = {}", layout.armor)?;
    writeln!(output, "internals = {}", layout.internal)?;
    if layout.rear > 0 {
        writeln!(output, "rear = {}", layout.rear)?;
    }
    if let Some(config) = &layout.configuration {
        writeln!(output, "config = {}", quote(config))?;
    }
    let in_mount = |slot: u8| {
        mounts.iter().any(|mount| {
            (Some(mount.primary) == section.mech && (mount.first..=mount.last).contains(&slot))
                || (Some(mount.extension) == section.mech && mount.extension_slots.contains(&slot))
        })
    };
    let mut runs: Vec<(u8, u8, &CriticalDefinition)> = Vec::new();
    for (&slot, critical) in &layout.criticals {
        if in_mount(slot) {
            continue;
        }
        ensure!(
            !is_split_proxy(&critical.equipment),
            "unlinked split critical in {}",
            section.heading
        );
        match runs.last_mut() {
            Some((_, last, previous)) if *last + 1 == slot && *previous == critical => {
                *last = slot;
            }
            _ => runs.push((slot, slot, critical)),
        }
    }
    if runs.is_empty() {
        return Ok(());
    }
    writeln!(output, "slots = [")?;
    for (first, last, critical) in runs {
        let mut entry = format!("at = {}, item = {}", range(first, last), quote(&critical.equipment));
        if critical.data != "-" {
            let value: i64 = critical.data.parse().with_context(|| {
                format!("unsupported data {} on {}", critical.data, critical.equipment)
            })?;
            let ammunition = critical
                .equipment
                .get(..5)
                .is_some_and(|head| head.eq_ignore_ascii_case("Ammo_"));
            let key = if ammunition { "rounds" } else { "link" };
            write!(entry, ", {key} = {value}")?;
        }
        push_metadata(&mut entry, critical);
        writeln!(output, "    {{ {entry} }},")?;
    }
    writeln!(output, "]")?;
    Ok(())
}

/// Write one `[[split_mounts]]` entry.
fn render_split_mount(
    output: &mut String,
    mount: &RenderedMount,
    sections: &[RenderSection<'_>],
) -> Result<()> {
    let primary = sections
        .iter()
        .find(|section| section.mech == Some(mount.primary))
        .context("split mount primary section is missing")?;
    let extension = sections
        .iter()
        .find(|section| section.mech == Some(mount.extension))
        .context("split mount extension section is missing")?;
    let weapon = &primary.layout.criticals[&mount.first];
    let first = *mount.extension_slots.iter().min().unwrap();
    let last = *mount.extension_slots.iter().max().unwrap();
    ensure!(
        usize::from(last - first) + 1 == mount.extension_slots.len(),
        "split mount extension is not contiguous"
    );
    writeln!(output, "\n[[split_mounts]]")?;
    writeln!(output, "item = {}", quote(&weapon.equipment))?;
    if !weapon.modes.is_empty() {
        writeln!(output, "modes = {}", quoted_list(&weapon.modes))?;
    }
    if let Some(brand) = weapon.brand {
        writeln!(output, "brand = {brand}")?;
    }
    writeln!(output, "placements = [")?;
    writeln!(
        output,
        "    {{ section = {}, at = {} }},",
        quote(&primary.heading),
        range(mount.first, mount.last)
    )?;
    writeln!(
        output,
        "    {{ section = {}, at = {} }},",
        quote(&extension.heading),
        range(first, last)
    )?;
    writeln!(output, "]")?;
    Ok(())
}

/// Append modes and brand to an inline slot entry.
fn push_metadata(entry: &mut String, critical: &CriticalDefinition) {
    if !critical.modes.is_empty() {
        let _ = write!(entry, ", modes = {}", quoted_list(&critical.modes));
    }
    if let Some(brand) = critical.brand {
        let _ = write!(entry, ", brand = {brand}");
    }
}

/// Render strings as a one-line TOML array.
fn quoted_list(values: &[String]) -> String {
    let values: Vec<_> = values.iter().map(|value| quote(value)).collect();
    format!("[{}]", values.join(", "))
}

/// Render an inclusive zero-based range as a one-based slot or run.
fn range(first: u8, last: u8) -> String {
    if first == last {
        (first + 1).to_string()
    } else {
        format!("\"{}-{}\"", first + 1, last + 1)
    }
}

/// Quote a TOML basic string.
fn quote(value: &str) -> String {
    toml::Value::String(value.to_owned()).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    const ZEUS_ARM: &str = r#"
name = "Zeus"
class = "mech"
movement = "biped"
tons = 80
max_speed = 64.5
specials = ["DoubleHS", "doublehs", "FlipArms"]

[sections.left_arm]
armor = 22
internals = 13
slots = [
    { at = 1, item = "ShoulderOrHip", brand = 3 },
    { at = "4-6", item = "IS.ERPPC", modes = ["OnTC"], brand = 3 },
]

[sections.left_torso]
armor = 25
internals = 17
rear = 6
slots = [
    { at = 7, item = "Ammo_IS.LRM-15", rounds = 8 },
    { at = 8, item = "ArtemisIV", link = 2 },
]
"#;

    #[test]
    fn documents_decode_into_internal_fields_and_slots() {
        let parsed = ParsedTemplate::parse("ZEU-9S", ZEUS_ARM).unwrap();
        assert_eq!(parsed.fields["reference"], "ZEU-9S");
        assert_eq!(parsed.fields["type"], "Mech");
        assert_eq!(parsed.fields["move_type"], "Biped");
        assert_eq!(parsed.fields["max_speed"], "64.5");
        assert_eq!(parsed.fields["specials"], "DoubleHS FlipArms");
        let arm = &parsed.sections["left_arm"];
        assert_eq!((arm.armor, arm.internal), (22, 13));
        assert_eq!(arm.criticals.len(), 4);
        assert_eq!(arm.criticals[&5].modes, ["OnTC"]);
        assert_eq!(arm.criticals[&5].brand, Some(3));
        let torso = &parsed.sections["left_torso"];
        assert_eq!(torso.rear, 6);
        assert_eq!(torso.criticals[&6].data, "8");
        assert_eq!(torso.criticals[&7].data, "2");
    }

    #[test]
    fn malformed_documents_are_rejected() {
        for source in [
            "unknown = 1",
            "tons = \"80\"",
            "tons = 2147483648",
            "class = \"Mech\"",
            "class = \"tank\"",
            "max_speed = nan",
            "specials = [\"Two Words\"]",
            "[sections.head]\nslots = [{ at = 0, item = \"Cockpit\" }]",
            "[sections.head]\nslots = [{ at = \"3-2\", item = \"Cockpit\" }]",
            "[sections.head]\nslots = [{ at = 13, item = \"Cockpit\" }]",
            "[sections.head]\nslots = [{ at = 1, item = \"Cockpit\", colour = 1 }]",
            "[sections.head]\nslots = [{ at = \"1-2\", item = \"A\" }, { at = 2, item = \"B\" }]",
            "[sections.head]\nslots = [{ at = 1, item = \"IS.SmallLaser\", rounds = 4 }]",
            "[sections.head]\nslots = [{ at = 1, item = \"Ammo_IS.AC/2\", link = 4 }]",
            "[sections.head]\nslots = [{ at = 1, item = \"SplitCrit_Left\", link = 4 }]",
            "[sections.head]\nshield = 1",
        ] {
            assert!(ParsedTemplate::parse("X", source).is_err(), "{source}");
        }
    }

    #[test]
    fn split_mounts_link_extensions_to_their_primary_run() {
        let source = r#"
class = "mech"
movement = "biped"
[sections.center_torso]
[sections.left_torso]
[sections.left_arm]
[[split_mounts]]
item = "IS.AC/20"
brand = 4
placements = [
    { section = "center_torso", at = "11-12" },
    { section = "left_torso", at = "1-8" },
]
"#;
        let parsed = ParsedTemplate::parse("X", source).unwrap();
        let center = &parsed.sections["center_torso"];
        assert_eq!(center.criticals[&10].equipment, "IS.AC/20");
        assert_eq!(center.criticals[&11].brand, Some(4));
        let torso = &parsed.sections["left_torso"];
        assert_eq!(torso.criticals.len(), 8);
        assert_eq!(torso.criticals[&0].equipment, SPLIT_LEFT);
        assert_eq!(
            parse_split_link(&torso.criticals[&0].data).unwrap(),
            (BattleSection::CenterTorso, 10)
        );
        assert_eq!(torso.criticals[&0].brand, Some(4));

        for (placements, class) in [
            ("{ section = \"left_arm\", at = 1 }, { section = \"center_torso\", at = 1 }", "mech"),
            ("{ section = \"left_torso\", at = 1 }", "mech"),
            ("{ section = \"left_torso\", at = 1 }, { section = \"center_torso\", at = 1 }, { section = \"left_arm\", at = 1 }", "mech"),
            ("{ section = \"left_torso\", at = 1 }, { section = \"center_torso\", at = 1 }", "vehicle"),
        ] {
            let source = format!(
                "class = \"{class}\"\n[sections.center_torso]\n[sections.left_torso]\n[sections.left_arm]\n[[split_mounts]]\nitem = \"IS.AC/20\"\nplacements = [{placements}]"
            );
            assert!(ParsedTemplate::parse("X", &source).is_err(), "{source}");
        }
    }

    #[test]
    fn rendering_round_trips_slots_and_split_mounts() {
        let source = r#"
name = "Split"
class = "mech"
movement = "quad"
tons = 50
max_speed = 86.0
specials = ["Searchlight"]

[sections.front_left_leg]
armor = 1
internals = 2

[sections.left_torso]
armor = 3
internals = 4
rear = 5
config = "Case"
slots = [
    { at = "1-2", item = "HeatSink" },
    { at = 3, item = "Ammo_IS.AC/20", rounds = 5, modes = ["Halfton"] },
]

[sections.center_torso]
armor = 6
internals = 7
slots = [
    { at = "1-3", item = "Engine" },
]

[[split_mounts]]
item = "IS.AC/20"
brand = 3
placements = [
    { section = "center_torso", at = "11-12" },
    { section = "left_torso", at = "4-11" },
]
"#;
        let parsed = ParsedTemplate::parse("X", source).unwrap();
        let sections: Vec<_> = [
            ("front_left_leg", BattleSection::LeftArm),
            ("left_torso", BattleSection::LeftTorso),
            ("center_torso", BattleSection::CenterTorso),
        ]
        .into_iter()
        .map(|(heading, mech)| RenderSection {
            heading: heading.into(),
            mech: Some(mech),
            layout: &parsed.sections[heading],
        })
        .collect();
        let rendered = render(&parsed.fields, &sections).unwrap();
        assert_eq!(rendered.trim(), source.trim());
    }

    #[test]
    fn rendering_rejects_attributes_without_a_document_field() {
        let attributes = BTreeMap::from([("mystery".to_owned(), "1".to_owned())]);
        assert!(render(&attributes, &[]).is_err());
    }
}
